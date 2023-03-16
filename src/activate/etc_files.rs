mod etc_tree;

use anyhow::{anyhow, Result};
use im::HashMap;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs::{DirBuilder, Permissions};
use std::os::unix::prelude::PermissionsExt;
use std::path;
use std::path::{Path, PathBuf};
use std::{fs, io};

use crate::{
    create_link, create_store_link, remove_dir, remove_file, remove_link, StorePath,
    ETC_STATE_FILE_NAME, SYSTEM_MANAGER_STATE_DIR, SYSTEM_MANAGER_STATIC_NAME,
};
use etc_tree::EtcTree;

use self::etc_tree::EtcFileStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl std::fmt::Display for EtcFilesConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Files in config:")?;
        self.entries.values().try_for_each(|entry| {
            writeln!(
                f,
                "target: {}, source:{}, mode:{}",
                entry.target.display(),
                entry.source,
                entry.mode
            )
        })
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
    log::debug!("{config}");

    let etc_dir = etc_dir(ephemeral);
    log::info!("Creating /etc entries in {}", etc_dir.display());

    DirBuilder::new().recursive(true).create(&etc_dir)?;

    let old_state = read_created_files()?;
    let initial_state = EtcTree::root_node();

    let (state, status) = create_etc_static_link(
        SYSTEM_MANAGER_STATIC_NAME,
        &config.static_env,
        &etc_dir,
        initial_state,
    );
    status?;

    let new_state = create_etc_links(config.entries.values(), &etc_dir, state, &old_state)
        .update_state(old_state, &|path, status| {
            log::debug!("Deactivating: {}", path.display());
            false
        });

    serialise_state(new_state)?;

    log::info!("Done");
    Ok(())
}

pub fn deactivate() -> Result<()> {
    let state = read_created_files()?;
    log::debug!("{:?}", state);

    serialise_state(state.deactivate(&|path, status| {
        log::debug!("Deactivating: {}", path.display());
        try_delete_path(path, status)
            .map_err(|e| {
                log::error!("Error deleting path: {}", path.display());
                log::error!("{e}");
                e
            })
            .is_ok()
    }))?;

    log::info!("Done");
    Ok(())
}

fn try_delete_path(path: &Path, status: &EtcFileStatus) -> Result<()> {
    // exists() returns false for broken symlinks
    if path.exists() || path.is_symlink() {
        if path.is_symlink() {
            remove_link(path)
        } else if path.is_file() {
            remove_file(path)
        } else if path.is_dir() {
            if path.read_dir()?.next().is_none() {
                remove_dir(path)
            } else {
                if let EtcFileStatus::Managed = status {
                    log::warn!("Managed directory not empty, ignoring: {}", path.display());
                }
                Ok(())
            }
        } else {
            Err(anyhow!("Unsupported file type! {}", path.display()))
        }
    } else {
        Ok(())
    }
}

fn create_etc_links<'a, E>(
    entries: E,
    etc_dir: &Path,
    state: EtcTree,
    old_state: &EtcTree,
) -> EtcTree
where
    E: Iterator<Item = &'a EtcFile>,
{
    entries.fold(state, |state, entry| {
        let (new_state, status) = create_etc_entry(entry, etc_dir, state, old_state);
        match status {
            Ok(_) => new_state,
            Err(e) => {
                log::error!("Error while creating file in {}: {e}", etc_dir.display());
                new_state
            }
        }
    })
}

fn create_etc_static_link(
    static_dir_name: &str,
    store_path: &StorePath,
    etc_dir: &Path,
    state: EtcTree,
) -> (EtcTree, Result<()>) {
    let static_path = etc_dir.join(static_dir_name);
    let (new_state, status) = create_dir_recursively(static_path.parent().unwrap(), state);
    match status.and_then(|_| create_store_link(store_path, static_path.as_path())) {
        Ok(_) => (new_state.register_managed_entry(&static_path), Ok(())),
        e => (new_state, e),
    }
}

// TODO: should we make sure that an existing file is managed before replacing it?
fn create_etc_link(link_target: &OsStr, etc_dir: &Path, state: EtcTree) -> (EtcTree, Result<()>) {
    let link_path = etc_dir.join(link_target);
    let (new_state, status) = create_dir_recursively(link_path.parent().unwrap(), state);
    match status.and_then(|_| {
        create_link(
            Path::new(".")
                .join(SYSTEM_MANAGER_STATIC_NAME)
                .join("etc")
                .join(link_target)
                .as_path(),
            link_path.as_path(),
        )
    }) {
        Ok(_) => (new_state.register_managed_entry(&link_path), Ok(())),
        e => (new_state, e),
    }
}

// TODO split up this function, and treat symlinks and copied files the same in the state file (ie
// include the root for both).
fn create_etc_entry(
    entry: &EtcFile,
    etc_dir: &Path,
    state: EtcTree,
    old_state: &EtcTree,
) -> (EtcTree, Result<()>) {
    if entry.mode == "symlink" {
        if let Some(path::Component::Normal(link_target)) = entry.target.components().next() {
            create_etc_link(link_target, etc_dir, state)
        } else {
            (
                state,
                Err(anyhow!("Cannot create link: {}", entry.target.display(),)),
            )
        }
    } else {
        let target_path = etc_dir.join(entry.target.as_path());
        let (new_state, status) = create_dir_recursively(target_path.parent().unwrap(), state);
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
                old_state,
            )
        }) {
            Ok(_) => (new_state.register_managed_entry(&target_path), Ok(())),
            e => (new_state, e),
        }
    }
}

fn create_dir_recursively(dir: &Path, state: EtcTree) -> (EtcTree, Result<()>) {
    use itertools::FoldWhile::{Continue, Done};
    use path::Component;

    let dirbuilder = DirBuilder::new();
    let (new_state, _, status) = dir
        .components()
        .fold_while(
            (state, PathBuf::from(path::MAIN_SEPARATOR_STR), Ok(())),
            |(state, path, _), component| match component {
                Component::RootDir => Continue((state, path, Ok(()))),
                Component::Normal(dir) => {
                    let new_path = path.join(dir);
                    if !new_path.exists() {
                        log::debug!("Creating path: {}", new_path.display());
                        match dirbuilder.create(new_path.as_path()) {
                            Ok(_) => {
                                let new_state = state.register_managed_entry(&new_path);
                                Continue((new_state, new_path, Ok(())))
                            }
                            Err(e) => Done((state, path, Err(anyhow!(e)))),
                        }
                    } else {
                        Continue((state, new_path, Ok(())))
                    }
                }
                otherwise => Done((
                    state,
                    path,
                    Err(anyhow!(
                        "Unexpected path component encountered: {:?}",
                        otherwise
                    )),
                )),
            },
        )
        .into_inner();
    (new_state, status)
}

fn copy_file(source: &Path, target: &Path, mode: &str, old_state: &EtcTree) -> Result<()> {
    let exists = target.try_exists()?;
    let old_status = old_state.get_status(target);
    log::debug!("Status for target {}: {old_status:?}", target.display());
    if !exists || *old_status == EtcFileStatus::Managed {
        fs::copy(source, target)?;
        let mode_int = u32::from_str_radix(mode, 8)?;
        fs::set_permissions(target, Permissions::from_mode(mode_int))?;
        Ok(())
    } else {
        anyhow::bail!("File {} already exists, ignoring.", target.display());
    }
}

fn etc_dir(ephemeral: bool) -> PathBuf {
    if ephemeral {
        Path::new("/run").join("etc")
    } else {
        Path::new("/etc").to_path_buf()
    }
}

fn serialise_state<E>(created_files: Option<E>) -> Result<()>
where
    E: AsRef<EtcTree>,
{
    let state_file = Path::new(SYSTEM_MANAGER_STATE_DIR).join(ETC_STATE_FILE_NAME);
    DirBuilder::new()
        .recursive(true)
        .create(SYSTEM_MANAGER_STATE_DIR)?;

    log::info!("Writing state info into file: {}", state_file.display());
    let writer = io::BufWriter::new(fs::File::create(state_file)?);

    if let Some(e) = created_files {
        serde_json::to_writer(writer, e.as_ref())?;
    } else {
        serde_json::to_writer(writer, &EtcTree::default())?;
    }
    Ok(())
}

fn read_created_files() -> Result<EtcTree> {
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
    Ok(EtcTree::default())
}
