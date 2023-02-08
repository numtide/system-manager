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

fn main() {
    let args = Args::parse();

    // FIXME: set default level to info
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    match args.action {
        Action::Activate { store_path } => handle_toplevel_error(activate(store_path)),
        Action::Generate { flake_uri } => {
            handle_toplevel_error(service_manager::generate::generate(&flake_uri))
        }
    }
}

fn activate(store_path: StorePath) -> Result<()> {
    if !nix::unistd::Uid::is_root(nix::unistd::getuid()) {
        return Err(anyhow!("We need root permissions."));
    }
    service_manager::activate::activate(store_path)
}

fn handle_toplevel_error<T>(r: Result<T>) {
    if let Err(e) = r {
        log::error!("{}", e)
    }
}
