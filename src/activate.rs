mod etc_files;
mod services;

use anyhow::Result;
use std::process;

use crate::StorePath;

pub fn activate(store_path: &StorePath, ephemeral: bool) -> Result<()> {
    log::info!("Activating system-manager profile: {store_path}");
    if ephemeral {
        log::info!("Running in ephemeral mode");
    }

    log::info!("Running pre-activation assertions...");
    if !run_preactivation_assertions(store_path)?.success() {
        anyhow::bail!("Failure in pre-activation assertions.");
    }

    log::info!("Activating etc files...");
    etc_files::activate(store_path, ephemeral)?;

    log::info!("Activating systemd services...");
    services::activate(store_path, ephemeral)?;

    Ok(())
}

// TODO should we also remove the GC root for the profile if it exists?
pub fn deactivate() -> Result<()> {
    log::info!("Deactivating system-manager");
    etc_files::deactivate()?;
    services::deactivate()?;
    Ok(())
}

fn run_preactivation_assertions(store_path: &StorePath) -> Result<process::ExitStatus> {
    let status = process::Command::new(
        store_path
            .store_path
            .join("bin")
            .join("preActivationAssertions"),
    )
    .stderr(process::Stdio::inherit())
    .stdout(process::Stdio::inherit())
    .status()?;
    Ok(status)
}
