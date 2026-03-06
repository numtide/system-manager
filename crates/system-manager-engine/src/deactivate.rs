use anyhow::Result;
use lazy_errors::prelude::*;

use crate::activate::etc_files;
use crate::activate::services;
use crate::activate::users;
use crate::activate::{get_state_file, split_activation_result, State};

/// Deactivates system-manager by locking managed users, removing etc files,
/// and stopping systemd services.
pub fn deactivate() -> Result<()> {
    log::info!("Deactivating system-manager");
    let state_file = &get_state_file()?;
    let old_state = State::from_file(state_file)?;
    log::debug!("{old_state:?}");
    let mut errs = ErrorStash::new(|| "Deactivation completed with errors");

    let lock_result = users::lock_managed_users();
    if let Err(ref e) = lock_result {
        log::error!("Error locking managed user accounts: {e}");
    }
    lock_result.or_stash(&mut errs);

    let (etc_tree, etc_result) =
        split_activation_result(etc_files::deactivate(old_state.file_tree));
    if let Err(ref e) = etc_result {
        log::error!("Error during deactivation: {e:?}");
    }
    etc_result.or_stash(&mut errs);

    log::info!("Deactivating systemd services...");
    let (services, svc_result) = split_activation_result(services::deactivate(old_state.services));
    if let Err(ref e) = svc_result {
        log::error!("Error during deactivation: {e:?}");
    }
    svc_result.or_stash(&mut errs);

    let final_state = State {
        file_tree: etc_tree,
        services,
    };
    final_state.write_to_file(state_file).or_stash(&mut errs);

    Ok(Result::<(), _>::from(errs)?)
}
