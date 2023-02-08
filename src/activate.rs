use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;
use std::{fs, io, iter, str};

use super::{create_store_link, systemd, StorePath};

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

pub fn activate(store_path: StorePath) -> Result<()> {
    log::info!("Activating service-manager profile: {}", store_path);

    let file = fs::File::open(store_path.path + "/services/services.json")?;
    let reader = io::BufReader::new(file);

    let services: Vec<ServiceConfig> = serde_json::from_reader(reader)?;

    services.iter().try_for_each(|service| {
        create_store_link(
            &service.store_path(),
            Path::new(&format!("/run/systemd/system/{}", service.name)),
        )
    })?;

    let service_manager = systemd::ServiceManager::new_session()?;
    start_services(&service_manager, &services, &Some(Duration::from_secs(30)))?;

    Ok(())
}

fn start_services(
    service_manager: &systemd::ServiceManager,
    services: &[ServiceConfig],
    timeout: &Option<Duration>,
) -> Result<()> {
    service_manager.daemon_reload()?;

    let job_monitor = service_manager.monitor_jobs_init()?;

    let successful_services = services.iter().fold(HashSet::new(), |set, service| {
        match service_manager.restart_unit(&service.name) {
            Ok(_) => {
                log::info!("Restarting service {}...", service.name);
                set.into_iter()
                    .chain(iter::once(Box::new(service.name.to_owned())))
                    .collect()
            }
            Err(e) => {
                log::error!(
                    "Error restarting unit, please consult the logs: {}",
                    service.name
                );
                log::error!("{}", e);
                set
            }
        }
    });

    if !service_manager.monitor_jobs_finish(job_monitor, timeout, successful_services)? {
        anyhow::bail!("Timeout waiting for systemd jobs");
    }

    // TODO: do we want to propagate unit failures here in some way?
    Ok(())
}
