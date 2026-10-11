//! Answers the kernel's password question during a load.

use std::{
    ffi::{CStr, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// LOK_CALLBACK_DOCUMENT_PASSWORD.
const PASSWORD: i32 = 20;

/// The password for the load in progress, given once.
static GIVEN: Mutex<Option<String>> = Mutex::new(None);
/// The kernel asked and got none.
static REFUSED: AtomicBool = AtomicBool::new(false);

/// Readies `password` for the next load.
pub(super) fn arm(password: Option<String>) {
    *GIVEN.lock().expect("the password lock") = password;
    REFUSED.store(false, Ordering::SeqCst);
}

/// Whether the last load lacked a right password.
pub(super) fn refused() -> bool {
    REFUSED.load(Ordering::SeqCst)
}

/// Answers once; a second ask means the password failed.
pub(super) unsafe extern "C" fn on_kit(kind: i32, payload: *const c_char, _: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if kind != PASSWORD || payload.is_null() {
            return;
        }
        let url = unsafe { CStr::from_ptr(payload) }
            .to_string_lossy()
            .into_owned();
        let given = GIVEN.lock().unwrap_or_else(|p| p.into_inner()).take();
        log::info!(
            "lok-worker: a password asked, one given: {}",
            given.is_some()
        );
        REFUSED.store(given.is_none(), Ordering::SeqCst);
        super::lok().set_password(&url, given.as_deref());
    }));
}
