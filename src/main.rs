use anyhow::Result;
use clap::Parser;
use std::ffi::OsString;
use std::path::Path;
use std::process::{self, ExitCode};

use system_manager::StorePath;

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
}

#[derive(clap::Args, Debug)]
struct BuildArgs {
    #[arg(long = "flake", name = "FLAKE_URI")]
    /// The flake defining the system-manager profile
    flake_uri: String,
}

#[derive(clap::Args, Debug)]
struct GenerateArgs {
    #[arg(long = "flake", name = "FLAKE_URI")]
    /// The flake defining the system-manager profile
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
struct DeactivationArgs {
    #[arg(long)]
    /// The store path for the system-manager profile.
    /// You only need to specify this explicitly if it differs from the active
    /// system-manager profile
    store_path: Option<StorePath>,
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
    /// Build a new system-manager profile without registering it as a nix profile
    Build {
        #[command(flatten)]
        build_args: BuildArgs,
    },
    /// Deactivate the active system-manager profile, removing all managed configuration
    Deactivate {
        #[command(flatten)]
        deactivation_args: DeactivationArgs,
    },
    /// Generate a new system-manager profile and
    /// register is as the active system-manager profile
    Generate {
        #[command(flatten)]
        generate_args: GenerateArgs,
    },
    /// Generate a new system-manager profile and activate it
    Switch {
        #[command(flatten)]
        build_args: BuildArgs,
        #[command(flatten)]
        activation_args: ActivationArgs,
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
        use_remote_sudo,
    } = args;

    match action {
        Action::Activate {
            store_path,
            activation_args: ActivationArgs { ephemeral },
        } => {
            copy_closure(&store_path, &target_host)?;
            activate(&store_path, ephemeral, &target_host, use_remote_sudo)
        }
        Action::Build {
            build_args: BuildArgs { flake_uri },
        } => build(&flake_uri, &target_host),
        Action::Deactivate {
            deactivation_args: DeactivationArgs { store_path },
        } => deactivate(store_path, &target_host, use_remote_sudo),
        Action::Generate { generate_args } => {
            generate(&generate_args, &target_host, use_remote_sudo)
        }
        Action::Switch {
            build_args: BuildArgs { flake_uri },
            activation_args: ActivationArgs { ephemeral },
        } => {
            let store_path = do_build(&flake_uri)?;
            copy_closure(&store_path, &target_host)?;
            do_generate(&store_path, &target_host, use_remote_sudo)?;
            activate(&store_path, ephemeral, &target_host, use_remote_sudo)
        }
    }
}

fn build(flake_uri: &str, target_host: &Option<String>) -> Result<()> {
    let store_path = do_build(flake_uri)?;
    copy_closure(&store_path, target_host)?;
    // Print the raw store path to stdout
    println!("{store_path}");
    Ok(())
}

fn do_build(flake_uri: &str) -> Result<StorePath> {
    system_manager::generate::build(flake_uri)
}

fn generate(
    args: &GenerateArgs,
    target_host: &Option<String>,
    use_remote_sudo: bool,
) -> Result<()> {
    match args {
        GenerateArgs {
            flake_uri: Some(flake_uri),
            store_path: None,
        } => {
            let store_path = do_build(flake_uri)?;
            copy_closure(&store_path, target_host)?;
            do_generate(&store_path, target_host, use_remote_sudo)
        }
        GenerateArgs {
            flake_uri: None,
            store_path: Some(store_path),
        } => {
            copy_closure(store_path, target_host)?;
            do_generate(store_path, target_host, use_remote_sudo)
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
        invoke_remote_script(
            &store_path.store_path,
            "register-profile",
            target_host,
            use_remote_sudo,
        )?;
        Ok(())
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

fn deactivate(
    store_path: Option<StorePath>,
    target_host: &Option<String>,
    use_remote_sudo: bool,
) -> Result<()> {
    if let Some(target_host) = target_host {
        invoke_remote_script(
            &store_path.map_or_else(
                || Path::new(system_manager::PROFILE_DIR).join("system-manager"),
                |store_path| store_path.store_path,
            ),
            "deactivate",
            target_host,
            use_remote_sudo,
        )?;
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
    } else {
        log::error!("Error copying closure, {}", status);
    }
    Ok(())
}

fn invoke_remote_script(
    store_path: &Path,
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
            store_path
                .join("bin")
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

fn handle_toplevel_error<T>(r: Result<T>) -> ExitCode {
    if r.is_err() {
        log::error!("{:?}", r);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
