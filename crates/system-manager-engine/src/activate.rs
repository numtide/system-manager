pub(crate) mod etc_files;
pub(crate) mod services;
mod tmp_files;
pub(crate) mod users;

use anyhow::Result;
use lazy_errors::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::DirBuilder;
use std::path::{Path, PathBuf};
use std::{fmt, fs, io, process};
use thiserror::Error;

use crate::activate::etc_files::{EtcActivationResult, FileTree};
use crate::{StorePath, STATE_FILE_NAME, SYSTEM_MANAGER_STATE_DIR};

pub(crate) fn collect_activation_result_err<F, M>(
    res: EtcActivationResult,
    err_stash: &mut ErrorStash<F, M>,
) -> EtcActivationResult
where
    M: fmt::Display,
    F: FnOnce() -> M,
{
    let nres = res.map_err(|e| {
        let ActivationError::WithPartialResult {
            result: _,
            ref source,
        } = e;
        err_stash.push(source.to_string());
        e
    });
    nres
}

#[derive(Error, Debug)]
pub enum ActivationError<R> {
    #[error("")]
    WithPartialResult { result: R, source: anyhow::Error },
}

impl<R> ActivationError<R> {
    fn with_partial_result<E>(result: R, source: E) -> Self
    where
        E: Into<anyhow::Error>,
    {
        Self::WithPartialResult {
            result,
            source: source.into(),
        }
    }
}

pub type ActivationResult<R> = Result<R, ActivationError<R>>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub(crate) file_tree: FileTree,
    pub(crate) services: services::Services,
}

impl State {
    pub fn from_file(state_file: &Path) -> Result<Self> {
        if state_file.is_file() {
            log::info!("Reading state info from {}", state_file.display());
            let reader = io::BufReader::new(fs::File::open(state_file)?);
            serde_json::from_reader(reader).or_else(|e| {
                log::error!("Error reading the state file, ignoring.");
                log::error!("{e:?}");
                Ok(Self::default())
            })
        } else {
            Ok(Self::default())
        }
    }

    pub fn write_to_file(&self, state_file: &Path) -> Result<()> {
        log::info!("Writing state info into file: {}", state_file.display());
        let writer = io::BufWriter::new(fs::File::create(state_file)?);

        serde_json::to_writer(writer, self)?;
        Ok(())
    }
}

pub fn activate(store_path: &StorePath, ephemeral: bool) -> Result<()> {
    log::info!("Activating system-manager profile: {store_path}");
    if ephemeral {
        log::info!("Running in ephemeral mode");
    }

    log::info!("Running pre-activation assertions...");
    if !run_preactivation_assertions(store_path)?.success() {
        anyhow::bail!("Failure in pre-activation assertions.");
    }

    let state_file = &get_state_file()?;
    let old_state = State::from_file(state_file)?;
    let mut errs = ErrorStash::new(|| "Activation completed with errors");

    log::info!("Activating etc files...");

    let etc_result = collect_activation_result_err(
        etc_files::activate(store_path, old_state.file_tree, ephemeral),
        &mut errs,
    );
    let etc_ok = etc_result.is_ok();
    if let Err(ref e) = etc_result {
        log::error!("Error during activation: {e:?}");
    }

    // Only run daemon reload, userborn, tmpfiles, and services when etc files
    // were fully applied. Partial etc results mean services may reference
    // missing config files.
    let services = match etc_result {
        Ok(etc_tree) => {
            log::info!("Restarting sysinit-reactivation.target...");
            let sysinit_result = services::restart_sysinit_reactivation_target();
            if let Err(ref e) = sysinit_result {
                log::error!("Error restarting sysinit-reactivation.target: {e}");
            }
            sysinit_result.or_stash(&mut errs);

            // Restart userborn before tmpfiles so users exist when tmpfiles runs
            let userborn_result = services::restart_userborn_if_exists();
            if let Err(ref e) = userborn_result {
                log::error!("Error restarting userborn.service: {e}");
            }
            userborn_result.or_stash(&mut errs);

            log::info!("Activating tmp files...");
            let ((), tmp_result) = split_activation_result(tmp_files::activate(&etc_tree));
            if let Err(ref e) = tmp_result {
                log::error!("Error during activation of tmp files: {e}");
            }
            tmp_result.or_stash(&mut errs);

            log::info!("Activating systemd services...");
            let (services, svc_result) = split_activation_result(services::activate(
                store_path,
                old_state.services,
                ephemeral,
            ));
            if let Err(ref e) = svc_result {
                log::error!("Error during activation: {e:?}");
            }
            svc_result.or_stash(&mut errs);
            services
        }
        Err(_) => old_state.services,
    };

    let final_state = State {
        file_tree: etc_tree,
        services,
    };
    final_state.write_to_file(state_file).or_stash(&mut errs);

    Ok(Result::<(), _>::from(errs)?)
}

pub fn prepopulate(store_path: &StorePath, ephemeral: bool) -> Result<()> {
    log::info!("Pre-populating system-manager profile: {store_path}");
    if ephemeral {
        log::info!("Running in ephemeral mode");
    }

    log::info!("Running pre-activation assertions...");
    if !run_preactivation_assertions(store_path)?.success() {
        anyhow::bail!("Failure in pre-activation assertions.");
    }

    let state_file = &get_state_file()?;
    let old_state = State::from_file(state_file)?;
    let mut errs = ErrorStash::new(|| "Pre-population completed with errors");

    log::info!("Activating etc files...");

    let (etc_tree, etc_result) = split_activation_result(etc_files::activate(
        store_path,
        old_state.file_tree,
        ephemeral,
    ));
    let etc_ok = etc_result.is_ok();
    if let Err(ref e) = etc_result {
        log::error!("Error during activation: {e:?}");
    }
    etc_result.or_stash(&mut errs);

    // Only register services when etc files were fully applied, preserving
    // old service state on etc failure to avoid persisting state from a
    // partial run.
    let services = if etc_ok {
        log::info!("Registering systemd services...");
        let (services, svc_result) = split_activation_result(services::get_active_services(
            store_path,
            old_state.services,
        ));
        if let Err(ref e) = svc_result {
            log::error!("Error during activation: {e:?}");
        }
        svc_result.or_stash(&mut errs);
        services
    } else {
        old_state.services
    };

    let final_state = State {
        file_tree: etc_tree,
        services,
    };
    final_state.write_to_file(state_file).or_stash(&mut errs);

    Ok(Result::<(), _>::from(errs)?)
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

pub(crate) fn get_state_file() -> Result<PathBuf> {
    let state_file = Path::new(SYSTEM_MANAGER_STATE_DIR).join(STATE_FILE_NAME);
    DirBuilder::new()
        .recursive(true)
        .create(SYSTEM_MANAGER_STATE_DIR)?;
    Ok(state_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_stash_returns_ok() {
        let errs = ErrorStash::new(|| "Activation completed with errors");
        let result: std::result::Result<(), lazy_errors::prelude::Error> = errs.into();
        assert!(result.is_ok());
    }

    #[test]
    fn single_stashed_error_returns_err() {
        let mut errs = ErrorStash::new(|| "Activation completed with errors");
        Err::<(), _>(anyhow::anyhow!("userborn failed")).or_stash(&mut errs);
        let result: std::result::Result<(), lazy_errors::prelude::Error> = errs.into();
        let err = result.unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("userborn failed"), "message was: {msg}");
    }

    #[test]
    fn multiple_stashed_errors_returns_combined_err() {
        let mut errs = ErrorStash::new(|| "Deactivation completed with errors");
        Err::<(), _>(anyhow::anyhow!("userborn failed")).or_stash(&mut errs);
        Err::<(), _>(anyhow::anyhow!("tmpfiles failed")).or_stash(&mut errs);
        let result: std::result::Result<(), lazy_errors::prelude::Error> = errs.into();
        let err = result.unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("Deactivation"), "message was: {msg}");
        assert!(msg.contains("userborn failed"), "message was: {msg}");
        assert!(msg.contains("tmpfiles failed"), "message was: {msg}");
    }
}
