use anyhow::Result;
use clap::Parser;
use std::ffi::OsString;
use std::mem;
use std::path::{Path, PathBuf};
use std::process::{self, ExitCode};

use system_manager::{NixOptions, StorePath};

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
    /// Invoke the remote command with sudo.
    /// Only useful in combination with --target-host
    use_remote_sudo: bool,

    #[clap(long = "nix-option", num_args = 2)]
    nix_options: Option<Vec<String>>,
}

#[derive(clap::Args, Debug)]
struct BuildArgs {
    #[arg(long = "flake", name = "FLAKE_URI")]
    /// The flake URI defining the system-manager profile
    flake_uri: String,
}

#[derive(clap::Args, Debug)]
struct GenerateArgs {
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
    /// Activate a given system-manager profile
    Activate {
        #[arg(long)]
        /// The store path containing the system-manager profile to activate
        store_path: StorePath,
        #[command(flatten)]
        activation_args: ActivationArgs,
    },
    /// Put all files defined by the given generation in place, but do not start
    /// services. Useful in build scripts.
    PrePopulate {
        #[command(flatten)]
        store_or_flake_args: StoreOrFlakeArgs,
        #[command(flatten)]
        activation_args: ActivationArgs,
    },
    /// Build a new system-manager profile without registering it as a nix profile
    Build {
        #[command(flatten)]
        build_args: BuildArgs,
    },
    /// Deactivate the active system-manager profile, removing all managed configuration
    Deactivate {
        #[command(flatten)]
        optional_store_path_args: OptionalStorePathArg,
    },
    /// Generate a new system-manager profile and
    /// register is as the active system-manager profile
    Generate {
        #[command(flatten)]
        store_or_flake_args: StoreOrFlakeArgs,
    },
    /// Generate a new system-manager profile and activate it
    Switch {
        #[command(flatten)]
        build_args: BuildArgs,
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
        use_remote_sudo,
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
            .collect::<Vec<_>>()
    }));

    match action {
        Action::Activate {
            store_path,
            activation_args: ActivationArgs { ephemeral },
        } => {
            copy_closure(&store_path, &target_host)?;
            activate(&store_path, ephemeral, &target_host, use_remote_sudo)
        }
        Action::PrePopulate {
            store_or_flake_args,
            activation_args: ActivationArgs { ephemeral },
        } => prepopulate(
            store_or_flake_args,
            ephemeral,
            &target_host,
            use_remote_sudo,
            &nix_options,
        )
        .and_then(print_store_path),
        Action::Build {
            build_args: BuildArgs { flake_uri },
        } => build(&flake_uri, &target_host, &nix_options).and_then(print_store_path),
        Action::Deactivate {
            optional_store_path_args: OptionalStorePathArg { maybe_store_path },
        } => deactivate(maybe_store_path, &target_host, use_remote_sudo),
        Action::Generate {
            store_or_flake_args,
        } => generate(
            store_or_flake_args,
            &target_host,
            use_remote_sudo,
            &nix_options,
        )
        .and_then(print_store_path),
        Action::Switch {
            build_args: BuildArgs { flake_uri },
            activation_args: ActivationArgs { ephemeral },
        } => {
            let store_path = do_build(&flake_uri, &nix_options)?;
            copy_closure(&store_path, &target_host)?;
            do_generate(&store_path, &target_host, use_remote_sudo)?;
            activate(&store_path, ephemeral, &target_host, use_remote_sudo)
        }
    }
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
    system_manager::generate::build(flake_uri, nix_options)
}

fn generate(
    args: StoreOrFlakeArgs,
    target_host: &Option<String>,
    use_remote_sudo: bool,
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
            do_generate(&store_path, target_host, use_remote_sudo)?;
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
            do_generate(&store_path, target_host, use_remote_sudo)?;
            Ok(store_path)
        }
        _ => {
            anyhow::bail!("Supply either a flake URI or a store path.")
        }
    }
}

fn do_generate(
    store_path: &StorePath,
    target_host: &Option<String>,
    use_remote_sudo: bool,
) -> Result<()> {
    if let Some(target_host) = target_host {
        let status = invoke_remote_script(
            &store_path.store_path,
            "register-profile",
            target_host,
            use_remote_sudo,
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
        system_manager::generate::generate(store_path)
    }
}

fn activate(
    store_path: &StorePath,
    ephemeral: bool,
    target_host: &Option<String>,
    use_remote_sudo: bool,
) -> Result<()> {
    if let Some(target_host) = target_host {
        invoke_remote_script(
            &store_path.store_path,
            "activate",
            target_host,
            use_remote_sudo,
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
    use_remote_sudo: bool,
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
            do_generate(&store_path, target_host, use_remote_sudo)?;
            do_prepopulate(&store_path, ephemeral, target_host, use_remote_sudo)?;
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
            //TODO: this currently fails in the VM test, need to figure out why
            //do_generate(&store_path, target_host, use_remote_sudo)?;
            do_prepopulate(&store_path, ephemeral, target_host, use_remote_sudo)?;
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
    use_remote_sudo: bool,
) -> Result<()> {
    if let Some(target_host) = target_host {
        invoke_remote_script(
            &store_path.store_path,
            "pre-populate",
            target_host,
            use_remote_sudo,
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
    use_remote_sudo: bool,
) -> Result<()> {
    if let Some(target_host) = target_host {
        let store_path = store_path_or_active_profile(maybe_store_path);
        invoke_remote_script(&store_path, "deactivate", target_host, use_remote_sudo)?;
        Ok(())
    } else {
        check_root()?;
        system_manager::activate::deactivate()
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

fn invoke_remote_script(
    path: &Path,
    script_name: &str,
    target_host: &str,
    use_remote_sudo: bool,
) -> Result<process::ExitStatus> {
    let mut cmd = process::Command::new("ssh");
    cmd.arg(target_host).arg("--");
    if use_remote_sudo {
        cmd.arg("sudo");
    }
    let status = cmd
        .arg(OsString::from(
            path.join("bin")
                .join(script_name)
                .to_string_lossy()
                .into_owned(),
        ))
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .status()?;
    Ok(status)
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
