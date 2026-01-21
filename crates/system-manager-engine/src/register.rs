use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::DirBuilder;
use std::mem;
use std::path::Path;
use std::{fs, process, str};

use super::{
    create_store_link, NixOptions, StorePath, FLAKE_ATTR, GCROOT_PATH, PROFILE_DIR, PROFILE_NAME,
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NixBuildOutput {
    drv_path: String,
    outputs: HashMap<String, String>,
}

pub fn register(store_path: &StorePath, nix_options: &NixOptions) -> Result<()> {
    let profile_dir = Path::new(PROFILE_DIR);
    let profile_name = Path::new(PROFILE_NAME);

    log::info!("Creating new generation from {store_path}");
    let status = install_nix_profile(store_path, profile_dir, profile_name, nix_options)?;
    if !status.success() {
        anyhow::bail!("Error installing the nix profile, see above for details.");
    }

    log::info!("Registering GC root...");
    create_gcroot(GCROOT_PATH, &profile_dir.join(profile_name))?;

    log::info!("Done");
    Ok(())
}

fn install_nix_profile(
    store_path: &StorePath,
    profile_dir: &Path,
    profile_name: &Path,
    nix_options: &NixOptions,
) -> Result<process::ExitStatus> {
    DirBuilder::new()
        .recursive(true)
        .create(profile_dir)
        .context("While creating the profile dir.")?;
    let mut cmd = process::Command::new("nix-env");
    cmd.arg("--profile")
        .arg(profile_dir.join(profile_name))
        .arg("--set")
        .arg(&store_path.store_path);
    nix_options.options.iter().for_each(|option| {
        cmd.arg("--option").arg(&option.0).arg(&option.1);
    });
    let status = cmd
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .status()
        .context("While running nix-env.")?;
    Ok(status)
}

fn create_gcroot(gcroot_path: &str, profile_path: &Path) -> Result<()> {
    let profile_store_path = fs::canonicalize(profile_path)?;
    let store_path = StorePath::from(String::from(profile_store_path.to_string_lossy()));
    create_store_link(&store_path, Path::new(gcroot_path))
}

pub fn build(flake_uri: &str, nix_options: &NixOptions) -> Result<StorePath> {
    let full_flake_uri = find_flake_attr(flake_uri, nix_options)?;

    log::info!("Building new system-manager generation...");
    log::info!("Running nix build...");
    let store_path =
        run_nix_build(full_flake_uri.as_ref(), nix_options).and_then(get_store_path)?;
    log::info!("Built system-manager profile {store_path}");
    Ok(store_path)
}

fn find_flake_attr(flake_uri: &str, nix_options: &NixOptions) -> Result<String> {
    let flake_uri = flake_uri.trim_end_matches('#');
    let mut splitted = flake_uri.split('#');
    let flake = splitted
        .next()
        .ok_or_else(|| anyhow!("Invalid flake URI: {flake_uri}"))?;
    let attr = splitted.next();

    if splitted.next().is_some() {
        anyhow::bail!("Invalid flake URI, too many '#'s: {flake_uri}");
    }

    let system = get_nix_system(nix_options)?;

    if let Some(attr) = attr {
        let Some(full_uri) = try_flake_attr(flake, attr, nix_options, &system)? else {
            anyhow::bail!(
                "Explicitly provided flake URI does not point to a valid system-manager configuration: {flake}#{attr}"
            )
        };
        return Ok(full_uri);
    }

    let hostname_os = nix::unistd::gethostname()?;
    let hostname = escape_nix_string(&hostname_os.to_string_lossy());
    let default = "default";

    if let Some(full_uri) = try_flake_attr(flake, &hostname, nix_options, &system)? {
        return Ok(full_uri);
    } else if let Some(full_uri) = try_flake_attr(flake, default, nix_options, &system)? {
        return Ok(full_uri);
    };
    anyhow::bail!("No suitable flake attribute found, giving up.");
}

fn escape_nix_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    let mut i = 0;
    for (j, _) in s.match_indices(['"', '\\']) {
        out += &s[i..j];
        i = j;
    }
    out += &s[i..];
    out.push('"');
    out
}

fn try_flake_attr(
    flake: &str,
    attr: &str,
    nix_options: &NixOptions,
    system: &str,
) -> Result<Option<String>> {
    let try_flake_attr_impl = |attr: &str| {
        let full_uri = format!("{flake}#{FLAKE_ATTR}.{attr}");
        log::info!("Trying flake URI: {full_uri}...");
        let status = try_nix_eval(flake, attr, nix_options)?;
        if status {
            log::info!("Success, using {full_uri}");
            Ok(Some(full_uri))
        } else {
            log::info!("Attribute {full_uri} not found in flake.");
            Ok(None)
        }
    };
    if let Some(result) = try_flake_attr_impl(&format!("{system}.{attr}"))? {
        Ok(Some(result))
    } else {
        let attr = attr.strip_prefix(&format!("{FLAKE_ATTR}.")).unwrap_or(attr);
        try_flake_attr_impl(attr)
    }
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
    let mut results: Vec<NixBuildOutput> =
        serde_json::from_str(&output).context("Error reading nix build output")?;

    if let [result] = results.as_mut_slice() {
        if let Some(store_path) = result.outputs.get_mut(expected_output_name) {
            return Ok(StorePath::from(mem::take(store_path)));
        }
        anyhow::bail!("No output '{expected_output_name}' found in nix build result.")
    }
    anyhow::bail!("Multiple build results were returned, we cannot handle that yet.")
}

fn run_nix_build(flake_uri: &str, nix_options: &NixOptions) -> Result<process::Output> {
    let mut cmd = nix_cmd(nix_options);
    cmd.arg("build").arg(flake_uri).arg("--json");

    log::debug!("Running nix command: {cmd:?}");

    let output = cmd
        // Nix outputs progress info on stderr and the final output on stdout,
        // so we inherit and output stderr directly to the terminal, but we
        // capture stdout as the result of this call
        .stderr(process::Stdio::inherit())
        .output()?;
    Ok(output)
}

fn try_nix_eval(flake: &str, attr: &str, nix_options: &NixOptions) -> Result<bool> {
    let mut cmd = nix_cmd(nix_options);
    cmd.arg("eval")
        .arg(format!("{flake}#{FLAKE_ATTR}"))
        .arg("--json")
        .arg("--apply")
        .arg(format!("_: _ ? {attr}"));

    log::debug!("Running nix command: {cmd:?}");

    let output = cmd.stderr(process::Stdio::inherit()).output()?;
    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        let parsed_output: bool = serde_json::from_str(&stdout)?;
        Ok(parsed_output)
    } else {
        log::debug!("{}", String::from_utf8_lossy(output.stderr.as_ref()));
        Ok(false)
    }
}

fn get_nix_system(nix_options: &NixOptions) -> Result<String> {
    let mut cmd = nix_cmd(nix_options);
    cmd.arg("config").arg("show").arg("system");

    log::debug!("Running nix command: {cmd:?}");

    let output = cmd.stderr(process::Stdio::inherit()).output()?;
    if output.status.success() {
        Ok(std::str::from_utf8(&output.stdout)?.trim().to_string())
    } else {
        log::error!("{}", String::from_utf8_lossy(output.stderr.as_ref()));
        anyhow::bail!("Could not get currentSystem");
    }
}

fn nix_cmd(nix_options: &NixOptions) -> process::Command {
    let mut cmd = process::Command::new("nix");
    cmd.arg("--extra-experimental-features")
        .arg("nix-command flakes");
    nix_options.options.iter().for_each(|option| {
        cmd.arg("--option").arg(&option.0).arg(&option.1);
    });
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_nix_eval() {
        let flake = "./test/rust/register";
        let nix_options = &NixOptions::new(vec![]);

        assert!(try_nix_eval(flake, "identifier-key", nix_options).unwrap());
        assert!(try_nix_eval(flake, "\"string.literal/key\"", nix_options).unwrap());
        assert!(!try_nix_eval(flake, "_identifier-key", nix_options).unwrap());
        assert!(!try_nix_eval(flake, "\"_string.literal/key\"", nix_options).unwrap());
    }
}
