use anyhow::Result;
use lazy_errors::prelude::*;

use crate::activate::etc_files;
use crate::activate::services;
use crate::activate::users;
use crate::activate::{collect_activation_result_err, get_state_file, ActivationError, StateV1};

pub fn deactivate() -> Result<()> {
    log::info!("Deactivating system-manager");
    let state_file = &get_state_file()?;
    let old_state = StateV1::from_file(state_file)?;
    log::debug!("{old_state:?}");
    let mut errs = ErrorStash::new(|| "Deactivation completed with errors");

    let lock_result = users::lock_managed_users();
    if let Err(ref e) = lock_result {
        log::error!("Error locking managed user accounts: {e}");
    }
    lock_result.or_stash(&mut errs);

    if let Err(e) = users::restore_original_shells() {
        log::error!("Error restoring original shell paths: {e}");
    }

    let etc_result =
        collect_activation_result_err(etc_files::deactivate(old_state.file_tree), &mut errs);
    if let Err(ref e) = etc_result {
        log::error!("Error during deactivation: {e:?}");
    }
    let etc_tree = match etc_result {
        Ok(t) => t,
        Err(ActivationError::WithPartialResult { result, .. }) => result,
    };

    log::info!("Deactivating systemd services...");
    let svc_result =
        collect_activation_result_err(services::deactivate(old_state.services), &mut errs);
    if let Err(ref e) = svc_result {
        log::error!("Error during deactivation: {e:?}");
    }
    let services = match svc_result {
        Ok(s) => s,
        Err(ActivationError::WithPartialResult { result, .. }) => result,
    };

    let final_state = StateV1 {
        file_tree: etc_tree,
        services,
        version: 1,
    };
    final_state.write_to_file(state_file).or_stash(&mut errs);

    Ok(Result::<(), _>::from(errs)?)
}
