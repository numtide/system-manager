use clap::Parser;
use std::error::Error;
use std::os::unix;
use std::path::Path;
use std::{env, fs, process, str};

#[derive(Debug)]
struct StorePath {
    path: String,
}

impl From<String> for StorePath {
    fn from(path: String) -> Self {
        StorePath {
            path: path.trim().into(),
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[arg(short, long)]
    flake_uri: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let profile_name = "service-manager";
    // TODO: we temporarily put this under per-user to avoid needing root access
    // we will move this to /nix/var/nix/profiles/system later on.
    let user = env::var("USER").expect("USER env var undefined");
    let profile_path = format!("/nix/var/nix/profiles/per-user/{}/{}", user, profile_name);
    let gcroot_path = format!(
        "/nix/var/nix/gcroots/per-user/{}/{}-current",
        user, profile_name
    );

    let flake_attr = "serviceConfig.x86_64-linux";

    let nix_build_output = run_nix_build(&args.flake_uri, flake_attr);

    let store_path = get_store_path(nix_build_output)?;
    println!("Found store path: {:?}", store_path);
    print_out_and_err(install_nix_profile(&store_path, &profile_path));
    create_gcroot(&gcroot_path, &store_path).expect("Failed to create GC root.");
    Ok(())
}

fn install_nix_profile(store_path: &StorePath, profile_path: &str) -> process::Output {
    process::Command::new("nix-env")
        .arg("--profile")
        .arg(profile_path)
        .arg("--install")
        .arg(&store_path.path)
        .arg("--remove-all")
        .output()
        .expect("Failed to execute nix-env, is it on your path?")
}

fn create_gcroot(gcroot_path: &str, store_path: &StorePath) -> Result<(), Box<dyn Error>> {
    let path = Path::new(gcroot_path);
    if path.is_symlink() {
        fs::remove_file(path).expect("Error removing old GC root.");
    }
    unix::fs::symlink(&store_path.path, gcroot_path).map_err(Box::from)
}

fn get_store_path(nix_build_result: process::Output) -> Result<StorePath, Box<dyn Error>> {
    if nix_build_result.status.success() {
        String::from_utf8(nix_build_result.stdout)
            .map_err(Box::from)
            .map(StorePath::from)
    } else {
        String::from_utf8(nix_build_result.stderr).map_or_else(boxed_error(), boxed_error())
    }
}

fn run_nix_build(flake_uri: &str, flake_attr: &str) -> process::Output {
    process::Command::new("nix")
        .arg("build")
        .arg(format!("{}#{}", flake_uri, flake_attr))
        .arg("--print-out-paths")
        .output()
        .expect("Failed to execute nix, is it on your path?")
}

fn print_out_and_err(output: process::Output) -> process::Output {
    print_u8(&output.stdout);
    print_u8(&output.stderr);
    output
}

fn print_u8(bytes: &[u8]) {
    str::from_utf8(bytes).map_or((), |s| {
        if !s.is_empty() {
            println!("{}", s)
        }
    })
}

pub fn compose<A, B, C, G, F>(f: F, g: G) -> impl Fn(A) -> C
where
    F: Fn(B) -> C,
    G: Fn(A) -> B,
{
    move |x| f(g(x))
}

fn boxed_error<V, E>() -> impl Fn(E) -> Result<V, Box<dyn Error>>
where
    E: Into<Box<dyn Error>>,
{
    compose(Err, Into::into)
}
