use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::{fs, process, str};

use super::{create_store_link, StorePath, FLAKE_ATTR, GCROOT_PATH, PROFILE_PATH};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NixBuildOutput {
    drv_path: String,
    outputs: HashMap<String, String>,
}

pub fn generate(flake_uri: &str) -> Result<()> {
    // FIXME: we should not hard-code the system here
    let flake_attr = format!("{FLAKE_ATTR}.x86_64-linux");

    log::info!("Building new system-manager generation...");
    log::info!("Running nix build...");
    let store_path = run_nix_build(flake_uri, &flake_attr).and_then(get_store_path)?;

    log::info!("Creating new generation from {}", store_path);
    install_nix_profile(&store_path, PROFILE_PATH).map(print_out_and_err)?;

    log::info!("Registering GC root...");
    create_gcroot(GCROOT_PATH, PROFILE_PATH)?;

    log::info!("Done");
    Ok(())
}

fn install_nix_profile(store_path: &StorePath, profile_path: &str) -> Result<process::Output> {
    process::Command::new("nix-env")
        .arg("--profile")
        .arg(profile_path)
        .arg("--set")
        .arg(&store_path.store_path)
        .output()
        .map_err(anyhow::Error::from)
}

fn create_gcroot(gcroot_path: &str, profile_path: &str) -> Result<()> {
    let profile_store_path = fs::canonicalize(profile_path)?;
    let store_path = StorePath::from(String::from(profile_store_path.to_string_lossy()));
    create_store_link(&store_path, Path::new(gcroot_path))
}

fn get_store_path(nix_build_result: process::Output) -> Result<StorePath> {
    if nix_build_result.status.success() {
        String::from_utf8(nix_build_result.stdout)
            .map_err(anyhow::Error::from)
            .and_then(parse_nix_build_output)
    } else {
        String::from_utf8(nix_build_result.stderr)
            .map_err(anyhow::Error::from)
            .and_then(|e| {
                log::error!("{}", e);
                Err(anyhow!("Nix build failed."))
            })
    }
}

fn parse_nix_build_output(output: String) -> Result<StorePath> {
    let expected_output_name = "out";
    let results: Vec<NixBuildOutput> = serde_json::from_str(&output)?;

    if let [result] = results.as_slice() {
        if let Some(store_path) = result.outputs.get(expected_output_name) {
            return Ok(StorePath::from(store_path.to_owned()));
        }
        return Err(anyhow!(
            "No output '{}' found in nix build result.",
            expected_output_name
        ));
    }
    Err(anyhow!(
        "Multiple build results were returned, we cannot handle that yet."
    ))
}

fn run_nix_build(flake_uri: &str, flake_attr: &str) -> Result<process::Output> {
    process::Command::new("nix")
        .arg("build")
        .arg(format!("{flake_uri}#{flake_attr}"))
        .arg("--json")
        .output()
        .map_err(anyhow::Error::from)
}

fn print_out_and_err(output: process::Output) -> process::Output {
    print_u8(&output.stdout);
    print_u8(&output.stderr);
    output
}

fn print_u8(bytes: &[u8]) {
    str::from_utf8(bytes).map_or((), |s| {
        if !s.trim().is_empty() {
            log::info!("{}", s.trim())
        }
    })
}
