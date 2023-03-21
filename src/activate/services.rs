use anyhow::{Context, Result};
use im::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use std::fs::DirBuilder;
use std::path::{self, Path, PathBuf};
use std::time::Duration;
use std::{fs, io, str};

use crate::{
    create_link, etc_dir, systemd, StorePath, SERVICES_STATE_FILE_NAME, SYSTEM_MANAGER_STATE_DIR,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceConfig {
    store_path: StorePath,
}

type Services = HashMap<String, ServiceConfig>;

fn print_services(services: &Services) -> Result<String> {
    let out = itertools::intersperse(
        services
            .iter()
            .map(|(name, entry)| format!("name: {name}, source:{}", entry.store_path)),
        "\n".to_owned(),
    )
    .collect();
    Ok(out)
}

pub fn activate(store_path: &StorePath, ephemeral: bool) -> Result<()> {
    verify_systemd_dir(ephemeral)?;

    let old_services = read_saved_services()?;

    log::info!("Reading new service definitions...");
    let file = fs::File::open(
        Path::new(&store_path.store_path)
            .join("services")
            .join("services.json"),
    )?;
    let reader = io::BufReader::new(file);
    let services: Services = serde_json::from_reader(reader)?;
    log::debug!("{}", print_services(&services)?);

    serialise_saved_services(&services)?;

    let services_to_stop = old_services.clone().relative_complement(services.clone());

    let service_manager = systemd::ServiceManager::new_session()?;
    let job_monitor = service_manager.monitor_jobs_init()?;
    let timeout = Some(Duration::from_secs(30));

    // We need to do this before we reload the systemd daemon, so that the daemon
    // still knows about these units.
    wait_for_jobs(
        &service_manager,
        job_monitor,
        stop_services(&service_manager, &services_to_stop)?,
        &timeout,
    )?;

    // We added all new services and removed old ones, so let's reload the units
    // to tell systemd about them.
    log::info!("Reloading the systemd daemon...");
    service_manager.daemon_reload()?;

    let active_targets = get_active_targets(&service_manager);
    let services_to_reload = get_services_to_reload(services, old_services);

    let job_monitor = service_manager.monitor_jobs_init()?;
    wait_for_jobs(
        &service_manager,
        job_monitor,
        reload_services(&service_manager, &services_to_reload)?
            + start_units(&service_manager, active_targets?)?,
        &timeout,
    )?;

    log::info!("Done");
    Ok(())
}

fn get_active_targets(
    service_manager: &systemd::ServiceManager,
) -> Result<Vec<systemd::UnitStatus>> {
    // We exclude some targets that we do not want to start
    let excluded_targets: HashSet<String> =
        ["suspend.target", "hibernate.target", "hybrid-sleep.target"]
            .iter()
            .map(ToOwned::to_owned)
            .collect();
    Ok(service_manager
        .list_units_by_patterns(&["active", "activating"], &[])?
        .into_iter()
        .filter(|unit| {
            unit.name.ends_with(".target")
                && !excluded_targets.contains(&unit.name)
                && !service_manager
                    .unit_manager(unit)
                    .refuse_manual_start()
                    .unwrap_or_else(|e| {
                        log::error!("Error communicating with DBus: {}", e);
                        true
                    })
        })
        .collect())
}

fn get_services_to_reload(services: Services, old_services: Services) -> Services {
    let mut services_to_reload = services.intersection(old_services.clone());
    services_to_reload.retain(|name, service| {
        if let Some(old_service) = old_services.get(name) {
            service.store_path != old_service.store_path
        } else {
            // Since we run this on the intersection, this should never happen
            panic!("Something went terribly wrong!");
        }
    });
    services_to_reload
}

fn systemd_system_dir(ephemeral: bool) -> PathBuf {
    if ephemeral {
        return Path::new(path::MAIN_SEPARATOR_STR)
            .join("run")
            .join("systemd")
            .join("system");
    } else {
        return Path::new(path::MAIN_SEPARATOR_STR)
            .join("etc")
            .join("systemd")
            .join("system");
    }
}

fn verify_systemd_dir(ephemeral: bool) -> Result<()> {
    if ephemeral {
        let system_dir = systemd_system_dir(ephemeral);
        if system_dir.exists()
            && !system_dir.is_symlink()
            && system_dir.is_dir()
            && system_dir.read_dir()?.next().is_some()
        {
            anyhow::bail!(
                "The directory {} exists and is not empty, we cannot symlink it.",
                system_dir.display()
            );
        } else if system_dir.exists() {
            if !system_dir.is_symlink() && system_dir.is_dir() {
                fs::remove_dir(&system_dir).with_context(|| {
                    format!(
                        "Error while removing the empty dir at {}",
                        system_dir.display()
                    )
                })?;
            } else {
                fs::remove_file(&system_dir).with_context(|| {
                    format!(
                        "Error while removing the symlink at {}",
                        system_dir.display()
                    )
                })?;
            }
        }

        let target = etc_dir(ephemeral).join("systemd").join("system");
        create_link(&target, &system_dir).with_context(|| {
            format!(
                "Error while creating symlink: {} -> {}",
                system_dir.display(),
                target.display(),
            )
        })?;
    }
    Ok(())
}

pub fn deactivate() -> Result<()> {
    restore_ephemeral_system_dir()?;

    let old_services = read_saved_services()?;
    log::debug!("{:?}", old_services);

    let service_manager = systemd::ServiceManager::new_session()?;
    let job_monitor = service_manager.monitor_jobs_init()?;
    let timeout = Some(Duration::from_secs(30));

    // We need to do this before we reload the systemd daemon, so that the daemon
    // still knows about these units.
    wait_for_jobs(
        &service_manager,
        job_monitor,
        stop_services(&service_manager, &old_services)?,
        &timeout,
    )?;

    // We removed all old services, so let's reload the units so that
    // the systemd daemon is up-to-date
    log::info!("Reloading the systemd daemon...");
    service_manager.daemon_reload()?;

    serialise_saved_services(&HashMap::new())?;

    log::info!("Done");
    Ok(())
}

// If we turned the ephemeral systemd system dir under /run into a symlink,
// then systemd crashes when that symlink goes broken.
// To avoid this, we always check whether this directory exists and is correct,
// and we recreate it if needed.
// NOTE: We rely on the fact that the etc files get cleaned up first, before this runs!
fn restore_ephemeral_system_dir() -> Result<()> {
    let ephemeral_systemd_system_dir = systemd_system_dir(true);
    if !ephemeral_systemd_system_dir.exists() {
        if ephemeral_systemd_system_dir.is_symlink() {
            fs::remove_file(&ephemeral_systemd_system_dir)?;
        }
        fs::create_dir_all(&ephemeral_systemd_system_dir)?;
    }
    Ok(())
}

// FIXME: we should probably lock this file to avoid concurrent writes
fn serialise_saved_services(services: &Services) -> Result<()> {
    let state_file = Path::new(SYSTEM_MANAGER_STATE_DIR).join(SERVICES_STATE_FILE_NAME);
    DirBuilder::new()
        .recursive(true)
        .create(SYSTEM_MANAGER_STATE_DIR)?;

    log::info!("Writing state info into file: {}", state_file.display());
    let writer = io::BufWriter::new(fs::File::create(state_file)?);
    serde_json::to_writer(writer, services)?;
    Ok(())
}

fn read_saved_services() -> Result<Services> {
    let state_file = Path::new(SYSTEM_MANAGER_STATE_DIR).join(SERVICES_STATE_FILE_NAME);
    DirBuilder::new()
        .recursive(true)
        .create(SYSTEM_MANAGER_STATE_DIR)?;

    if Path::new(&state_file).is_file() {
        log::info!("Reading state info from {}", state_file.display());
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

fn stop_services(
    service_manager: &systemd::ServiceManager,
    services: &Services,
) -> Result<HashSet<JobId>> {
    for_each_service(|s| service_manager.stop_unit(s), services, "stopping")
}

fn reload_services(
    service_manager: &systemd::ServiceManager,
    services: &Services,
) -> Result<HashSet<JobId>> {
    for_each_service(|s| service_manager.reload_unit(s), services, "reloading")
}

fn for_each_service<F, R>(
    action: F,
    services: &Services,
    log_action: &str,
) -> Result<HashSet<JobId>>
where
    F: Fn(&str) -> Result<R>,
{
    let successful_services: HashSet<JobId> =
        services
            .clone()
            .into_iter()
            .fold(HashSet::new(), |mut set, (service, _)| {
                match action(&service) {
                    Ok(_) => {
                        log::info!("Service {}: {}...", service, log_action);
                        set.insert(JobId { id: service });
                        set
                    }
                    Err(e) => {
                        log::error!(
                            "Service {service}: error {log_action}, please consult the logs"
                        );
                        log::error!("{e}");
                        set
                    }
                }
            });
    // TODO: do we want to propagate unit failures here in some way?
    Ok(successful_services)
}

fn start_units(
    service_manager: &systemd::ServiceManager,
    units: Vec<systemd::UnitStatus>,
) -> Result<HashSet<JobId>> {
    for_each_unit(|s| service_manager.start_unit(&s.name), units, "restarting")
}

fn for_each_unit<F, R>(
    action: F,
    units: Vec<systemd::UnitStatus>,
    log_action: &str,
) -> Result<HashSet<JobId>>
where
    F: Fn(&systemd::UnitStatus) -> Result<R>,
{
    let successful_services: HashSet<JobId> =
        units
            .into_iter()
            .fold(HashSet::new(), |mut set, unit| match action(&unit) {
                Ok(_) => {
                    log::info!("Unit {}: {}...", unit.name, log_action);
                    set.insert(JobId { id: unit.name });
                    set
                }
                Err(e) => {
                    log::error!(
                        "Service {}: error {log_action}, please consult the logs",
                        unit.name
                    );
                    log::error!("{e}");
                    set
                }
            });

    // TODO: do we want to propagate unit failures here in some way?
    Ok(successful_services)
}

fn wait_for_jobs(
    service_manager: &systemd::ServiceManager,
    job_monitor: systemd::JobMonitor,
    jobs: HashSet<JobId>,
    timeout: &Option<Duration>,
) -> Result<()> {
    if !service_manager.monitor_jobs_finish(job_monitor, timeout, jobs)? {
        anyhow::bail!("Timeout waiting for systemd jobs");
    }
    Ok(())
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct JobId {
    id: String,
}

impl From<JobId> for String {
    fn from(value: JobId) -> Self {
        value.id
    }
}
