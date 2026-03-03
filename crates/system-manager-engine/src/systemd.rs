mod manager;
mod unit;

use anyhow::Error;
use std::{
    hash::Hash,
    pin::Pin,
    result::Result,
    thread,
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};
use zbus::export::futures_core::Stream;
use zbus::zvariant::OwnedObjectPath;

use manager::ManagerProxyBlocking;
use unit::UnitProxyBlocking;

pub struct ServiceManager {
    proxy: ManagerProxyBlocking<'static>,
}

pub struct UnitManager<'a> {
    proxy: UnitProxyBlocking<'a>,
}

#[derive(Debug)]
pub struct UnitStatus {
    pub name: String,
    pub description: String,
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub followed: String,
    pub object_path: OwnedObjectPath,
    pub queued_job: u32,
    pub queued_job_type: String,
    pub queued_job_path: OwnedObjectPath,
}

#[derive(Debug)]
pub struct Job {
    #[allow(dead_code)]
    path: OwnedObjectPath,
}

pub struct JobMonitor {
    signals: zbus::MessageStream,
}

impl Drop for ServiceManager {
    fn drop(&mut self) {
        self.proxy.unsubscribe().unwrap_or(());
    }
}

impl ServiceManager {
    pub fn new_session() -> Result<ServiceManager, Error> {
        let conn = zbus::blocking::Connection::system()?;
        let proxy = ManagerProxyBlocking::new(&conn)?;
        proxy.subscribe()?;
        Ok(ServiceManager { proxy })
    }

    /// Performs a systemd daemon reload, blocking until complete.
    pub fn daemon_reload(&self) -> Result<(), Error> {
        let signals = self.proxy.receive_reloading()?;
        self.proxy.reload()?;
        // Wait for the Reloading(active=false) signal indicating reload is done
        for signal in signals {
            let args = signal.args()?;
            if !args.active {
                return Ok(());
            }
        }
        anyhow::bail!("Reloading signal stream ended unexpectedly")
    }

    pub fn reset_failed(&self) -> Result<(), Error> {
        self.proxy.reset_failed()?;
        Ok(())
    }

    /// Builds a unit manager for the unit with the given status.
    pub fn unit_manager(&self, unit_status: &UnitStatus) -> Result<UnitManager<'_>, Error> {
        let proxy = UnitProxyBlocking::builder(self.proxy.inner().connection())
            .path(unit_status.object_path.clone())?
            .build()?;
        Ok(UnitManager { proxy })
    }

    /// Starts listening for JobRemoved signals. Must be called before issuing
    /// unit operations so that signals emitted during those operations are
    /// buffered and not lost.
    pub fn monitor_jobs_init(&self) -> Result<JobMonitor, Error> {
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender("org.freedesktop.systemd1")?
            .path("/org/freedesktop/systemd1")?
            .interface("org.freedesktop.systemd1.Manager")?
            .member("JobRemoved")?
            .build();
        let signals = zbus::block_on(zbus::MessageStream::for_match_rule(
            rule,
            self.proxy.inner().connection().inner(),
            Some(32),
        ))?;
        Ok(JobMonitor { signals })
    }

    /// Waits for the monitored jobs to finish. Returns `true` if all jobs
    /// finished before the timeout, `false` otherwise.
    pub fn monitor_jobs_finish<I>(
        &self,
        job_monitor: &mut JobMonitor,
        timeout: &Option<Duration>,
        services: I,
    ) -> Result<bool, Error>
    where
        I: IntoIterator,
        I::Item: Into<String> + Eq + Hash,
    {
        let mut waiting_for: im::HashSet<String> = services.into_iter().map(Into::into).collect();
        let total_jobs = waiting_for.len();

        if total_jobs == 0 {
            return Ok(true);
        }

        log::info!("Waiting for jobs to finish...");
        log::debug!("Waiting for jobs to finish... (0/{})", total_jobs);

        let start_time = Instant::now();
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        while !waiting_for.is_empty() {
            loop {
                match Pin::new(&mut job_monitor.signals).poll_next(&mut cx) {
                    Poll::Ready(Some(signal)) => {
                        let signal = signal?;
                        let (_id, _job, unit, _result): (
                            u32,
                            OwnedObjectPath,
                            String,
                            String,
                        ) = signal.body().deserialize()?;
                        log::debug!("Job for {} done", unit);
                        waiting_for.remove(&unit);

                        if waiting_for.is_empty() {
                            log::info!("All jobs finished.");
                            return Ok(true);
                        }

                        log::debug!(
                            "Waiting for jobs to finish... ({}/{})",
                            total_jobs - waiting_for.len(),
                            total_jobs
                        );
                        log::debug!("Waiting for: {waiting_for:?}");
                    }
                    Poll::Ready(None) => {
                        anyhow::bail!("JobRemoved signal stream ended unexpectedly");
                    }
                    Poll::Pending => break,
                }
            }

            if timeout
                .map(|t| start_time.elapsed() > t)
                .unwrap_or_default()
            {
                return Ok(false);
            }

            let wait_duration = timeout
                .map(|t| t.saturating_sub(start_time.elapsed()))
                .unwrap_or(Duration::from_millis(50))
                .min(Duration::from_millis(50));

            if wait_duration.is_zero() {
                return Ok(false);
            }

            thread::sleep(wait_duration);
        }

        Ok(true)
    }

    pub fn reload_or_restart_unit(&self, unit_name: &str) -> Result<Job, Error> {
        Ok(Job {
            path: self
                .proxy
                .reload_or_restart_unit(unit_name, "replace")?,
        })
    }

    pub fn restart_unit(&self, unit_name: &str) -> Result<Job, Error> {
        Ok(Job {
            path: self.proxy.restart_unit(unit_name, "replace")?,
        })
    }

    pub fn start_unit(&self, unit_name: &str) -> Result<Job, Error> {
        Ok(Job {
            path: self.proxy.start_unit(unit_name, "replace")?,
        })
    }

    pub fn stop_unit(&self, unit_name: &str) -> Result<Job, Error> {
        Ok(Job {
            path: self.proxy.stop_unit(unit_name, "replace")?,
        })
    }

    pub fn mask_unit_files(&self, units: &[&str], runtime: bool) -> Result<(), Error> {
        let changes = self
            .proxy
            .mask_unit_files(units.to_vec(), runtime, true)?;
        for (change_type, from, to) in &changes {
            log::debug!("Mask change: {change_type} {from} -> {to}");
        }
        Ok(())
    }

    pub fn unmask_unit_files(&self, units: &[&str], runtime: bool) -> Result<(), Error> {
        let changes = self.proxy.unmask_unit_files(units.to_vec(), runtime)?;
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
        let units = self
            .proxy
            .list_units_by_patterns(states.to_vec(), patterns.to_vec())?;

        Ok(units.iter().map(Self::to_unit_status).collect())
    }

    fn to_unit_status(t: &(
        String,
        String,
        String,
        String,
        String,
        String,
        OwnedObjectPath,
        u32,
        String,
        OwnedObjectPath,
    )) -> UnitStatus {
        UnitStatus {
            name: t.0.clone(),
            description: t.1.clone(),
            load_state: t.2.clone(),
            active_state: t.3.clone(),
            sub_state: t.4.clone(),
            followed: t.5.clone(),
            object_path: t.6.clone(),
            queued_job: t.7,
            queued_job_type: t.8.clone(),
            queued_job_path: t.9.clone(),
        }
    }
}

impl UnitManager<'_> {
    pub fn refuse_manual_start(&self) -> Result<bool, Error> {
        Ok(self.proxy.refuse_manual_start()?)
    }

    pub fn refuse_manual_stop(&self) -> Result<bool, Error> {
        Ok(self.proxy.refuse_manual_stop()?)
    }
}
