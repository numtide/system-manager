pub mod activate;
pub mod generate;
mod systemd;

use anyhow::Result;
use std::os::unix;
use std::path::Path;
use std::{fs, str};

const FLAKE_ATTR: &str = "serviceConfig";
const PROFILE_NAME: &str = "service-manager";

#[derive(Debug, Clone)]
pub struct StorePath {
    pub path: String,
}

impl From<String> for StorePath {
    fn from(path: String) -> Self {
        StorePath {
            path: path.trim().into(),
        }
    }
}

impl std::fmt::Display for StorePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)
    }
}

fn create_store_link(store_path: &StorePath, from: &Path) -> Result<()> {
    log::info!("Creating symlink: {} -> {}", from.display(), store_path);
    if from.is_symlink() {
        fs::remove_file(from)?;
    }
    unix::fs::symlink(&store_path.path, from).map_err(anyhow::Error::from)
}

pub fn compose<A, B, C, G, F>(f: F, g: G) -> impl Fn(A) -> C
where
    F: Fn(B) -> C,
    G: Fn(A) -> B,
{
    move |x| f(g(x))
}
