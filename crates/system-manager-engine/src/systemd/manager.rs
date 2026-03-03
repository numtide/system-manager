use zbus::proxy;
use zbus::zvariant::OwnedObjectPath;

type UnitStatusTuple = (
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
);

type UnitFileChange = (String, String, String);

#[proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1"
)]
pub trait Manager {
    fn subscribe(&self) -> zbus::Result<()>;
    fn unsubscribe(&self) -> zbus::Result<()>;
    fn reload(&self) -> zbus::Result<()>;
    fn reset_failed(&self) -> zbus::Result<()>;

    fn start_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    fn stop_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    fn restart_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    fn reload_or_restart_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;

    fn mask_unit_files(
        &self,
        files: Vec<&str>,
        runtime: bool,
        force: bool,
    ) -> zbus::Result<Vec<UnitFileChange>>;
    fn unmask_unit_files(
        &self,
        files: Vec<&str>,
        runtime: bool,
    ) -> zbus::Result<Vec<UnitFileChange>>;

    fn list_units_by_patterns(
        &self,
        states: Vec<&str>,
        patterns: Vec<&str>,
    ) -> zbus::Result<Vec<UnitStatusTuple>>;

    #[zbus(signal)]
    fn job_removed(&self, id: u32, job: OwnedObjectPath, unit: String, result: String)
        -> zbus::Result<()>;

    #[zbus(signal)]
    fn reloading(&self, active: bool) -> zbus::Result<()>;
}
