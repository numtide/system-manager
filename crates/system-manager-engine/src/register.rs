use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::DirBuilder;
use std::mem;
use std::path::Path;
use std::{fs, process, str};

use crate::NixBuildOptions;

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

pub fn build(nix_build_options: &NixBuildOptions, nix_options: &NixOptions) -> Result<StorePath> {
    let attr = find_flake_attr(nix_build_options, nix_options)?;

    log::info!("Building new system-manager generation...");
    log::info!("Running nix build...");
    let store_path =
        run_nix_build(nix_build_options, attr, nix_options).and_then(get_store_path)?;
    log::info!("Built system-manager profile {store_path}");
    Ok(store_path)
}

fn find_flake_attr(
    nix_build_options: &NixBuildOptions,
    nix_options: &NixOptions,
) -> Result<String> {
    let system = get_nix_system(nix_options)?;
    let path = &nix_build_options.path;
    if let Some(attr) = &nix_build_options.attr {
        let Some(full_attr) = try_flake_attr(nix_build_options, attr, nix_options, &system)? else {
            anyhow::bail!(
                "Explicitly provided flake URI does not point to a valid system-manager configuration: {path}#{attr}"
            )
        };
        return Ok(full_attr);
    }

    let hostname_os = nix::unistd::gethostname()?;
    let hostname = escape_nix_string(&hostname_os.to_string_lossy());
    let default = "default";

    if let Some(full_attr) = try_flake_attr(nix_build_options, &hostname, nix_options, &system)? {
        return Ok(full_attr);
    } else if let Some(full_attr) =
        try_flake_attr(nix_build_options, default, nix_options, &system)?
    {
        return Ok(full_attr);
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
    nix_build_options: &NixBuildOptions,
    attr: &str,
    nix_options: &NixOptions,
    system: &str,
) -> Result<Option<String>> {
    let try_flake_attr_impl = |attr: &str| {
        let full_attr = format!("{FLAKE_ATTR}.{attr}");
        log::info!("Trying attribute: {full_attr}...");
        let status = try_nix_eval(nix_build_options, attr, nix_options)?;
        if status {
            log::info!("Success, using {full_attr}");
            Ok(Some(full_attr))
        } else {
            log::info!("Attribute {full_attr} not found.");
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

fn run_nix_build(
    nix_build_options: &NixBuildOptions,
    attr: String,
    nix_options: &NixOptions,
) -> Result<process::Output> {
    let path = &nix_build_options.path;
    let mut cmd = nix_cmd(nix_options);
    cmd.arg("build");
    if nix_build_options.is_flake {
        cmd.arg(format!("{path}#{attr}"));
    } else {
        cmd.arg("-f").arg(path).arg(attr);
    }
    cmd.arg("--json");
    if nix_build_options.refresh {
        cmd.arg("--refresh");
    }

    log::debug!("Running nix command: {cmd:?}");

    let output = cmd
        // Nix outputs progress info on stderr and the final output on stdout,
        // so we inherit and output stderr directly to the terminal, but we
        // capture stdout as the result of this call
        .stderr(process::Stdio::inherit())
        .output()?;
    Ok(output)
}

fn try_nix_eval(
    nix_build_options: &NixBuildOptions,
    attr: &str,
    nix_options: &NixOptions,
) -> Result<bool> {
    let path = &nix_build_options.path;
    let mut cmd = nix_cmd(nix_options);
    cmd.arg("eval");
    if nix_build_options.is_flake {
        cmd.arg(format!("{path}#{FLAKE_ATTR}"));
    } else {
        cmd.arg("-f").arg(path).arg(FLAKE_ATTR);
    }
    cmd.arg("--json")
        .arg("--apply")
        .arg(format!("_: _ ? {attr}"));
    if nix_build_options.refresh {
        cmd.arg("--refresh");
    }

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
        .arg("nix-command flakes")
        .arg("--extra-substituters")
        .arg("https://cache.numtide.com")
        .arg("--extra-trusted-public-keys")
        .arg("niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=");
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
        let nix_build_options_flake = NixBuildOptions {
            is_flake: true,
            path: "./test/rust/register".to_string(),
            attr: None,
            refresh: false,
        };
        let nix_build_options_classic = NixBuildOptions {
            is_flake: false,
            path: "./test/rust/register".to_string(),
            attr: None,
            refresh: false,
        };
        let nix_options = &NixOptions::new(vec![]);

        assert!(try_nix_eval(&nix_build_options_flake, "identifier-key", nix_options).unwrap());
        assert!(try_nix_eval(&nix_build_options_classic, "identifier-key", nix_options).unwrap());
        assert!(try_nix_eval(
            &nix_build_options_flake,
            "\"string.literal/key\"",
            nix_options
        )
        .unwrap());
        assert!(try_nix_eval(
            &nix_build_options_classic,
            "\"string.literal/key\"",
            nix_options
        )
        .unwrap());
        assert!(!try_nix_eval(&nix_build_options_flake, "_identifier-key", nix_options).unwrap());
        assert!(!try_nix_eval(&nix_build_options_classic, "_identifier-key", nix_options).unwrap());
        assert!(!try_nix_eval(
            &nix_build_options_flake,
            "\"_string.literal/key\"",
            nix_options
        )
        .unwrap());
        assert!(!try_nix_eval(
            &nix_build_options_classic,
            "\"_string.literal/key\"",
            nix_options
        )
        .unwrap());
    }
}
