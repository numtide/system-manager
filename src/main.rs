use std::process::{self, ExitCode, ExitStatus};

use anyhow::Result;
use clap::Parser;

use system_manager::StorePath;

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[command(subcommand)]
    action: Action,

    #[arg(long)]
    /// The host to deploy the system-manager profile to
    target_host: Option<String>,

    #[arg(long, action)]
    use_remote_sudo: bool,
}

#[derive(clap::Args, Debug)]
struct BuildArgs {
    #[arg(long)]
    /// The flake defining the system-manager profile
    flake_uri: String,
}

#[derive(clap::Args, Debug)]
struct GenerateArgs {
    #[arg(long)]
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
    /// Build a new system-manager generation without registering it as a nix profile
    Build {
        #[command(flatten)]
        build_args: BuildArgs,
    },
    Deactivate,
    /// Generate a new system-manager generation
    Generate {
        #[command(flatten)]
        generate_args: GenerateArgs,
    },
    /// Generate a new system-manager generation and activate it
    Switch {
        #[command(flatten)]
        build_args: BuildArgs,
        #[command(flatten)]
        activation_args: ActivationArgs,
    },
}

fn main() -> ExitCode {
    // FIXME: set default level to info
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
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
            // FIXME handle target_host
            copy_closure(&store_path, &target_host)?;
            activate(&store_path, ephemeral, &target_host, use_remote_sudo)
        }
        Action::Build {
            build_args: BuildArgs { flake_uri },
        } => build(flake_uri),
        Action::Deactivate => {
            check_root()?;
            // FIXME handle target_host
            deactivate()
        }
        Action::Generate { generate_args } => {
            generate(generate_args, &target_host, use_remote_sudo)
        }
        Action::Switch {
            build_args: BuildArgs { flake_uri },
            activation_args: ActivationArgs { ephemeral },
        } => {
            let store_path = do_build(flake_uri)?;
            copy_closure(&store_path, &target_host)?;
            do_generate(&store_path, &target_host, use_remote_sudo)?;
            activate(&store_path, ephemeral, &target_host, use_remote_sudo)
        }
    }
}

fn generate(args: GenerateArgs, target_host: &Option<String>, use_remote_sudo: bool) -> Result<()> {
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
            copy_closure(&store_path, target_host)?;
            do_generate(&store_path, target_host, use_remote_sudo)
        }
        _ => {
            anyhow::bail!("Supply either a flake URI or a store path.")
        }
    }
}

fn build(flake_uri: String) -> Result<()> {
    let store_path = do_build(flake_uri)?;
    log::info!("Build system-manager profile {store_path}");
    // Print the raw store path to stdout
    println!("{store_path}");
    Ok(())
}

fn do_build(flake_uri: String) -> Result<StorePath> {
    system_manager::generate::build(&flake_uri)
}

fn do_generate(
    store_path: &StorePath,
    target_host: &Option<String>,
    use_remote_sudo: bool,
) -> Result<()> {
    if let Some(target_host) = target_host {
        invoke_remote_script(store_path, "register-profile", target_host, use_remote_sudo)?;
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
        invoke_remote_script(store_path, "activate", target_host, use_remote_sudo)?;
        Ok(())
    } else {
        check_root()?;
        system_manager::activate::activate(store_path, ephemeral)
    }
}

fn deactivate() -> Result<()> {
    system_manager::activate::deactivate()
}

fn copy_closure(store_path: &StorePath, target_host: &Option<String>) -> Result<()> {
    target_host
        .as_ref()
        .map_or(Ok(()), |target| do_copy_closure(store_path, target))
}

fn do_copy_closure(store_path: &StorePath, target_host: &str) -> Result<()> {
    process::Command::new("nix-copy-closure")
        .arg("--to")
        .arg(target_host)
        .arg("--use-substitutes")
        .arg(&store_path.store_path)
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .status()?;
    Ok(())
}

fn invoke_remote_script(
    store_path: &StorePath,
    script_name: &str,
    target_host: &str,
    use_remote_sudo: bool,
) -> Result<ExitStatus> {
    let mut cmd = process::Command::new("ssh");
    cmd.arg(target_host).arg("--");
    if use_remote_sudo {
        cmd.arg("sudo");
    }
    cmd.arg(format!("{store_path}/bin/{script_name}"))
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .status()
        .map_err(anyhow::Error::from)
}

fn check_root() -> Result<()> {
    if !nix::unistd::Uid::is_root(nix::unistd::getuid()) {
        anyhow::bail!("We need root permissions.")
    }
    Ok(())
}

fn handle_toplevel_error<T>(r: Result<T>) -> ExitCode {
    if let Err(e) = r {
        log::error!("{e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
