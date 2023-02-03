use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::os::unix;
use std::path::Path;
use std::{env, fs, io, process, str};

const FLAKE_ATTR: &str = "serviceConfig";
const PROFILE_NAME: &str = "service-manager";

#[derive(Debug, Clone)]
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

impl std::fmt::Display for StorePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)
    }
}

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

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    match args.action {
        Action::Activate { store_path } => activate(store_path),
        Action::Generate { flake_uri } => generate(&flake_uri),
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceConfig {
    name: String,
    service: String,
}

impl ServiceConfig {
    fn store_path(&self) -> StorePath {
        StorePath::from(self.service.to_owned())
    }
}

fn activate(store_path: StorePath) -> Result<(), Box<dyn Error>> {
    if !nix::unistd::Uid::is_root(nix::unistd::getuid()) {
        return Err("We need root permissions.".into());
    }
    println!("Activating service-manager profile: {}", store_path);

    let file = fs::File::open(store_path.path + "/services/services.json")?;
    let reader = io::BufReader::new(file);

    let services: Vec<ServiceConfig> = serde_json::from_reader(reader)?;
    println!("{:?}", services);

    services.iter().try_for_each(|service| {
        create_store_link(
            &service.store_path(),
            Path::new(&format!("/run/systemd/system/{}.service", service.name)),
        )
    })?;

    start_services(&services);

    Ok(())
}

fn start_services(services: &[ServiceConfig]) {
    if process::Command::new("systemctl")
        .arg("daemon-reload")
        .output()
        .expect("Unable to run systemctl.")
        .status
        .success()
    {
        services.iter().for_each(|service| {
            println!("Starting service {} ...", service.name);
            let output = print_out_and_err(
                process::Command::new("systemctl")
                    .arg("start")
                    .arg(&service.name)
                    .output()
                    .expect("Unable to run systemctl"),
            );
            if output.status.success() {
                println!("Started service {}", service.name);
            } else {
                println!("Error starting service {}", service.name);
            }
        });
    }
}

fn generate(flake_uri: &str) -> Result<(), Box<dyn Error>> {
    let user = env::var("USER")?;
    // TODO: we temporarily put this under per-user to avoid needing root access
    // we will move this to /nix/var/nix/profiles/ later on.
    let profiles_dir = format!("profiles/per-user/{}", user);
    let gcroots_dir = format!("gcroots/per-user/{}", user);
    let profile_path = format!("/nix/var/nix/{}/{}", profiles_dir, PROFILE_NAME);
    let gcroot_path = format!("/nix/var/nix/{}/{}-current", gcroots_dir, PROFILE_NAME);

    // FIXME: we should not hard-code the system here
    let flake_attr = format!("{}.x86_64-linux", FLAKE_ATTR);

    println!("Running nix build...");
    let nix_build_output = run_nix_build(flake_uri, &flake_attr);
    let store_path = get_store_path(nix_build_output)?;

    println!("Generating new generation from {}", store_path);
    print_out_and_err(install_nix_profile(&store_path, &profile_path));

    println!("Registering GC root...");
    let profile_store_path = std::fs::canonicalize(&profile_path)?;
    create_gcroot(
        &gcroot_path,
        &StorePath::from(String::from(profile_store_path.to_string_lossy())),
    )?;
    Ok(())
}

fn install_nix_profile(store_path: &StorePath, profile_path: &str) -> process::Output {
    process::Command::new("nix-env")
        .arg("--profile")
        .arg(profile_path)
        .arg("--set")
        .arg(&store_path.path)
        .output()
        .expect("Failed to execute nix-env, is it on your path?")
}

fn create_gcroot(gcroot_path: &str, store_path: &StorePath) -> Result<(), Box<dyn Error>> {
    create_store_link(store_path, Path::new(gcroot_path))
}

fn create_store_link(store_path: &StorePath, from: &Path) -> Result<(), Box<dyn Error>> {
    println!("Creating symlink: {} -> {}", from.display(), store_path);
    if from.is_symlink() {
        fs::remove_file(from)
            .unwrap_or_else(|_| panic!("Error removing old symlink: {}.", from.display()));
    }
    unix::fs::symlink(&store_path.path, from).map_err(Box::from)
}

fn get_store_path(nix_build_result: process::Output) -> Result<StorePath, Box<dyn Error>> {
    if nix_build_result.status.success() {
        String::from_utf8(nix_build_result.stdout)
            .map_err(Box::from)
            .and_then(parse_nix_build_output)
    } else {
        String::from_utf8(nix_build_result.stderr).map_or_else(boxed_error(), boxed_error())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NixBuildOutput {
    drv_path: String,
    outputs: HashMap<String, String>,
}

fn parse_nix_build_output(output: String) -> Result<StorePath, Box<dyn Error>> {
    let expected_output_name = "out";
    let results: Vec<NixBuildOutput> = serde_json::from_str(&output)?;

    if let [result] = results.as_slice() {
        if let Some(store_path) = result.outputs.get(expected_output_name) {
            return Ok(StorePath::from(store_path.to_owned()));
        }
        return Err(format!(
            "No output '{}' found in nix build result.",
            expected_output_name
        )
        .into());
    }
    Err("Multiple build results were returned, we cannot handle that yet.".into())
}

fn run_nix_build(flake_uri: &str, flake_attr: &str) -> process::Output {
    process::Command::new("nix")
        .arg("build")
        .arg(format!("{}#{}", flake_uri, flake_attr))
        .arg("--json")
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
        if !s.trim().is_empty() {
            println!("{}", s.trim())
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
