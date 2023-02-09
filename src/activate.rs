use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::DirBuilder;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, io, str};

use super::{
    create_store_link, remove_store_link, systemd, StorePath, STATE_FILE_NAME, SYSTEMD_UNIT_DIR,
    SYSTEM_MANAGER_STATE_DIR,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceConfig {
    #[serde(flatten)]
    store_path: StorePath,
}

type Services = HashMap<String, ServiceConfig>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinkedServiceConfig {
    #[serde(flatten)]
    service_config: ServiceConfig,
    #[serde(rename = "linkedPath")]
    path: String,
}

impl LinkedServiceConfig {
    fn linked_path(&self) -> PathBuf {
        PathBuf::from(self.path.to_owned())
    }

    fn new(service_config: ServiceConfig, path: PathBuf) -> Result<Self> {
        if let Some(path) = path.to_str() {
            return Ok(LinkedServiceConfig {
                service_config,
                path: String::from(path),
            });
        }
        anyhow::bail!("Could not decode path")
    }
}

type LinkedServices = HashMap<String, LinkedServiceConfig>;

pub fn activate(store_path: StorePath) -> Result<()> {
    log::info!("Activating system-manager profile: {}", store_path);

    let old_linked_services = read_linked_services()?;
    log::debug!("{:?}", old_linked_services);

    log::info!("Reading service definitions...");
    let file = fs::File::open(store_path.store_path + "/services/services.json")?;
    let reader = io::BufReader::new(file);
    let services: Services = serde_json::from_reader(reader)?;

    let linked_services = link_services(services)?;
    serialise_linked_services(&linked_services)?;

    let services_to_stop = old_linked_services
        .into_iter()
        .filter(|(name, _)| !linked_services.contains_key(name))
        .collect();

    let service_manager = systemd::ServiceManager::new_session()?;
    let timeout = Some(Duration::from_secs(30));

    service_manager.daemon_reload()?;
    stop_services(&service_manager, &services_to_stop, &timeout)?;
    unlink_services(&services_to_stop)?;
    start_services(&service_manager, &linked_services, &timeout)?;

    log::info!("Done");
    Ok(())
}

fn unlink_services(services: &LinkedServices) -> Result<()> {
    services
        .values()
        .try_for_each(|linked_service| remove_store_link(linked_service.linked_path().as_path()))
}

fn link_services(services: Services) -> Result<LinkedServices> {
    services.iter().try_fold(
        HashMap::with_capacity(services.len()),
        |mut linked_services, (name, service_config)| {
            let linked_path = PathBuf::from(format!("{}/{}", SYSTEMD_UNIT_DIR, name));
            match create_store_link(&service_config.store_path, linked_path.as_path()) {
                Ok(_) => {
                    linked_services.insert(
                        name.to_owned(),
                        LinkedServiceConfig::new(service_config.to_owned(), linked_path)?,
                    );
                    Ok(linked_services)
                }
                e @ Err(_) => {
                    log::error!("Error linking service {}, skipping.", name);
                    log::error!("{:?}", e);
                    Ok(linked_services)
                }
            }
        },
    )
}

// FIXME: we should probably lock this file to avoid concurrent writes
fn serialise_linked_services(linked_services: &LinkedServices) -> Result<()> {
    let state_file = format!("{}/{}", SYSTEM_MANAGER_STATE_DIR, STATE_FILE_NAME);
    DirBuilder::new()
        .recursive(true)
        .create(SYSTEM_MANAGER_STATE_DIR)?;

    log::info!("Writing state info into file: {}", state_file);
    let writer = io::BufWriter::new(fs::File::create(state_file)?);
    serde_json::to_writer(writer, linked_services)?;
    Ok(())
}

fn read_linked_services() -> Result<LinkedServices> {
    let state_file = format!("{}/{}", SYSTEM_MANAGER_STATE_DIR, STATE_FILE_NAME);
    DirBuilder::new()
        .recursive(true)
        .create(SYSTEM_MANAGER_STATE_DIR)?;

    if Path::new(&state_file).is_file() {
        log::info!("Reading state info from {}", state_file);
        let reader = io::BufReader::new(fs::File::open(state_file)?);
        match serde_json::from_reader(reader) {
            Ok(linked_services) => return Ok(linked_services),
            Err(e) => {
                log::error!("Error reading the state file, ignoring.");
                log::error!("{:?}", e);
            }
        }
    }
    Ok(HashMap::default())
}

fn start_services(
    service_manager: &systemd::ServiceManager,
    services: &LinkedServices,
    timeout: &Option<Duration>,
) -> Result<()> {
    for_each_service(
        |s| service_manager.start_unit(s),
        service_manager,
        services,
        timeout,
        "restarting",
    )
}

fn stop_services(
    service_manager: &systemd::ServiceManager,
    services: &LinkedServices,
    timeout: &Option<Duration>,
) -> Result<()> {
    for_each_service(
        |s| service_manager.stop_unit(s),
        service_manager,
        services,
        timeout,
        "stopping",
    )
}

fn for_each_service<F, R>(
    f: F,
    service_manager: &systemd::ServiceManager,
    services: &LinkedServices,
    timeout: &Option<Duration>,
    log_action: &str,
) -> Result<()>
where
    F: Fn(&str) -> Result<R>,
{
    let job_monitor = service_manager.monitor_jobs_init()?;

    let successful_services = services.keys().fold(
        HashSet::with_capacity(services.len()),
        |mut set, service| match f(service) {
            Ok(_) => {
                log::info!("Service {}: {}...", service, log_action);
                set.insert(Box::new(service.to_owned()));
                set
            }
            Err(e) => {
                log::error!(
                    "Service {}: Error {}, please consult the logs",
                    service,
                    log_action
                );
                log::error!("{}", e);
                set
            }
        },
    );

    if !service_manager.monitor_jobs_finish(job_monitor, timeout, successful_services)? {
        anyhow::bail!("Timeout waiting for systemd jobs");
    }

    // TODO: do we want to propagate unit failures here in some way?
    Ok(())
}
