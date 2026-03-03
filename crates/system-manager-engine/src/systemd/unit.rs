use zbus::proxy;

#[proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1"
)]
pub trait Unit {
    #[zbus(property)]
    fn refuse_manual_start(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn refuse_manual_stop(&self) -> zbus::Result<bool>;
}
