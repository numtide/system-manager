use anyhow::Context;
use im::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use std::path::{self, Path, PathBuf};
use std::time::Duration;
use std::{fs, io};

use super::ActivationResult;
use crate::activate::ActivationError;
use crate::unit_info::{parse_systemd_bool, parse_unit};
use crate::{create_link, etc_dir, systemd, StorePath};

type ServiceActivationResult = ActivationResult<Services>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceConfig {
    /// absent for masked units
    store_path: Option<StorePath>,
    #[serde(default)]
    masked: bool,
}

pub type Services = HashMap<String, ServiceConfig>;

fn print_services(services: &Services) -> String {
    let out = itertools::intersperse(
        services.iter().map(|(name, entry)| {
            if entry.masked {
                format!("name: {name}, masked")
            } else if let Some(ref path) = entry.store_path {
                format!("name: {name}, source:{path}")
            } else {
                format!("name: {name}, source:<none>")
            }
        }),
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

    let (masked, active): (Services, Services) = services
        .clone()
        .into_iter()
        .partition(|(_, cfg)| cfg.masked);

    let services_to_stop =
        get_services_to_stop(old_services.clone().relative_complement(active.clone()));
    let services_to_reload = get_services_to_reload(active.clone(), old_services.clone());

    let service_manager = systemd::ServiceManager::new_session()
        .map_err(|e| ActivationError::with_partial_result(old_services.clone(), e))?;
    let job_monitor = service_manager
        .monitor_jobs_init()
        .map_err(|e| ActivationError::with_partial_result(old_services.clone(), e))?;
    let timeout = Some(Duration::from_secs(30));

    // Units systemd refused a job for: the activation isn't complete.
    let mut refused = vec![];

    // Stop removed services and any masked services that might still be running
    // (e.g. distro-provided units). Must happen before daemon-reload so systemd
    // still knows about these units.
    let mut units_to_stop = convert_services(&services_to_stop);
    units_to_stop.extend(convert_services(&masked));
    wait_for_jobs(
        &service_manager,
        &job_monitor,
        stop_services(&service_manager, units_to_stop, &mut refused),
        &timeout,
    )
    .map_err(|e| ActivationError::with_partial_result(services.clone(), e))?;

    wait_for_jobs(
        &service_manager,
        &job_monitor,
        reload_or_restart_units(
            &service_manager,
            convert_services(&services_to_reload),
            &mut refused,
        ) + start_units(&service_manager, ["system-manager.target"], &mut refused),
        &timeout,
    )
    .map_err(|e| ActivationError::with_partial_result(services.clone(), e))?;

    if !masked.is_empty() {
        let unit_names: Vec<&str> = masked.keys().map(AsRef::as_ref).collect();
        service_manager
            .mask_unit_files(&unit_names, ephemeral)
            .with_context(|| format!("masking {} unit(s)", masked.len()))
            .map_err(|e| ActivationError::with_partial_result(services.clone(), e))?;

        log::info!("Reloading systemd daemon after masking...");
        service_manager
            .daemon_reload()
            .map_err(|e| ActivationError::with_partial_result(services.clone(), e))?;

        log::info!(
            "Masked {} unit(s): {}",
            masked.len(),
            masked.keys().cloned().collect::<Vec<_>>().join(", ")
        );
    }

    if !refused.is_empty() {
        return Err(ActivationError::with_partial_result(
            services,
            anyhow::anyhow!(refused.join("; ")),
        ));
    }

    log::info!("Done");
    Ok(services)
}

/// The managed units systemd reports as failed once activation is done: as NixOS'
/// switch-to-configuration does, a unit that failed to (re)start doesn't fail the activation
/// by itself, but it is reported, so that the caller can judge it against what was failing
/// before.
pub fn failed_units(services: &Services) -> anyhow::Result<Vec<String>> {
    let names: Vec<&str> = services
        .iter()
        .filter(|(_, cfg)| !cfg.masked)
        .map(|(name, _)| name.as_str())
        .collect();
    if names.is_empty() {
        return Ok(vec![]);
    }
    let service_manager = systemd::ServiceManager::new_session()?;
    let mut failed: Vec<String> = service_manager
        .list_units_by_patterns(&["failed"], &names)?
        .into_iter()
        .map(|unit| unit.name)
        .collect();
    failed.sort();
    Ok(failed)
}

fn get_services_to_reload(services: Services, old_services: Services) -> Services {
    let mut services_to_reload = services.intersection(old_services.clone());
    services_to_reload.retain(|name, service| {
        if service.masked {
            return false;
        }
        if let Some(old_service) = old_services.get(name) {
            if service.store_path == old_service.store_path {
                return false;
            }
            if let Some(ref path) = service.store_path {
                let unit_info = parse_unit(&path.store_path)
                    .inspect_err(|e| log::debug!("unable to parse unit file {path}: {e}"))
                    .ok();
                if !parse_systemd_bool(unit_info.as_ref(), "Service", "X-RestartIfChanged", true) {
                    log::info!("Skipping restart of {name}: X-RestartIfChanged=false");
                    return false;
                }
            }
            true
        } else {
            // Since we run this on the intersection, this should never happen
            panic!("Something went terribly wrong!");
        }
    });
    services_to_reload
}

fn get_services_to_stop(services_to_stop: Services) -> Services {
    let mut services_to_stop = services_to_stop;
    services_to_stop.retain(|name, service| {
        if let Some(ref path) = service.store_path {
            let unit_data = parse_unit(Path::new(&path.store_path)).ok();
            if !parse_systemd_bool(unit_data.as_ref(), "Unit", "X-StopOnRemoval", true) {
                log::info!("Skipping stop of {name}: X-StopOnRemoval=false");
                return false;
            }
        }
        true
    });
    services_to_stop
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

    // masked units can't be running, skip them
    let stoppable: Services = old_services
        .clone()
        .into_iter()
        .filter(|(_, cfg)| !cfg.masked)
        .collect();

    let service_manager = systemd::ServiceManager::new_session()
        .map_err(|e| ActivationError::with_partial_result(old_services.clone(), e))?;
    if !stoppable.is_empty() {
        let job_monitor = service_manager
            .monitor_jobs_init()
            .map_err(|e| ActivationError::with_partial_result(old_services.clone(), e))?;
        let timeout = Some(Duration::from_secs(30));

        let mut units_to_stop = convert_services(&stoppable);
        units_to_stop.push("system-manager.target");
        // We need to do this before we reload the systemd daemon, so that the daemon
        // still knows about these units.
        wait_for_jobs(
            &service_manager,
            &job_monitor,
            stop_services(&service_manager, units_to_stop, &mut vec![]),
            &timeout,
        )
        // We consider all jobs stopped now..
        .map_err(|e| ActivationError::with_partial_result(im::HashMap::new(), e))?;
    } else {
        log::info!("No services to deactivate.");
    }

    // Unmask previously masked units via D-Bus, try both persistent and
    // runtime paths since we don't know which mode was used during activation
    let masked_names: Vec<&str> = old_services
        .iter()
        .filter(|(_, cfg)| cfg.masked)
        .map(|(name, _)| name.as_str())
        .collect();
    if !masked_names.is_empty() {
        for runtime in [false, true] {
            if let Err(e) = service_manager.unmask_unit_files(&masked_names, runtime) {
                log::error!("Error unmasking units (runtime={runtime}): {e}");
            }
        }
        log::info!(
            "Unmasked {} unit(s): {}",
            masked_names.len(),
            masked_names.join(", ")
        );
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

fn stop_services<'a, U>(
    service_manager: &systemd::ServiceManager,
    units: U,
    refused: &mut Vec<String>,
) -> HashSet<JobId>
where
    U: AsRef<[&'a str]>,
{
    for_each_unit(
        |s| service_manager.stop_unit(s),
        units.as_ref(),
        "stopping",
        // An inactive unit whose file is gone isn't loaded any more: it is already stopped.
        is_not_loaded,
        refused,
    )
}

fn is_not_loaded(e: &anyhow::Error) -> bool {
    e.downcast_ref::<dbus::Error>()
        .and_then(dbus::Error::name)
        .is_some_and(|name| name == "org.freedesktop.systemd1.NoSuchUnit")
}

fn reload_or_restart_units<'a, U>(
    service_manager: &systemd::ServiceManager,
    units: U,
    refused: &mut Vec<String>,
) -> HashSet<JobId>
where
    U: AsRef<[&'a str]>,
{
    for_each_unit(
        |s| service_manager.reload_or_restart_unit(s),
        units.as_ref(),
        "reloading",
        |_| false,
        refused,
    )
}

fn start_units<'a, U>(
    service_manager: &systemd::ServiceManager,
    units: U,
    refused: &mut Vec<String>,
) -> HashSet<JobId>
where
    U: AsRef<[&'a str]>,
{
    for_each_unit(
        |unit| service_manager.start_unit(unit),
        units.as_ref(),
        "starting",
        |_| false,
        refused,
    )
}

fn convert_services(services: &Services) -> Vec<&str> {
    services.keys().map(AsRef::as_ref).collect::<Vec<&str>>()
}

/// Queues `action` for each unit, returning the jobs to wait for. A unit systemd refuses a
/// job for is logged and added to `refused`, unless `nothing_to_do` says the error means the
/// unit is already where `action` would put it.
fn for_each_unit<'a, F, N, R, S>(
    action: F,
    units: S,
    log_action: &str,
    nothing_to_do: N,
    refused: &mut Vec<String>,
) -> HashSet<JobId>
where
    F: Fn(&str) -> anyhow::Result<R>,
    N: Fn(&anyhow::Error) -> bool,
    S: AsRef<[&'a str]>,
{
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
            Err(e) if nothing_to_do(&e) => {
                log::debug!("Unit {unit}: nothing to do for {log_action}: {e}");
                set
            }
            Err(e) => {
                log::error!(
                    "Service {}: error {log_action}, please consult the logs",
                    unit
                );
                log::error!("{e}");
                refused.push(format!("{unit}: error {log_action}: {e}"));
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

/// A job systemd refuses is added to `refused` rather than returned, so that the activation
/// carries on and records what it applied.
pub fn restart_sysinit_reactivation_target(refused: &mut Vec<String>) -> anyhow::Result<()> {
    let service_manager = systemd::ServiceManager::new_session()?;
    let job_monitor = service_manager.monitor_jobs_init()?;
    let timeout = Some(Duration::from_secs(30));

    log::info!("Reloading the systemd daemon...");
    service_manager.daemon_reload()?;

    let jobs = for_each_unit(
        |unit| service_manager.restart_unit(unit),
        ["sysinit-reactivation.target"],
        "restarting",
        |_| false,
        refused,
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
    let mut refused = vec![];
    let jobs = for_each_unit(
        |unit| service_manager.restart_unit(unit),
        ["userborn.service"],
        "restarting",
        |_| false,
        &mut refused,
    );

    wait_for_jobs(&service_manager, &job_monitor, jobs, &timeout)?;
    if !refused.is_empty() {
        anyhow::bail!(refused.join("; "));
    }
    // A oneshot's job finishes whether it succeeded or not: users exist only if it did.
    if !service_manager
        .list_units_by_patterns(&["failed"], &["userborn.service"])?
        .is_empty()
    {
        anyhow::bail!("userborn.service failed");
    }
    log::info!("userborn.service completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unit_systemd_has_not_loaded_needs_no_stop() {
        let not_loaded: anyhow::Error = dbus::Error::new_custom(
            "org.freedesktop.systemd1.NoSuchUnit",
            "Unit gone.service not loaded.",
        )
        .into();
        assert!(is_not_loaded(&not_loaded));

        let refused: anyhow::Error = dbus::Error::new_custom(
            "org.freedesktop.systemd1.OnlyByDependency",
            "Operation refused, unit pinned.service may be requested by dependency only.",
        )
        .into();
        assert!(!is_not_loaded(&refused));
        assert!(!is_not_loaded(&anyhow::anyhow!("something else")));
    }
}
