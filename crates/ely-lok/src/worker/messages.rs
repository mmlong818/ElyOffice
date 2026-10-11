//! Windows: the LOK loop pumps native work while one reader answers requests.

use std::{
    ffi::c_void,
    io,
    sync::atomic::{AtomicUsize, Ordering},
};

use super::{answer, events, lok, password, pin, quit, teardown};
use crate::ffi::Lok;
use crate::ipc;

/// Requests read and not yet answered.
static PENDING: AtomicUsize = AtomicUsize::new(0);

pub(super) fn serve(lok: Lok, input: impl io::Read + Send + 'static) -> io::Result<()> {
    let lok = Box::leak(Box::new(lok));
    pin(lok);
    if let Err(error) = lok.register_callback(password::on_kit) {
        lok.quarantine();
        return Err(error);
    }
    lok.register_any_input(waiting);
    let requests = std::thread::spawn(move || reader(input));
    events::start();
    // Unipoll needs hooks; the Windows loop never calls them.
    extern "C" fn poll(_: *mut c_void, _: i32) -> i32 {
        0
    }
    extern "C" fn wake(_: *mut c_void) {}
    lok.run_loop(Some(poll), Some(wake));
    log::info!("lok-worker: the kernel's loop ended");

    // A normal shutdown destroys the kit on the request thread, then exits
    // there after its reply is written. Wait for that sole cleanup path.
    if !super::claim_teardown() {
        if requests.join().is_err() {
            log::error!("lok-worker: request thread panicked during teardown");
        }
        quit(1);
    }

    // The native loop ended before an orderly request-side shutdown. Its
    // owner can no longer be assumed to service teardown tasks, so fail
    // closed and let the parent-owned Job reap this worker.
    lok.quarantine();
    log::error!("lok-worker: the kernel's loop ended before teardown began");
    quit(1)
}

fn reader(mut input: impl io::Read) {
    loop {
        match ipc::read_frame(&mut input) {
            Ok(Some((tag, id, payload))) => {
                PENDING.fetch_add(1, Ordering::SeqCst);
                let answered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    answer(lok(), tag, id, &payload);
                }));
                PENDING.fetch_sub(1, Ordering::SeqCst);
                if answered.is_err() {
                    log::error!("lok-worker: request handler panicked");
                    quit(1);
                }
                if super::TEARDOWN_STARTED.load(Ordering::SeqCst) {
                    return;
                }
            }
            Ok(None) => {
                log::info!("lok-worker: stdin closed");
                if super::claim_teardown() {
                    teardown::teardown(0);
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

/// Idle jobs yield to a waiting request, including while its owner task runs.
extern "C" fn waiting(_: *mut c_void, _: i32) -> bool {
    PENDING.load(Ordering::SeqCst) > 0
}
