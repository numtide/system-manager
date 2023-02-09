use std::process::ExitCode;

use anyhow::{anyhow, Result};
use clap::Parser;

use service_manager::StorePath;

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[command(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    Activate {
        #[arg(long)]
        store_path: StorePath,
    },
    Generate {
        #[arg(long)]
        flake_uri: String,
    },
}

fn main() -> ExitCode {
    let args = Args::parse();

    // FIXME: set default level to info
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    handle_toplevel_error(go(args.action))
}

fn go(action: Action) -> Result<()> {
    check_root()?;
    match action {
        Action::Activate { store_path } => service_manager::activate::activate(store_path),
        Action::Generate { flake_uri } => service_manager::generate::generate(&flake_uri),
    }
}

fn check_root() -> Result<()> {
    if !nix::unistd::Uid::is_root(nix::unistd::getuid()) {
        return Err(anyhow!("We need root permissions."));
    }
    Ok(())
}

fn handle_toplevel_error<T>(r: Result<T>) -> ExitCode {
    if let Err(e) = &r {
        log::error!("{}", e);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
