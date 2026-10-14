use anyhow::Context;
use im::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use std::path::{self, Path, PathBuf};
use std::time::Duration;
use std::{fmt, fs, io};

use super::ActivationResult;
use crate::activate::ActivationError;
use crate::unit_info::{parse_systemd_bool, parse_unit};
use crate::{create_link, etc_dir, systemd, StorePath};

type ServiceActivationResult = ActivationResult<Services>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceConfig {
    /// absent for masked units
    store_path: Option<StorePath>,
    #[serde(default)]
    masked: bool,
}

pub type Services = HashMap<String, ServiceConfig>;

/// A job systemd accepted and then didn't complete: its unit, and the result systemd's
/// JobRemoved signal gave (`failed`, `dependency`, `timeout`, `canceled`, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnfinishedJob {
    unit: String,
    result: String,
}

impl fmt::Display for UnfinishedJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (job {})", self.unit, self.result)
    }
}

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

/// A (re)start or reload job that didn't complete, on a unit the profile keeps, is added to
/// `failed_jobs`: that is a unit failure, for `failed_units` to report.
pub fn activate(
    store_path: &StorePath,
    old_services: Services,
    ephemeral: bool,
    failed_jobs: &mut Vec<UnfinishedJob>,
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

    // What wasn't applied: units systemd refused a job for, and units the profile drops or
    // masks whose stop didn't complete, which may still be running. The activation isn't
    // complete.
    let mut incomplete = vec![];

    // Stop removed services and any masked services that might still be running
    // (e.g. distro-provided units). Must happen before daemon-reload so systemd
    // still knows about these units.
    let mut units_to_stop = convert_services(&services_to_stop);
    units_to_stop.extend(convert_services(&masked));
    let mut refused_stops = vec![];
    let mut unfinished_stops = vec![];
    let stopped = wait_for_jobs(
        &service_manager,
        &job_monitor,
        stop_services(
            &service_manager,
            units_to_stop,
            &mut incomplete,
            &mut refused_stops,
        ),
        &timeout,
        &mut unfinished_stops,
    );
    incomplete.extend(
        unfinished_stops
            .iter()
            .map(|job| format!("{} not stopped (job {})", job.unit, job.result)),
    );
    // A unit the profile drops keeps its entry in the state until its stop completes, so
    // that activating the profile again retries the stop and deactivating still stops it.
    // When the wait is cut short, which stops completed isn't known: they all keep theirs.
    stopped.map_err(|e| {
        ActivationError::with_partial_result(
            keep_old_entries(&services, &old_services, services_to_stop.keys()),
            cut_short(&incomplete, e),
        )
    })?;
    let applied = keep_old_entries(
        &services,
        &old_services,
        refused_stops
            .iter()
            .chain(unfinished_stops.iter().map(|job| &job.unit)),
    );

    wait_for_jobs(
        &service_manager,
        &job_monitor,
        reload_or_restart_units(
            &service_manager,
            convert_services(&services_to_reload),
            &mut incomplete,
        ) + start_units(&service_manager, ["system-manager.target"], &mut incomplete),
        &timeout,
        failed_jobs,
    )
    .map_err(|e| {
        ActivationError::with_partial_result(applied.clone(), cut_short(&incomplete, e))
    })?;

    if !masked.is_empty() {
        let unit_names: Vec<&str> = masked.keys().map(AsRef::as_ref).collect();
        service_manager
            .mask_unit_files(&unit_names, ephemeral)
            .with_context(|| format!("masking {} unit(s)", masked.len()))
            .map_err(|e| ActivationError::with_partial_result(applied.clone(), e))?;

        log::info!("Reloading systemd daemon after masking...");
        service_manager
            .daemon_reload()
            .map_err(|e| ActivationError::with_partial_result(applied.clone(), e))?;

        log::info!(
            "Masked {} unit(s): {}",
            masked.len(),
            masked.keys().cloned().collect::<Vec<_>>().join(", ")
        );
    }

    if !incomplete.is_empty() {
        return Err(ActivationError::with_partial_result(
            applied,
            anyhow::anyhow!(incomplete.join("; ")),
        ));
    }

    log::info!("Done");
    Ok(services)
}

/// The error for a services phase cut short (a job timeout, a D-Bus error), naming what
/// wasn't applied before it was, so that the diagnostic doesn't lose it.
fn cut_short(incomplete: &[String], e: anyhow::Error) -> anyhow::Error {
    if incomplete.is_empty() {
        e
    } else {
        anyhow::anyhow!("{}; {e:#}", incomplete.join("; "))
    }
}

/// The services to record as applied: `services`, plus the old entry of each of `units` the
/// profile drops. A unit the profile keeps or masks has its new entry already.
fn keep_old_entries<'a, I>(services: &Services, old_services: &Services, units: I) -> Services
where
    I: IntoIterator<Item = &'a String>,
{
    units
        .into_iter()
        .fold(services.clone(), |mut applied, unit| {
            if !applied.contains_key(unit) {
                if let Some(old) = old_services.get(unit) {
                    applied.insert(unit.clone(), old.clone());
                }
            }
            applied
        })
}

/// The managed units that failed: those systemd reports as failed once activation is done,
/// and those whose (re)start or reload job didn't complete (`failed_jobs`, from `activate`),
/// which can leave a unit active on its old configuration or inactive rather than failed. As
/// NixOS' switch-to-configuration does, a unit that failed doesn't fail the activation by
/// itself, but it is reported, so that the caller can judge it against what was failing
/// before.
pub fn failed_units(
    services: &Services,
    failed_jobs: &[UnfinishedJob],
) -> anyhow::Result<Vec<String>> {
    let names: Vec<&str> = services
        .iter()
        .filter(|(_, cfg)| !cfg.masked)
        .map(|(name, _)| name.as_str())
        .collect();
    let in_failed_state = if names.is_empty() {
        vec![]
    } else {
        systemd::ServiceManager::new_session()?
            .list_units_by_patterns(&["failed"], &names)?
            .into_iter()
            .map(|unit| unit.name)
            .collect()
    };
    Ok(name_failed(in_failed_state, failed_jobs))
}

/// Names each failed unit once: by its name when systemd reports it failed, else by its
/// job's result.
fn name_failed(mut failed: Vec<String>, failed_jobs: &[UnfinishedJob]) -> Vec<String> {
    let not_failed_state: Vec<String> = failed_jobs
        .iter()
        .filter(|job| !failed.contains(&job.unit))
        .map(ToString::to_string)
        .collect();
    failed.extend(not_failed_state);
    failed.sort();
    failed
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
            stop_services(&service_manager, units_to_stop, &mut vec![], &mut vec![]),
            &timeout,
            &mut vec![],
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

/// Queues a stop for each unit. A unit systemd refuses a stop for is also added to
/// `not_stopped`: it may still be running.
fn stop_services<'a, U>(
    service_manager: &systemd::ServiceManager,
    units: U,
    refused: &mut Vec<String>,
    not_stopped: &mut Vec<String>,
) -> HashSet<JobId>
where
    U: AsRef<[&'a str]>,
{
    for_each_unit(
        |unit| {
            service_manager.stop_unit(unit).inspect_err(|e| {
                if !is_not_loaded(e) {
                    not_stopped.push(unit.to_owned());
                }
            })
        },
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
fn for_each_unit<'a, 'm, F, N, S>(
    mut action: F,
    units: S,
    log_action: &str,
    nothing_to_do: N,
    refused: &mut Vec<String>,
) -> HashSet<JobId>
where
    F: FnMut(&str) -> anyhow::Result<systemd::Job<'m>>,
    N: Fn(&anyhow::Error) -> bool,
    S: AsRef<[&'a str]>,
{
    units
        .as_ref()
        .iter()
        .fold(HashSet::new(), |mut set, unit| match action(unit) {
            Ok(job) => {
                log::debug!("Unit {}: {} ({})...", unit, log_action, job.path());
                set.insert(JobId { id: job.path() });
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

/// Waits for `jobs`, adding each one that finished without completing to `unfinished`, also
/// when the wait is then cut short.
fn wait_for_jobs(
    service_manager: &systemd::ServiceManager,
    job_monitor: &systemd::JobMonitor,
    jobs: HashSet<JobId>,
    timeout: &Option<Duration>,
    unfinished: &mut Vec<UnfinishedJob>,
) -> anyhow::Result<()> {
    let mut results = vec![];
    let finished = service_manager.monitor_jobs_finish(job_monitor, timeout, jobs, &mut results);
    unfinished.extend(
        results
            .into_iter()
            .filter(|(_, result)| !job_completed(result))
            .map(|(unit, result)| {
                log::error!("Unit {unit}: job didn't complete: {result}");
                UnfinishedJob { unit, result }
            }),
    );
    if !finished? {
        anyhow::bail!("Timeout waiting for systemd jobs");
    }
    Ok(())
}

/// Whether a job did what it was asked. `skipped` is a job that didn't apply to the unit's
/// current state: there was nothing for it to do. Anything else but `done` (`failed`,
/// `dependency`, `timeout`, `canceled`, ...) means the unit isn't where the job was to put it.
fn job_completed(result: &str) -> bool {
    matches!(result, "done" | "skipped")
}

/// A job's object path.
#[derive(PartialEq, Eq, Hash, Clone)]
struct JobId {
    id: String,
}

impl From<JobId> for String {
    fn from(value: JobId) -> Self {
        value.id
    }
}

/// Forgets the failed state and the start rate limit of each unit the profile manages, as
/// NixOS' switch-to-configuration does for every unit. An activation restarts some units
/// every time (userborn.service twice: through sysinit-reactivation.target, then on its
/// own), so a few activations in a row would reach their start limit (by default, 5 starts
/// in 10 seconds) and systemd would refuse the next restart. The host's other units keep
/// their failed states: they aren't the profile's.
pub fn reset_failed_units(store_path: &StorePath) -> anyhow::Result<()> {
    let file = fs::File::open(
        Path::new(&store_path.store_path)
            .join("services")
            .join("services.json"),
    )?;
    let services: Services = serde_json::from_reader(io::BufReader::new(file))?;
    let service_manager = systemd::ServiceManager::new_session()?;
    for unit in services.keys() {
        // A unit systemd hasn't loaded (a new one, before the daemon reload) has none.
        if let Err(e) = service_manager.reset_failed_unit(unit) {
            log::debug!("Unit {unit}: failed state not reset: {e}");
        }
    }
    Ok(())
}

/// A job systemd refuses, or that doesn't complete, is added to `incomplete` rather than
/// returned, so that the activation carries on and records what it applied.
pub fn restart_sysinit_reactivation_target(incomplete: &mut Vec<String>) -> anyhow::Result<()> {
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
        incomplete,
    );

    let mut unfinished = vec![];
    let finished = wait_for_jobs(
        &service_manager,
        &job_monitor,
        jobs,
        &timeout,
        &mut unfinished,
    );
    incomplete.extend(unfinished.iter().map(ToString::to_string));
    finished
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
    let mut incomplete = vec![];
    let jobs = for_each_unit(
        |unit| service_manager.restart_unit(unit),
        ["userborn.service"],
        "restarting",
        |_| false,
        &mut incomplete,
    );

    // A oneshot's job fails when it does: users exist only if the job completed.
    let mut unfinished = vec![];
    wait_for_jobs(
        &service_manager,
        &job_monitor,
        jobs,
        &timeout,
        &mut unfinished,
    )?;
    incomplete.extend(unfinished.iter().map(ToString::to_string));
    if !incomplete.is_empty() {
        anyhow::bail!(incomplete.join("; "));
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

    #[test]
    fn only_done_and_skipped_jobs_completed() {
        assert!(job_completed("done"));
        assert!(job_completed("skipped"));
        for result in ["failed", "dependency", "timeout", "canceled", "assert"] {
            assert!(!job_completed(result), "{result}");
        }
    }

    #[test]
    fn a_phase_cut_short_still_names_what_was_not_applied() {
        let timeout = || anyhow::anyhow!("Timeout waiting for systemd jobs");
        assert_eq!(
            cut_short(&[], timeout()).to_string(),
            "Timeout waiting for systemd jobs"
        );
        assert_eq!(
            cut_short(
                &[
                    "pinned.service: error stopping: refused".to_owned(),
                    "busy.mount not stopped (job failed)".to_owned(),
                ],
                timeout().context("stopping")
            )
            .to_string(),
            "pinned.service: error stopping: refused; busy.mount not stopped (job failed); \
             stopping: Timeout waiting for systemd jobs"
        );
    }

    #[test]
    fn a_failed_unit_is_named_once() {
        let job = |unit: &str, result: &str| UnfinishedJob {
            unit: unit.to_owned(),
            result: result.to_owned(),
        };
        assert_eq!(
            name_failed(
                vec!["b.service".to_owned()],
                &[
                    // Failed state and failed job: named once, by the failed state.
                    job("b.service", "failed"),
                    // A reload that failed leaves the unit active.
                    job("c.service", "failed"),
                    job("a.service", "dependency"),
                ],
            ),
            [
                "a.service (job dependency)",
                "b.service",
                "c.service (job failed)"
            ]
        );
        assert!(name_failed(vec![], &[]).is_empty());
    }

    #[test]
    fn a_dropped_unit_keeps_its_entry_until_its_stop_completes() {
        let unit = |path: &str| ServiceConfig {
            store_path: Some(StorePath {
                store_path: PathBuf::from(path),
            }),
            masked: false,
        };
        let masked = ServiceConfig {
            store_path: None,
            masked: true,
        };
        let services = |entries: &[(&str, &ServiceConfig)]| -> Services {
            entries
                .iter()
                .map(|(name, cfg)| ((*name).to_owned(), (*cfg).clone()))
                .collect()
        };
        let busy = unit("/nix/store/aaa-unit-busy.mount/busy.mount");
        let gone = unit("/nix/store/bbb-unit-gone.service/gone.service");
        let old_kept = unit("/nix/store/ccc-unit-kept.service/kept.service");
        let new_kept = unit("/nix/store/ddd-unit-kept.service/kept.service");
        let pinned = unit("/nix/store/eee-unit-pinned.service/pinned.service");

        let old = services(&[
            ("busy.mount", &busy),
            ("gone.service", &gone),
            ("kept.service", &old_kept),
            ("pinned.service", &pinned),
        ]);
        let new = services(&[("kept.service", &new_kept), ("pinned.service", &masked)]);

        // busy.mount's stop failed and gone.service's completed; pinned.service, which the
        // profile masks, didn't stop either.
        let not_stopped = ["busy.mount".to_owned(), "pinned.service".to_owned()];
        let applied = keep_old_entries(&new, &old, &not_stopped);
        assert_eq!(
            applied,
            services(&[
                ("busy.mount", &busy),
                ("kept.service", &new_kept),
                ("pinned.service", &masked),
            ])
        );

        // Activating the profile again retries the stop.
        let active: Services = new.clone().into_iter().filter(|(_, c)| !c.masked).collect();
        let to_stop = get_services_to_stop(applied.clone().relative_complement(active));
        assert!(to_stop.contains_key("busy.mount"));
        assert!(!to_stop.contains_key("gone.service"));

        // A rollback to the profile that has it finds it unchanged: no restart.
        let to_reload = get_services_to_reload(old.clone(), applied.clone());
        assert!(!to_reload.contains_key("busy.mount"));
        assert!(to_reload.contains_key("kept.service"));

        // Nothing left to stop: the new services, as they are.
        assert_eq!(keep_old_entries(&new, &old, &[]), new);
    }
}
