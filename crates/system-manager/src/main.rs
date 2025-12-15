//! System Manager CLI
//!
//! This is the command-line wrapper for system-manager that handles:
//! - Argument parsing and validation
//! - Nix build operations (unprivileged)
//! - Orchestration of privileged operations via system-manager-engine
//! - Remote deployment via SSH
//! - Uniform sudo handling (local and remote)

use anyhow::{anyhow, bail, Result};
use clap::Parser;
use rpassword::prompt_password;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::mem;
use std::path::{Path, PathBuf};
use std::process::{self, ExitCode};

use system_manager_engine::{NixOptions, StorePath, PROFILE_DIR};

/// The bytes for the NixOS flake template is included in the binary to avoid unnecessary
/// network calls when initializing a system-manager configuration from the command line.
pub const NIXOS_FLAKE_TEMPLATE: &[u8; 683] = include_bytes!("../../../templates/nixos/flake.nix");

/// The bytes for the standalone flake template is included in the binary to avoid unnecessary
/// network calls when initializing a system-manager configuration from the command line.
pub const STANDALONE_FLAKE_TEMPLATE: &[u8; 739] =
    include_bytes!("../../../templates/standalone/flake.nix");

/// The bytes for the standalone module template is included in the binary to avoid unnecessary
/// network calls when initializing a system-manager configuration from the command line.
pub const SYSTEM_MODULE_TEMPLATE: &[u8; 1153] = include_bytes!("../../../templates/system.nix");

/// Name of the engine binary in the store path
const ENGINE_BIN: &str = "system-manager-engine";

#[derive(Debug)]
struct SudoOptions {
    enabled: bool,
    password: Option<String>,
}

impl SudoOptions {
    fn new(enabled: bool, password: Option<String>) -> Self {
        if enabled {
            Self { enabled, password }
        } else {
            Self::disabled()
        }
    }

    fn disabled() -> Self {
        Self {
            enabled: false,
            password: None,
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(clap::Args, Debug)]
struct SudoArgs {
    #[arg(long, action)]
    /// Prefix commands with sudo for privilege escalation.
    /// Works both locally and with --target-host.
    sudo: bool,

    #[arg(long, action)]
    /// Prompt for the sudo password. Implies --sudo.
    ask_sudo_password: bool,
}

impl SudoArgs {
    fn to_sudo_options(&self, legacy_sudo: bool) -> Result<SudoOptions> {
        let sudo_enabled = self.sudo || self.ask_sudo_password || legacy_sudo;
        let sudo_password = if self.ask_sudo_password {
            Some(read_sudo_password()?)
        } else {
            None
        };
        Ok(SudoOptions::new(sudo_enabled, sudo_password))
    }
}

#[derive(clap::Parser, Debug)]
#[command(
    author,
    version,
    about,
    long_about = "System Manager -- Manage system configuration with Nix on any distro"
)]
struct Args {
    #[command(subcommand)]
    action: Action,

    #[arg(long)]
    /// The host to deploy the system-manager profile to
    target_host: Option<String>,

    #[arg(long = "use-remote-sudo", action, hide = true)]
    /// Deprecated: use --sudo on the subcommand instead
    legacy_use_remote_sudo: bool,

    #[clap(long = "nix-option", num_args = 2, global = true)]
    nix_options: Option<Vec<String>>,
}

#[derive(clap::Args, Debug)]
struct InitArgs {
    /// The path to initialize the configuration at.
    #[arg(
        default_value = "~/.config/system-manager",
        value_parser = |src: &str| -> Result<PathBuf> {
            if src.starts_with("~") {
                if let Some(home) = std::env::home_dir() {
                    let expanded = src.replace("~", &home.to_string_lossy());
                    return Ok(PathBuf::from(expanded));
                }
                bail!("Failed to determine a home directory to initialize the configuration in.")
            }
            Ok(PathBuf::from(src))
        },
    )]
    path: PathBuf,
    /// Whether or not to include a 'flake.nix' as part of the new configuration.
    #[arg(long, default_value = "false")]
    no_flake: bool,
}

#[derive(clap::Args, Debug)]
struct BuildArgs {
    #[arg(long = "flake", name = "FLAKE_URI")]
    /// The flake URI defining the system-manager profile
    flake_uri: String,
}

#[derive(clap::Args, Debug)]
struct ActivationArgs {
    #[arg(long, action)]
    /// If true, only write under /run, otherwise write under /etc
    ephemeral: bool,
}

#[derive(clap::Args, Debug)]
struct OptionalStorePathArg {
    #[arg(long = "store-path", name = "STORE_PATH")]
    /// The store path for the system-manager profile.
    maybe_store_path: Option<StorePath>,
}

#[derive(clap::Args, Debug)]
struct OptionalFlakeUriArg {
    #[arg(long = "flake", name = "FLAKE_URI")]
    /// The flake URI defining the system-manager profile
    maybe_flake_uri: Option<String>,
}

#[derive(clap::Args, Debug)]
struct StoreOrFlakeArgs {
    #[command(flatten)]
    optional_store_path_arg: OptionalStorePathArg,

    #[command(flatten)]
    optional_flake_uri_arg: OptionalFlakeUriArg,
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    /// Initializes a configuration in the given directory.
    Init {
        #[command(flatten)]
        init_args: InitArgs,
    },
    /// Build a new system-manager generation, register it as the active profile, and activate it
    Switch {
        #[command(flatten)]
        build_args: BuildArgs,
        #[command(flatten)]
        activation_args: ActivationArgs,
        #[command(flatten)]
        sudo_args: SudoArgs,
    },
    /// Build a new system-manager generation and register it as the active system-manager profile
    Register {
        #[command(flatten)]
        store_or_flake_args: StoreOrFlakeArgs,
        #[command(flatten)]
        sudo_args: SudoArgs,
    },
    /// Build a new system-manager profile without registering it as a profile
    Build {
        #[command(flatten)]
        build_args: BuildArgs,
    },
    /// Deactivate the active system-manager profile, removing all managed configuration
    Deactivate {
        #[command(flatten)]
        optional_store_path_args: OptionalStorePathArg,
        #[command(flatten)]
        sudo_args: SudoArgs,
    },
    /// Put all files defined by the given generation in place, but do not start services
    PrePopulate {
        #[command(flatten)]
        store_or_flake_args: StoreOrFlakeArgs,
        #[command(flatten)]
        activation_args: ActivationArgs,
        #[command(flatten)]
        sudo_args: SudoArgs,
    },
    /// Activate a given system-manager profile (low-level, hidden)
    #[clap(hide = true)]
    Activate {
        #[arg(long)]
        store_path: StorePath,
        #[command(flatten)]
        activation_args: ActivationArgs,
        #[command(flatten)]
        sudo_args: SudoArgs,
    },
}

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    handle_toplevel_error(go(Args::parse()))
}

fn go(args: Args) -> Result<()> {
    let Args {
        action,
        target_host,
        legacy_use_remote_sudo,
        nix_options,
    } = args;

    if legacy_use_remote_sudo {
        log::warn!(
            "--use-remote-sudo is deprecated and will be removed in a future release. \
             Use --sudo on the subcommand instead. \
             Example: system-manager switch --sudo --flake ."
        );
    }

    let nix_options = NixOptions::new(nix_options.map_or(Vec::new(), |mut vals| {
        vals.chunks_mut(2)
            .map(|slice| {
                (
                    mem::take(slice.get_mut(0).expect("Error parsing nix-option values")),
                    mem::take(slice.get_mut(1).expect("Error parsing nix-option values")),
                )
            })
            .collect()
    }));

    match action {
        Action::PrePopulate {
            store_or_flake_args,
            activation_args: ActivationArgs { ephemeral },
            sudo_args,
        } => {
            let sudo_options = sudo_args.to_sudo_options(legacy_use_remote_sudo)?;
            prepopulate(
                store_or_flake_args,
                ephemeral,
                &target_host,
                &sudo_options,
                &nix_options,
            )
            .and_then(print_store_path)
        }

        Action::Build {
            build_args: BuildArgs { flake_uri },
        } => build(&flake_uri, &target_host, &nix_options).and_then(print_store_path),

        Action::Deactivate {
            optional_store_path_args: OptionalStorePathArg { maybe_store_path },
            sudo_args,
        } => {
            let sudo_options = sudo_args.to_sudo_options(legacy_use_remote_sudo)?;
            deactivate(maybe_store_path, &target_host, &sudo_options)
        }

        Action::Register {
            store_or_flake_args,
            sudo_args,
        } => {
            let sudo_options = sudo_args.to_sudo_options(legacy_use_remote_sudo)?;
            register(
                store_or_flake_args,
                &target_host,
                &sudo_options,
                &nix_options,
            )
            .and_then(print_store_path)
        }

        Action::Init {
            init_args: InitArgs { mut path, no_flake },
        } => {
            create_dir_all(&path).map_err(|err| {
                anyhow!(
                    "encountered an error while creating configuration directory '{}': {err:?}",
                    path.display()
                )
            })?;
            path = path.canonicalize().map_err(|err| {
                anyhow!(
                    "failed to resolve '{}' into an absolute path: {err:?}",
                    path.display()
                )
            })?;
            log::info!(
                "Initializing new system-manager configuration at '{}'",
                path.display()
            );

            let system_config_filepath = path.join("system.nix");
            init_config_file(&system_config_filepath, SYSTEM_MODULE_TEMPLATE)?;

            let has_flake_support = process::Command::new("nix")
                .arg("show-config")
                .output()
                .is_ok_and(|output| {
                    let out_str = String::from_utf8_lossy(&output.stdout);
                    out_str.contains("experimental-features")
                        && out_str.contains("flakes")
                        && out_str.contains("nix-command")
                });
            if !no_flake && has_flake_support {
                let flake_config_filepath = path.join("flake.nix");
                let is_nixos = process::Command::new("nixos-version")
                    .output()
                    .is_ok_and(|output| !output.stdout.is_empty());
                if is_nixos {
                    init_config_file(&flake_config_filepath, NIXOS_FLAKE_TEMPLATE)?
                } else {
                    init_config_file(&flake_config_filepath, STANDALONE_FLAKE_TEMPLATE)?
                }
            }
            log::info!("Configuration '{}' ready for activation!", path.display());
            Ok(())
        }

        Action::Switch {
            build_args: BuildArgs { flake_uri },
            activation_args: ActivationArgs { ephemeral },
            sudo_args,
        } => {
            let sudo_options = sudo_args.to_sudo_options(legacy_use_remote_sudo)?;
            let store_path = do_build(&flake_uri, &nix_options)?;
            copy_closure(&store_path, &target_host)?;
            invoke_engine_register(&store_path, &target_host, &sudo_options)?;
            invoke_engine_activate(&store_path, ephemeral, &target_host, &sudo_options)
        }

        Action::Activate {
            store_path,
            activation_args: ActivationArgs { ephemeral },
            sudo_args,
        } => {
            let sudo_options = sudo_args.to_sudo_options(legacy_use_remote_sudo)?;
            copy_closure(&store_path, &target_host)?;
            invoke_engine_activate(&store_path, ephemeral, &target_host, &sudo_options)
        }
    }
}

/// Create and write all bytes from a buffer into a new config file if it doesn't already exist.
fn init_config_file(filepath: &Path, buf: &[u8]) -> Result<()> {
    match OpenOptions::new()
        .create_new(true)
        .write(true)
        .truncate(false)
        .open(filepath)
    {
        Ok(mut file) => {
            file.write_all(buf)?;
            log::info!("{}B written to '{}'", buf.len(), filepath.display())
        }
        Err(err) if matches!(err.kind(), std::io::ErrorKind::AlreadyExists) => {
            log::warn!(
                "'{}' already exists, leaving it unchanged...",
                filepath.display()
            )
        }
        Err(err) => {
            bail!(
                "failed to initialize system configuration at '{}': {err:?}",
                filepath.display()
            )
        }
    }
    Ok(())
}

fn print_store_path<SP: AsRef<StorePath>>(store_path: SP) -> Result<()> {
    println!("{}", store_path.as_ref());
    Ok(())
}

fn read_sudo_password() -> Result<String> {
    prompt_password("Enter sudo password: ")
        .map_err(|err| anyhow!("failed to read sudo password: {err}"))
}

fn build(
    flake_uri: &str,
    target_host: &Option<String>,
    nix_options: &NixOptions,
) -> Result<StorePath> {
    let store_path = do_build(flake_uri, nix_options)?;
    copy_closure(&store_path, target_host)?;
    Ok(store_path)
}

fn do_build(flake_uri: &str, nix_options: &NixOptions) -> Result<StorePath> {
    system_manager_engine::register::build(flake_uri, nix_options)
}

fn register(
    args: StoreOrFlakeArgs,
    target_host: &Option<String>,
    sudo_options: &SudoOptions,
    nix_options: &NixOptions,
) -> Result<StorePath> {
    match args {
        StoreOrFlakeArgs {
            optional_store_path_arg:
                OptionalStorePathArg {
                    maybe_store_path: None,
                },
            optional_flake_uri_arg:
                OptionalFlakeUriArg {
                    maybe_flake_uri: Some(flake_uri),
                },
        } => {
            let store_path = do_build(&flake_uri, nix_options)?;
            copy_closure(&store_path, target_host)?;
            invoke_engine_register(&store_path, target_host, sudo_options)?;
            Ok(store_path)
        }
        StoreOrFlakeArgs {
            optional_store_path_arg:
                OptionalStorePathArg {
                    maybe_store_path: Some(store_path),
                },
            optional_flake_uri_arg:
                OptionalFlakeUriArg {
                    maybe_flake_uri: None,
                },
        } => {
            copy_closure(&store_path, target_host)?;
            invoke_engine_register(&store_path, target_host, sudo_options)?;
            Ok(store_path)
        }
        _ => {
            anyhow::bail!("Supply either a flake URI or a store path.")
        }
    }
}

fn prepopulate(
    args: StoreOrFlakeArgs,
    ephemeral: bool,
    target_host: &Option<String>,
    sudo_options: &SudoOptions,
    nix_options: &NixOptions,
) -> Result<StorePath> {
    match args {
        StoreOrFlakeArgs {
            optional_store_path_arg:
                OptionalStorePathArg {
                    maybe_store_path: None,
                },
            optional_flake_uri_arg:
                OptionalFlakeUriArg {
                    maybe_flake_uri: Some(flake_uri),
                },
        } => {
            let store_path = do_build(&flake_uri, nix_options)?;
            copy_closure(&store_path, target_host)?;
            invoke_engine_register(&store_path, target_host, sudo_options)?;
            invoke_engine_prepopulate(&store_path, ephemeral, target_host, sudo_options)?;
            Ok(store_path)
        }
        StoreOrFlakeArgs {
            optional_store_path_arg: OptionalStorePathArg { maybe_store_path },
            optional_flake_uri_arg:
                OptionalFlakeUriArg {
                    maybe_flake_uri: None,
                },
        } => {
            let store_path = StorePath::try_from(store_path_or_active_profile(maybe_store_path))?;
            copy_closure(&store_path, target_host)?;
            invoke_engine_register(&store_path, target_host, sudo_options)?;
            invoke_engine_prepopulate(&store_path, ephemeral, target_host, sudo_options)?;
            Ok(store_path)
        }
        _ => {
            anyhow::bail!("Supply either a flake URI or a store path.")
        }
    }
}

fn deactivate(
    maybe_store_path: Option<StorePath>,
    target_host: &Option<String>,
    sudo_options: &SudoOptions,
) -> Result<()> {
    let store_path = store_path_or_active_profile(maybe_store_path);
    invoke_engine_deactivate(&store_path, target_host, sudo_options)
}

// --- Engine invocation functions ---

/// Invoke the engine's register subcommand
fn invoke_engine_register(
    store_path: &StorePath,
    target_host: &Option<String>,
    sudo_options: &SudoOptions,
) -> Result<()> {
    let engine_path = store_path.store_path.join("bin").join(ENGINE_BIN);
    let args = vec![
        "register".to_string(),
        "--store-path".to_string(),
        store_path.to_string(),
    ];
    invoke_engine(&engine_path, &args, target_host, sudo_options)
}

/// Invoke the engine's activate subcommand
fn invoke_engine_activate(
    store_path: &StorePath,
    ephemeral: bool,
    target_host: &Option<String>,
    sudo_options: &SudoOptions,
) -> Result<()> {
    let engine_path = store_path.store_path.join("bin").join(ENGINE_BIN);
    let mut args = vec![
        "activate".to_string(),
        "--store-path".to_string(),
        store_path.to_string(),
    ];
    if ephemeral {
        args.push("--ephemeral".to_string());
    }
    invoke_engine(&engine_path, &args, target_host, sudo_options)
}

/// Invoke the engine's prepopulate subcommand
fn invoke_engine_prepopulate(
    store_path: &StorePath,
    ephemeral: bool,
    target_host: &Option<String>,
    sudo_options: &SudoOptions,
) -> Result<()> {
    let engine_path = store_path.store_path.join("bin").join(ENGINE_BIN);
    let mut args = vec![
        "prepopulate".to_string(),
        "--store-path".to_string(),
        store_path.to_string(),
    ];
    if ephemeral {
        args.push("--ephemeral".to_string());
    }
    invoke_engine(&engine_path, &args, target_host, sudo_options)
}

/// Invoke the engine's deactivate subcommand
fn invoke_engine_deactivate(
    store_path: &Path,
    target_host: &Option<String>,
    sudo_options: &SudoOptions,
) -> Result<()> {
    // For deactivate, we need to find the engine in the profile
    // If we have a specific store path, use it; otherwise use the active profile
    let engine_path = if store_path.starts_with("/nix/store") {
        store_path.join("bin").join(ENGINE_BIN)
    } else {
        // store_path is the profile symlink, resolve it fully
        // (profile -> generation -> store path)
        let resolved = store_path
            .canonicalize()
            .unwrap_or_else(|_| store_path.to_path_buf());
        resolved.join("bin").join(ENGINE_BIN)
    };
    let args = vec!["deactivate".to_string()];
    invoke_engine(&engine_path, &args, target_host, sudo_options)
}

/// Core engine invocation - handles local/remote and sudo
fn invoke_engine(
    engine_path: &Path,
    args: &[String],
    target_host: &Option<String>,
    sudo_options: &SudoOptions,
) -> Result<()> {
    let status = if let Some(host) = target_host {
        invoke_engine_remote(engine_path, args, host, sudo_options)?
    } else {
        invoke_engine_local(engine_path, args, sudo_options)?
    };

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!(
            "Engine command exited with status {}",
            status
                .code()
                .map_or("unknown".to_string(), |c| c.to_string())
        )
    }
}

/// Invoke engine locally, optionally with sudo
fn invoke_engine_local(
    engine_path: &Path,
    args: &[String],
    sudo_options: &SudoOptions,
) -> Result<process::ExitStatus> {
    if sudo_options.is_enabled() {
        let mut cmd = process::Command::new("sudo");
        if sudo_options.password.is_some() {
            cmd.arg("-S");
        }
        cmd.arg(engine_path).args(args);
        cmd.stdout(process::Stdio::inherit())
            .stderr(process::Stdio::inherit());

        if let Some(password) = &sudo_options.password {
            cmd.stdin(process::Stdio::piped());
            let mut child = cmd.spawn()?;
            {
                let mut stdin = child
                    .stdin
                    .take()
                    .ok_or_else(|| anyhow!("failed to pass sudo password"))?;
                stdin.write_all(password.as_bytes())?;
                stdin.write_all(b"\n")?;
            }
            Ok(child.wait()?)
        } else {
            Ok(cmd.status()?)
        }
    } else {
        // No sudo - invoke engine directly
        let status = process::Command::new(engine_path)
            .args(args)
            .stdout(process::Stdio::inherit())
            .stderr(process::Stdio::inherit())
            .status()?;
        Ok(status)
    }
}

/// Invoke engine on remote host via SSH, optionally with sudo
fn invoke_engine_remote(
    engine_path: &Path,
    args: &[String],
    target_host: &str,
    sudo_options: &SudoOptions,
) -> Result<process::ExitStatus> {
    let mut cmd = process::Command::new("ssh");
    cmd.arg(target_host).arg("--");

    if sudo_options.is_enabled() {
        cmd.arg("sudo");
        if sudo_options.password.is_some() {
            cmd.arg("-S");
        }
    }

    cmd.arg(engine_path.to_string_lossy().to_string());
    for arg in args {
        cmd.arg(arg);
    }

    cmd.stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit());

    if let Some(password) = &sudo_options.password {
        cmd.stdin(process::Stdio::piped());
        let mut child = cmd.spawn()?;
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| anyhow!("failed to pass sudo password to remote command"))?;
            stdin.write_all(password.as_bytes())?;
            stdin.write_all(b"\n")?;
        }
        Ok(child.wait()?)
    } else {
        Ok(cmd.status()?)
    }
}

fn copy_closure(store_path: &StorePath, target_host: &Option<String>) -> Result<()> {
    target_host
        .as_ref()
        .map_or(Ok(()), |target| do_copy_closure(store_path, target))
}

fn do_copy_closure(store_path: &StorePath, target_host: &str) -> Result<()> {
    log::info!("Copying closure to target host...");
    let status = process::Command::new("nix-copy-closure")
        .arg("--to")
        .arg(target_host)
        .arg("--use-substitutes")
        .arg(&store_path.store_path)
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .status()?;
    if status.success() {
        log::info!("Successfully copied closure to target host");
        Ok(())
    } else {
        anyhow::bail!("Error copying closure, {}", status);
    }
}

fn store_path_or_active_profile(maybe_store_path: Option<StorePath>) -> PathBuf {
    maybe_store_path.map_or_else(
        || {
            let path = Path::new(PROFILE_DIR).join("system-manager");
            log::info!("No store path provided, using {}", path.display());
            path
        },
        |store_path| store_path.store_path,
    )
}

fn handle_toplevel_error<T>(r: Result<T>) -> ExitCode {
    if let Err(e) = r {
        log::error!("{:?}", e);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn legacy_use_remote_sudo_flag_is_accepted() {
        // The deprecated flag should still parse successfully (emits warning at runtime)
        let args = Args::try_parse_from([
            "system-manager",
            "--use-remote-sudo",
            "switch",
            "--flake",
            ".#test",
        ])
        .expect("failed to parse args");

        assert!(args.legacy_use_remote_sudo);
    }

    #[test]
    fn sudo_flag_works_on_subcommand() {
        let args =
            Args::try_parse_from(["system-manager", "switch", "--sudo", "--flake", ".#test"])
                .expect("failed to parse args");

        match args.action {
            Action::Switch { sudo_args, .. } => {
                assert!(sudo_args.sudo);
            }
            _ => panic!("Expected Switch action"),
        }
    }

    #[test]
    fn sudo_flag_not_available_on_build() {
        // --sudo should not be recognized on the build subcommand
        let result =
            Args::try_parse_from(["system-manager", "build", "--sudo", "--flake", ".#test"]);
        assert!(result.is_err());
    }
}
