//! System Manager Engine
//!
//! This binary handles privileged operations for system-manager:
//! - activate: Apply /etc files and start systemd services
//! - deactivate: Remove managed configuration
//! - prepopulate: Place files without starting services
//! - register: Register a store path as the active profile
//!
//! It is designed to be invoked by the system-manager CLI wrapper,
//! either directly (with sudo) or via SSH for remote deployments.
//! This allows uniform handling of privilege escalation.

use anyhow::Result;
use clap::Parser;
use std::process::ExitCode;

use system_manager_engine::{NixOptions, StorePath, PROFILE_DIR};

#[derive(clap::Parser, Debug)]
#[command(
    author,
    version,
    about = "System Manager Engine - privileged operations for system-manager"
)]
struct Args {
    #[command(subcommand)]
    action: Action,

    #[clap(long = "nix-option", num_args = 2, global = true)]
    nix_options: Vec<String>,
}

#[derive(clap::Args, Debug)]
struct ActivationArgs {
    #[arg(long, action)]
    /// If true, only write under /run, otherwise write under /etc
    ephemeral: bool,
}

#[derive(clap::Args, Debug)]
struct StorePathArg {
    #[arg(long = "store-path")]
    /// The store path containing the system-manager profile
    store_path: StorePath,
}

#[derive(clap::Args, Debug)]
struct OptionalStorePathArg {
    #[arg(long = "store-path")]
    /// The store path for the system-manager profile.
    /// If not specified, uses the active profile.
    store_path: Option<StorePath>,
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    /// Activate a system-manager profile (apply files and start services)
    Activate {
        #[command(flatten)]
        store_path_arg: StorePathArg,
        #[command(flatten)]
        activation_args: ActivationArgs,
    },
    /// Deactivate the system-manager profile (remove managed configuration)
    Deactivate {
        #[command(flatten)]
        store_path_arg: OptionalStorePathArg,
    },
    /// Pre-populate files without starting services
    Prepopulate {
        #[command(flatten)]
        store_path_arg: StorePathArg,
        #[command(flatten)]
        activation_args: ActivationArgs,
    },
    /// Register a store path as the active profile
    Register {
        #[command(flatten)]
        store_path_arg: StorePathArg,
    },
}

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    handle_toplevel_error(go(Args::parse()))
}

fn go(args: Args) -> Result<()> {
    let nix_options = NixOptions::new(
        args.nix_options
            .chunks(2)
            .map(|pair| (pair[0].clone(), pair[1].clone()))
            .collect(),
    );

    match args.action {
        Action::Activate {
            store_path_arg: StorePathArg { store_path },
            activation_args: ActivationArgs { ephemeral },
        } => system_manager_engine::activate::activate(&store_path, ephemeral),

        Action::Deactivate {
            store_path_arg: OptionalStorePathArg { store_path },
        } => {
            // Log which store path we're using if it was auto-detected
            if store_path.is_none() {
                let path = std::path::Path::new(PROFILE_DIR).join("system-manager");
                log::info!("No store path provided, using {}", path.display());
            }
            system_manager_engine::activate::deactivate()
        }

        Action::Prepopulate {
            store_path_arg: StorePathArg { store_path },
            activation_args: ActivationArgs { ephemeral },
        } => system_manager_engine::activate::prepopulate(&store_path, ephemeral),

        Action::Register {
            store_path_arg: StorePathArg { store_path },
        } => system_manager_engine::register::register(&store_path, &nix_options),
    }
}

fn handle_toplevel_error<T>(r: Result<T>) -> ExitCode {
    if let Err(e) = r {
        log::error!("{:?}", e);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
