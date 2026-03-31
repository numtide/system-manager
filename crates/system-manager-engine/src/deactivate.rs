use anyhow::Result;

use crate::activate::etc_files;
use crate::activate::services;
use crate::activate::users;
use crate::activate::{get_state_file, ActivationError, StateV1};

/// Deactivates system-manager by locking managed users, removing etc files,
/// and stopping systemd services.
pub fn deactivate() -> Result<()> {
    log::info!("Deactivating system-manager");
    let state_file = &get_state_file()?;
    let old_state = StateV1::from_file(state_file)?;
    log::debug!("{old_state:?}");

    if let Err(e) = users::lock_managed_users() {
        log::error!("Error locking managed user accounts: {e}");
    }

    if let Err(e) = users::restore_original_shells() {
        log::error!("Error restoring original shell paths: {e}");
    }

    match etc_files::deactivate(old_state.file_tree) {
        Ok(etc_tree) => {
            log::info!("Deactivating systemd services...");
            match services::deactivate(old_state.services) {
                Ok(services) => StateV1 {
                    file_tree: etc_tree,
                    services,
                    version: Default::default(),
                },
                Err(ActivationError::WithPartialResult { result, source }) => {
                    log::error!("Error during deactivation: {source:?}");
                    StateV1 {
                        file_tree: etc_tree,
                        services: result,
                        version: Default::default(),
                    }
                }
            }
        }
        Err(ActivationError::WithPartialResult { result, source }) => {
            log::error!("Error during deactivation: {source:?}");
            StateV1 {
                file_tree: result,
                ..old_state
            }
        }
    }
    .write_to_file(state_file)?;

    Ok(())
}
