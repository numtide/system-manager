// TODO: Remove this
#![allow(dead_code)]

mod manager;
mod unit;

use crate::{
    systemd::manager::{OrgFreedesktopSystemd1Manager, OrgFreedesktopSystemd1ManagerJobRemoved},
    systemd::unit::OrgFreedesktopSystemd1Unit,
};
use anyhow::Error;
use dbus::{
    blocking::{Connection, Proxy},
    channel::Token,
    Message, Path,
};
use std::{
    hash::Hash,
    result::Result,
    sync::Arc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

const SD_DESTINATION: &str = "org.freedesktop.systemd1";
const SD_PATH: &str = "/org/freedesktop/systemd1";

pub struct ServiceManager {
    proxy: Proxy<'static, Box<Connection>>,
}

pub struct UnitManager<'a> {
    proxy: Proxy<'static, &'a Connection>,
}

#[derive(Debug)]
pub struct UnitFile {
    pub unit_name: String,
    pub status: String,
}

#[derive(Debug)]
pub struct UnitStatus {
    pub name: String,
    pub description: String,
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub followed: String,
    pub object_path: Path<'static>,
    pub queued_job: u32,
    pub queued_job_type: String,
    pub queued_job_path: Path<'static>,
}

/// A tuple representation of `UnitStatus` for use in the dbus API.
type UnitStatusTuple = (
    String,
    String,
    String,
    String,
    String,
    String,
    Path<'static>,
    u32,
    String,
    Path<'static>,
);

#[derive(Debug)]
pub struct Job<'a> {
    path: Path<'a>,
}

impl Job<'_> {
    /// The job's object path, by which systemd's JobRemoved signal names it.
    pub fn path(&self) -> String {
        self.path.to_string()
    }
}

pub struct JobMonitor<'a> {
    /// The jobs systemd removed since monitor_jobs_finish last looked.
    removed_jobs: Arc<Mutex<Vec<OrgFreedesktopSystemd1ManagerJobRemoved>>>,
    tokens: im::HashSet<Token>,
    service_manager: &'a ServiceManager,
}

impl Drop for JobMonitor<'_> {
    fn drop(&mut self) {
        self.tokens.iter().for_each(|t| {
            self.service_manager
                .proxy
                .match_stop(*t, true)
                .unwrap_or_else(|e|
                    log::error!("Error while stopping match listener, memory might leak...\n  Caused by: {e}")
                )
        });
    }
}

impl Drop for ServiceManager {
    fn drop(&mut self) {
        OrgFreedesktopSystemd1Manager::unsubscribe(&self.proxy).unwrap_or(());
    }
}

impl ServiceManager {
    pub fn new_session() -> Result<ServiceManager, Error> {
        let conn = Connection::new_system()?;
        let proxy = Proxy::new(
            SD_DESTINATION,
            SD_PATH,
            Duration::from_secs(10),
            Box::new(conn),
        );

        OrgFreedesktopSystemd1Manager::subscribe(&proxy)?;

        Ok(ServiceManager { proxy })
    }

    pub fn unique_name(&self) -> String {
        self.proxy.connection.unique_name().to_string()
    }

    /// Performs a systemd daemon reload, blocking until complete.
    pub fn daemon_reload(&self) -> Result<(), Error> {
        let ready = Arc::new(AtomicBool::from(false));
        let ready_closure = ready.clone();

        self.proxy.match_signal(
            move |res: manager::OrgFreedesktopSystemd1ManagerReloading,
                  _: &Connection,
                  _: &Message| {
                if !res.active {
                    ready_closure.store(true, Ordering::Relaxed);
                }
                res.active
            },
        )?;

        OrgFreedesktopSystemd1Manager::reload(&self.proxy)?;

        while !ready.load(Ordering::Relaxed) {
            self.proxy.connection.process(Duration::from_secs(10))?;
        }

        Ok(())
    }

    pub fn reset_failed(&self) -> Result<(), Error> {
        OrgFreedesktopSystemd1Manager::reset_failed(&self.proxy)?;
        Ok(())
    }

    /// Resets a unit's failed state, and with it its start rate limit.
    pub fn reset_failed_unit(&self, unit_name: &str) -> Result<(), Error> {
        OrgFreedesktopSystemd1Manager::reset_failed_unit(&self.proxy, unit_name)?;
        Ok(())
    }

    /// Builds a unit manager for the unit with the given status.
    pub fn unit_manager(&'_ self, unit_status: &UnitStatus) -> UnitManager<'_> {
        UnitManager {
            proxy: self.proxy.connection.with_proxy(
                SD_DESTINATION,
                unit_status.object_path.clone(),
                Duration::from_secs(2),
            ),
        }
    }

    pub fn monitor_jobs_init(&'_ self) -> Result<JobMonitor<'_>, Error> {
        let removed_jobs = Arc::new(Mutex::from(
            Vec::<OrgFreedesktopSystemd1ManagerJobRemoved>::new(),
        ));

        let removed_jobs_clone = Arc::clone(&removed_jobs);
        let token = self.proxy.match_signal(
            move |h: OrgFreedesktopSystemd1ManagerJobRemoved, _: &Connection, _: &Message| {
                log::debug!("Job for {} done: {}", h.unit, h.result);
                {
                    // Record the job, and let the lock go out of scope immediately
                    removed_jobs_clone.lock().unwrap().push(h);
                }
                // The callback gets removed at the end of monitor_jobs_finish
                true
            },
        )?;

        Ok(JobMonitor {
            removed_jobs: Arc::clone(&removed_jobs),
            tokens: im::HashSet::unit(token),
            service_manager: self,
        })
    }

    /// Waits for the monitored jobs, given by their object paths, to finish, adding
    /// the unit and result of each one that does to `results` (systemd's JobRemoved
    /// result: `done`, `failed`, `dependency`, ...). Matching by path rather than by
    /// unit leaves out another job on the same unit, such as one ours replaced.
    /// Returns `true` if all jobs finished before the timeout, `false` otherwise.
    pub fn monitor_jobs_finish<I>(
        &self,
        job_monitor: &JobMonitor,
        timeout: &Option<Duration>,
        jobs: I,
        results: &mut Vec<(String, String)>,
    ) -> Result<bool, Error>
    where
        I: IntoIterator,
        I::Item: Into<String> + Eq + Hash,
    {
        let start_time = Instant::now();

        let mut waiting_for: im::HashSet<String> = jobs.into_iter().map(Into::into).collect();
        let total_jobs = waiting_for.len();

        if total_jobs > 0 {
            log::info!("Waiting for jobs to finish...");
            log::debug!("Waiting for jobs to finish... (0/{})", total_jobs);
        }

        while !waiting_for.is_empty() {
            self.proxy.connection.process(Duration::from_millis(50))?;

            if timeout
                .map(|t| start_time.elapsed() > t)
                .unwrap_or_default()
            {
                return Ok(false);
            }
            let removed_jobs = std::mem::take(&mut *job_monitor.removed_jobs.lock().unwrap());
            if !removed_jobs.is_empty() {
                for removed in removed_jobs {
                    if waiting_for.remove(&*removed.job).is_some() {
                        results.push((removed.unit, removed.result));
                    }
                }
                if !waiting_for.is_empty() {
                    log::debug!(
                        "Waiting for jobs to finish... ({}/{})",
                        total_jobs - waiting_for.len(),
                        total_jobs
                    );
                    log::debug!("Waiting for: {waiting_for:?}")
                }
            }
        }

        if total_jobs > 0 {
            log::info!("All jobs finished.");
        }
        Ok(true)
    }

    pub fn reload_or_restart_unit(&'_ self, unit_name: &str) -> Result<Job<'_>, Error> {
        Ok(Job {
            path: OrgFreedesktopSystemd1Manager::reload_or_restart_unit(
                &self.proxy,
                unit_name,
                "replace",
            )?,
        })
    }

    pub fn restart_unit(&'_ self, unit_name: &str) -> Result<Job<'_>, Error> {
        Ok(Job {
            path: OrgFreedesktopSystemd1Manager::restart_unit(&self.proxy, unit_name, "replace")?,
        })
    }

    pub fn start_unit(&'_ self, unit_name: &str) -> Result<Job<'_>, Error> {
        Ok(Job {
            path: OrgFreedesktopSystemd1Manager::start_unit(&self.proxy, unit_name, "replace")?,
        })
    }

    pub fn stop_unit(&'_ self, unit_name: &str) -> Result<Job<'_>, Error> {
        Ok(Job {
            path: OrgFreedesktopSystemd1Manager::stop_unit(&self.proxy, unit_name, "replace")?,
        })
    }

    pub fn mask_unit_files(&self, units: &[&str], runtime: bool) -> Result<(), Error> {
        let changes = OrgFreedesktopSystemd1Manager::mask_unit_files(
            &self.proxy,
            units.to_vec(),
            runtime,
            true, // force: replace existing symlinks
        )?;
        for (change_type, from, to) in &changes {
            log::debug!("Mask change: {change_type} {from} -> {to}");
        }
        Ok(())
    }

    pub fn unmask_unit_files(&self, units: &[&str], runtime: bool) -> Result<(), Error> {
        let changes =
            OrgFreedesktopSystemd1Manager::unmask_unit_files(&self.proxy, units.to_vec(), runtime)?;
        for (change_type, from, to) in &changes {
            log::debug!("Unmask change: {change_type} {from} -> {to}");
        }
        Ok(())
    }

    pub fn list_units_by_patterns(
        &self,
        states: &[&str],
        patterns: &[&str],
    ) -> Result<Vec<UnitStatus>, Error> {
        let units = OrgFreedesktopSystemd1Manager::list_units_by_patterns(
            &self.proxy,
            states.to_vec(),
            patterns.to_vec(),
        )?;

        Ok(units.iter().map(|t| self.to_unit_status(t)).collect())
    }

    fn to_unit_status(&self, t: &UnitStatusTuple) -> UnitStatus {
        UnitStatus {
            name: String::from(&t.0),
            description: String::from(&t.1),
            load_state: String::from(&t.2),
            active_state: String::from(&t.3),
            sub_state: String::from(&t.4),
            followed: String::from(&t.5),
            object_path: t.6.clone(),
            queued_job: t.7,
            queued_job_type: String::from(&t.8),
            queued_job_path: t.9.clone(),
        }
    }
}

impl UnitManager<'_> {
    pub fn refuse_manual_start(&self) -> Result<bool, Error> {
        Ok(OrgFreedesktopSystemd1Unit::refuse_manual_start(
            &self.proxy,
        )?)
    }

    pub fn refuse_manual_stop(&self) -> Result<bool, Error> {
        Ok(OrgFreedesktopSystemd1Unit::refuse_manual_stop(&self.proxy)?)
    }
}
