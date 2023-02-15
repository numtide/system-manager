use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;

use system_manager::StorePath;

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[command(subcommand)]
    action: Action,
}

#[derive(clap::Args, Debug)]
struct BuildArgs {
    #[arg(long)]
    /// The flake defining the system-manager profile
    flake_uri: String,
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
    /// Generate a new system-manager generation
    Generate {
        #[command(flatten)]
        build_args: BuildArgs,
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
    let args = Args::parse();

    // FIXME: set default level to info
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    handle_toplevel_error(go(args.action))
}

fn go(action: Action) -> Result<()> {
    match action {
        Action::Activate {
            store_path,
            activation_args: ActivationArgs { ephemeral },
        } => {
            check_root()?;
            activate(store_path, ephemeral)
        }
        Action::Build {
            build_args: BuildArgs { flake_uri },
        } => build(flake_uri),
        Action::Generate {
            build_args: BuildArgs { flake_uri },
        } => {
            check_root()?;
            generate(flake_uri).map(|_| ())
        }
        Action::Switch {
            build_args: BuildArgs { flake_uri },
            activation_args: ActivationArgs { ephemeral },
        } => {
            check_root()?;
            let store_path = generate(flake_uri)?;
            activate(store_path, ephemeral)
        }
    }
}

fn build(flake_uri: String) -> Result<()> {
    let store_path = do_build(flake_uri)?;
    log::info!("{store_path}");
    Ok(())
}

fn do_build(flake_uri: String) -> Result<StorePath> {
    system_manager::generate::build(&flake_uri)
}

fn generate(flake_uri: String) -> Result<StorePath> {
    system_manager::generate::generate(&flake_uri)
}

fn activate(store_path: StorePath, ephemeral: bool) -> Result<()> {
    system_manager::activate::activate(store_path, ephemeral)
}

fn check_root() -> Result<()> {
    if !nix::unistd::Uid::is_root(nix::unistd::getuid()) {
        anyhow::bail!("We need root permissions.")
    }
    Ok(())
}

fn handle_toplevel_error<T>(r: Result<T>) -> ExitCode {
    if let Err(e) = r {
        log::error!("{}", e);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
