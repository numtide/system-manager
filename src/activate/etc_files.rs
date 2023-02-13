use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::DirBuilder;
use std::path;
use std::path::{Path, PathBuf};
use std::{fs, io};

use crate::{create_link, create_store_link, StorePath};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EtcFilesConfig {
    entries: EtcFiles,
    static_env: StorePath,
}

pub fn activate(store_path: StorePath, ephemeral: bool) -> Result<()> {
    log::info!("Reading etc file definitions...");
    let file = fs::File::open(Path::new(&store_path.store_path).join("etcFiles/etcFiles.json"))?;
    let reader = io::BufReader::new(file);
    let config: EtcFilesConfig = serde_json::from_reader(reader)?;
    log::debug!("{:?}", config);

    let etc_dir = etc_dir(ephemeral);
    log::debug!("Storing /etc entries in {}", etc_dir.display());

    DirBuilder::new().recursive(true).create(&etc_dir)?;
    create_store_link(
        &config.static_env,
        etc_dir.join(".system-manager-static").as_path(),
    )?;

    config
        .entries
        .into_iter()
        .try_for_each(|(name, entry)| create_etc_link(&name, &entry, &etc_dir))?;

    Ok(())
}

fn create_etc_link(name: &str, entry: &EtcFile, etc_dir: &Path) -> Result<()> {
    if entry.mode == "symlink" {
        if let Some(path::Component::Normal(link_target)) =
            entry.target.components().into_iter().next()
        {
            create_link(
                Path::new(".")
                    .join(".system-manager-static")
                    .join("etc")
                    .join(link_target)
                    .as_path(),
                etc_dir.join(link_target).as_path(),
            )
        } else {
            anyhow::bail!("Cannot create link for this entry ({}).", name)
        }
    } else {
        Ok(())
    }
}

fn etc_dir(ephemeral: bool) -> PathBuf {
    if ephemeral {
        Path::new("/run").join("etc")
    } else {
        Path::new("/etc").to_path_buf()
    }
}
