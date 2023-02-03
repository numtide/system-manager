// FIXME: Remove this
#![allow(dead_code)]

mod manager;
mod unit;

use anyhow::Error;

use crate::{
    systemd::manager::{OrgFreedesktopSystemd1Manager, OrgFreedesktopSystemd1ManagerJobRemoved},
    systemd::unit::OrgFreedesktopSystemd1Unit,
};
use dbus::{
    blocking::{Connection, Proxy},
    Message, Path,
};
use std::{
    collections::HashSet,
    rc::Rc,
    result::Result,
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    time::{Duration, Instant},
};

const SD_DESTINATION: &str = "org.freedesktop.systemd1";
const SD_PATH: &str = "/org/freedesktop/systemd1";

pub struct ServiceManager {
    proxy: Proxy<'static, Rc<Connection>>,
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

pub struct JobMonitor {
    ready: Arc<AtomicBool>,
}

impl Drop for ServiceManager {
    fn drop(&mut self) {
        OrgFreedesktopSystemd1Manager::unsubscribe(&self.proxy).unwrap_or(());
    }
}

impl ServiceManager {
    pub fn new_session() -> Result<ServiceManager, Error> {
        let conn = Connection::new_session()?;
        let proxy = Proxy::new(
            SD_DESTINATION,
            SD_PATH,
            Duration::from_secs(2),
            Rc::new(conn),
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
            self.proxy.connection.process(Duration::from_secs(2))?;
        }

        Ok(())
    }

    pub fn reset_failed(&self) -> Result<(), Error> {
        OrgFreedesktopSystemd1Manager::reset_failed(&self.proxy)?;
        Ok(())
    }

    /// Builds a unit manager for the unit with the given status.
    pub fn unit_manager(&self, unit_status: &UnitStatus) -> UnitManager {
        UnitManager {
            proxy: self.proxy.connection.with_proxy(
                SD_DESTINATION,
                unit_status.object_path.clone(),
                Duration::from_secs(2),
            ),
        }
    }

    pub fn monitor_jobs_init<F, I>(&self, names: I, handler: F) -> Result<JobMonitor, Error>
    where
        F: Fn(&str, &str) + Send + 'static,
        I: IntoIterator,
        I::Item: AsRef<String>,
    {
        let mut names_remaining = names
            .into_iter()
            .map(|n| String::from(n.as_ref()))
            .collect::<HashSet<_>>();
        let ready = Arc::new(AtomicBool::from(false));
        let ready_jobs_removed = Arc::clone(&ready);

        self.proxy.match_signal(
            move |h: OrgFreedesktopSystemd1ManagerJobRemoved, _: &Connection, _: &Message| {
                names_remaining.remove(&h.unit);
                handler(&h.unit, &h.result);

                let res = names_remaining.is_empty();
                ready_jobs_removed.store(res, Ordering::Relaxed);
                !res
            },
        )?;

        Ok(JobMonitor { ready })
    }

    /// Waits for the monitored jobs to finish. Returns `true` if all jobs
    /// finished before the timeout, `false` otherwise.
    pub fn monitor_jobs_finish(
        &self,
        job_monitor: &JobMonitor,
        timeout: &Option<Duration>,
    ) -> Result<bool, Error> {
        let start_time = Instant::now();

        while !job_monitor.ready.load(Ordering::Relaxed) {
            self.proxy.connection.process(Duration::from_millis(50))?;

            if timeout
                .map(|t| start_time.elapsed() > t)
                .unwrap_or_default()
            {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub fn reload_unit(&self, unit_name: &str) -> Result<Job, Error> {
        Ok(Job {
            path: OrgFreedesktopSystemd1Manager::reload_unit(&self.proxy, unit_name, "replace")?,
        })
    }

    pub fn restart_unit(&self, unit_name: &str) -> Result<Job, Error> {
        Ok(Job {
            path: OrgFreedesktopSystemd1Manager::restart_unit(&self.proxy, unit_name, "replace")?,
        })
    }

    pub fn start_unit(&self, unit_name: &str) -> Result<Job, Error> {
        Ok(Job {
            path: OrgFreedesktopSystemd1Manager::start_unit(&self.proxy, unit_name, "replace")?,
        })
    }

    pub fn stop_unit(&self, unit_name: &str) -> Result<Job, Error> {
        Ok(Job {
            path: OrgFreedesktopSystemd1Manager::stop_unit(&self.proxy, unit_name, "replace")?,
        })
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
