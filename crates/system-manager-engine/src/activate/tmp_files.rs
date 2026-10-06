use crate::activate;

use super::ActivationResult;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process;

type TmpFilesActivationResult = ActivationResult<()>;

fn find_systemd_tmpfiles_bin() -> Option<PathBuf> {
    const FALLBACK_DIRS: &[&str] = &["/usr/bin", "/bin", "/run/current-system/sw/bin"];

    let path_var = std::env::var("PATH").unwrap_or_default();
    let search_dirs = path_var
        .split(':')
        .filter(|d| !d.is_empty())
        .chain(FALLBACK_DIRS.iter().copied());

    for dir in search_dirs {
        let p = Path::new(dir).join("systemd-tmpfiles");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn activate(etc_tree: &HashSet<PathBuf>) -> TmpFilesActivationResult {
    let tmp_files_prefix = PathBuf::from("/etc/tmpfiles.d");
    // List and collect managed files under /etc/tmpFiles.d
    let tmpfiles_conf_files: Vec<&str> = etc_tree
        .iter()
        .filter_map(|p| {
            if p.starts_with(&tmp_files_prefix) {
                p.to_str()
            } else {
                None
            }
        })
        .collect();

    let exe = find_systemd_tmpfiles_bin().ok_or_else(|| {
        let path_env = std::env::var("PATH").unwrap_or_else(|_| "<not set>".to_string());
        activate::ActivationError::WithPartialResult {
            result: (),
            source: anyhow::anyhow!(
                "Could not find 'systemd-tmpfiles' binary (PATH: '{}', checked standard paths: /usr/bin, /bin, /run/current-system/sw/bin)",
                path_env
            ),
        }
    })?;

    let mut cmd = process::Command::new(exe);
    cmd.arg("--create")
        .arg("--remove")
        .args(tmpfiles_conf_files);
    log::debug!("running {:#?}", cmd);
    let output = cmd
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|e| activate::ActivationError::WithPartialResult {
            result: (),
            source: anyhow::anyhow!("Error executing systemd-tmpfiles: {e}"),
        })?;

    output.status.success().then_some(()).ok_or_else(|| {
        activate::ActivationError::WithPartialResult {
            result: (),
            source: anyhow::anyhow!(
                "Error while creating tmpfiles\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(output.stdout.as_ref()),
                String::from_utf8_lossy(output.stderr.as_ref())
            ),
        }
    })
}
