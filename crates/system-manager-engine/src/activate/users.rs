use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

const USERBORN_PREVIOUS_CONFIG: &str = "/var/lib/userborn/previous-userborn.json";
const SYSTEM_MANAGER_SW_PREFIX: &str = "/run/system-manager/sw";

/// Locks user accounts that were previously managed by userborn.
pub fn lock_managed_users() -> Result<()> {
    if Command::new("which")
        .arg("userborn")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| !s.success())
        .unwrap_or(true)
    {
        log::debug!("userborn not found in PATH, skipping user account locking");
        return Ok(());
    }

    log::info!("Locking previously managed user accounts...");

    // Create a temporary file with an empty userborn config
    let empty_config = serde_json::json!({
        "users": [],
        "groups": []
    });

    let mut temp_file = NamedTempFile::new().context("Failed to create temporary config file")?;
    serde_json::to_writer(&mut temp_file, &empty_config)
        .context("Failed to write empty userborn config")?;
    temp_file.flush()?;

    let temp_path = temp_file.path();

    let output = Command::new("userborn")
        .arg(temp_path)
        .arg("/etc")
        .env("USERBORN_MUTABLE_USERS", "true")
        .env("USERBORN_PREVIOUS_CONFIG", USERBORN_PREVIOUS_CONFIG)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .context("Failed to execute userborn")?;

    if !output.status.success() {
        anyhow::bail!(
            "userborn exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    log::info!("Successfully locked managed user accounts");
    Ok(())
}

/// Resolves a base shell path (after prefix stripping) to an existing FHS location.
fn resolve_shell(base: &str) -> &str {
    if Path::new(base).exists() {
        return base;
    }

    match base {
        "/bin/nologin" | "/sbin/nologin" => {
            for fallback in ["/usr/sbin/nologin", "/usr/bin/nologin"] {
                if Path::new(fallback).exists() {
                    return fallback;
                }
            }
            base
        }
        _ => "/bin/sh",
    }
}

/// Restores original shell paths in `/etc/passwd` after deactivation.
///
/// During activation, userborn rewrites shell fields to point under
/// `/run/system-manager/sw/` (e.g. `/run/system-manager/sw/bin/bash`).
/// After deactivation that prefix becomes a dangling path.
/// This function reads `/etc/passwd` to find affected users, then uses
/// `usermod -s` to restore each shell to its FHS equivalent.
pub fn restore_original_shells() -> Result<()> {
    let content = fs::read_to_string("/etc/passwd").context("Failed to read /etc/passwd")?;
    let mut failure_count = 0;

    for line in content.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() != 7 {
            continue;
        }

        let username = fields[0];
        let shell = fields[6];
        let Some(base) = shell.strip_prefix(SYSTEM_MANAGER_SW_PREFIX) else {
            continue;
        };

        let resolved = resolve_shell(base);
        log::info!(
            "Restoring shell for user '{}': {} -> {}",
            username,
            shell,
            resolved
        );

        let output = Command::new("usermod")
            .args(["-s", resolved, username])
            .output()
            .with_context(|| format!("Failed to execute usermod for user '{username}'"))?;

        if !output.status.success() {
            log::error!(
                "usermod failed for user '{}': {}",
                username,
                String::from_utf8_lossy(&output.stderr).trim()
            );
            failure_count += 1;
        }
    }

    if failure_count > 0 {
        anyhow::bail!("Failed to restore shells for {} user(s)", failure_count);
    }

    Ok(())
}
