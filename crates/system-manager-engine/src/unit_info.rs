// Systemd unit file parsing, ported from nixpkgs switch-to-configuration-ng.
//
// Handles systemd INI quirks: empty values clear previous definitions,
// multiple values for the same key accumulate, [Install] sections are skipped.
// Also merges drop-in override files from `.d/*.conf` directories.

use anyhow::{anyhow, Context};
use glob::glob;
use ini::{Ini, ParseOption};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

pub type UnitInfo = HashMap<String, HashMap<String, Vec<String>>>;

// This function takes a single ini file that specified systemd configuration like unit
// configuration and parses it into a HashMap where the keys are the sections of the unit file and
// the values are HashMaps themselves. These HashMaps have the unit file keys as their keys (left
// side of =) and an array of all values that were set as their values. If a value is empty (for
// example `ExecStart=`), then all current definitions are removed.
//
// Instead of returning the HashMap, this function takes a mutable reference to a HashMap to return
// the data in. This allows calling the function multiple times with the same Hashmap to parse
// override files.
pub fn parse_systemd_ini(data: &mut UnitInfo, mut unit_file: impl Read) -> anyhow::Result<()> {
    let mut unit_file_content = String::new();
    _ = unit_file
        .read_to_string(&mut unit_file_content)
        .context("Failed to read unit file")?;

    let ini = Ini::load_from_str_opt(
        &unit_file_content,
        ParseOption {
            enabled_quote: true,
            enabled_indented_mutiline_value: false,
            enabled_preserve_key_leading_whitespace: false,
            // Allow for escaped characters that won't get interpreted by the INI parser. These
            // often show up in systemd unit files device/mount/swap unit names (e.g. dev-disk-by\x2dlabel-root.device).
            enabled_escape: false,
        },
    )
    .context("Failed parse unit file as INI")?;

    // Copy over all sections
    for (section, properties) in ini.iter() {
        let Some(section) = section else {
            continue;
        };

        if section == "Install" {
            // Skip the [Install] section because it has no relevant keys for us
            continue;
        }

        let section_map = if let Some(section_map) = data.get_mut(section) {
            section_map
        } else {
            data.insert(section.to_string(), HashMap::new());
            data.get_mut(section)
                .ok_or(anyhow!("section name should exist in hashmap"))?
        };

        for (ini_key, _) in properties {
            let values = properties.get_all(ini_key);
            let values = values
                .into_iter()
                .map(String::from)
                .collect::<Vec<String>>();

            // Go over all values
            let mut new_vals = Vec::new();
            let mut clear_existing = false;

            for val in values {
                // If a value is empty, it's an override that tells us to clean the value
                if val.is_empty() {
                    new_vals.clear();
                    clear_existing = true;
                } else {
                    new_vals.push(val);
                }
            }

            match (section_map.get_mut(ini_key), clear_existing) {
                (Some(existing_vals), false) => existing_vals.extend(new_vals),
                _ => {
                    _ = section_map.insert(ini_key.to_string(), new_vals);
                }
            };
        }
    }

    Ok(())
}

// This function takes the path to a systemd configuration file (like a unit configuration) and
// parses it into a UnitInfo structure.
//
// If a directory with the same basename ending in .d exists next to the unit file, it will be
// assumed to contain override files which will be parsed as well and handled properly.
pub fn parse_unit(unit_file: &Path) -> anyhow::Result<UnitInfo> {
    // Parse the main unit and all overrides
    let mut unit_data = HashMap::new();

    let base_unit_file = std::fs::File::open(unit_file)
        .with_context(|| format!("Failed to open unit file {}", unit_file.display()))?;
    parse_systemd_ini(&mut unit_data, base_unit_file)
        .with_context(|| format!("Failed to parse systemd unit file {}", unit_file.display()))?;

    for entry in
        glob(&format!("{}.d/*.conf", unit_file.display())).context("Invalid glob pattern")?
    {
        let Ok(entry) = entry else {
            continue;
        };

        let unit_file = std::fs::File::open(&entry)
            .with_context(|| format!("Failed to open unit file {}", entry.display()))?;
        parse_systemd_ini(&mut unit_data, unit_file)?;
    }

    Ok(unit_data)
}

// Checks whether a specified boolean in a systemd unit is true or false, with a default that is
// applied when the value is not set.
pub fn parse_systemd_bool(
    unit_data: Option<&UnitInfo>,
    section_name: &str,
    bool_name: &str,
    default: bool,
) -> bool {
    if let Some(Some(Some(Some(b)))) = unit_data.map(|data| {
        data.get(section_name).map(|section| {
            section.get(bool_name).map(|vals| {
                vals.last()
                    .map(|last| matches!(last.as_str(), "1" | "yes" | "true" | "on"))
            })
        })
    }) {
        b
    } else {
        default
    }
}
