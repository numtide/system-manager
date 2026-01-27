use anyhow::Context;
use im::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use std::path::{self, Path, PathBuf};
use std::time::Duration;
use std::{fs, io, str};

use super::ActivationResult;
use crate::activate::ActivationError;
use crate::{create_link, etc_dir, systemd, StorePath};

type ServiceActivationResult = ActivationResult<Services>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceConfig {
    store_path: StorePath,
}

pub type Services = HashMap<String, ServiceConfig>;

fn print_services(services: &Services) -> String {
    let out = itertools::intersperse(
        services
            .iter()
            .map(|(name, entry)| format!("name: {name}, source:{}", entry.store_path)),
        "\n".to_owned(),
    )
    .collect();
    out
}

pub fn get_active_services(
    store_path: &StorePath,
    old_services: Services,
) -> ServiceActivationResult {
    log::info!("Reading new service definitions...");
    let file = fs::File::open(
        Path::new(&store_path.store_path)
            .join("services")
            .join("services.json"),
    )
    .map_err(|e| ActivationError::with_partial_result(old_services.clone(), e))?;
    let reader = io::BufReader::new(file);
    let services: Services = serde_json::from_reader(reader)
        .map_err(|e| ActivationError::with_partial_result(old_services.clone(), e))?;
    log::debug!("{}", print_services(&services));
    Ok(services)
}

pub fn activate(
    store_path: &StorePath,
    old_services: Services,
    ephemeral: bool,
) -> ServiceActivationResult {
    verify_systemd_dir(ephemeral)
        .map_err(|e| ActivationError::with_partial_result(old_services.clone(), e))?;

    let services = get_active_services(store_path, old_services.clone())?;

    let services_to_stop = old_services.clone().relative_complement(services.clone());
    let services_to_reload = get_services_to_reload(services.clone(), old_services.clone());

    let service_manager = systemd::ServiceManager::new_session()
        .map_err(|e| ActivationError::with_partial_result(old_services.clone(), e))?;
    let job_monitor = service_manager
        .monitor_jobs_init()
        .map_err(|e| ActivationError::with_partial_result(old_services.clone(), e))?;
    let timeout = Some(Duration::from_secs(30));

    // We need to do this before we reload the systemd daemon, so that the daemon
    // still knows about these units.
    // TODO: handle jobs that were not running, this throws an error now.
    wait_for_jobs(
        &service_manager,
        &job_monitor,
        stop_services(&service_manager, convert_services(&services_to_stop)),
        &timeout,
    )
    .map_err(|e| ActivationError::with_partial_result(services.clone(), e))?;

    wait_for_jobs(
        &service_manager,
        &job_monitor,
        reload_or_restart_units(&service_manager, convert_services(&services_to_reload))
            + start_units(&service_manager, ["system-manager.target"]),
        &timeout,
    )
    .map_err(|e| ActivationError::with_partial_result(services.clone(), e))?;

    log::info!("Done");
    Ok(services)
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
        Path::new(path::MAIN_SEPARATOR_STR)
            .join("run")
            .join("systemd")
            .join("system")
    } else {
        Path::new(path::MAIN_SEPARATOR_STR)
            .join("etc")
            .join("systemd")
            .join("system")
    }
}

fn verify_systemd_dir(ephemeral: bool) -> anyhow::Result<()> {
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

pub fn deactivate(old_services: Services) -> ServiceActivationResult {
    log::debug!("{:?}", old_services);

    restore_ephemeral_system_dir()
        .map_err(|e| ActivationError::with_partial_result(old_services.clone(), e))?;

    let service_manager = systemd::ServiceManager::new_session()
        .map_err(|e| ActivationError::with_partial_result(old_services.clone(), e))?;
    if !old_services.is_empty() {
        let job_monitor = service_manager
            .monitor_jobs_init()
            .map_err(|e| ActivationError::with_partial_result(old_services.clone(), e))?;
        let timeout = Some(Duration::from_secs(30));

        let mut units_to_stop = convert_services(&old_services);
        units_to_stop.push("system-manager.target");
        // We need to do this before we reload the systemd daemon, so that the daemon
        // still knows about these units.
        wait_for_jobs(
            &service_manager,
            &job_monitor,
            stop_services(&service_manager, units_to_stop),
            &timeout,
        )
        // We consider all jobs stopped now..
        .map_err(|e| ActivationError::with_partial_result(im::HashMap::new(), e))?;
    } else {
        log::info!("No services to deactivate.");
    }
    log::info!("Reloading the systemd daemon...");
    service_manager
        .daemon_reload()
        .map_err(|e| ActivationError::with_partial_result(im::HashMap::new(), e))?;

    log::info!("Done");
    Ok(im::HashMap::new())
}

// If we turned the ephemeral systemd system dir under /run into a symlink,
// then systemd crashes when that symlink goes broken.
// To avoid this, we always check whether this directory exists and is correct,
// and we recreate it if needed.
// NOTE: We rely on the fact that the etc files get cleaned up first, before this runs!
fn restore_ephemeral_system_dir() -> anyhow::Result<()> {
    let ephemeral_systemd_system_dir = systemd_system_dir(true);
    if !ephemeral_systemd_system_dir.exists() {
        if ephemeral_systemd_system_dir.is_symlink() {
            fs::remove_file(&ephemeral_systemd_system_dir)?;
        }
        fs::create_dir_all(&ephemeral_systemd_system_dir)?;
    }
    Ok(())
}

fn stop_services<'a, U>(service_manager: &systemd::ServiceManager, units: U) -> HashSet<JobId>
where
    U: AsRef<[&'a str]>,
{
    for_each_unit(|s| service_manager.stop_unit(s), units.as_ref(), "stopping")
}

fn reload_or_restart_units<'a, U>(
    service_manager: &systemd::ServiceManager,
    units: U,
) -> HashSet<JobId>
where
    U: AsRef<[&'a str]>,
{
    for_each_unit(
        |s| service_manager.reload_or_restart_unit(s),
        units.as_ref(),
        "reloading",
    )
}

fn start_units<'a, U>(service_manager: &systemd::ServiceManager, units: U) -> HashSet<JobId>
where
    U: AsRef<[&'a str]>,
{
    for_each_unit(
        |unit| service_manager.start_unit(unit),
        units.as_ref(),
        "starting",
    )
}

fn convert_services(services: &Services) -> Vec<&str> {
    services.keys().map(AsRef::as_ref).collect::<Vec<&str>>()
}

fn for_each_unit<'a, F, R, S>(action: F, units: S, log_action: &str) -> HashSet<JobId>
where
    F: Fn(&str) -> anyhow::Result<R>,
    S: AsRef<[&'a str]>,
{
    // TODO: do we want to propagate unit failures here in some way?
    units
        .as_ref()
        .iter()
        .fold(HashSet::new(), |mut set, unit| match action(unit) {
            Ok(_) => {
                log::debug!("Unit {}: {}...", unit, log_action);
                set.insert(JobId {
                    id: (*unit).to_owned(),
                });
                set
            }
            Err(e) => {
                log::error!(
                    "Service {}: error {log_action}, please consult the logs",
                    unit
                );
                log::error!("{e}");
                set
            }
        })
}

fn wait_for_jobs(
    service_manager: &systemd::ServiceManager,
    job_monitor: &systemd::JobMonitor,
    jobs: HashSet<JobId>,
    timeout: &Option<Duration>,
) -> anyhow::Result<()> {
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

pub fn restart_sysinit_reactivation_target() -> anyhow::Result<()> {
    let service_manager = systemd::ServiceManager::new_session()?;
    let job_monitor = service_manager.monitor_jobs_init()?;
    let timeout = Some(Duration::from_secs(30));

    log::info!("Reloading the systemd daemon...");
    service_manager.daemon_reload()?;

    let jobs = for_each_unit(
        |unit| service_manager.restart_unit(unit),
        ["sysinit-reactivation.target"],
        "restarting",
    );

    wait_for_jobs(&service_manager, &job_monitor, jobs, &timeout)?;
    Ok(())
}

/// This must be called after daemon-reload so systemd knows about the unit,
/// but before tmpfiles activation since tmpfiles may reference users that
/// userborn needs to create.
pub fn restart_userborn_if_exists() -> anyhow::Result<()> {
    let service_manager = systemd::ServiceManager::new_session()?;

    // Check if userborn.service exists by listing units matching the pattern
    let units = service_manager.list_units_by_patterns(&[], &["userborn.service"])?;

    if units.is_empty() {
        log::debug!("userborn.service not found, skipping");
        return Ok(());
    }

    log::info!("Restarting userborn.service to create users before tmpfiles...");
    let job_monitor = service_manager.monitor_jobs_init()?;
    let timeout = Some(Duration::from_secs(30));

    // We use restart rather than start because userborn is a oneshot service
    // with RemainAfterExit=true.
    let jobs = for_each_unit(
        |unit| service_manager.restart_unit(unit),
        ["userborn.service"],
        "restarting",
    );

    wait_for_jobs(&service_manager, &job_monitor, jobs, &timeout)?;
    log::info!("userborn.service completed");
    Ok(())
}
