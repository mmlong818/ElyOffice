use std::sync::OnceLock;
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tempfile::tempdir;
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, FILETIME, HANDLE, WAIT_OBJECT_0},
    System::Threading::{
        GetExitCodeProcess, GetProcessId, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW, WaitForSingleObject,
    },
};

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

use crate::Worker;

const TEST_TIMEOUT: Duration = Duration::from_millis(600);
static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[test]
fn job_bootstrap_normal_exit_queue_and_stall_are_bounded() {
    let _serial = TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fixture = fixture_bin();
    assert!(
        fixture.is_file(),
        "build ely-lok-worker-fixture before running this test"
    );
    // Only this test reads the override; each child inherits a concrete fixture path.
    unsafe { std::env::set_var("ELY_LOK_WORKER", fixture) };

    let tmp = tempdir().expect("temp root");
    let install = tmp.path().join("install");
    fs::create_dir_all(&install).expect("install directory");

    let profile = tmp.path().join("normal");
    let mut worker = Worker::launch(&install, &profile).expect("launch fake worker");
    assert!(
        worker
            .build_id()
            .expect("normal reply")
            .contains("LibreOffice")
    );
    assert!(
        profile.join("bootstrap.ok").is_file(),
        "worker passed the gate after job assignment"
    );
    worker.shutdown().expect("clean shutdown");
    assert_eq!(worker.supervisor.active_processes().expect("query job"), 0);
    eprintln!("JOB_ACTIVE_PROCESSES=0 scenario=normal-shutdown");

    let profile = tmp.path().join("queued");
    fs::create_dir_all(&profile).expect("profile directory");
    fs::write(profile.join("mode.txt"), "delay-pair").expect("mode");
    let mut worker = Worker::launch(&install, &profile).expect("launch queued worker");
    worker.request_timeout = TEST_TIMEOUT;
    let shared = Arc::new(Mutex::new(worker));
    let (signal, entered) = mpsc::channel();
    let first_shared = Arc::clone(&shared);
    let first = thread::spawn(move || {
        let mut worker = first_shared.lock().expect("worker mutex");
        signal.send(()).expect("signal request");
        worker.build_id().expect("first request after startup");
        worker.build_id().expect("second request after startup");
    });
    entered.recv().expect("first caller entered");
    wait_file(&profile.join("accepted-2"));
    let queued_at = Instant::now();
    let second_shared = Arc::clone(&shared);
    let second = thread::spawn(move || {
        let mut worker = second_shared.lock().expect("worker mutex");
        worker.build_id()
    });
    first
        .join()
        .expect("first caller completes within its request deadlines");
    let queued_wait = queued_at.elapsed();
    eprintln!("QUEUE_WAIT_WITH_TWO_REQUESTS={queued_wait:?} request_deadline={TEST_TIMEOUT:?}");
    assert!(
        queued_wait >= TEST_TIMEOUT,
        "the waiter waited beyond one request timeout: {queued_wait:?}"
    );
    assert!(
        second.join().expect("second caller thread").is_ok(),
        "queued request gets its own deadline after lock acquisition"
    );
    let mut worker = shared.lock().expect("worker mutex after queue");
    worker.shutdown().expect("queued worker shutdown");
    assert_eq!(worker.supervisor.active_processes().expect("query job"), 0);
    eprintln!("JOB_ACTIVE_PROCESSES=0 scenario=queued-generation");

    let profile = tmp.path().join("stalled");
    fs::create_dir_all(&profile).expect("profile directory");
    fs::write(profile.join("mode.txt"), "stall-second").expect("mode");
    let mut worker = Worker::launch(&install, &profile).expect("launch stalled worker");
    worker.request_timeout = TEST_TIMEOUT;
    let shared = Arc::new(Mutex::new(worker));
    let first_shared = Arc::clone(&shared);
    let first = thread::spawn(move || {
        let mut worker = first_shared.lock().expect("worker mutex");
        worker.ready().expect("fake worker readiness");
        worker.request(crate::ipc::tag::POST_UNO, b"accepted edit")
    });
    wait_file(&profile.join("accepted-2"));
    let queued_at = Instant::now();
    let second_shared = Arc::clone(&shared);
    let second = thread::spawn(move || {
        let mut worker = second_shared.lock().expect("worker mutex");
        let started = Instant::now();
        let outcome = worker.build_id();
        (outcome, started.elapsed())
    });
    let timed = first
        .join()
        .expect("stalled caller returns")
        .expect_err("deadline fails accepted edit without retry");
    assert_eq!(timed.kind(), std::io::ErrorKind::TimedOut);
    let (late, after_lock) = second.join().expect("queued caller returns");
    eprintln!(
        "ACCEPTED_NO_DONE_TIMEOUT={:?} queued_failure_after_lock={after_lock:?}",
        queued_at.elapsed()
    );
    assert!(late.is_err(), "a failed worker rejects queued work");
    assert!(
        after_lock < Duration::from_millis(150),
        "failed state is immediate after lock: {after_lock:?}"
    );
    assert!(
        queued_at.elapsed() >= Duration::from_millis(500),
        "the waiter waited through the active request deadline: {:?}",
        queued_at.elapsed()
    );
    assert!(
        !profile.join("accepted-3").exists(),
        "the client did not retry the accepted edit"
    );
    let mut worker = shared.lock().expect("worker mutex after timeout");
    let cleanup = worker.shutdown();
    assert!(cleanup.is_err(), "shutdown reports the timed out worker");
    assert_eq!(
        worker
            .supervisor
            .active_processes()
            .expect("query killed job"),
        0
    );
    eprintln!("JOB_ACTIVE_PROCESSES=0 scenario=accepted-no-done");
}

#[test]
fn bootstrap_gate_and_assign_failure_never_enter_worker_runtime() {
    let _serial = TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fixture = fixture_bin();
    unsafe { std::env::set_var("ELY_LOK_WORKER", &fixture) };
    let tmp = tempdir().expect("temp root");
    let install = tmp.path().join("install");
    fs::create_dir_all(&install).expect("install directory");

    let gated = tmp.path().join("no-token");
    fs::create_dir_all(&gated).expect("gated profile");
    let mut child = std::process::Command::new(&fixture)
        .arg(&install)
        .arg(&gated)
        .arg("--bootstrap-gate")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("start gated fixture");
    wait_file(&gated.join("prebootstrap.pid"));
    thread::sleep(Duration::from_millis(100));
    assert!(
        !gated.join("bootstrap.ok").exists(),
        "no token means no runtime entry"
    );
    assert!(child.try_wait().expect("check gated process").is_none());
    child.kill().expect("stop gated fixture");
    child.wait().expect("reap gated fixture");
    eprintln!("BOOTSTRAP_WITHOUT_TOKEN=not-entered child=reaped");

    let refused = tmp.path().join("assign-failure");
    fs::create_dir_all(&refused).expect("refused profile");
    unsafe { std::env::set_var("ELY_LOK_TEST_FAIL_ASSIGN", "1") };
    unsafe { std::env::set_var("ELY_LOK_TEST_ASSIGN_PID_FILE", refused.join("child.pid")) };
    let result = Worker::launch(&install, &refused);
    unsafe { std::env::remove_var("ELY_LOK_TEST_FAIL_ASSIGN") };
    unsafe { std::env::remove_var("ELY_LOK_TEST_ASSIGN_PID_FILE") };
    assert!(result.is_err(), "assignment failure is fail-closed");
    wait_file(&refused.join("child.pid"));
    assert!(
        !refused.join("bootstrap.ok").exists(),
        "assignment failure sends no token"
    );
    assert!(
        !refused.join("prebootstrap.pid").exists(),
        "failed child never reached bootstrap gate"
    );
    let pid = fs::read_to_string(refused.join("child.pid"))
        .expect("fixture pid")
        .parse::<u32>()
        .expect("numeric pid");
    assert_process_gone(pid);
    eprintln!("ASSIGN_FAILURE=child-reaped bootstrap=not-entered");
}

#[test]
fn startup_deadline_survives_first_long_request_deadline() {
    let _serial = TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fixture = fixture_bin();
    unsafe { std::env::set_var("ELY_LOK_WORKER", fixture) };
    unsafe { std::env::set_var("ELY_LOK_TEST_STARTUP_MS", "300") };
    let tmp = tempdir().expect("temp root");
    let install = tmp.path().join("install");
    let profile = tmp.path().join("startup-deadline");
    fs::create_dir_all(&install).expect("install directory");
    fs::create_dir_all(&profile).expect("profile directory");
    fs::write(profile.join("mode.txt"), "stall-startup").expect("mode");
    let mut worker = Worker::launch(&install, &profile).expect("launch fake worker");
    worker.request_timeout = Duration::from_secs(2);
    let pid = worker.child.id();
    let started = Instant::now();
    let error = worker
        .ready()
        .expect_err("startup deadline must kill a worker that never becomes ready");
    let elapsed = started.elapsed();
    unsafe { std::env::remove_var("ELY_LOK_TEST_STARTUP_MS") };
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut, "{error}");
    assert!(profile.join("startup-stalled").is_file());
    assert!(
        elapsed < Duration::from_millis(1500),
        "the 300 ms startup timer was extended by the 2 s first request: {elapsed:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(200),
        "the fixture should reach the post-token startup stall first: {elapsed:?}"
    );
    let cleanup = worker.shutdown();
    assert!(cleanup.is_err(), "shutdown retains the startup failure");
    let probe = worker.supervisor.probe();
    assert_eq!(
        worker
            .supervisor
            .active_processes()
            .expect("query startup job"),
        0
    );
    drop(worker);
    assert_eq!(
        probe.active_processes().expect("query stopped startup job"),
        0
    );
    drop(probe);
    assert_process_gone(pid);
    eprintln!(
        "STARTUP_DEADLINE_WINS={elapsed:?} startup=300ms first_request=2s job_active_processes=0"
    );
}

#[test]
fn blocked_stdin_write_is_released_by_independent_job_deadline() {
    let _serial = TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fixture = fixture_bin();
    unsafe { std::env::set_var("ELY_LOK_WORKER", fixture) };
    let tmp = tempdir().expect("temp root");
    let install = tmp.path().join("install");
    let profile = tmp.path().join("blocked-write");
    fs::create_dir_all(&install).expect("install directory");
    fs::create_dir_all(&profile).expect("profile directory");
    fs::write(profile.join("mode.txt"), "block-payload").expect("mode");
    let mut worker = Worker::launch(&install, &profile).expect("launch fake worker");
    worker.request_timeout = TEST_TIMEOUT;
    worker.ready().expect("complete fake worker readiness");
    let payload = vec![0x55; 16 * 1024 * 1024];
    let started = Instant::now();
    let error = worker
        .request(250, &payload)
        .expect_err("full pipe write must be interrupted by job termination");
    wait_file(&profile.join("accepted-payload"));
    assert_eq!(
        error.kind(),
        std::io::ErrorKind::TimedOut,
        "deadline wins over broken pipe"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "write returned after bounded cleanup"
    );
    eprintln!(
        "BLOCKED_STDIN_WRITE_RETURNED={:?} request_deadline={TEST_TIMEOUT:?}",
        started.elapsed()
    );
    let cleanup = worker.shutdown();
    assert!(cleanup.is_err(), "shutdown reports the failed worker");
    assert_eq!(
        worker
            .supervisor
            .active_processes()
            .expect("query stopped job"),
        0
    );
    eprintln!("JOB_ACTIVE_PROCESSES=0 scenario=blocked-stdin-write");
}

#[test]
fn drop_completes_normal_shutdown_and_reaps_child() {
    let _serial = TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fixture = fixture_bin();
    unsafe { std::env::set_var("ELY_LOK_WORKER", fixture) };
    let tmp = tempdir().expect("temp root");
    let install = tmp.path().join("install");
    let profile = tmp.path().join("drop");
    fs::create_dir_all(&install).expect("install directory");
    fs::create_dir_all(&profile).expect("profile directory");
    let worker = Worker::launch(&install, &profile).expect("launch fake worker");
    let pid = worker.child.id();
    let probe = worker.supervisor.probe();
    drop(worker);
    assert_process_gone(pid);
    assert_eq!(probe.active_processes().expect("query dropped job"), 0);
    eprintln!("JOB_ACTIVE_PROCESSES=0 scenario=drop-normal");
    eprintln!("DROP_NORMAL=child-reaped");

    let profile = tmp.path().join("drop-stalled-shutdown");
    fs::create_dir_all(&profile).expect("stalled drop profile");
    fs::write(profile.join("mode.txt"), "stall-shutdown").expect("mode");
    let mut worker = Worker::launch(&install, &profile).expect("launch stalled shutdown fixture");
    worker.shutdown_timeout = TEST_TIMEOUT;
    worker.ready().expect("stalled fixture readiness");
    let pid = worker.child.id();
    let probe = worker.supervisor.probe();
    let started = Instant::now();
    drop(worker);
    assert!(
        profile.join("shutdown-accepted").exists(),
        "fixture received shutdown"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "Drop stayed within deadline and cleanup"
    );
    assert_process_gone(pid);
    assert_eq!(
        probe.active_processes().expect("query dropped stalled job"),
        0
    );
    eprintln!("DROP_STALLED_SHUTDOWN=child-reaped within-deadline");
    eprintln!("JOB_ACTIVE_PROCESSES=0 scenario=drop-stalled-shutdown");
}

#[test]
fn persistent_query_failure_quarantines_profile_and_closes_owned_job() {
    let _serial = TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fixture = fixture_bin();
    unsafe { std::env::set_var("ELY_LOK_WORKER", &fixture) };
    let root = PathBuf::from(
        std::env::var_os("ELY_LOK_SUPERVISION_TEST_ROOT")
            .expect("controlled test root must point to the persistent D task directory"),
    );
    fs::create_dir_all(&root).expect("persistent D test root");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let profile = root.join(format!("query-error-{}-{unique}", std::process::id()));
    fs::create_dir_all(&profile).expect("persistent profile");
    let install = root.join("install");
    fs::create_dir_all(&install).expect("install directory");
    let mut worker = Worker::launch(&install, &profile).expect("launch fake worker");
    worker.ready().expect("fake worker readiness");
    let pid = worker.child.id();

    unsafe { std::env::set_var("ELY_LOK_TEST_QUERY_ERROR", "1") };
    let started = Instant::now();
    let error = worker
        .shutdown()
        .expect_err("persistent query failure must fail cleanup");
    unsafe { std::env::remove_var("ELY_LOK_TEST_QUERY_ERROR") };
    let elapsed = started.elapsed();
    assert!(
        error
            .to_string()
            .contains("ActiveProcesses==0 was not confirmed"),
        "{error}"
    );
    assert!(
        error.to_string().contains("profile quarantined at"),
        "{error}"
    );
    assert!(error.to_string().contains("monitor joined"), "{error}");
    assert!(error.to_string().contains("child_reaped=true"), "{error}");
    assert!(
        elapsed < Duration::from_secs(6),
        "query error cleanup exceeded the bounded child reap: {elapsed:?}"
    );
    let marker = profile.join(".ely-lok-worker-quarantine-v1");
    assert!(marker.is_file(), "persistent profile marker must remain");
    assert!(
        profile.is_dir(),
        "the persistent profile must not be removed"
    );
    assert!(
        worker.supervisor.monitor_joined(),
        "monitor thread was joined"
    );
    eprintln!(
        "QUERY_ERROR_CLEANUP_RETURNED={elapsed:?} result=Err job0=unconfirmed monitor=joined marker=persisted"
    );

    crate::client::supervise::forget_profile_for_test(&profile);
    let reuse = match Worker::launch(&install, &profile) {
        Err(error) => error,
        Ok(_) => panic!("quarantine marker must survive process-local registry reset"),
    };
    assert_eq!(reuse.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(
        profile.is_dir() && marker.is_file(),
        "blocked reuse preserves the quarantined profile"
    );
    drop(worker);
    assert_process_gone(pid);
    eprintln!("QUARANTINE_REUSE=refused marker=persisted profile=preserved child_pid_gone=true");
}

#[test]
fn fatal_child_eof_breaks_request_reaps_job_and_preserves_profile() {
    let _serial = TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fixture = fixture_bin();
    unsafe { std::env::set_var("ELY_LOK_WORKER", &fixture) };
    let tmp = tempdir().expect("temp root");
    let install = tmp.path().join("install");
    let profile = tmp.path().join("fatal-eof");
    fs::create_dir_all(&install).expect("install directory");
    fs::create_dir_all(&profile).expect("profile directory");
    fs::write(profile.join("mode.txt"), "fatal-second").expect("fatal mode");

    let mut worker = Worker::launch(&install, &profile).expect("launch gated fake worker");
    worker.request_timeout = Duration::from_secs(5);
    worker
        .ready()
        .expect("worker readiness completes after Job assignment");
    let pid = worker.child.id();
    let process_handle = worker.child.as_raw_handle() as HANDLE;
    let process_identity = |handle: HANDLE| {
        let handle_pid = unsafe { GetProcessId(handle) };
        assert_ne!(handle_pid, 0, "read the owned Child handle PID");
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
            unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) },
            0,
            "read process creation time from the owned Child handle"
        );
        let created_at = ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;
        assert_ne!(created_at, 0, "recorded process creation time");
        (handle_pid, created_at)
    };
    let identity_before = process_identity(process_handle);
    assert_eq!(
        identity_before.0, pid,
        "the Child handle belongs to its PID"
    );
    assert_eq!(
        fs::read_to_string(profile.join("prebootstrap.pid")).expect("prebootstrap child PID"),
        pid.to_string(),
        "the observed process is the gated child assigned to this Worker"
    );
    assert!(
        profile.join("bootstrap.ok").is_file(),
        "child passed assignment gate"
    );
    let probe = worker.supervisor.probe();
    let job_pids = probe.process_ids().expect("query assigned Job members");
    let active = probe.active_processes().expect("query assigned Job");
    let member_images = job_pids
        .iter()
        .map(|member_pid| {
            let member = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, *member_pid) };
            assert!(!member.is_null(), "open only Job member PID {member_pid}");
            let mut path = [0u16; 32768];
            let mut length = path.len() as u32;
            let queried = unsafe {
                QueryFullProcessImageNameW(member, 0, path.as_mut_ptr(), &mut length)
            };
            let query_error = if queried == 0 {
                Some(std::io::Error::last_os_error())
            } else {
                None
            };
            unsafe { CloseHandle(member) };
            if let Some(error) = query_error {
                panic!("query image for owned Job member PID {member_pid}: {error}");
            }
            format!("{member_pid}={}", String::from_utf16_lossy(&path[..length as usize]))
        })
        .collect::<Vec<_>>();
    eprintln!(
        "EOF_JOB_INITIAL active={active} pids={job_pids:?} images={member_images:?} tracked_worker_pid={pid}"
    );
    assert!(active >= 1, "assigned Job has at least one process");
    assert!(job_pids.contains(&pid), "tracked worker PID is in assigned Job");

    let shared = Arc::new(Mutex::new(worker));
    let (signal, entered) = mpsc::channel();
    let request_worker = Arc::clone(&shared);
    let request = thread::spawn(move || {
        let mut worker = request_worker.lock().expect("worker mutex");
        signal.send(()).expect("signal request entry");
        worker.build_id()
    });
    entered.recv().expect("request caller entered");
    wait_file(&profile.join("accepted-2"));
    let started = Instant::now();
    let result = request.join().expect("fatal request caller joins");
    let elapsed = started.elapsed();
    let error = result.expect_err("EOF before reply fails the active request");
    assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof, "{error}");
    assert!(
        elapsed < Duration::from_secs(3),
        "EOF returns promptly: {elapsed:?}"
    );
    assert!(
        !profile.join("accepted-3").exists(),
        "client did not retry the request"
    );
    assert_eq!(
        fs::read_to_string(profile.join("accepted-2")).expect("accepted request marker"),
        "yes",
        "fatal child exited after accepting the request"
    );

    let mut worker = shared.lock().expect("worker mutex after EOF");
    let exit_status = worker.ended().expect("the owned child has exited");
    assert!(
        exit_status.contains("exit code: 1"),
        "the fixture takes the fatal exit(1) path: {exit_status}"
    );
    let cleanup = worker.shutdown();
    assert!(cleanup.is_err(), "shutdown reports fatal worker exit");
    assert_eq!(
        worker
            .supervisor
            .active_processes()
            .expect("query fatal Job"),
        0,
        "EOF cleanup reaped the owned Job"
    );
    assert!(
        profile.is_dir(),
        "normal fatal cleanup preserves the profile"
    );
    assert!(
        !profile.join(".ely-lok-worker-quarantine-v1").exists(),
        "confirmed Job zero does not quarantine the profile"
    );
    assert!(
        !profile.join(".~lock.").exists(),
        "fake worker created no profile lock"
    );
    assert_eq!(
        unsafe { WaitForSingleObject(process_handle, 0) },
        WAIT_OBJECT_0,
        "the same owned process handle is signaled after shutdown reaps the child"
    );
    let mut exit_code = 0;
    assert_ne!(
        unsafe { GetExitCodeProcess(process_handle, &mut exit_code) },
        0,
        "read exit code from the owned Child handle"
    );
    assert_eq!(exit_code, 1, "fatal child exited with code one");
    assert_ne!(exit_code, 259, "the owned process is not STILL_ACTIVE");
    let identity_after = process_identity(process_handle);
    assert_eq!(identity_after.0, pid, "same Child handle PID after reap");
    assert_eq!(
        identity_after.1, identity_before.1,
        "same process creation identity"
    );
    drop(worker);
    drop(shared);
    drop(probe);
    eprintln!("FATAL_CHILD_EOF=unexpected-eof no-retry job0 child-reaped elapsed={elapsed:?}");
}
fn fixture_bin() -> PathBuf {
    std::env::current_exe()
        .expect("test executable")
        .parent()
        .and_then(|path| path.parent())
        .expect("target debug directory")
        .join("ely-lok-worker-fixture.exe")
}

fn wait_file(path: &std::path::Path) {
    let until = Instant::now() + Duration::from_secs(3);
    while Instant::now() < until {
        if path.is_file() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("worker did not create {}", path.display());
}

#[cfg(windows)]
fn assert_process_gone(pid: u32) {
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if !handle.is_null() {
        let signaled = unsafe { WaitForSingleObject(handle, 0) } == WAIT_OBJECT_0;
        let mut exit_code = 259u32;
        let readable = unsafe { GetExitCodeProcess(handle, &mut exit_code) } != 0;
        unsafe { CloseHandle(handle) };
        assert!(signaled && readable && exit_code != 259, "process {pid} is still active after bounded reap");
    }
}
