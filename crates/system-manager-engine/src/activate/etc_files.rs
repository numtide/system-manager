mod etc_tree;

use anyhow::{anyhow, Context};
use im::HashMap;
use regex;
use serde::{Deserialize, Serialize};
use std::fs::Permissions;
use std::os::unix::prelude::PermissionsExt;
use std::os::unix::{self, fs as unixfs};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{fs, io};

use self::etc_tree::FileStatus;
use super::ActivationResult;
use crate::activate::ActivationError;
use crate::{create_link, etc_dir, remove_dir, remove_file, remove_link, StorePath};

pub use etc_tree::FileTree;

type EtcActivationResult = ActivationResult<FileTree>;

static UID_GID_REGEX: OnceLock<regex::Regex> = OnceLock::new();

fn get_uid_gid_regex() -> &'static regex::Regex {
    UID_GID_REGEX.get_or_init(|| regex::Regex::new(r"^\+[0-9]+$").expect("could not compile regex"))
}

const BACKUP_SUFFIX: &str = ".system-manager-backup";

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
    #[serde(default)]
    replace_existing: bool,
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
        let out: String = itertools::intersperse(
            self.entries.values().map(|entry| {
                format!(
                    "target: {}, source:{}, mode:{}",
                    entry.target.display(),
                    entry.source,
                    entry.mode
                )
            }),
            "\n".to_owned(),
        )
        .collect();
        write!(f, "{out}")
    }
}

fn read_config(store_path: &StorePath) -> anyhow::Result<EtcFilesConfig> {
    log::info!("Reading etc file definitions...");
    let file = fs::File::open(
        Path::new(&store_path.store_path)
            .join("etcFiles")
            .join("etcFiles.json"),
    )?;
    let reader = io::BufReader::new(file);
    let config: EtcFilesConfig = serde_json::from_reader(reader)?;
    log::debug!("{config}");
    Ok(config)
}

pub fn activate(
    store_path: &StorePath,
    old_state: FileTree,
    ephemeral: bool,
) -> EtcActivationResult {
    let config = read_config(store_path)
        .map_err(|e| ActivationError::with_partial_result(old_state.clone(), e))?;

    let etc_dir = etc_dir(ephemeral);
    log::info!("Creating /etc entries in {}", etc_dir.display());

    let initial_state = FileTree::root_node();

    // Walk through static link, list entries
    let mut entries = match list_static_entries(config.static_env) {
        Ok(e) => e,
        Err(e) => {
            return Err(ActivationError::WithPartialResult {
                result: initial_state.clone(),
                source: e,
            })
        }
    };
    let mut non_static_entries: Vec<EtcFile> = config
        .entries
        .values()
        .filter(|v| v.mode != "symlink")
        .cloned()
        .map(|mut v| {
            v.source.store_path = v.source.store_path.join(&v.target);
            v
        })
        .collect();
    entries.append(&mut non_static_entries);
    // Create dirs and link/copy entries
    let final_state = match create_etc_files(entries, initial_state.clone(), old_state) {
        Ok(e) => e,
        Err(e) => {
            return Err(ActivationError::WithPartialResult {
                result: initial_state,
                source: e.into(),
            })
        }
    };
    log::info!("Done");
    Ok(final_state)
}

pub fn deactivate(old_state: FileTree) -> EtcActivationResult {
    let final_state = old_state.deactivate(&try_delete_path).unwrap_or_default();

    log::info!("Done");
    Ok(final_state)
}

fn backup_path_for(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(BACKUP_SUFFIX);
    PathBuf::from(s)
}

fn backup_existing_file(path: &Path) -> anyhow::Result<()> {
    let backup_path = backup_path_for(path);
    log::info!(
        "Backing up existing file {} to {}",
        path.display(),
        backup_path.display()
    );
    fs::rename(path, &backup_path)?;
    Ok(())
}

fn restore_backup(path: &Path) -> anyhow::Result<()> {
    let backup_path = backup_path_for(path);
    if backup_path.exists() || backup_path.is_symlink() {
        log::info!(
            "Restoring backup {} to {}",
            backup_path.display(),
            path.display()
        );
        fs::rename(&backup_path, path)?;
    } else {
        log::warn!(
            "Backup file {} not found, cannot restore",
            backup_path.display()
        );
    }
    Ok(())
}

fn try_delete_path(path: &Path, status: &FileStatus) -> bool {
    fn do_try_delete(path: &Path, status: &FileStatus) -> anyhow::Result<()> {
        // exists() returns false for broken symlinks
        if path.exists() || path.is_symlink() {
            if path.is_symlink() {
                remove_link(path)?;
            } else if path.is_file() {
                remove_file(path)?;
            } else if path.is_dir() {
                if path.read_dir()?.next().is_none() {
                    remove_dir(path)?;
                } else {
                    if matches!(status, FileStatus::Managed | FileStatus::ManagedWithBackup) {
                        log::warn!("Managed directory not empty, ignoring: {}", path.display());
                    }
                    return Ok(());
                }
            } else {
                anyhow::bail!("Unsupported file type! {}", path.display())
            }
        }

        if *status == FileStatus::ManagedWithBackup {
            restore_backup(path)?;
        }

        Ok(())
    }

    log::debug!("Deactivating: {}", path.display());
    do_try_delete(path, status)
        .map_err(|e| {
            log::error!("Error deleting path: {}", path.display());
            log::error!("{e}");
            e
        })
        .is_ok()
}

fn list_static_entries(static_path: StorePath) -> anyhow::Result<Vec<EtcFile>> {
    let mut files = Vec::new();
    #[derive(Clone)]
    struct DirToVisit {
        absolute_path: PathBuf,
        path_from_root: PathBuf,
    }
    let mut dirs_to_visit: Vec<DirToVisit> = vec![DirToVisit {
        absolute_path: static_path.store_path,
        path_from_root: PathBuf::from(""),
    }];
    let mut i = 0;

    while i < dirs_to_visit.len() {
        let dir = dirs_to_visit
            .get(i)
            .context("ERROR: index error in dir loop")?
            .clone();
        let dir_content = fs::read_dir(&dir.absolute_path)?;
        for file in dir_content {
            let file = file?;
            let canon_path = fs::canonicalize(file.path()).context(format!(
                "Failed to get the canonical path of {}",
                file.path().display()
            ))?;
            if canon_path.is_dir() {
                log::debug!("{} is a dir", canon_path.display());
                let dirname = file.file_name();
                let mut path_from_root = dir.path_from_root.clone();
                path_from_root.push(dirname);
                dirs_to_visit.push(DirToVisit {
                    absolute_path: canon_path,
                    path_from_root,
                });
            } else {
                log::debug!("{} is a file", file.path().display());
                let etc_file = EtcFile {
                    source: StorePath {
                        store_path: canon_path,
                    },
                    target: PathBuf::from("/etc")
                        .join(dir.path_from_root.clone())
                        .join(file.file_name()),
                    uid: 0,
                    gid: 0,
                    group: "".to_string(),
                    user: "".to_string(),
                    mode: "symlink".to_string(),
                    replace_existing: false,
                };
                log::debug!(
                    "add file: {:?}, path_from_root: {:?}, absolute_path: {:?}",
                    etc_file,
                    dir.path_from_root,
                    dir.absolute_path
                );
                files.push(etc_file);
            }
        }
        i += 1;
    }
    Ok(files)
}

fn create_etc_files(
    mut files: Vec<EtcFile>,
    mut state: FileTree,
    old_state: FileTree,
) -> EtcActivationResult {
    files.sort_by(|a, b| a.target.cmp(&b.target));
    log::debug!("FILES: {:?}", files);
    for file in files {
        let target = PathBuf::from("/etc").join(&file.target);
        log::debug!(
            "Creating {} to {} ({})",
            file.source,
            target.display(),
            file.target.display()
        );
        // Create all dirs
        log::debug!("Creating all dirs up to {:?}", target.parent());
        target.parent().map(fs::create_dir_all);

        // Target is a symlink
        if file.mode == "symlink" {
            log::debug!("{} is a symlink", file.source);
            // TODO: add condition to se if it's already managed
            if target.exists() {
                if old_state.is_managed(&target) {
                    log::debug!(
                        "{} is managed by system-manager. Deleting.",
                        &target.display()
                    );
                    fs::remove_file(&target).map_err(|e| ActivationError::WithPartialResult {
                        result: state.clone(),
                        source: e.into(),
                    })?;
                    log::debug!("{} is managed by system-manager.", &target.display());
                    unix::fs::symlink(file.source.store_path, &target).map_err(|e| {
                        ActivationError::WithPartialResult {
                            result: state.clone(),
                            source: e.into(),
                        }
                    })?;
                    state = state.register_managed_entry(&target);
                } else if file.replace_existing {
                    log::debug!("{} already exist. Backup and link again.", file.source);
                    state = backup_and_link(&target, &file.source.store_path, state)?;
                } else {
                    return Err(ActivationError::WithPartialResult {
                        result: state.clone(),
                        source: anyhow!("{} already exists. Set replace_existing if you're willing to override it.", target.display()),
                    });
                }
            } else {
                log::debug!("Symlink {} => {}", file.source, target.display());
                unix::fs::symlink(file.source.store_path, &target).map_err(|e| {
                    ActivationError::WithPartialResult {
                        result: state.clone(),
                        source: e.into(),
                    }
                })?;
                state = state.register_managed_entry(&target);
            }
        } else {
            log::debug!("{} is a regular file", file.source);
            // target is a regular file
            // TODO: add condition to se if it's already managed
            if target.exists() {
                if old_state.is_managed(&target) {
                    log::debug!(
                        "{} is managed by system-manager, deleting.",
                        &target.display()
                    );
                    fs::remove_file(&target).map_err(|e| ActivationError::WithPartialResult {
                        result: state.clone(),
                        source: e.into(),
                    })?;
                    copy_file(&file.source.store_path, &target, &file, &state).map_err(|e| {
                        ActivationError::WithPartialResult {
                            result: state.clone(),
                            source: e,
                        }
                    })?;
                    state = state.register_managed_entry(&target)
                } else if file.replace_existing {
                    log::debug!("{} already exists. Backup and link again.", file.source);
                    backup_existing_file(target.as_path()).map_err(|e| {
                        ActivationError::WithPartialResult {
                            result: state.clone(),
                            source: e,
                        }
                    })?;
                } else {
                    return Err(ActivationError::WithPartialResult {
                        result: state.clone(),
                        source: anyhow!("{} already exists. Set replace_existing if you're willing to override it", target.display()),
                    });
                }
            } else {
                log::debug!(
                    "Copy {} => {}",
                    file.source.store_path.display(),
                    target.display()
                );
                copy_file(&file.source.store_path, &target, &file, &state).map_err(|e| {
                    ActivationError::WithPartialResult {
                        result: state.clone(),
                        source: e,
                    }
                })?;
                state = state.register_managed_entry(&target)
            }
        }
    }
    Ok(state)
}

fn backup_and_link(target: &Path, link_path: &Path, dir_state: FileTree) -> EtcActivationResult {
    backup_existing_file(link_path)
        .map_err(|e| ActivationError::with_partial_result(dir_state.clone(), e))?;
    create_link(target, link_path)
        .map_err(|e| ActivationError::with_partial_result(dir_state.clone(), e))?;
    Ok(dir_state.register_backed_up_entry(link_path))
}

fn find_uid(entry: &EtcFile) -> anyhow::Result<u32> {
    if !get_uid_gid_regex().is_match(&entry.user) {
        nix::unistd::User::from_name(&entry.user)
            .map(|maybe_user| {
                maybe_user.map_or_else(
                    || {
                        log::warn!(
                            "Specified user {} not found, defaulting to root",
                            &entry.user
                        );
                        0
                    },
                    |user| user.uid.as_raw(),
                )
            })
            .map_err(|err| anyhow::anyhow!(err).context("Failed to determine user"))
    } else {
        Ok(entry.uid)
    }
}

fn find_gid(entry: &EtcFile) -> anyhow::Result<u32> {
    if !get_uid_gid_regex().is_match(&entry.group) {
        nix::unistd::Group::from_name(&entry.group)
            .map(|maybe_group| {
                maybe_group.map_or_else(
                    || {
                        log::warn!(
                            "Specified group {} not found, defaulting to root",
                            &entry.group
                        );
                        0
                    },
                    |group| group.gid.as_raw(),
                )
            })
            .map_err(|err| anyhow::anyhow!(err).context("Failed to determine group"))
    } else {
        Ok(entry.gid)
    }
}

/// Copy a file from source to target. Returns `Ok(true)` if a pre-existing
/// file was backed up, `Ok(false)` if no backup was needed.
fn copy_file(
    source: &Path,
    target: &Path,
    entry: &EtcFile,
    old_state: &FileTree,
) -> anyhow::Result<bool> {
    let exists = target.try_exists()?;
    let backed_up = if exists && !old_state.is_managed(target) {
        if entry.replace_existing {
            backup_existing_file(target)?;
            true
        } else {
            anyhow::bail!("File {} already exists, ignoring.", target.display());
        }
    } else {
        false
    };
    log::debug!(
        "Copying file {} to {}...",
        source.display(),
        target.display()
    );
    fs::copy(source, target)?;
    let mode_int = u32::from_str_radix(&entry.mode, 8)?;
    fs::set_permissions(target, Permissions::from_mode(mode_int))?;
    unixfs::chown(target, Some(find_uid(entry)?), Some(find_gid(entry)?))?;
    Ok(backed_up)
}
