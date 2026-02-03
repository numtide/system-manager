use anyhow::Result;

use crate::activate::etc_files;
use crate::activate::services;
use crate::activate::users;
use crate::activate::{get_state_file, ActivationError, State};

/// Deactivates system-manager by locking managed users, removing etc files,
/// and stopping systemd services.
pub fn deactivate() -> Result<()> {
    log::info!("Deactivating system-manager");
    let state_file = &get_state_file()?;
    let old_state = State::from_file(state_file)?;
    log::debug!("{old_state:?}");

    if let Err(e) = users::lock_managed_users() {
        log::error!("Error locking managed user accounts: {e}");
    }

    match etc_files::deactivate(old_state.file_tree) {
        Ok(etc_tree) => {
            log::info!("Deactivating systemd services...");
            match services::deactivate(old_state.services) {
                Ok(services) => State {
                    file_tree: etc_tree,
                    services,
                },
                Err(ActivationError::WithPartialResult { result, source }) => {
                    log::error!("Error during deactivation: {source:?}");
                    State {
                        file_tree: etc_tree,
                        services: result,
                    }
                }
            }
        }
        Err(ActivationError::WithPartialResult { result, source }) => {
            log::error!("Error during deactivation: {source:?}");
            State {
                file_tree: result,
                ..old_state
            }
        }
    }
    .write_to_file(state_file)?;

    Ok(())
}
