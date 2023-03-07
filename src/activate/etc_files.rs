use anyhow::{anyhow, Result};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{DirBuilder, Permissions};
use std::os::unix::prelude::PermissionsExt;
use std::path;
use std::path::{Path, PathBuf};
use std::{fs, io};

use crate::{
    create_link, create_store_link, remove_dir, remove_file, remove_link, StorePath,
    ETC_STATE_FILE_NAME, SYSTEM_MANAGER_STATE_DIR,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EtcFile {
    source: StorePath,
    target: PathBuf,
    uid: u32,
    gid: u32,
    group: String,
    user: String,
    mode: String,
}

type EtcFiles = HashMap<String, EtcFile>;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EtcFilesConfig {
    entries: EtcFiles,
    static_env: StorePath,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EtcStateInfo {
    status: EtcFileStatus,
    // TODO directories and files are now both represented as a string associated with a nested
    // map. For files the nested map is simple empty.
    // We could potentially optimise this.
    nested: HashMap<String, EtcStateInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum EtcFileStatus {
    Managed,
    Unmanaged,
}

impl EtcFileStatus {
    fn merge(&self, other: &Self) -> Self {
        use EtcFileStatus::*;

        match (self, other) {
            (Unmanaged, Unmanaged) => Unmanaged,
            _ => Managed,
        }
    }
}

impl EtcStateInfo {
    fn new() -> Self {
        Self::with_status(EtcFileStatus::Unmanaged)
    }

    fn with_status(status: EtcFileStatus) -> Self {
        Self {
            status,
            nested: HashMap::new(),
        }
    }

    // TODO unit tests
    fn register_managed_path(mut self, components: &[String], path: String) -> Self {
        let state = components.iter().fold(&mut self, |state, component| {
            if !state.nested.contains_key(component) {
                let new_state = Self::new();
                state.nested.insert(component.to_owned(), new_state);
            }
            state.nested.get_mut(component).unwrap()
        });

        state
            .nested
            .insert(path, Self::with_status(EtcFileStatus::Managed));

        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatedEtcFile {
    path: PathBuf,
}

pub fn activate(store_path: &StorePath, ephemeral: bool) -> Result<()> {
    log::info!("Reading etc file definitions...");
    let file = fs::File::open(Path::new(&store_path.store_path).join("etcFiles/etcFiles.json"))?;
    let reader = io::BufReader::new(file);
    let config: EtcFilesConfig = serde_json::from_reader(reader)?;
    log::debug!("{:?}", config);

    let etc_dir = etc_dir(ephemeral);
    log::debug!("Storing /etc entries in {}", etc_dir.display());

    DirBuilder::new().recursive(true).create(&etc_dir)?;

    // TODO: constant?
    let static_dir_name = ".system-manager-static";
    let static_path = etc_dir.join(static_dir_name);
    create_store_link(&config.static_env, static_path.as_path())?;

    // TODO remove all paths that are in the state file from the previous generation
    // and not in the current one.

    let state = read_created_files()?;
    let new_state = create_etc_links(config.entries.values(), &etc_dir, state);
    serialise_created_files(&new_state.register_managed_path(&[], static_dir_name.to_owned()))?;

    Ok(())
}

pub fn deactivate() -> Result<()> {
    let old_created_files = read_created_files()?;
    log::debug!("{:?}", old_created_files);

    // TODO
    //old_created_files
    //    .iter()
    //    .try_for_each(|created_file| remove_created_file(&created_file.path, "etc"))?;

    serialise_created_files(&EtcStateInfo::new())?;

    log::info!("Done");
    Ok(())
}

fn remove_created_file<P>(created_file: &P, root_dir: &str) -> Result<()>
where
    P: AsRef<Path>,
{
    let path = created_file.as_ref();
    let recurse = if path.is_file() {
        remove_file(path)?;
        true
    } else if path.is_symlink() {
        remove_link(path)?;
        true
    } else if path.is_dir() && fs::read_dir(created_file)?.next().is_none() {
        log::info!("We will remove the following directory: {}", path.display());
        remove_dir(path)?;
        true
    } else {
        log::debug!("Stopped at directory {}.", path.display());
        false
    };

    if recurse {
        if let Some(parent) = path.parent() {
            if parent
                .file_name()
                .filter(|name| &root_dir != name)
                .is_some()
            {
                log::debug!("Recursing up into directory {}...", parent.display());
                return remove_created_file(&parent, root_dir);
            }
            log::debug!("Reached root dir: {}", parent.display());
        }
    }
    Ok(())
}

fn create_etc_links<'a, E>(entries: E, etc_dir: &Path, state: EtcStateInfo) -> EtcStateInfo
where
    E: Iterator<Item = &'a EtcFile>,
{
    entries.fold(state, |state, entry| {
        let (new_state, status) = create_etc_link(entry, etc_dir, state);
        match status {
            Ok(_) => new_state,
            Err(e) => {
                log::error!("Error while creating file in {}: {e}", etc_dir.display());
                new_state
            }
        }
    })
}

fn create_etc_link(
    entry: &EtcFile,
    etc_dir: &Path,
    state: EtcStateInfo,
) -> (EtcStateInfo, Result<()>) {
    if entry.mode == "symlink" {
        if let Some(path::Component::Normal(link_target)) =
            entry.target.components().into_iter().next()
        {
            let link_name = etc_dir.join(link_target);
            match create_link(
                Path::new(".")
                    .join(".system-manager-static")
                    .join("etc")
                    .join(link_target)
                    .as_path(),
                link_name.as_path(),
            ) {
                Ok(_) => (
                    state.register_managed_path(&[], link_target.to_string_lossy().into_owned()),
                    Ok(()),
                ),
                e => (state, e),
            }
        } else {
            (
                state,
                Err(anyhow!("Cannot create link: {}", entry.target.display(),)),
            )
        }
    } else {
        let dirbuilder = DirBuilder::new();
        let target_path = etc_dir.join(entry.target.as_path());

        // Create all parent dirs that do not exist yet
        let (new_state, created_paths, _, status) = target_path
            .parent()
            .unwrap() // TODO
            .components()
            .fold_while(
                (state, Vec::new(), PathBuf::from("/"), Ok(())),
                |(state, mut created, path, _), component| {
                    use itertools::FoldWhile::{Continue, Done};
                    use path::Component;

                    match component {
                        Component::RootDir => Continue((state, created, path, Ok(()))),
                        Component::Normal(dir) => {
                            let new_path = path.join(dir);
                            if !new_path.exists() {
                                log::debug!("Creating path: {}", new_path.display());
                                match dirbuilder.create(new_path.as_path()) {
                                    Ok(_) => {
                                        let new_state = state.register_managed_path(
                                            &created,
                                            dir.to_string_lossy().into_owned(),
                                        );
                                        created.push(dir.to_string_lossy().into_owned());
                                        Continue((new_state, created, new_path, Ok(())))
                                    }
                                    Err(e) => Done((state, created, path, Err(anyhow!(e)))),
                                }
                            } else {
                                created.push(dir.to_string_lossy().into_owned());
                                Continue((state, created, new_path, Ok(())))
                            }
                        }
                        otherwise => Done((
                            state,
                            created,
                            path,
                            Err(anyhow!(
                                "Unexpected path component encountered: {:?}",
                                otherwise
                            )),
                        )),
                    }
                },
            )
            .into_inner();

        match status.and_then(|_| {
            copy_file(
                entry
                    .source
                    .store_path
                    .join("etc")
                    .join(&entry.target)
                    .as_path(),
                &target_path,
                &entry.mode,
            )
        }) {
            Ok(_) => (
                new_state.register_managed_path(
                    &created_paths,
                    target_path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                ),
                Ok(()),
            ),
            e => (new_state, e),
        }
    }
}

fn copy_file(source: &Path, target: &Path, mode: &str) -> Result<()> {
    fs::copy(source, target)?;
    let mode_int = u32::from_str_radix(mode, 8).map_err(anyhow::Error::from)?;
    fs::set_permissions(target, Permissions::from_mode(mode_int))?;
    Ok(())
}

fn etc_dir(ephemeral: bool) -> PathBuf {
    if ephemeral {
        Path::new("/run").join("etc")
    } else {
        Path::new("/etc").to_path_buf()
    }
}

fn serialise_created_files(created_files: &EtcStateInfo) -> Result<()> {
    let state_file = Path::new(SYSTEM_MANAGER_STATE_DIR).join(ETC_STATE_FILE_NAME);
    DirBuilder::new()
        .recursive(true)
        .create(SYSTEM_MANAGER_STATE_DIR)?;

    log::info!("Writing state info into file: {}", state_file.display());
    let writer = io::BufWriter::new(fs::File::create(state_file)?);
    serde_json::to_writer(writer, created_files)?;
    Ok(())
}

fn read_created_files() -> Result<EtcStateInfo> {
    let state_file = Path::new(SYSTEM_MANAGER_STATE_DIR).join(ETC_STATE_FILE_NAME);
    DirBuilder::new()
        .recursive(true)
        .create(SYSTEM_MANAGER_STATE_DIR)?;

    if Path::new(&state_file).is_file() {
        log::info!("Reading state info from {}", state_file.display());
        let reader = io::BufReader::new(fs::File::open(state_file)?);
        match serde_json::from_reader(reader) {
            Ok(created_files) => return Ok(created_files),
            Err(e) => {
                log::error!("Error reading the state file, ignoring.");
                log::error!("{:?}", e);
            }
        }
    }
    Ok(EtcStateInfo::new())
}
