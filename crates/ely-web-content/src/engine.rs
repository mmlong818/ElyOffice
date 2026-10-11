//! CEF itself: the framework, its processes, its message pump.

use std::{
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};

use anyhow::{Context as _, Result};
use cef::{args::Args, *};
use futures::{FutureExt as _, StreamExt as _, channel::mpsc};
use gpui::{App, AsyncApp, Global, WeakEntity};

use crate::{handlers, surface::Shared, view::WebView};

/// CEF beside the app, else the fetched one.
fn framework_dir() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        const NAME: &str = "Chromium Embedded Framework.framework";
        let exe = std::env::current_exe()?;
        let mut places: Vec<PathBuf> = Vec::new();
        if let Some(dir) = std::env::var_os("ELY_CEF") {
            places.push(PathBuf::from(dir).join(NAME));
        }
        if let Some(contents) = exe.parent().and_then(Path::parent) {
            places.push(contents.join("Frameworks").join(NAME));
            // A helper sits in Frameworks/<App> Helper.app/Contents/MacOS.
            if let Some(frameworks) = contents.parent().and_then(Path::parent) {
                places.push(frameworks.join(NAME));
            }
        }
        if let Some(built) = option_env!("CEF_PATH") {
            places.push(Path::new(built).join(NAME));
        }
        places
            .into_iter()
            .find(|place| place.is_dir())
            .with_context(|| format!("no {NAME} beside {} or in engines/cef", exe.display()))
    }

    #[cfg(target_os = "windows")]
    {
        const NAME: &str = "libcef.dll";
        let exe = std::env::current_exe()?;
        let mut places = Vec::new();
        if let Some(dir) = std::env::var_os("ELY_CEF") {
            places.push(PathBuf::from(dir));
        }
        if let Some(dir) = exe.parent() {
            places.push(dir.to_path_buf());
        }
        if let Some(built) = option_env!("CEF_PATH") {
            places.push(Path::new(built).to_path_buf());
        }
        places
            .into_iter()
            .find(|place| place.join(NAME).is_file())
            .with_context(|| format!("no {NAME} beside {} or in engines/cef", exe.display()))
    }
}

static LOADED: OnceLock<std::result::Result<PathBuf, String>> = OnceLock::new();

/// Loads the framework once; every process needs it first.
fn load() -> Result<PathBuf> {
    LOADED
        .get_or_init(|| {
            let dir = framework_dir().map_err(|error| format!("{error:#}"))?;
            #[cfg(target_os = "macos")]
            let library = dir.join("Chromium Embedded Framework");
            #[cfg(target_os = "macos")]
            let name = std::ffi::CString::new(library.as_os_str().as_encoded_bytes())
                .map_err(|error| error.to_string())?;
            #[cfg(target_os = "macos")]
            if unsafe { load_library(Some(&*name.as_ptr().cast())) } != 1 {
                return Err(format!("CEF did not load from {}", library.display()));
            }
            // Windows imports libcef.dll at process startup. The linker and the
            // Windows DLL search path require it beside the executable.
            let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
            log::info!("web: CEF loaded from {}", dir.display());
            Ok(dir)
        })
        .clone()
        .map_err(anyhow::Error::msg)
}

/// In a CEF helper process, runs it to its end.
pub fn subprocess() -> Option<i32> {
    // CEF's own parser needs the framework; plain args do not.
    if !std::env::args().any(|arg| arg.starts_with("--type=")) {
        return None;
    }
    if let Err(error) = load() {
        eprintln!("web: {error:#}");
        return Some(1);
    }
    let args = Args::new();
    let mut app = handlers::app();
    let code = execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    Some(code)
}

/// The longest the pump waits between turns, as in cefclient.
const FALLBACK: Duration = Duration::from_millis(1000 / 30);

/// Pump work CEF asked for, from any thread.
static WAKE: Mutex<Option<mpsc::UnboundedSender<i64>>> = Mutex::new(None);

/// CEF wants its loop run within `delay_ms`.
pub(crate) fn schedule(delay_ms: i64) {
    if let Some(wake) = WAKE.lock().expect("the pump lock").as_ref() {
        let _ = wake.unbounded_send(delay_ms);
    }
}

/// The live views the pump hands frames to.
#[derive(Default)]
pub(crate) struct Engine {
    pub views: Vec<WeakEntity<WebView>>,
}

impl Global for Engine {}

thread_local! {
    /// CEF calls asked inside GPUI's updates, run after them.
    static LATER: std::cell::RefCell<Vec<Box<dyn FnOnce()>>> = Default::default();
}

/// Runs `work` on the pump's next turn.
pub(crate) fn later(work: impl FnOnce() + 'static) {
    LATER.with_borrow_mut(|queue| queue.push(Box::new(work)));
    schedule(0);
}

/// In asked order; work may ask for more.
fn run_later() {
    loop {
        let batch = LATER.with_borrow_mut(std::mem::take);
        if batch.is_empty() {
            return;
        }
        for work in batch {
            work();
        }
    }
}

/// Browsers asked for, made on the pump's next turn.
static PENDING: Mutex<Vec<(Shared, String)>> = Mutex::new(Vec::new());

/// Why CEF could not start; every view is told.
static FAILED: Mutex<Option<String>> = Mutex::new(None);

pub(crate) fn failed() -> Option<String> {
    FAILED.lock().expect("the start lock").clone()
}

/// Queues a browser for `surface` at `url`.
pub(crate) fn ask_browser(surface: Shared, url: String) {
    PENDING
        .lock()
        .expect("the pending lock")
        .push((surface, url));
    schedule(0);
}

/// Outside GPUI's updates: CEF may spin the run loop.
fn make_browsers() {
    let pending = std::mem::take(&mut *PENDING.lock().expect("the pending lock"));
    for (surface, url) in pending {
        if surface.lock().expect("the surface lock").closed {
            continue;
        }
        // Off-screen pages work only in CEF's Alloy style.
        let info = WindowInfo {
            windowless_rendering_enabled: true as _,
            runtime_style: RuntimeStyle::ALLOY,
            ..Default::default()
        };
        let settings = BrowserSettings {
            windowless_frame_rate: 60,
            ..Default::default()
        };
        let made = browser_host_create_browser(
            Some(&info),
            Some(&mut handlers::client(&surface)),
            Some(&url.as_str().into()),
            Some(&settings),
            None,
            None,
        );
        if made != 1 {
            surface.lock().expect("the surface lock").failed =
                Some(format!("CEF made no browser for {url}"));
        }
    }
}

/// Starts CEF in this process; once per app.
pub fn start(data: &Path, cx: &mut App) -> Result<()> {
    if cx.has_global::<Engine>() {
        return Ok(());
    }
    crate::input::bind(cx);
    let framework = load()?;
    #[cfg(target_os = "macos")]
    crate::mac::adopt_app_protocol()?;
    let exe = std::env::current_exe()?;
    // Bundled, CEF finds its helper apps; else this binary helps.
    let bundled = framework.parent().is_some_and(|dir| {
        let name = exe.file_name().unwrap_or_default().to_string_lossy();
        dir.join(format!("{name} Helper.app")).is_dir()
    });
    let helper = if bundled {
        String::new()
    } else {
        exe.to_string_lossy().into_owned()
    };
    let cache = data.join("web");
    std::fs::create_dir_all(&cache)?;
    let settings = Settings {
        windowless_rendering_enabled: true as _,
        external_message_pump: true as _,
        no_sandbox: true as _,
        browser_subprocess_path: helper.as_str().into(),
        #[cfg(target_os = "macos")]
        framework_dir_path: framework.to_string_lossy().as_ref().into(),
        root_cache_path: cache.to_string_lossy().as_ref().into(),
        persist_session_cookies: false as _,
        log_severity: LogSeverity::WARNING,
        ..Default::default()
    };
    let (wake, mut asked) = mpsc::unbounded();
    *WAKE.lock().expect("the pump lock") = Some(wake);
    cx.set_global(Engine::default());
    cx.spawn(async move |cx: &mut AsyncApp| {
        // Not inside an update: CEF may run AppKit's loop here.
        let args = Args::new();
        let mut app = handlers::app();
        let started = initialize(
            Some(args.as_main_args()),
            Some(&settings),
            Some(&mut app),
            std::ptr::null_mut(),
        );
        if started != 1 {
            let why = held(&cache).unwrap_or_else(|| "CEF did not initialize".into());
            log::error!("web: {why}");
            *FAILED.lock().expect("the start lock") = Some(why);
            cx.update(|cx| crate::view::deliver(cx));
            return;
        }
        log::info!("web: CEF started, cache {}", cache.display());
        let quitting = cx.update(|cx| cx.on_app_quit(|_| async { stop() }));
        quitting.detach();
        let mut due: Option<Duration> = Some(Duration::ZERO);
        loop {
            let timer = match due {
                Some(delay) => cx.background_executor().timer(delay).boxed_local(),
                None => futures::future::pending().boxed_local(),
            };
            futures::select_biased! {
                delay = asked.next() => match delay {
                    Some(delay) => {
                        let delay = Duration::from_millis(delay.max(0) as u64);
                        due = Some(due.map_or(delay, |now| now.min(delay)));
                        continue;
                    }
                    None => return,
                },
                _ = timer.fuse() => {}
            }
            run_later();
            make_browsers();
            do_message_loop_work();
            cx.update(|cx| crate::view::deliver(cx));
            // As cefclient: CEF does not ask for all work.
            due = Some(FALLBACK);
        }
    })
    .detach();
    Ok(())
}

/// Stops CEF as the app quits.
fn stop() {
    WAKE.lock().expect("the pump lock").take();
    shutdown();
    log::info!("web: CEF stopped");
}

/// Why CEF would refuse this cache: another app holds it.
fn held(cache: &Path) -> Option<String> {
    let lock = std::fs::read_link(cache.join("SingletonLock")).ok()?;
    let lock = lock.to_string_lossy();
    let pid: i32 = lock.rsplit('-').next()?.parse().ok()?;
    // Only a live process holds it.
    #[cfg(unix)]
    let alive = unsafe { libc::kill(pid, 0) } == 0;
    #[cfg(target_os = "windows")]
    let alive = windows_process_alive(pid as u32);
    alive.then(|| format!("another copy (process {pid}) is using {}", cache.display()))
}

#[cfg(target_os = "windows")]
fn windows_process_alive(pid: u32) -> bool {
    use std::ffi::c_void;

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, process_id: u32) -> *mut c_void;
        fn GetExitCodeProcess(process: *mut c_void, exit_code: *mut u32) -> i32;
        fn CloseHandle(object: *mut c_void) -> i32;
        fn GetLastError() -> u32;
    }

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        // An invalid PID is gone. Access denied or any other error is unknown;
        // preserve the profile lock rather than treating a live process as dead.
        return unsafe { GetLastError() } != 87;
    }
    let mut exit_code = 0;
    let queried = unsafe { GetExitCodeProcess(process, &mut exit_code) != 0 };
    unsafe {
        CloseHandle(process);
    }
    !queried || exit_code == STILL_ACTIVE
}
