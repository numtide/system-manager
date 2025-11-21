use anyhow::{anyhow, bail, Result};
use clap::Parser;
use std::ffi::OsString;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::mem;
use std::path::{Path, PathBuf};
use std::process::{self, ExitCode};

use rpassword::prompt_password;

use system_manager::{NixOptions, StorePath};

/// The bytes for the NixOS flake template is included in the binary to avoid unnecessary
/// network calls when initializing a system-manager configuration from the command line.
pub const NIXOS_FLAKE_TEMPLATE: &[u8; 683] = include_bytes!("../templates/nixos/flake.nix");

/// The bytes for the standalone flake template is included in the binary to avoid unnecessary
/// network calls when initializing a system-manager configuration from the command line.
pub const STANDALONE_FLAKE_TEMPLATE: &[u8; 739] =
    include_bytes!("../templates/standalone/flake.nix");

/// The bytes for the standalone module template is included in the binary to avoid unnecessary
/// network calls when initializing a system-manager configuration from the command line.
pub const SYSTEM_MODULE_TEMPLATE: &[u8; 1153] = include_bytes!("../templates/system.nix");

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

    #[arg(long, action)]
    /// Prefix remote commands (build/register/activate/etc.) with sudo.
    /// Mostly relevant when deploying via --target-host.
    sudo: bool,

    #[arg(long = "use-remote-sudo", action, hide = true)]
    /// Deprecated alias for --sudo
    legacy_use_remote_sudo: bool,

    #[arg(long, action)]
    /// Prompt for the sudo password used on the target host.
    /// Implies --sudo.
    ask_sudo_password: bool,

    #[clap(long = "nix-option", num_args = 2, global = true)]
    nix_options: Option<Vec<String>>,
}

#[derive(clap::Args, Debug)]
struct InitArgs {
    /// The path to initialize the configuration at.
    #[arg(
        // The default_value is not resolved at this point so we must
        // parse it ourselves with a value_parser closure.
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
    /// By default, if the host has the 'flakes' and 'nix-command' experimental features
    /// enabled, a 'flake.nix' will be included. A flake template is automatically selected
    /// by checking the host system's features. Flake templates are available on the system-manager
    /// flake attribute 'outputs.templates'.
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
struct RegisterArgs {
    #[arg(long = "flake", name = "FLAKE_URI")]
    /// The flake URI defining the system-manager profile
    flake_uri: Option<String>,

    #[arg(long)]
    /// The store path containing the system-manager profile
    store_path: Option<StorePath>,
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
    /// You only need to specify this explicitly if it differs from the active
    /// system-manager profile.
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
    /// Initializes a configuration in the given directory. If the directory
    /// does not exist, then it will be created. The default directory is
    /// '~/.config/system-manager'.
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
    },
    /// Build a new system-manager generation and register is as the active system-manager profile
    Register {
        #[command(flatten)]
        store_or_flake_args: StoreOrFlakeArgs,
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
    },
    /// Put all files defined by the given generation in place, but do not start
    /// services. Useful in build scripts.
    PrePopulate {
        #[command(flatten)]
        store_or_flake_args: StoreOrFlakeArgs,
        #[command(flatten)]
        activation_args: ActivationArgs,
    },
    /// Activate a given system-manager profile.
    /// This is a low-level action that should not be used directly.
    #[clap(hide = true)]
    Activate {
        #[arg(long)]
        /// The store path containing the system-manager profile to activate
        store_path: StorePath,
        #[command(flatten)]
        activation_args: ActivationArgs,
    },
}

// TODO: create a general lock while we are running to avoid running system-manager concurrently
fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    handle_toplevel_error(go(Args::parse()))
}

fn go(args: Args) -> Result<()> {
    let Args {
        action,
        target_host,
        sudo,
        legacy_use_remote_sudo,
        ask_sudo_password,
        nix_options,
    } = args;

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

    let mut sudo_enabled = sudo || legacy_use_remote_sudo;
    if legacy_use_remote_sudo {
        log::warn!("--use-remote-sudo is deprecated; use --sudo instead");
    }
    let mut sudo_password = None;
    if ask_sudo_password {
        sudo_enabled = true;
        if target_host.is_some() {
            sudo_password = Some(read_sudo_password()?);
        } else {
            log::warn!("--ask-sudo-password has no effect without --target-host");
        }
    }
    let sudo_options = SudoOptions::new(sudo_enabled, sudo_password);

    match action {
        Action::PrePopulate {
            store_or_flake_args,
            activation_args: ActivationArgs { ephemeral },
        } => prepopulate(
            store_or_flake_args,
            ephemeral,
            &target_host,
            &sudo_options,
            &nix_options,
        )
        .and_then(print_store_path),
        Action::Build {
            build_args: BuildArgs { flake_uri },
        } => build(&flake_uri, &target_host, &nix_options).and_then(print_store_path),
        Action::Deactivate {
            optional_store_path_args: OptionalStorePathArg { maybe_store_path },
        } => deactivate(maybe_store_path, &target_host, &sudo_options),
        Action::Register {
            store_or_flake_args,
        } => register(
            store_or_flake_args,
            &target_host,
            &sudo_options,
            &nix_options,
        )
        .and_then(print_store_path),
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

            let system_config_filepath = {
                let mut buf = path.clone();
                buf.push("system.nix");
                buf
            };
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
                let flake_config_filepath = {
                    let mut buf = path.clone();
                    buf.push("flake.nix");
                    buf
                };
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
        } => {
            let store_path = do_build(&flake_uri, &nix_options)?;
            copy_closure(&store_path, &target_host)?;
            do_register(&store_path, &target_host, &sudo_options, &nix_options)?;
            activate(&store_path, ephemeral, &target_host, &sudo_options)
        }
        Action::Activate {
            store_path,
            activation_args: ActivationArgs { ephemeral },
        } => {
            copy_closure(&store_path, &target_host)?;
            activate(&store_path, ephemeral, &target_host, &sudo_options)
        }
    }
}

/// Create and write all bytes from a buffer into a new config file if it doesn't already exist.
fn init_config_file(filepath: &PathBuf, buf: &[u8]) -> Result<()> {
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
    // Print the raw store path to stdout
    println!("{}", store_path.as_ref());
    Ok(())
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
    system_manager::register::build(flake_uri, nix_options)
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
            do_register(&store_path, target_host, sudo_options, nix_options)?;
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
            do_register(&store_path, target_host, sudo_options, nix_options)?;
            Ok(store_path)
        }
        _ => {
            anyhow::bail!("Supply either a flake URI or a store path.")
        }
    }
}

fn do_register(
    store_path: &StorePath,
    target_host: &Option<String>,
    sudo_options: &SudoOptions,
    nix_options: &NixOptions,
) -> Result<()> {
    if let Some(target_host) = target_host {
        let status = invoke_remote_script(
            &store_path.store_path,
            "register-profile",
            target_host,
            sudo_options,
        )?;
        if status.success() {
            Ok(())
        } else {
            anyhow::bail!(
                "Remote command exited with exit status {}",
                status
                    .code()
                    .map_or("unknown".to_string(), |c| c.to_string())
            )
        }
    } else {
        check_root()?;
        system_manager::register::register(store_path, nix_options)
    }
}

fn activate(
    store_path: &StorePath,
    ephemeral: bool,
    target_host: &Option<String>,
    sudo_options: &SudoOptions,
) -> Result<()> {
    if let Some(target_host) = target_host {
        invoke_remote_script(
            &store_path.store_path,
            "activate",
            target_host,
            sudo_options,
        )?;
        Ok(())
    } else {
        check_root()?;
        system_manager::activate::activate(store_path, ephemeral)
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
            do_register(&store_path, target_host, sudo_options, nix_options)?;
            do_prepopulate(&store_path, ephemeral, target_host, sudo_options)?;
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
            do_register(&store_path, target_host, sudo_options, nix_options)?;
            do_prepopulate(&store_path, ephemeral, target_host, sudo_options)?;
            Ok(store_path)
        }
        _ => {
            anyhow::bail!("Supply either a flake URI or a store path.")
        }
    }
}

fn do_prepopulate(
    store_path: &StorePath,
    ephemeral: bool,
    target_host: &Option<String>,
    sudo_options: &SudoOptions,
) -> Result<()> {
    if let Some(target_host) = target_host {
        invoke_remote_script(
            &store_path.store_path,
            "prepopulate",
            target_host,
            sudo_options,
        )?;
        Ok(())
    } else {
        check_root()?;
        system_manager::activate::prepopulate(store_path, ephemeral)
    }
}

fn deactivate(
    maybe_store_path: Option<StorePath>,
    target_host: &Option<String>,
    sudo_options: &SudoOptions,
) -> Result<()> {
    if let Some(target_host) = target_host {
        let store_path = store_path_or_active_profile(maybe_store_path);
        invoke_remote_script(&store_path, "deactivate", target_host, sudo_options)?;
        Ok(())
    } else {
        check_root()?;
        system_manager::activate::deactivate()
    }
}

fn read_sudo_password() -> Result<String> {
    prompt_password("Enter sudo password for target host: ")
        .map_err(|err| anyhow!("failed to read sudo password: {err}"))
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

fn invoke_remote_script(
    path: &Path,
    script_name: &str,
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
    cmd.arg(OsString::from(
        path.join("bin")
            .join(script_name)
            .to_string_lossy()
            .to_string(),
    ))
    .stdout(process::Stdio::inherit())
    .stderr(process::Stdio::inherit())
    .stdin(if sudo_options.password.is_some() {
        process::Stdio::piped()
    } else {
        process::Stdio::inherit()
    });

    if let Some(password) = &sudo_options.password {
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

fn check_root() -> Result<()> {
    if !nix::unistd::Uid::is_root(nix::unistd::getuid()) {
        anyhow::bail!("We need root permissions.")
    }
    Ok(())
}

fn store_path_or_active_profile(maybe_store_path: Option<StorePath>) -> PathBuf {
    maybe_store_path.map_or_else(
        || {
            let path = Path::new(system_manager::PROFILE_DIR).join("system-manager");
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
    fn legacy_use_remote_sudo_flag_is_supported() {
        let args = Args::try_parse_from([
            "system-manager",
            "--use-remote-sudo",
            "--target-host",
            "example",
            "switch",
            "--flake",
            ".#test",
        ])
        .expect("failed to parse args");

        assert!(args.legacy_use_remote_sudo);
    }
}
