mod etc_files;
mod services;

use anyhow::Result;

use crate::StorePath;

pub fn activate(store_path: &StorePath, ephemeral: bool) -> Result<()> {
    log::info!("Activating system-manager profile: {store_path}");
    if ephemeral {
        log::info!("Running in ephemeral mode");
    }

    etc_files::activate(store_path, ephemeral)?;
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
