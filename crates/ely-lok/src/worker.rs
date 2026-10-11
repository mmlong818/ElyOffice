use std::{
    io::{self, Write},
    path::Path,
    sync::Mutex,
};

use crate::ffi::{Document, Lok};
use crate::ipc::{self, Pack, Unpack, tag};

mod events;
#[cfg(unix)]
mod fatal;
mod input;
mod password;
mod revision;
mod slides;
mod teardown;
mod text;

/// English kernel, desktop hints; macros stay off (LOK-11).
const LOAD_OPTIONS: &str = "Language=en-US,DeviceFormFactor=desktop";

/// The worker's whole life: init, serve frames, die on shutdown.
pub fn run(
    install: &str,
    profile_dir: &str,
    input: impl io::Read + Send + 'static,
    output: impl Write + Send + 'static,
) -> io::Result<()> {
    set_output(Box::new(output));
    let profile_dir = std::fs::canonicalize(profile_dir)?;
    let profile_url = profile_url_of(&profile_dir);
    let _ = teardown::PROFILE.set(profile_dir.clone());
    let lok = Lok::hook(Path::new(install), &profile_url)?;
    #[cfg(unix)]
    fatal::watch();
    log::info!("lok-worker: up, profile {}", profile_dir.display());
    serve(lok, input)
}

#[cfg(target_os = "macos")]
fn serve(lok: Lok, input: impl io::Read + Send + 'static) -> io::Result<()> {
    let lok = Box::leak(Box::new(lok));
    pin(lok);
    lok.register_callback(password::on_kit)?;
    lok.register_any_input(waiting);
    unsafe {
        gcd::dispatch_async_f(gcd::main_queue(), std::ptr::null_mut(), mark_pumping);
    }
    std::thread::spawn(move || reader(input));
    events::start();
    lok.run_loop();
    Ok(())
}

/// Requests queued for the main thread and not yet answered.
#[cfg(target_os = "macos")]
static PENDING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Lets idle jobs yield to a waiting request.
#[cfg(target_os = "macos")]
extern "C" fn waiting(_: *mut std::ffi::c_void, _: i32) -> bool {
    PENDING.load(std::sync::atomic::Ordering::SeqCst) > 0
}

/// The queue's first drained block.
#[cfg(target_os = "macos")]
extern "C" fn mark_pumping(_: *mut std::ffi::c_void) {
    PUMPING.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Set once the main queue drains its first block.
#[cfg(target_os = "macos")]
static PUMPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The queue's last drained block.
#[cfg(target_os = "macos")]
extern "C" fn quit_block(_: *mut std::ffi::c_void) {
    log::info!("lok-worker: quit on drained queue");
    teardown::teardown(0);
}

#[cfg(target_os = "macos")]
fn reader(mut input: impl io::Read) {
    while !PUMPING.load(std::sync::atomic::Ordering::SeqCst) {
        std::thread::yield_now();
    }
    loop {
        match ipc::read_frame(&mut input) {
            Ok(Some((tag, id, payload))) => {
                let request = Box::into_raw(Box::new((tag, id, payload)));
                PENDING.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                unsafe {
                    gcd::dispatch_async_f(gcd::main_queue(), request.cast(), handle);
                }
                if tag == tag::SHUTDOWN {
                    return;
                }
            }
            Ok(None) => {
                log::info!("lok-worker: stdin closed");
                unsafe {
                    gcd::dispatch_async_f(gcd::main_queue(), std::ptr::null_mut(), quit_block);
                }
                return;
            }
            Err(error) => {
                log::warn!("lok-worker: frame read failed: {error}");
                quit(1);
            }
        }
    }
}

#[cfg(target_os = "macos")]
extern "C" fn handle(context: *mut std::ffi::c_void) {
    let (tag, id, payload) = *unsafe { Box::from_raw(context.cast::<(u8, u64, Vec<u8>)>()) };
    let lok = unsafe { &*LOK.load(std::sync::atomic::Ordering::SeqCst) };
    PENDING.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    answer(lok, tag, id, &payload);
}

/// The pinned kit, for callbacks.
fn lok() -> &'static Lok {
    unsafe { &*LOK.load(std::sync::atomic::Ordering::SeqCst) }
}

static LOK: std::sync::atomic::AtomicPtr<Lok> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

#[cfg(target_os = "macos")]
mod gcd {
    use std::ffi::c_void;
    unsafe extern "C" {
        static _dispatch_main_q: c_void;
        pub fn dispatch_async_f(
            queue: *mut c_void,
            context: *mut c_void,
            work: extern "C" fn(*mut c_void),
        );
    }
    /// The main queue: the header's macro target.
    pub fn main_queue() -> *mut c_void {
        unsafe { &_raw as *const c_void as *mut c_void }
    }
    use _dispatch_main_q as _raw;
}

#[cfg(not(target_os = "macos"))]
fn serve(lok: Lok, mut input: impl io::Read) -> io::Result<()> {
    let lok = Box::leak(Box::new(lok));
    pin(lok);
    if let Err(error) = lok.register_callback(password::on_kit) {
        #[cfg(windows)]
        lok.quarantine();
        return Err(error);
    }
    events::start();
    loop {
        match ipc::read_frame(&mut input)? {
            Some((tag, id, payload)) => answer(lok, tag, id, &payload),
            None => {
                log::info!("lok-worker: stdin closed");
                teardown::teardown(0);
            }
        }
    }
}

/// Handles one frame and writes its answer.
fn answer(lok: &Lok, tag: u8, id: u64, payload: &[u8]) {
    if tag == tag::SHUTDOWN {
        let outcome = teardown::teardown_work();
        #[cfg(windows)]
        terminate_if_owner_failed(lok.owner_failed_closed());
        let (reply_tag, body) = match &outcome {
            Ok(()) => (tag::OK, Vec::new()),
            Err(error) => (tag::ERR, Pack::new().str(&error.to_string()).bytes()),
        };
        let out = OUT.get().expect("the output is set");
        let _ = ipc::write_frame(
            &mut *out.lock().unwrap_or_else(|p| p.into_inner()),
            reply_tag,
            id,
            &body,
        );
        log::info!("lok-worker: shutdown");
        quit(if outcome.is_ok() { 0 } else { 1 });
    }
    // Bumped first: events it causes carry the new revision.
    revision::bump(tag, payload);
    let mut unpack = Unpack::new(payload);
    let unpack = &mut unpack;
    let outcome = match tag {
        tag::BUILD_ID => lok.build_id().map(|id| Pack::new().str(&id).bytes()),
        tag::LOAD => unpack
            .str()
            .and_then(|url| Ok((url, unpack.str()?)))
            .and_then(|(url, password)| load(lok, &url, password, ""))
            .map(|id| Pack::new().u64(id).bytes()),
        tag::IMPORT => text::import(lok, unpack).map(|id| Pack::new().u64(id).bytes()),
        tag::EXPORT_TEXT => text::export(unpack).map(|()| Vec::new()),
        tag::DOC_SIZE => unpack
            .u64()
            .and_then(|id| with_doc(id, |doc| doc.size()))
            .map(|(width, height)| Pack::new().i64(width).i64(height).bytes()),
        tag::SAVE_AS => save(unpack).map(|()| Vec::new()),
        tag::EXPORT_PDF => export_pdf(unpack).map(|()| Vec::new()),
        tag::CLOSE_DOC => unpack
            .u64()
            .map(|id| close(id))
            .and_then(|closed| closed.map(|()| Vec::new())),
        tag::EXPECT => revision::expect(unpack),
        tag::REVISION => revision::current(unpack),
        tag::PAINT_TILE => input::painted(unpack),
        tag::PAINT_PART => input::part_painted(unpack),
        tag::SLIDE_LAYERS => slides::slide_layers(unpack),
        tag::VERIFY => unpack
            .str()
            .and_then(|url| verify(lok, &url))
            .map(|count| Pack::new().u64(count).bytes()),
        tag::RUN_MACRO => unpack.str().and_then(|url| {
            if dev_mode() && cfg!(debug_assertions) {
                Ok(Pack::new().u64(lok.run_macro(&url) as u64).bytes())
            } else {
                Err(io::Error::other("run_macro is a dev-only probe"))
            }
        }),
        tag::POST_KEY => input::key(unpack),
        tag::POST_MOUSE => input::mouse(unpack),
        tag::GRAPHIC => input::graphic(unpack),
        tag::POST_TEXT => input::text(unpack),
        tag::POST_UNO => input::uno(unpack),
        tag::COPY => input::copy(unpack),
        tag::PASTE => input::paste(unpack),
        tag::CUT => input::cut(unpack),
        tag::VALUES => input::values(unpack),
        tag::PAGES => unpack
            .u64()
            .and_then(|id| with_doc(id, |doc| doc.page_rects()))
            .and_then(|rects| rects)
            .map(|rects| Pack::new().str(&rects).bytes()),
        tag::SHEETS => unpack
            .u64()
            .and_then(|id| with_doc(id, sheets))
            .and_then(|sheets| sheets),
        tag::SHOW_SHEET => unpack
            .u64()
            .and_then(|id| Ok((id, unpack.u64()?)))
            .and_then(|(id, part)| {
                let part = i32::try_from(part).map_err(io::Error::other)?;
                with_doc(id, |doc| doc.set_part(part))
            })
            .map(|()| Vec::new()),
        tag::VISIBLE_AREA => unpack
            .u64()
            .and_then(|id| {
                let mut number = || i32::try_from(unpack.i64()?).map_err(io::Error::other);
                Ok((id, (number()?, number()?, number()?, number()?)))
            })
            .and_then(|(id, (x, y, w, h))| with_doc(id, |doc| doc.set_visible_area(x, y, w, h)))
            .map(|()| Vec::new()),
        tag::SELECTION_TEXT => unpack
            .u64()
            .and_then(|id| with_doc(id, |doc| doc.selection_text()))
            .and_then(|text| text)
            .map(|text| Pack::new().str(&text).bytes()),
        tag::COMPLETE_FUNCTION => unpack
            .u64()
            .and_then(|id| {
                let name = unpack.str()?;
                with_doc(id, |doc| doc.complete_function(&name)).and_then(|done| done)
            })
            .map(|()| Vec::new()),
        tag::SET_AUTHOR => unpack
            .str()
            .and_then(|name| lok.set_option("author", &name))
            .map(|()| Vec::new()),
        other => Err(io::Error::other(format!("unknown request {other}"))),
    };
    #[cfg(windows)]
    if outcome.as_ref().is_err_and(|error| {
        error
            .get_ref()
            .and_then(|cause| cause.downcast_ref::<crate::events::ResultWaitError>())
            .is_some()
    }) {
        lok.quarantine();
    }
    #[cfg(windows)]
    terminate_if_owner_failed(lok.owner_failed_closed());
    let reply = match outcome {
        Ok(body) => (tag::OK, body),
        Err(error) => {
            // Text may quote the document: reply, never log.
            log::warn!("lok-worker: request {tag} failed ({:?})", error.kind());
            (tag::ERR, Pack::new().str(&error.to_string()).bytes())
        }
    };
    let out = OUT.get().expect("the output is set");
    let mut out = out.lock().unwrap_or_else(|p| p.into_inner());
    let mut written = ipc::write_frame(&mut *out, reply.0, id, &reply.1);
    // An oversized answer is refused; the worker lives.
    if let Err(error) = &written
        && error.kind() == io::ErrorKind::InvalidInput
    {
        log::warn!("lok-worker: request {tag}: {error}");
        written = ipc::write_frame(
            &mut *out,
            tag::ERR,
            id,
            &Pack::new().str(&error.to_string()).bytes(),
        );
    }
    if let Err(error) = written {
        log::warn!("lok-worker: frame write failed: {error}");
        quit(1);
    }
}

/// Whether the dev-only probes answer (a `--dev` argv flag only).
fn dev_mode() -> bool {
    static DEV: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DEV.get_or_init(|| std::env::args().any(|arg| arg == "--dev"))
}

/// Loads a document with macros disabled (LOK-11).
fn load(lok: &Lok, url: &str, password: String, filter: &str) -> io::Result<u64> {
    if password.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a password with NUL",
        ));
    }
    password::arm(Some(password).filter(|password| !password.is_empty()));
    // LOK hands what it does not take to the filter.
    let options = match filter {
        "" => LOAD_OPTIONS.to_owned(),
        filter => format!("{LOAD_OPTIONS},{filter}"),
    };
    let loaded = lok.load(url, &options);
    let refused = password::refused();
    password::arm(None);
    let doc = match loaded {
        Err(_) if refused => return Err(io::Error::other(ipc::LOCKED)),
        loaded => loaded?,
    };
    // A session number per load; native pointers get reused.
    let id = revision::session();
    let setup = (|| {
        crate::events::watch(id, &doc)?;
        doc.enable_a11y();
        crate::events::seed_a11y(id, &doc.a11y_paragraph()?);
        // Settle the initial layout now; later paints see it (LOK-03).
        doc.post_uno(".uno:Repaginate", None)?;
        Ok::<(), io::Error>(())
    })();
    if let Err(error) = setup {
        let _ = crate::events::unwatch(id, &doc);
        #[cfg(windows)]
        let _ = doc.dispose_on_owner();
        return Err(error);
    }
    let mut docs = docs().lock().expect("the docs lock");
    docs.insert(id, doc);
    log::info!("lok-worker: {} documents open", docs.len());
    drop(docs);
    log::info!("lok-worker: loaded {url}, filter options {filter:?}");
    Ok(id)
}

/// Stops watching and frees document `id`.
fn close(id: u64) -> io::Result<()> {
    let doc = docs().lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
    revision::forget(id);
    if let Some(doc) = doc {
        crate::events::unwatch(id, &doc)?;
        #[cfg(windows)]
        doc.dispose_on_owner()?;
    }
    let count = docs().lock().unwrap_or_else(|p| p.into_inner()).len();
    log::info!("lok-worker: {count} documents open");
    Ok(())
}

/// The live documents, freed on CLOSE_DOC.
fn docs() -> &'static Mutex<std::collections::HashMap<u64, Document>> {
    static DOCS: std::sync::OnceLock<Mutex<std::collections::HashMap<u64, Document>>> =
        std::sync::OnceLock::new();
    DOCS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// Loads and paints; the bytes gained (R-01).
fn verify(lok: &Lok, url: &str) -> io::Result<u64> {
    let doc = lok.load(url, LOAD_OPTIONS)?;
    let mut tile = vec![0xABu8; 64 * 64 * 4];
    doc.paint_tile(&mut tile, 64, 64);
    #[cfg(windows)]
    doc.dispose_on_owner()?;
    drop(doc);
    let gained = tile.iter().filter(|byte| **byte != 0xAB).count() as u64;
    log::info!("lok-worker: tile self-check gained {gained} bytes");
    Ok(gained)
}

/// Borrows a live document for a read call.
fn with_doc<T>(id: u64, call: impl FnOnce(&Document) -> T) -> io::Result<T> {
    #[cfg(windows)]
    let doc = docs()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&id)
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such document"))?;
    #[cfg(not(windows))]
    let docs = docs().lock().expect("the docs lock");
    #[cfg(not(windows))]
    let Some(doc) = docs.get(&id) else {
        return Err(io::Error::new(io::ErrorKind::NotFound, "no such document"));
    };
    // Calc reads the current view; every request uses its own.
    doc.activate();
    Ok(call(&doc))
}

/// The shown part, then each part's name and info.
fn sheets(doc: &Document) -> io::Result<Vec<u8>> {
    let count = doc.parts();
    let mut pack = Pack::new().u64(doc.part() as u64).u64(count as u64);
    for part in 0..count {
        pack = pack.str(&doc.part_name(part)?).str(&doc.part_info(part)?);
    }
    Ok(pack.bytes())
}

/// Saves a document under a new url and format (LOK-09).
fn save(unpack: &mut Unpack) -> io::Result<()> {
    let id = unpack.u64()?;
    let url = unpack.str()?;
    let format = unpack.str()?;
    let options = unpack.str()?;
    if !options.is_empty() && options != "TakeOwnership" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown save option",
        ));
    }
    with_doc(id, |doc| doc.save_as(&url, &format, &options))??;
    log::info!("lok-worker: saved as {format} to {url} ({options})");
    Ok(())
}

/// Writes a PDF from typed options, never raw text.
fn export_pdf(unpack: &mut Unpack) -> io::Result<()> {
    let (id, url) = (unpack.u64()?, unpack.str()?);
    let (from, to) = (unpack.u64()?, unpack.u64()?);
    let (comments, changes) = (unpack.u64()? != 0, unpack.u64()? != 0);
    let handouts = unpack.u64()?;
    let flag = |on: bool| serde_json::json!({ "type": "boolean", "value": on.to_string() });
    let mut options = serde_json::json!({
        "ExportNotes": flag(false),
        "ExportNotesInMargin": flag(comments),
        "ExportTrackedChanges": flag(changes),
    });
    if handouts != 0 {
        let per = handouts.to_string();
        options["ExportHandouts"] = serde_json::json!({ "type": "long", "value": per });
    }
    if from != 0 {
        if from > to {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a page range ends first",
            ));
        }
        let range = format!("{from}-{to}");
        options["PageRange"] = serde_json::json!({ "type": "string", "value": range });
    }
    with_doc(id, |doc| doc.save_as(&url, "pdf", &options.to_string()))??;
    log::info!(
        "lok-worker: exported PDF pages {from}-{to}, comments {comments}, changes {changes}, handouts {handouts}"
    );
    Ok(())
}

/// The one shared output for every handler.
static OUT: std::sync::OnceLock<Mutex<Box<dyn Write + Send>>> = std::sync::OnceLock::new();

/// Sets the response sink once.
pub fn set_output(out: Box<dyn Write + Send>) {
    let _ = OUT.set(Mutex::new(out));
}

/// Sets the kit for the handlers once.
fn pin(lok: &'static Lok) {
    LOK.store(
        lok as *const Lok as *mut Lok,
        std::sync::atomic::Ordering::SeqCst,
    );
}

/// The profile directory as a `file:` URL.
fn profile_url_of(dir: &Path) -> String {
    ipc::file_url_of(dir)
}

/// Exits without LO's atexit handlers (R-01's probe does the same).
#[cfg(unix)]
fn quit(code: i32) -> ! {
    unsafe { libc::_exit(code) };
}

/// Exits without LO's atexit handlers, the msvcrt way.
#[cfg(windows)]
fn quit(code: i32) -> ! {
    unsafe extern "C" {
        fn _exit(code: i32) -> !;
    }
    unsafe { _exit(code) };
}

#[cfg(windows)]
pub(super) fn terminate_if_owner_failed(failed_closed: bool) {
    if failed_closed {
        // The owner retains native contexts and the Kit in quarantine. Exit
        // without writing a reply or running Lok/Document destructors; the
        // client-owned Job reaps us.
        quit(1);
    }
}

#[cfg(all(test, windows))]
mod owner_exit_tests {
    use super::terminate_if_owner_failed;
    use std::{
        os::windows::io::AsRawHandle,
        process::{Command, Stdio},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
            QueryInformationJobObject, SetInformationJobObject,
        },
        System::Threading::{
            CreateEventW, EVENT_ALL_ACCESS, GetProcessTimes, INFINITE, OpenEventW, SetEvent,
            WaitForSingleObject,
        },
    };

    const CHILD_ENV: &str = "ELY_LOK_TEST_OWNER_EXIT_CHILD";
    const GATE_ENV: &str = "ELY_LOK_TEST_OWNER_EXIT_GATE";
    const DROP_MARKER: &str = "ELY_LOK_DROP_MARKER";

    struct DropMarker;
    impl Drop for DropMarker {
        fn drop(&mut self) {
            println!("{DROP_MARKER}");
        }
    }

    #[test]
    fn owner_failure_exit_child() {
        if std::env::var_os(CHILD_ENV).is_none() {
            return;
        }
        let gate_name = std::env::var(GATE_ENV).expect("parent supplied fatal-exit gate");
        let gate_name: Vec<u16> = gate_name.encode_utf16().chain(Some(0)).collect();
        let gate = unsafe { OpenEventW(EVENT_ALL_ACCESS, 0, gate_name.as_ptr()) };
        assert!(!gate.is_null(), "open parent fatal-exit gate");
        let _marker = DropMarker;
        assert_eq!(
            unsafe { WaitForSingleObject(gate, INFINITE) },
            0,
            "wait for parent gate"
        );
        unsafe { CloseHandle(gate) };
        terminate_if_owner_failed(true);
        panic!("quarantined owner did not terminate the process");
    }

    #[test]
    fn quarantined_owner_exits_without_reply_or_native_drop_and_job_empties() {
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        assert!(!job.is_null(), "create isolated test Job Object");
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        assert_ne!(configured, 0, "configure kill-on-close test Job");

        let gate_id = std::process::id();
        let gate_env = format!("Local\\ElyLokOwnerExitGate-{gate_id}");
        let gate_name: Vec<u16> = gate_env.encode_utf16().chain(Some(0)).collect();
        let gate = unsafe { CreateEventW(std::ptr::null(), 1, 0, gate_name.as_ptr()) };
        assert!(!gate.is_null(), "create fatal-exit gate");

        let executable = std::env::current_exe().expect("current test executable");
        let child = Command::new(executable)
            .args([
                "worker::owner_exit_tests::owner_failure_exit_child",
                "--exact",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .env(GATE_ENV, &gate_env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start controlled fatal-exit child");
        let assigned = unsafe { AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) };
        assert_ne!(assigned, 0, "assign fatal-exit child to Job");
        let child_pid = child.id();
        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut kernel = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut user = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        assert_ne!(
            unsafe {
                GetProcessTimes(
                    child.as_raw_handle() as HANDLE,
                    &mut creation,
                    &mut exit,
                    &mut kernel,
                    &mut user,
                )
            },
            0,
            "record child process start time"
        );
        let started = ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;
        assert_ne!(started, 0, "recorded child process start timestamp");
        assert_ne!(unsafe { SetEvent(gate) }, 0, "release fatal-exit child");
        let output = child
            .wait_with_output()
            .expect("fatal-exit child is reaped");
        unsafe { CloseHandle(gate) };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "fatal worker exit code; child stdout={stdout}; stderr={stderr}"
        );
        assert!(child_pid != 0, "recorded fatal child PID");
        assert!(!stdout.contains(DROP_MARKER), "fatal exit skips Rust Drop");
        assert!(
            !stdout.contains("ERR"),
            "quarantine emits no worker ERR reply"
        );

        let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { std::mem::zeroed() };
        let queried = unsafe {
            QueryInformationJobObject(
                job,
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(queried, 0, "query test Job after child exit");
        assert_eq!(accounting.ActiveProcesses, 0, "fatal worker Job is empty");
        unsafe { CloseHandle(job) };
    }

    #[test]
    fn non_quarantined_request_error_keeps_reply_path_available() {
        terminate_if_owner_failed(false);
        let mut reply = Vec::new();
        crate::ipc::write_frame(
            &mut reply,
            crate::ipc::tag::ERR,
            17,
            &crate::ipc::Pack::new()
                .str("ordinary request timed out")
                .bytes(),
        )
        .expect("ordinary request errors still produce a reply frame");
        assert!(!reply.is_empty(), "ordinary ERR frame was written");
    }
}
