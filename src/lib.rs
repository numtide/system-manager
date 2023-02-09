pub mod activate;
pub mod generate;
mod systemd;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::os::unix;
use std::path::Path;
use std::{fs, str};

const FLAKE_ATTR: &str = "serviceConfig";
const PROFILE_PATH: &str = "/nix/var/nix/profiles/system-manager";
const GCROOT_PATH: &str = "/nix/var/nix/gcroots/system-manager-current";
const SYSTEMD_UNIT_DIR: &str = "/run/systemd/system";
const SYSTEM_MANAGER_STATE_DIR: &str = "/var/lib/system-manager/state";
const STATE_FILE_NAME: &str = "services.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorePath {
    pub store_path: String,
}

impl From<String> for StorePath {
    fn from(path: String) -> Self {
        StorePath {
            store_path: path.trim().into(),
        }
    }
}

impl std::fmt::Display for StorePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.store_path)
    }
}

fn create_store_link(store_path: &StorePath, from: &Path) -> Result<()> {
    log::info!("Creating symlink: {} -> {}", from.display(), store_path);
    if from.is_symlink() {
        fs::remove_file(from)?;
    }
    unix::fs::symlink(&store_path.store_path, from).map_err(anyhow::Error::from)
}

fn remove_store_link(from: &Path) -> Result<()> {
    log::info!("Removing symlink: {}", from.display());
    if from.is_symlink() {
        fs::remove_file(from)?;
        return Ok(());
    }
    anyhow::bail!("Not a symlink!")
}

pub fn compose<A, B, C, G, F>(f: F, g: G) -> impl Fn(A) -> C
where
    F: Fn(B) -> C,
    G: Fn(A) -> B,
{
    move |x| f(g(x))
}
