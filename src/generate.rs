use anyhow::{anyhow, Context, Result};
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
    let full_flake_uri = find_flake_attr(flake_uri)?;

    log::info!("Building new system-manager generation...");
    log::info!("Running nix build...");
    let store_path = run_nix_build(full_flake_uri.as_ref()).and_then(get_store_path)?;
    log::info!("Build system-manager profile {store_path}");
    Ok(store_path)
}

fn find_flake_attr(flake_uri: &str) -> Result<String> {
    fn full_uri(flake: &str, attr: &str) -> String {
        format!("{flake}#{FLAKE_ATTR}.{attr}")
    }

    let flake_uri = flake_uri.trim_end_matches('#');
    let mut splitted = flake_uri.split('#');
    let flake = splitted
        .next()
        .ok_or_else(|| anyhow!("Invalid flake URI: {flake_uri}"))?;
    let attr = splitted.next();

    if splitted.next().is_some() {
        anyhow::bail!("Invalid flake URI, too many '#'s: {flake_uri}");
    }

    if let Some(attr) = attr {
        if try_flake_attr(flake, attr)? {
            return Ok(full_uri(flake, attr));
        } else {
            anyhow::bail!(
                "Explicitly provided flake URI does not point to a valid system-manager configuration: {}",
                format!("{flake}#{attr}")
            )
        }
    }

    let hostname_os = nix::unistd::gethostname()?;
    let hostname = hostname_os.to_string_lossy();
    let default = "default";

    if try_flake_attr(flake, &hostname)? {
        return Ok(full_uri(flake, &hostname));
    } else if try_flake_attr(flake, default)? {
        return Ok(full_uri(flake, default));
    };
    anyhow::bail!("No suitable flake attribute found, giving up.");
}

fn try_flake_attr(flake: &str, attr: &str) -> Result<bool> {
    let full_uri = format!("{flake}#{FLAKE_ATTR}.{attr}");
    log::info!("Trying flake URI: {full_uri}...");
    let status = try_nix_eval(flake, attr)?;
    if status {
        log::info!("Success, using {full_uri}");
    } else {
        log::info!("Attribute {full_uri} not found in flake.");
    };
    Ok(status)
}

fn get_store_path(nix_build_result: process::Output) -> Result<StorePath> {
    if nix_build_result.status.success() {
        String::from_utf8(nix_build_result.stdout)
            .map_err(|e| anyhow::anyhow!(e).context("Error reading nix build output."))
            .and_then(parse_nix_build_output)
    } else {
        anyhow::bail!("Nix build failed, see console output for details.")
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

fn run_nix_build(flake_uri: &str) -> Result<process::Output> {
    let output = nix_cmd()
        .arg("build")
        .arg(flake_uri)
        .arg("--json")
        // Nix outputs progress info on stderr and the final output on stdout,
        // so we inherit and output stderr directly to the terminal, but we
        // capture stdout as the result of this call
        .stderr(process::Stdio::inherit())
        .output()?;
    Ok(output)
}

fn try_nix_eval(flake: &str, attr: &str) -> Result<bool> {
    let output = nix_cmd()
        .arg("eval")
        .arg(format!("{flake}#{FLAKE_ATTR}"))
        .arg("--json")
        .arg("--apply")
        .arg(format!("a: a ? {attr}"))
        .stderr(process::Stdio::inherit())
        .output()?;
    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        let parsed_output: bool = serde_json::from_str(&stdout)?;
        Ok(parsed_output)
    } else {
        log::debug!("{}", String::from_utf8_lossy(output.stderr.as_ref()));
        Ok(false)
    }
}

fn nix_cmd() -> process::Command {
    let mut cmd = process::Command::new("nix");
    cmd.arg("--extra-experimental-features")
        .arg("nix-command flakes");
    cmd
}
