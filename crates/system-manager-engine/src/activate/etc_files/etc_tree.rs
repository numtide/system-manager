use im::HashMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::activate::{services, EtcFilesState, StateV1};

/// Legacy datatype used to migrate to the new state format.
///
/// This should be deleted from the codebase at some point. Once we assume most users migrated to the new version.
/// It cannot be before next release.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateV0 {
    pub(crate) file_tree: FileTree,
    pub(crate) services: services::Services,
}

impl From<StateV0> for StateV1 {
    fn from(v0: StateV0) -> StateV1 {
        let services = v0.services;
        let file_tree: EtcFilesState = v0.file_tree.into();
        StateV1 {
            file_tree,
            services,
            version: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
    Managed,
    ManagedWithBackup,
    Unmanaged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTree {
    status: FileStatus,
    pub(crate) path: PathBuf,
    // TODO directories and files are now both represented as a string associated with a nested
    // map. For files the nested map is simple empty.
    // We could potentially optimise this.
    pub(crate) nested: HashMap<String, FileTree>,
}

impl From<FileTree> for EtcFilesState {
    fn from(ft: FileTree) -> EtcFilesState {
        let mut paths_to_go: Vec<FileTree> = vec![ft];
        let mut etc_files = EtcFilesState::default();
        let mut i = 0;
        while i < paths_to_go.len() {
            let elem = paths_to_go
                .get(i)
                .expect("ERROR: index error in paths_to_go loop")
                .clone();
            for nested in elem.nested.clone() {
                paths_to_go.push(nested.1);
            }
            if !elem.path.is_dir() {
                match elem.status {
                    FileStatus::Managed => {
                        etc_files.files.insert(elem.path);
                    }
                    FileStatus::ManagedWithBackup => {
                        etc_files.backed_up_files.insert(elem.path);
                    }
                    FileStatus::Unmanaged => {}
                };
            }
            i += 1;
        }
        etc_files
    }
}
