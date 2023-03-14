use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::DirBuilder;
use std::path::Path;
use std::{fs, process, str};

use super::{create_store_link, StorePath, FLAKE_ATTR, GCROOT_PATH, PROFILE_DIR, PROFILE_NAME};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NixBuildOutput {
    drv_path: String,
    outputs: HashMap<String, String>,
}

pub fn generate(store_path: &StorePath) -> Result<()> {
    let profile_dir = Path::new(PROFILE_DIR);
    let profile_name = Path::new(PROFILE_NAME);

    log::info!("Creating new generation from {store_path}");
    install_nix_profile(store_path, profile_dir, profile_name)?;

    log::info!("Registering GC root...");
    create_gcroot(GCROOT_PATH, &profile_dir.join(profile_name))?;

    log::info!("Done");
    Ok(())
}

fn install_nix_profile(
    store_path: &StorePath,
    profile_dir: &Path,
    profile_name: &Path,
) -> Result<process::ExitStatus> {
    DirBuilder::new().recursive(true).create(profile_dir)?;
    let status = process::Command::new("nix-env")
        .arg("--profile")
        .arg(profile_dir.join(profile_name))
        .arg("--set")
        .arg(&store_path.store_path)
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .status()?;
    Ok(status)
}

fn create_gcroot(gcroot_path: &str, profile_path: &Path) -> Result<()> {
    let profile_store_path = fs::canonicalize(profile_path)?;
    let store_path = StorePath::from(String::from(profile_store_path.to_string_lossy()));
    create_store_link(&store_path, Path::new(gcroot_path))
}

pub fn build(flake_uri: &str) -> Result<StorePath> {
    // FIXME: we should not hard-code the system here
    let flake_attr = format!("{FLAKE_ATTR}.x86_64-linux");

    log::info!("Building new system-manager generation...");
    log::info!("Running nix build...");
    let store_path = run_nix_build(flake_uri, &flake_attr).and_then(get_store_path)?;
    log::info!("Build system-manager profile {store_path}");
    Ok(store_path)
}

fn get_store_path(nix_build_result: process::Output) -> Result<StorePath> {
    if nix_build_result.status.success() {
        String::from_utf8(nix_build_result.stdout)
            .map_err(anyhow::Error::from)
            .and_then(parse_nix_build_output)
    } else {
        String::from_utf8(nix_build_result.stderr)
            .map_err(anyhow::Error::from)
            .and_then(|e| {
                log::error!("{e}");
                anyhow::bail!("Nix build failed.")
            })
    }
}

fn parse_nix_build_output(output: String) -> Result<StorePath> {
    let expected_output_name = "out";
    let results: Vec<NixBuildOutput> =
        serde_json::from_str(&output).context("Error reading nix build output")?;

    if let [result] = results.as_slice() {
        if let Some(store_path) = result.outputs.get(expected_output_name) {
            return Ok(StorePath::from(store_path.to_owned()));
        }
        anyhow::bail!("No output '{expected_output_name}' found in nix build result.")
    }
    anyhow::bail!("Multiple build results were returned, we cannot handle that yet.")
}

fn run_nix_build(flake_uri: &str, flake_attr: &str) -> Result<process::Output> {
    let output = process::Command::new("nix")
        .arg("build")
        .arg(format!("{flake_uri}#{flake_attr}"))
        .arg("--json")
        // Nix outputs progress info on stderr and the final output on stdout,
        // so we inherit and output stderr directly to the terminal, but we
        // capture stdout as the result of this call
        .stderr(process::Stdio::inherit())
        .output()?;
    Ok(output)
}
