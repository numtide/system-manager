pub(crate) mod etc_files;
pub(crate) mod services;
mod tmp_files;
pub(crate) mod users;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::error::Category;
use std::collections::HashSet;
use std::fs::DirBuilder;
use std::io::Seek;
use std::path::{Path, PathBuf};
use std::{fs, io, process};
use thiserror::Error;

use crate::activate::etc_files::etc_tree::StateV0;
use crate::{StorePath, STATE_FILE_NAME, SYSTEM_MANAGER_STATE_DIR};

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

    log::info!("Activating etc files...");

    match etc_files::activate(store_path, old_state.file_tree, ephemeral) {
        Ok(etc_tree) => {
            log::info!("Restarting sysinit-reactivation.target...");
            services::restart_sysinit_reactivation_target()?;

            // Restart userborn before tmpfiles so users exist when tmpfiles runs
            if let Err(e) = services::restart_userborn_if_exists() {
                log::error!("Error restarting userborn.service: {e}");
            }

            log::info!("Activating tmp files...");
            let tmp_result = tmp_files::activate(&etc_tree.files);
            if let Err(e) = &tmp_result {
                log::error!("Error during activation of tmp files");
                log::error!("{e}");
            } else {
                log::debug!("Successfully created tmp files");
            }

            log::info!("Activating systemd services...");
            let final_state = match services::activate(store_path, old_state.services, ephemeral) {
                Ok(services) => StateV1 {
                    file_tree: etc_tree,
                    services,
                    version: 1,
                },
                Err(ActivationError::WithPartialResult { result, source }) => {
                    log::error!("Error during activation: {source:?}");
                    StateV1 {
                        file_tree: etc_tree,
                        services: result,
                        version: 1,
                    }
                }
            };
            final_state.write_to_file(state_file)?;

            if let Err(e) = tmp_result {
                return Err(e.into());
            }

            Ok(())
        }
        Err(ActivationError::WithPartialResult { result, source }) => {
            log::error!("Error during activation: {source:?}");
            log::debug!("Resulting file tree: {:?}", result);
            let final_state = StateV1 {
                file_tree: result,
                ..old_state
            };
            final_state.write_to_file(state_file)?;
            Ok(())
        }
    }
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

    log::info!("Activating etc files...");

    match etc_files::activate(store_path, old_state.file_tree, ephemeral) {
        Ok(etc_tree) => {
            log::info!("Registering systemd services...");
            match services::get_active_services(store_path, old_state.services) {
                Ok(services) => StateV1 {
                    file_tree: etc_tree,
                    services,
                    version: 1,
                },
                Err(ActivationError::WithPartialResult { result, source }) => {
                    log::error!("Error during activation: {source:?}");
                    StateV1 {
                        file_tree: etc_tree,
                        services: result,
                        version: 1,
                    }
                }
            }
        }
        Err(ActivationError::WithPartialResult { result, source }) => {
            log::error!("Error during activation: {source:?}");
            StateV1 {
                file_tree: result,
                ..old_state
            }
        }
    }
    .write_to_file(state_file)?;
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

pub(crate) fn get_state_file() -> Result<PathBuf> {
    let state_file = Path::new(SYSTEM_MANAGER_STATE_DIR).join(STATE_FILE_NAME);
    DirBuilder::new()
        .recursive(true)
        .create(SYSTEM_MANAGER_STATE_DIR)?;
    Ok(state_file)
}
