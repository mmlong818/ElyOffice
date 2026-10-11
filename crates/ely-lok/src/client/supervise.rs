use std::{
    collections::HashSet,
    ffi::c_void,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    mem::{size_of, zeroed},
    os::windows::io::AsRawHandle,
    path::{Path, PathBuf},
    process::{Child, ExitStatus},
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_BASIC_PROCESS_ID_LIST,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicAccountingInformation,
        JobObjectBasicProcessIdList, JobObjectExtendedLimitInformation, QueryInformationJobObject,
        SetInformationJobObject, TerminateJobObject,
    },
};

pub(super) const BOOTSTRAP_ARG: &str = "--bootstrap-gate";
pub(super) const BOOTSTRAP_TOKEN: &[u8] = b"ELY-LOK-BOOTSTRAP-1\n";
pub(super) const BOOTSTRAP_TOKEN_LEN: usize = BOOTSTRAP_TOKEN.len();
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

fn startup_timeout() -> Duration {
    #[cfg(all(test, feature = "supervisor-test"))]
    if let Some(milliseconds) = std::env::var_os("ELY_LOK_TEST_STARTUP_MS")
        .and_then(|value| value.to_string_lossy().parse::<u64>().ok())
    {
        return Duration::from_millis(milliseconds);
    }
    STARTUP_TIMEOUT
}
pub(super) const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
pub(super) const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
const MONITOR_STOP_TIMEOUT: Duration = Duration::from_secs(1);
const TERMINATION_CODE: u32 = 0xe1_70_0001;
const QUARANTINE_MARKER: &str = ".ely-lok-worker-quarantine-v1";

static QUARANTINED_PROFILES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

struct Job(AtomicPtr<c_void>);

impl Job {
    fn new(handle: HANDLE) -> Self {
        Self(AtomicPtr::new(handle.cast()))
    }

    fn handle(&self) -> HANDLE {
        self.0.load(Ordering::SeqCst).cast()
    }

    fn close(&self) {
        let handle: HANDLE = self
            .0
            .swap(std::ptr::null_mut::<c_void>(), Ordering::SeqCst)
            .cast();
        if !handle.is_null() {
            unsafe { CloseHandle(handle) };
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.close();
    }
}

struct State {
    job: Job,
    profile: PathBuf,
    failed: AtomicBool,
    assigned: AtomicBool,
    cleaned: AtomicBool,
    stop: AtomicBool,
    startup: Mutex<Option<Instant>>,
    operation: Mutex<Option<(u64, Instant)>>,
    generation: AtomicU64,
    wake: Condvar,
}

pub(super) struct Supervisor {
    state: Arc<State>,
    monitor: Option<JoinHandle<()>>,
    monitor_done: mpsc::Receiver<()>,
}

pub(super) struct Deadline {
    state: Arc<State>,
    generation: u64,
}

#[cfg(all(test, feature = "supervisor-test"))]
pub(super) struct JobProbe(Arc<State>);

#[cfg(all(test, feature = "supervisor-test"))]
impl JobProbe {
    pub(super) fn active_processes(&self) -> io::Result<u32> {
        query_active(self.0.job.handle())
    }

    pub(super) fn process_ids(&self) -> io::Result<Vec<u32>> {
        query_process_ids(self.0.job.handle())
    }
}

pub(super) fn ensure_profile_available(profile: &Path) -> io::Result<PathBuf> {
    let canonical = fs::canonicalize(profile)?;
    let key = profile_key(&canonical);
    let quarantined = QUARANTINED_PROFILES
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(&key);
    let marker_exists = match fs::symlink_metadata(canonical.join(QUARANTINE_MARKER)) {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    if quarantined || marker_exists {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "lok profile {} is quarantined after an unconfirmed prior cleanup",
                canonical.display()
            ),
        ));
    }
    Ok(canonical)
}

#[cfg(all(test, feature = "supervisor-test"))]
pub(super) fn forget_profile_for_test(profile: &Path) {
    let Ok(canonical) = fs::canonicalize(profile) else {
        return;
    };
    QUARANTINED_PROFILES
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&profile_key(&canonical));
}

fn profile_key(canonical: &Path) -> String {
    canonical.to_string_lossy().to_lowercase()
}

fn quarantine_profile(profile: &Path, reason: &str) -> io::Result<PathBuf> {
    let canonical = fs::canonicalize(profile).unwrap_or_else(|_| profile.to_path_buf());
    let key = profile_key(&canonical);
    QUARANTINED_PROFILES
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(key);

    let marker = canonical.join(QUARANTINE_MARKER);
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(marker),
        Err(error) => return Err(error),
    };
    file.write_all(reason.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(marker)
}

impl Drop for Deadline {
    fn drop(&mut self) {
        let mut operation = self.state.operation.lock().unwrap();
        if operation.is_some_and(|(generation, _)| generation == self.generation) {
            *operation = None;
        }
        drop(operation);
        self.state.wake.notify_all();
    }
}

impl Supervisor {
    pub(super) fn create(profile: PathBuf) -> io::Result<Self> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Job::new(handle);
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                job.handle(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(io::Error::last_os_error());
        }
        let state = Arc::new(State {
            job,
            profile,
            failed: AtomicBool::new(false),
            assigned: AtomicBool::new(false),
            cleaned: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            startup: Mutex::new(Some(Instant::now() + startup_timeout())),
            operation: Mutex::new(None),
            generation: AtomicU64::new(0),
            wake: Condvar::new(),
        });
        let watched = Arc::clone(&state);
        let (done_tx, done_rx) = mpsc::channel();
        let monitor = thread::Builder::new()
            .name("lok-job-supervisor".into())
            .spawn(move || {
                if std::panic::catch_unwind(|| monitor(Arc::clone(&watched))).is_err() {
                    watched.failed.store(true, Ordering::SeqCst);
                    watched.stop.store(true, Ordering::SeqCst);
                    let handle = watched.job.handle();
                    if !handle.is_null() {
                        let _ = unsafe { TerminateJobObject(handle, TERMINATION_CODE) };
                    }
                }
                let _ = done_tx.send(());
            })?;
        Ok(Self {
            state,
            monitor: Some(monitor),
            monitor_done: done_rx,
        })
    }

    pub(super) fn assign_child(&self, child: &Child) -> io::Result<()> {
        self.check()?;
        #[cfg(feature = "supervisor-test")]
        if std::env::var_os("ELY_LOK_TEST_FAIL_ASSIGN").is_some() {
            if let Some(path) = std::env::var_os("ELY_LOK_TEST_ASSIGN_PID_FILE") {
                std::fs::write(path, child.id().to_string())?;
            }
            return Err(io::Error::other("controlled Job assignment failure"));
        }
        let assigned = unsafe {
            AssignProcessToJobObject(self.state.job.handle(), child.as_raw_handle() as HANDLE)
        };
        if assigned == 0 {
            return Err(io::Error::last_os_error());
        }
        self.state.assigned.store(true, Ordering::SeqCst);
        self.check()
    }

    pub(super) fn operation(&self, timeout: Duration) -> io::Result<Deadline> {
        self.check()?;
        let generation = self.state.generation.fetch_add(1, Ordering::SeqCst) + 1;
        *self.state.operation.lock().unwrap() = Some((generation, Instant::now() + timeout));
        self.state.wake.notify_all();
        Ok(Deadline {
            state: Arc::clone(&self.state),
            generation,
        })
    }

    pub(super) fn ready(&self) -> io::Result<()> {
        self.check()?;
        *self.state.startup.lock().unwrap() = None;
        self.state.wake.notify_all();
        Ok(())
    }

    pub(super) fn check(&self) -> io::Result<()> {
        let now = Instant::now();
        let startup = *self.state.startup.lock().unwrap();
        let operation = *self.state.operation.lock().unwrap();
        let expired = startup.is_some_and(|deadline| now >= deadline)
            || operation.is_some_and(|(_, deadline)| now >= deadline);
        if expired {
            self.expire();
        }
        if self.state.failed.load(Ordering::SeqCst) {
            Err(self.timed_out())
        } else {
            Ok(())
        }
    }

    pub(super) fn timed_out(&self) -> io::Error {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "the lok worker exceeded its deadline; its job was terminated",
        )
    }

    pub(super) fn expire(&self) {
        self.state.failed.store(true, Ordering::SeqCst);
        self.state.stop.store(true, Ordering::SeqCst);
        let handle = self.state.job.handle();
        if !handle.is_null() {
            let _ = unsafe { TerminateJobObject(handle, TERMINATION_CODE) };
        }
        self.state.wake.notify_all();
    }

    pub(super) fn reap(&mut self, child: &mut Child, force: bool) -> io::Result<ExitStatus> {
        if force || self.state.failed.load(Ordering::SeqCst) {
            self.expire();
        }
        let until = Instant::now() + CLEANUP_TIMEOUT;
        let mut status = None;
        loop {
            if status.is_none() {
                status = match child.try_wait() {
                    Ok(status) => status,
                    Err(error) => {
                        return Err(self.cleanup_failed(
                            child,
                            format!("could not query child exit: {error}"),
                            true,
                        ));
                    }
                };
            }
            let active = match self.active_processes() {
                Ok(active) => active,
                Err(error) => {
                    return Err(self.cleanup_failed(
                        child,
                        format!("Job Object ActiveProcesses query failed: {error}"),
                        true,
                    ));
                }
            };
            if active == 0 {
                if let Some(status) = status {
                    self.state.cleaned.store(true, Ordering::SeqCst);
                    if let Err(error) = self.stop_monitor() {
                        return Err(self.cleanup_failed(
                            child,
                            format!("supervisor monitor did not stop cleanly: {error}"),
                            false,
                        ));
                    }
                    return Ok(status);
                }
            }
            if Instant::now() >= until {
                return Err(self.cleanup_failed(
                    child,
                    "child exit or Job Object ActiveProcesses==0 was not confirmed within 5 seconds".into(),
                    false,
                ));
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    pub(super) fn active_processes(&self) -> io::Result<u32> {
        #[cfg(all(test, feature = "supervisor-test"))]
        if std::env::var_os("ELY_LOK_TEST_QUERY_ERROR").is_some() {
            return Err(io::Error::other("controlled persistent Job query error"));
        }
        let handle = self.state.job.handle();
        if handle.is_null() {
            return Err(io::Error::other(
                "the owned Job handle was already closed; ActiveProcesses is unconfirmed",
            ));
        }
        let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
        let ok = unsafe {
            QueryInformationJobObject(
                handle,
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(accounting.ActiveProcesses)
        }
    }

    fn stop_monitor(&mut self) -> io::Result<()> {
        self.state.stop.store(true, Ordering::SeqCst);
        self.state.wake.notify_all();
        let Some(monitor) = self.monitor.take() else {
            return Ok(());
        };
        match self.monitor_done.recv_timeout(MONITOR_STOP_TIMEOUT) {
            Ok(()) => monitor
                .join()
                .map_err(|_| io::Error::other("lok job supervisor panicked")),
            Err(error) => {
                self.monitor = Some(monitor);
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("lok job supervisor monitor did not stop within one second: {error}"),
                ))
            }
        }
    }

    fn cleanup_failed(
        &mut self,
        child: &mut Child,
        cause: String,
        wait_for_child: bool,
    ) -> io::Error {
        self.state.failed.store(true, Ordering::SeqCst);
        let marker = quarantine_profile(&self.state.profile, &cause);
        self.expire();
        let monitor = self.stop_monitor();
        let child_reaped = if wait_for_child {
            let until = Instant::now() + CLEANUP_TIMEOUT;
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break true,
                    Ok(None) if Instant::now() < until => thread::sleep(Duration::from_millis(20)),
                    Ok(None) | Err(_) => break false,
                }
            }
        } else {
            matches!(child.try_wait(), Ok(Some(_)))
        };
        if self.monitor.is_none() {
            // The monitor no longer uses the handle. Closing the last owned Job handle
            // invokes KILL_ON_JOB_CLOSE; it does not establish ActiveProcesses==0.
            self.state.job.close();
        }
        let marker_status = match marker {
            Ok(path) => format!("profile quarantined at {}", path.display()),
            Err(error) => format!(
                "profile remains quarantined in-process, but marker persistence failed: {error}"
            ),
        };
        let monitor_status = match monitor {
            Ok(()) => "monitor joined".to_owned(),
            Err(error) => format!("monitor stop failed: {error}"),
        };
        io::Error::other(format!(
            "lok cleanup failure: {cause}; Job Object ActiveProcesses==0 was not confirmed; child_reaped={child_reaped}; {marker_status}; {monitor_status}; profile reuse is unsafe"
        ))
    }

    #[cfg(all(test, feature = "supervisor-test"))]
    pub(super) fn probe(&self) -> JobProbe {
        JobProbe(Arc::clone(&self.state))
    }

    #[cfg(all(test, feature = "supervisor-test"))]
    pub(super) fn monitor_joined(&self) -> bool {
        self.monitor.is_none()
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        if self.state.assigned.load(Ordering::SeqCst) && !self.state.cleaned.load(Ordering::SeqCst)
        {
            let _ = quarantine_profile(
                &self.state.profile,
                "Supervisor dropped before Job zero was confirmed",
            );
            self.expire();
        }
        if self.stop_monitor().is_ok()
            && (!self.state.assigned.load(Ordering::SeqCst)
                || !self.state.cleaned.load(Ordering::SeqCst))
        {
            self.state.job.close();
        } else {
            // A successful Job-zero path leaves the handle valid for any observer
            // Arc; otherwise the monitor retains State until it has stopped.
            self.monitor.take();
        }
    }
}

fn monitor(state: Arc<State>) {
    loop {
        if state.stop.load(Ordering::SeqCst) {
            return;
        }

        let now = Instant::now();
        let startup_guard = state.startup.lock().unwrap();
        let operation_guard = state.operation.lock().unwrap();
        let startup = *startup_guard;
        let operation = *operation_guard;
        let deadline = [startup, operation.map(|(_, deadline)| deadline)]
            .into_iter()
            .flatten()
            .min();
        if deadline.is_some_and(|deadline| now >= deadline)
            && (startup.is_some_and(|deadline| now >= deadline)
                || operation.is_some_and(|(_, deadline)| now >= deadline))
        {
            state.failed.store(true, Ordering::SeqCst);
            state.stop.store(true, Ordering::SeqCst);
            let handle = state.job.handle();
            if !handle.is_null() {
                let _ = unsafe { TerminateJobObject(handle, TERMINATION_CODE) };
            }
            drop(operation_guard);
            drop(startup_guard);
            state.wake.notify_all();
            continue;
        }
        drop(operation_guard);
        drop(startup_guard);
        let delay = deadline.map_or(Duration::from_millis(100), |deadline| {
            deadline
                .saturating_duration_since(now)
                .min(Duration::from_millis(100))
        });
        let guard = state.operation.lock().unwrap();
        let (guard, _) = state.wake.wait_timeout(guard, delay).unwrap();
        drop(guard);
    }
}

#[cfg(all(test, feature = "supervisor-test"))]
fn query_active(job: HANDLE) -> io::Result<u32> {
    let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
    let ok = unsafe {
        QueryInformationJobObject(
            job,
            JobObjectBasicAccountingInformation,
            (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
            size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(accounting.ActiveProcesses)
    }
}

#[cfg(all(test, feature = "supervisor-test"))]
fn query_process_ids(job: HANDLE) -> io::Result<Vec<u32>> {
    const MAX_JOB_PROBE_PROCESSES: usize = 64;
    let header_bytes = size_of::<JOBOBJECT_BASIC_PROCESS_ID_LIST>() - size_of::<usize>();
    let buffer_bytes = header_bytes + MAX_JOB_PROBE_PROCESSES * size_of::<usize>();
    let mut buffer = vec![0usize; buffer_bytes.div_ceil(size_of::<usize>())];
    let mut returned = 0u32;
    let ok = unsafe {
        QueryInformationJobObject(
            job,
            JobObjectBasicProcessIdList,
            buffer.as_mut_ptr().cast(),
            buffer_bytes as u32,
            &mut returned,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    if (returned as usize) < header_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Job process-id query returned a truncated header",
        ));
    }
    let list = unsafe { &*buffer.as_ptr().cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>() };
    let count = list.NumberOfProcessIdsInList as usize;
    if list.NumberOfAssignedProcesses as usize > MAX_JOB_PROBE_PROCESSES
        || count > MAX_JOB_PROBE_PROCESSES
        || count > (returned as usize - header_bytes) / size_of::<usize>()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Job process-id query exceeded its bounded result buffer",
        ));
    }
    let pids = unsafe { std::slice::from_raw_parts(list.ProcessIdList.as_ptr(), count) };
    Ok(pids
        .iter()
        .map(|pid| *pid as u32)
        .collect())
}

pub(super) fn release(child: &mut Child) -> io::Result<()> {
    let stdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| io::Error::other("lok worker stdin was not piped"))?;
    stdin.write_all(BOOTSTRAP_TOKEN)?;
    stdin.flush()
}

pub(crate) fn await_bootstrap() -> io::Result<()> {
    let mut token = [0; BOOTSTRAP_TOKEN_LEN];
    io::stdin().read_exact(&mut token)?;
    if token != BOOTSTRAP_TOKEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid lok worker bootstrap token",
        ));
    }
    Ok(())
}
