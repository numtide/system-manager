mod etc_tree;

use im::HashMap;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::fs::{DirBuilder, Permissions};
use std::os::unix::prelude::PermissionsExt;
use std::path;
use std::path::{Path, PathBuf};
use std::{fs, io};

use self::etc_tree::FileStatus;
use super::ActivationResult;
use crate::activate::ActivationError;
use crate::{
    create_link, create_store_link, etc_dir, remove_dir, remove_file, remove_link, StorePath,
    SYSTEM_MANAGER_STATIC_NAME,
};

pub use etc_tree::FileTree;

type EtcActivationResult = ActivationResult<FileTree>;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatedEtcFile {
    path: PathBuf,
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

    let state = create_etc_static_link(
        SYSTEM_MANAGER_STATIC_NAME,
        &config.static_env,
        &etc_dir,
        initial_state,
    )?;

    // Create the rest of the links
    let final_state = create_etc_links(config.entries.values(), &etc_dir, state, &old_state)
        .update_state(old_state, &try_delete_path)
        .unwrap_or_default();

    log::info!("Done");
    Ok(final_state)
}

pub fn deactivate(old_state: FileTree) -> EtcActivationResult {
    let final_state = old_state.deactivate(&try_delete_path).unwrap_or_default();

    log::info!("Done");
    Ok(final_state)
}

fn try_delete_path(path: &Path, status: &FileStatus) -> bool {
    fn do_try_delete(path: &Path, status: &FileStatus) -> anyhow::Result<()> {
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
                    if let FileStatus::Managed = status {
                        log::warn!("Managed directory not empty, ignoring: {}", path.display());
                    }
                    Ok(())
                }
            } else {
                anyhow::bail!("Unsupported file type! {}", path.display())
            }
        } else {
            Ok(())
        }
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

fn create_etc_links<'a, E>(
    entries: E,
    etc_dir: &Path,
    state: FileTree,
    old_state: &FileTree,
) -> FileTree
where
    E: Iterator<Item = &'a EtcFile>,
{
    entries.fold(state, |state, entry| {
        let new_state = create_etc_entry(entry, etc_dir, state, old_state);
        match new_state {
            Ok(new_state) => new_state,
            Err(ActivationError::WithPartialResult { result, source }) => {
                log::error!(
                    "Error while creating file in {}: {source:?}",
                    etc_dir.display()
                );
                result
            }
        }
    })
}

fn create_etc_static_link(
    static_dir_name: &str,
    store_path: &StorePath,
    etc_dir: &Path,
    state: FileTree,
) -> EtcActivationResult {
    let static_path = etc_dir.join(static_dir_name);
    let new_state = create_dir_recursively(static_path.parent().unwrap(), state);
    new_state.and_then(|new_state| {
        create_store_link(store_path, &static_path).map_or_else(
            |e| Err(ActivationError::with_partial_result(new_state.clone(), e)),
            |_| Ok(new_state.clone().register_managed_entry(&static_path)),
        )
    })
}

fn create_etc_link<P>(
    link_target: &P,
    etc_dir: &Path,
    state: FileTree,
    old_state: &FileTree,
) -> EtcActivationResult
where
    P: AsRef<Path>,
{
    fn link_dir_contents(
        link_target: &Path,
        absolute_target: &Path,
        etc_dir: &Path,
        state: FileTree,
        old_state: &FileTree,
        upwards_path: &Path,
    ) -> EtcActivationResult {
        let link_path = etc_dir.join(link_target);
        // Create the dir if it doesn't exist yet
        let dir_state = if !link_path.exists() {
            create_dir_recursively(&link_path, state)?
        } else {
            state
        };
        log::debug!("Entering into directory {}...", link_path.display());
        Ok(absolute_target
            .read_dir()
            .expect("Error reading the directory.")
            .fold(dir_state, |state, entry| {
                let new_state = go(
                    &link_target.join(
                        entry
                            .expect("Error reading the directory entry.")
                            .file_name(),
                    ),
                    etc_dir,
                    state,
                    old_state,
                    &upwards_path.join(".."),
                );
                match new_state {
                    Ok(new_state) => new_state,
                    Err(ActivationError::WithPartialResult { result, source }) => {
                        log::error!(
                            "Error while trying to link directory {}: {source:?}",
                            absolute_target.display()
                        );
                        result
                    }
                }
            }))
    }

    fn go(
        link_target: &Path,
        etc_dir: &Path,
        state: FileTree,
        old_state: &FileTree,
        upwards_path: &Path,
    ) -> EtcActivationResult {
        let link_path = etc_dir.join(link_target);
        let dir_state = create_dir_recursively(link_path.parent().unwrap(), state)?;
        let target = upwards_path
            .join(SYSTEM_MANAGER_STATIC_NAME)
            .join(link_target);
        let absolute_target = etc_dir.join(SYSTEM_MANAGER_STATIC_NAME).join(link_target);

        // Some versions of systemd ignore .wants and .requires directories when they are symlinks.
        // We therefore create them as actual directories and link their contents instead.
        let is_systemd_dependency_dir = absolute_target.is_dir()
            && absolute_target
                .parent()
                .map(|p| p.ends_with("systemd/system"))
                .unwrap_or(false)
            && link_target
                .extension()
                .filter(|ext| ["wants", "requires"].iter().any(|other| other == ext))
                .is_some();

        if (link_path.exists() && link_path.is_dir() && !old_state.is_managed(&link_path))
            || is_systemd_dependency_dir
        {
            if absolute_target.is_dir() {
                link_dir_contents(
                    link_target,
                    &absolute_target,
                    etc_dir,
                    dir_state,
                    old_state,
                    upwards_path,
                )
            } else {
                Err(ActivationError::with_partial_result(
                    dir_state,
                    anyhow::anyhow!(
                        "Unmanaged file or directory {} already exists, ignoring...",
                        link_path.display()
                    ),
                ))
            }
        } else if link_path.is_symlink()
            && link_path.read_link().expect("Error reading link.") == target
        {
            log::debug!("Link {} up to date.", link_path.display());
            Ok(dir_state.register_managed_entry(&link_path))
        } else if link_path.exists() && !old_state.is_managed(&link_path) {
            Err(ActivationError::with_partial_result(
                dir_state,
                anyhow::anyhow!("Unmanaged path already exists in filesystem, please remove it and run system-manager again: {}",
                                link_path.display()),
            ))
        } else {
            let result = if link_path.exists() {
                fs::remove_file(&link_path)
                    .map_err(|e| ActivationError::with_partial_result(dir_state.clone(), e))
            } else {
                Ok(())
            };

            match result.and_then(|_| {
                create_link(&target, &link_path)
                    .map_err(|e| ActivationError::with_partial_result(dir_state.clone(), e))
            }) {
                Ok(_) => Ok(dir_state.register_managed_entry(&link_path)),
                Err(e) => Err(e),
            }
        }
    }

    go(
        link_target.as_ref(),
        etc_dir,
        state,
        old_state,
        Path::new("."),
    )
}

fn create_etc_entry(
    entry: &EtcFile,
    etc_dir: &Path,
    state: FileTree,
    old_state: &FileTree,
) -> EtcActivationResult {
    if entry.mode == "symlink" {
        if let Some(path::Component::Normal(link_target)) = entry.target.components().next() {
            create_etc_link(&link_target, etc_dir, state, old_state)
        } else {
            Err(ActivationError::with_partial_result(
                state,
                anyhow::anyhow!("Cannot create link: {}", entry.target.display()),
            ))
        }
    } else {
        let target_path = etc_dir.join(&entry.target);
        let new_state = create_dir_recursively(target_path.parent().unwrap(), state)?;
        match copy_file(
            &entry.source.store_path.join(&entry.target),
            &target_path,
            &entry.mode,
            old_state,
        ) {
            Ok(_) => Ok(new_state.register_managed_entry(&target_path)),
            Err(e) => Err(ActivationError::with_partial_result(new_state, e)),
        }
    }
}

fn create_dir_recursively(dir: &Path, state: FileTree) -> EtcActivationResult {
    use itertools::FoldWhile::{Continue, Done};
    use path::Component;

    let dirbuilder = DirBuilder::new();
    let (new_state, _) = dir
        .components()
        .fold_while(
            (Ok(state), PathBuf::from(path::MAIN_SEPARATOR_STR)),
            |(state, path), component| match (state, component) {
                (Ok(state), Component::RootDir) => Continue((Ok(state), path)),
                (Ok(state), Component::Normal(dir)) => {
                    let new_path = path.join(dir);
                    if !new_path.exists() {
                        log::debug!("Creating path: {}", new_path.display());
                        match dirbuilder.create(&new_path) {
                            Ok(_) => {
                                let new_state = state.register_managed_entry(&new_path);
                                Continue((Ok(new_state), new_path))
                            }
                            Err(e) => Done((
                                Err(ActivationError::with_partial_result(
                                    state,
                                    anyhow::anyhow!(e).context(format!(
                                        "Error creating directory {}",
                                        new_path.display()
                                    )),
                                )),
                                path,
                            )),
                        }
                    } else {
                        Continue((Ok(state), new_path))
                    }
                }
                (Ok(state), otherwise) => Done((
                    Err(ActivationError::with_partial_result(
                        state,
                        anyhow::anyhow!("Unexpected path component encountered: {:?}", otherwise),
                    )),
                    path,
                )),
                (Err(e), _) => {
                    panic!("Something went horribly wrong! We should not get here: {e:?}.")
                }
            },
        )
        .into_inner();
    new_state
}

fn copy_file(source: &Path, target: &Path, mode: &str, old_state: &FileTree) -> anyhow::Result<()> {
    let exists = target.try_exists()?;
    if !exists || old_state.is_managed(target) {
        log::debug!(
            "Copying file {} to {}...",
            source.display(),
            target.display()
        );
        fs::copy(source, target)?;
        let mode_int = u32::from_str_radix(mode, 8)?;
        fs::set_permissions(target, Permissions::from_mode(mode_int))?;
        Ok(())
    } else {
        anyhow::bail!("File {} already exists, ignoring.", target.display());
    }
}
