use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::DirBuilder;
use std::path::Path;
use std::time::Duration;
use std::{fs, io, str};

use super::{create_store_link, systemd, StorePath, SERVICE_MANAGER_STATE_DIR, SYSTEMD_UNIT_DIR};

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
    linked_path: String,
}

type LinkedServices = HashMap<String, LinkedServiceConfig>;

pub fn activate(store_path: StorePath) -> Result<()> {
    log::info!("Activating service-manager profile: {}", store_path);

    log::debug!("{:?}", read_linked_services()?);

    log::info!("Reading service definitions...");
    let file = fs::File::open(store_path.store_path + "/services/services.json")?;
    let reader = io::BufReader::new(file);
    let services: Services = serde_json::from_reader(reader)?;

    let linked_services = link_services(services);
    serialise_linked_services(&linked_services)?;

    let service_manager = systemd::ServiceManager::new_session()?;
    start_services(
        &service_manager,
        linked_services,
        &Some(Duration::from_secs(30)),
    )?;
    Ok(())
}

fn link_services(services: Services) -> LinkedServices {
    services.iter().fold(
        HashMap::with_capacity(services.len()),
        |mut linked_services, (name, service_config)| {
            let linked_path = format!("{}/{}", SYSTEMD_UNIT_DIR, name);
            match create_store_link(&service_config.store_path, Path::new(&linked_path)) {
                Ok(_) => {
                    linked_services.insert(
                        name.to_owned(),
                        LinkedServiceConfig {
                            service_config: service_config.to_owned(),
                            linked_path,
                        },
                    );
                    linked_services
                }
                e @ Err(_) => {
                    log::error!("Error linking service {}, skipping.", name);
                    log::error!("{:?}", e);
                    linked_services
                }
            }
        },
    )
}

// FIXME: we should probably lock this file to avoid concurrent writes
fn serialise_linked_services(linked_services: &LinkedServices) -> Result<()> {
    let state_file = format!("{}/services.json", SERVICE_MANAGER_STATE_DIR);
    DirBuilder::new()
        .recursive(true)
        .create(SERVICE_MANAGER_STATE_DIR)?;

    log::info!("Writing state info into file: {}", state_file);
    let writer = io::BufWriter::new(fs::File::create(state_file)?);
    serde_json::to_writer(writer, linked_services)?;
    Ok(())
}

fn read_linked_services() -> Result<LinkedServices> {
    let state_file = format!("{}/services.json", SERVICE_MANAGER_STATE_DIR);
    DirBuilder::new()
        .recursive(true)
        .create(SERVICE_MANAGER_STATE_DIR)?;

    if Path::new(&state_file).is_file() {
        log::info!("Reading state info from {}", state_file);
        let reader = io::BufReader::new(fs::File::open(state_file)?);
        let linked_services = serde_json::from_reader(reader)?;
        return Ok(linked_services);
    }
    Ok(HashMap::default())
}

fn start_services(
    service_manager: &systemd::ServiceManager,
    services: LinkedServices,
    timeout: &Option<Duration>,
) -> Result<()> {
    service_manager.daemon_reload()?;

    let job_monitor = service_manager.monitor_jobs_init()?;

    let successful_services = services.keys().fold(
        HashSet::with_capacity(services.len()),
        |mut set, service| match service_manager.restart_unit(service) {
            Ok(_) => {
                log::info!("Restarting service {}...", service);
                set.insert(Box::new(service.to_owned()));
                set
            }
            Err(e) => {
                log::error!(
                    "Error restarting unit, please consult the logs: {}",
                    service
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
