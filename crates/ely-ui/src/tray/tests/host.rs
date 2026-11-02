use std::io::BufRead as _;
use std::sync::{Mutex, MutexGuard};

/// One private bus at a time: the address is process-wide.
static ONE: Mutex<()> = Mutex::new(());

/// The bus process; it stops with the test.
pub struct Bus {
    child: std::process::Child,
    before: Option<std::ffi::OsString>,
    // Last, so it unlocks after the rest.
    _one: MutexGuard<'static, ()>,
}

impl Drop for Bus {
    fn drop(&mut self) {
        self.child.kill().ok();
        // Under the lock: no other test reads it.
        match self.before.take() {
            Some(before) => unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", before) },
            None => unsafe { std::env::remove_var("DBUS_SESSION_BUS_ADDRESS") },
        }
    }
}

/// A private session bus, or None without dbus-daemon.
pub fn bus() -> Option<Bus> {
    // A failed test only poisons; the bus is still one.
    let one = ONE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut child = std::process::Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    let mut address = String::new();
    std::io::BufReader::new(child.stdout.take().expect("its address"))
        .read_line(&mut address)
        .expect("reads the address");
    let before = std::env::var_os("DBUS_SESSION_BUS_ADDRESS");
    unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", address.trim()) };
    Some(Bus { child, before, _one: one })
}

struct Watcher;

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, _service: &str) {}
}

/// A bus with a stand-in watcher, or None without dbus-daemon.
pub fn serve() -> Option<(Bus, zbus::Connection)> {
    let bus = bus()?;
    let watcher = futures::executor::block_on(async {
        zbus::connection::Builder::session()?
            .name("org.kde.StatusNotifierWatcher")?
            .serve_at("/StatusNotifierWatcher", Watcher)?
            .build()
            .await
    })
    .expect("the stand-in watcher");
    Some((bus, watcher))
}
