pub(crate) mod etc_files;
pub(crate) mod services;
mod tmp_files;
pub(crate) mod users;

use anyhow::{anyhow, Result};
use lazy_errors::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::error::Category;
use std::collections::HashSet;
use std::fs::DirBuilder;
use std::io::Seek;
use std::path::{Path, PathBuf};
use std::{fmt, fs, io, process};
use thiserror::Error;

use crate::activate::etc_files::etc_tree::StateV0;
use crate::{StorePath, STATE_FILE_NAME, SYSTEM_MANAGER_STATE_DIR};

pub(crate) fn collect_activation_result_err<R, F, M>(
    res: ActivationResult<R>,
    err_stash: &mut ErrorStash<F, M>,
) -> ActivationResult<R>
where
    M: fmt::Display,
    F: FnOnce() -> M,
{
    res.map_err(|e| {
        let ActivationError::WithPartialResult {
            result: _,
            ref source,
        } = e;
        err_stash.push(source.to_string());
        e
    })
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
    Managed,
    ManagedWithBackup,
}

type EtcTree = HashSet<PathBuf>;
type BackedUpFiles = HashSet<PathBuf>;
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EtcFilesState {
    pub files: EtcTree,
    pub backed_up_files: BackedUpFiles,
}

impl EtcFilesState {
    pub fn contains(&self, path: &Path) -> bool {
        self.files.contains(path) || self.backed_up_files.contains(path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateV1 {
    pub(crate) file_tree: EtcFilesState,
    pub(crate) services: services::Services,
    pub(crate) version: u32,
}

impl Default for StateV1 {
    fn default() -> Self {
        Self {
            file_tree: EtcFilesState::default(),
            services: services::Services::default(),
            version: 1,
        }
    }
}

impl StateV1 {
    pub fn from_file(state_file: &Path) -> Result<Self> {
        if state_file.is_file() {
            log::info!("Reading state info from {}", state_file.display());
            let mut reader = io::BufReader::new(fs::File::open(state_file)?);
            // if state is v1
            let rv1: serde_json::Result<StateV1> = serde_json::from_reader(&mut reader);
            match rv1 {
                Ok(v1) => Ok(v1),
                Err(e) => {
                    // State might be v0. Let's try to parse it.
                    if e.classify() == Category::Data {
                        reader.rewind()?;
                        let filetree: StateV0 =
                            serde_json::from_reader(&mut reader).map_err(|e| {
                                anyhow!(
                                    "Cannot parse state, it doesn't match any supported format: {}",
                                    e
                                )
                            })?;
                        log::info!("The state is in the V0 format. Migrating it to the V1 format.");
                        // Backup the old state, just in case. Better be safe than sorry.
                        let mut backup_path = state_file.to_owned();
                        backup_path.add_extension("v0back");
                        log::info!(
                            "Create a backup of the v0 state at {}.",
                            &backup_path.display()
                        );
                        fs::copy(state_file, backup_path)?;
                        Ok(filetree.into())
                    } else {
                        // We don't know what that state is.
                        Err(anyhow!("Unexpected serde_json error: {}", e))
                    }
                }
            }
            // else parse v0 then migrate
        } else {
            Ok(Self::default())
        }
    }

    pub fn write_to_file(&self, state_file: &Path) -> Result<()> {
        log::info!("Writing state info into file: {}", state_file.display());
        log::debug!("State: {:?}", self);
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
    let old_state = StateV1::from_file(state_file)?;
    let mut errs = ErrorStash::new(|| "Activation completed with errors");

    log::info!("Activating etc files...");

    let etc_result = collect_activation_result_err(
        etc_files::activate(store_path, old_state.file_tree, ephemeral),
        &mut errs,
    );
    if let Err(ref e) = etc_result {
        log::error!("Error during activation: {e:?}");
    }

    // Only run daemon reload, userborn, tmpfiles, and services when etc files
    // were fully applied. Partial etc results mean services may reference
    // missing config files.
    let (etc_tree, services) = match etc_result {
        Ok(etc_tree) => {
            log::info!("Restarting sysinit-reactivation.target...");
            let sysinit_result = services::restart_sysinit_reactivation_target();
            if let Err(ref e) = sysinit_result {
                log::error!("Error restarting sysinit-reactivation.target: {e}");
            } else {
                log::info!("Successfully restarted sysinit-reactivation.target");
            }
            sysinit_result.or_stash(&mut errs);

            // Restart userborn before tmpfiles so users exist when tmpfiles runs
            let userborn_result = services::restart_userborn_if_exists();
            if let Err(ref e) = userborn_result {
                log::error!("Error restarting userborn.service: {e}");
            }
            userborn_result.or_stash(&mut errs);

            log::info!("Activating tmp files...");
            let tmp_result =
                collect_activation_result_err(tmp_files::activate(&etc_tree.files), &mut errs);
            if let Err(ref e) = tmp_result {
                log::error!("Error during activation of tmp files: {e}");
            } else {
                log::info!("Successfully created tmp files");
            }

            log::info!("Activating systemd services...");
            let svc_result = collect_activation_result_err(
                services::activate(store_path, old_state.services, ephemeral),
                &mut errs,
            );
            if let Err(ref e) = svc_result {
                log::error!("Error during activation: {e:?}");
            } else {
                log::info!("Successfully activated systemd services");
            }
            let services = match svc_result {
                Ok(s) => s,
                Err(ActivationError::WithPartialResult { result, .. }) => result,
            };
            (etc_tree, services)
        }
        Err(ActivationError::WithPartialResult { result, .. }) => (result, old_state.services),
    };

    let final_state = StateV1 {
        file_tree: etc_tree,
        services,
        version: 1,
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
    let old_state = StateV1::from_file(state_file)?;
    let mut errs = ErrorStash::new(|| "Pre-population completed with errors");

    log::info!("Activating etc files...");

    let etc_result = collect_activation_result_err(
        etc_files::activate(store_path, old_state.file_tree, ephemeral),
        &mut errs,
    );
    if let Err(ref e) = etc_result {
        log::error!("Error during activation: {e:?}");
    }

    // Only register services when etc files were fully applied, preserving
    // old service state on etc failure to avoid persisting state from a
    // partial run.
    let (etc_tree, services) = match etc_result {
        Ok(etc_tree) => {
            log::info!("Registering systemd services...");
            let svc_result = collect_activation_result_err(
                services::get_active_services(store_path, old_state.services),
                &mut errs,
            );
            if let Err(ref e) = svc_result {
                log::error!("Error during activation: {e:?}");
            }
            let services = match svc_result {
                Ok(s) => s,
                Err(ActivationError::WithPartialResult { result, .. }) => result,
            };
            (etc_tree, services)
        }
        Err(ActivationError::WithPartialResult { result, .. }) => (result, old_state.services),
    };

    let final_state = StateV1 {
        file_tree: etc_tree,
        services,
        version: 1,
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
