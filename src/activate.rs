mod etc_files;
mod services;
mod tmp_files;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::DirBuilder;
use std::path::{Path, PathBuf};
use std::{fs, io, process};
use thiserror::Error;

use crate::activate::etc_files::FileTree;
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    file_tree: FileTree,
    services: services::Services,
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

    log::info!("Activating etc files...");

    match etc_files::activate(store_path, old_state.file_tree, ephemeral) {
        Ok(etc_tree) => {
            log::info!("Activating tmp files...");
            match tmp_files::activate() {
                Ok(_) => {
                    log::debug!("Successfully created tmp files");
                }
                Err(e) => {
                    log::error!("Error during activation of tmp files");
                    log::error!("{e}");
                }
            };

            log::info!("Activating systemd services...");
            match services::activate(store_path, old_state.services, ephemeral) {
                Ok(services) => State {
                    file_tree: etc_tree,
                    services,
                },
                Err(ActivationError::WithPartialResult { result, source }) => {
                    log::error!("Error during activation: {source:?}");
                    State {
                        file_tree: etc_tree,
                        services: result,
                    }
                }
            }
        }
        Err(ActivationError::WithPartialResult { result, source }) => {
            log::error!("Error during activation: {source:?}");
            State {
                file_tree: result,
                ..old_state
            }
        }
    }
    .write_to_file(state_file)?;

    Ok(())
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

    log::info!("Activating etc files...");

    match etc_files::activate(store_path, old_state.file_tree, ephemeral) {
        Ok(etc_tree) => {
            log::info!("Registering systemd services...");
            match services::get_active_services(store_path, old_state.services) {
                Ok(services) => State {
                    file_tree: etc_tree,
                    services,
                },
                Err(ActivationError::WithPartialResult { result, source }) => {
                    log::error!("Error during activation: {source:?}");
                    State {
                        file_tree: etc_tree,
                        services: result,
                    }
                }
            }
        }
        Err(ActivationError::WithPartialResult { result, source }) => {
            log::error!("Error during activation: {source:?}");
            State {
                file_tree: result,
                ..old_state
            }
        }
    }
    .write_to_file(state_file)?;
    Ok(())
}

pub fn deactivate() -> Result<()> {
    log::info!("Deactivating system-manager");
    let state_file = &get_state_file()?;
    let old_state = State::from_file(state_file)?;
    log::debug!("{old_state:?}");

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

fn get_state_file() -> Result<PathBuf> {
    let state_file = Path::new(SYSTEM_MANAGER_STATE_DIR).join(STATE_FILE_NAME);
    DirBuilder::new()
        .recursive(true)
        .create(SYSTEM_MANAGER_STATE_DIR)?;
    Ok(state_file)
}
