use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

const USERBORN_PREVIOUS_CONFIG: &str = "/var/lib/userborn/previous-userborn.json";

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
