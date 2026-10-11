//! Windows LibreOfficeKit owner-thread dispatch primitives.
//!
//! Windows LibreOfficeKit owner-thread dispatch and lifetime state.

use std::{
    any::Any,
    cell::Cell,
    ffi::c_void,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub(super) type OwnerTask = unsafe extern "C" fn(*mut c_void);
pub(super) type OwnerDone = unsafe extern "C" fn(*mut c_void, u32);
pub(super) type IsCurrent = unsafe extern "C" fn(*mut c_void) -> u32;
pub(super) type Post =
    unsafe extern "C" fn(*mut c_void, OwnerTask, *mut c_void, OwnerDone, *mut c_void) -> u32;
pub(super) type BufferFree = unsafe extern "C" fn(*mut c_void);
pub(super) type KitDestroy = unsafe extern "C" fn(*mut c_void);

#[derive(Clone, Copy)]
pub(super) struct NativeApi {
    pub(super) is_current: IsCurrent,
    pub(super) post: Post,
    pub(super) buffer_free: BufferFree,
    pub(super) destroy: KitDestroy,
}

thread_local! {
    static CURRENT_OWNER: Cell<usize> = const { Cell::new(0) };
}

static PROCESS_PINS: OnceLock<Mutex<Vec<Arc<Inner>>>> = OnceLock::new();
#[cfg(test)]
static PANIC_DONE: AtomicBool = AtomicBool::new(false);

/// Dispatch handle for one native kit. Construct only after all three exports
/// have been resolved from the same loaded module.
#[derive(Clone)]
pub(super) struct OwnerState {
    inner: Arc<Inner>,
}

struct Inner {
    kit: std::sync::atomic::AtomicPtr<c_void>,
    api: NativeApi,
    // Retained for process lifetime. A future explicit release needs a proven
    // native quiet barrier, which the current ABI does not provide.
    _library_pin: Arc<dyn Any + Send + Sync>,
    deadline: Duration,
    destroyed: AtomicBool,
    quarantined: AtomicBool,
    admission: Mutex<Admission>,
    drained: Condvar,
}

unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

#[derive(Default)]
struct Admission {
    closed: bool,
    pending: usize,
    submitting: usize,
    accepted_not_done: usize,
}

struct CallCompletion {
    state: Mutex<CompletionState>,
    ready: Condvar,
}

#[derive(Default)]
struct CompletionState {
    result: Option<Result<Box<dyn Any + Send>, OwnerError>>,
    done_status: Option<u32>,
}

struct TaskContext {
    owner: Arc<Inner>,
    completion: Arc<CallCompletion>,
    task_entered: Arc<AtomicBool>,
    task: Mutex<Option<Box<dyn FnOnce() -> Box<dyn Any + Send> + Send>>>,
}

struct DoneContext {
    owner: Arc<Inner>,
    completion: Arc<CallCompletion>,
    task_raw: *const TaskContext,
    task_entered: Arc<AtomicBool>,
}

unsafe impl Send for DoneContext {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OwnerError {
    Closed,
    Rejected,
    TimedOut,
    TaskPanicked,
    BadResultType,
    NativeDidNotEnterTask,
}

impl fmt::Display for OwnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Closed => "LOK owner admission is closed",
            Self::Rejected => "LOK owner rejected the task",
            Self::TimedOut => "LOK owner task timed out; owner is quarantined",
            Self::TaskPanicked => "LOK owner task panicked",
            Self::BadResultType => "LOK owner returned a mismatched result type",
            Self::NativeDidNotEnterTask => "LOK owner event did not enter its task",
        })
    }
}

impl std::error::Error for OwnerError {}

impl OwnerState {
    /// Creates owner dispatch for one live LOK kit.
    ///
    /// # Safety
    ///
    /// `kit` must remain valid and its native kit must not be destroyed while
    /// any accepted task may still run. The caller must keep the owning `Lok`
    /// alive until all native tasks are quiet, or keep the whole worker inside
    /// its supervised process until termination and reap. In particular, a
    /// timeout does not make dropping `Lok` safe: the process-lifetime pin in
    /// this module retains the API and library, but cannot retain or prevent
    /// `Lok::drop` from destroying the native kit. The native `is_current` and
    /// `post` exports may block; their hard deadline is provided only by the
    /// external worker supervisor.
    pub(super) unsafe fn new(
        kit: *mut c_void,
        api: NativeApi,
        library_pin: Arc<dyn Any + Send + Sync>,
        deadline: Duration,
    ) -> Self {
        let inner = Arc::new(Inner {
            kit: std::sync::atomic::AtomicPtr::new(kit),
            api,
            _library_pin: library_pin,
            deadline,
            destroyed: AtomicBool::new(false),
            quarantined: AtomicBool::new(false),
            admission: Mutex::new(Admission::default()),
            drained: Condvar::new(),
        });
        Self { inner }
    }

    /// Runs a closure on this kit's native owner. Callers must capture only
    /// owned data whose destructor is safe on either the owner or host thread.
    pub(super) fn invoke<T: Any + Send>(
        &self,
        task: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, OwnerError> {
        self.invoke_with(task, false)
    }

    pub(super) fn invoke_internal<T: Any + Send>(
        &self,
        task: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, OwnerError> {
        self.invoke_with(task, true)
    }

    fn invoke_with<T: Any + Send>(
        &self,
        task: impl FnOnce() -> T + Send + 'static,
        internal: bool,
    ) -> Result<T, OwnerError> {
        self.admit(internal)?;
        let deadline = Instant::now() + self.inner.deadline;
        if is_current_owner(&self.inner) {
            let result = run_on_owner(&self.inner, task);
            self.finish_inline();
            return result;
        }

        let completion = Arc::new(CallCompletion {
            state: Mutex::new(CompletionState::default()),
            ready: Condvar::new(),
        });
        let task_entered = Arc::new(AtomicBool::new(false));
        let task_context = Arc::new(TaskContext {
            owner: Arc::clone(&self.inner),
            completion: Arc::clone(&completion),
            task_entered: Arc::clone(&task_entered),
            task: Mutex::new(Some(Box::new(move || {
                Box::new(task()) as Box<dyn Any + Send>
            }))),
        });
        let task_raw = Arc::into_raw(task_context);
        let done_context = Arc::new(DoneContext {
            owner: Arc::clone(&self.inner),
            completion: Arc::clone(&completion),
            task_raw,
            task_entered: Arc::clone(&task_entered),
        });
        let done_raw = Arc::into_raw(done_context);

        // Reserve the accepted slot before crossing the native boundary. A
        // callback may run before post returns, so this is deliberately early.
        {
            let mut admission = lock(&self.inner.admission);
            admission.submitting += 1;
            admission.accepted_not_done += 1;
        }
        let accepted = unsafe {
            (self.inner.api.post)(
                self.inner.kit.load(Ordering::Acquire),
                task_trampoline,
                task_raw.cast_mut().cast(),
                done_trampoline,
                done_raw.cast_mut().cast(),
            )
        };
        {
            let mut admission = lock(&self.inner.admission);
            admission.submitting -= 1;
            if accepted == 0 {
                admission.accepted_not_done -= 1;
                admission.pending -= 1;
                self.inner.drained.notify_all();
            }
        }
        if accepted == 0 {
            unsafe {
                drop(Arc::from_raw(task_raw));
                drop(Arc::from_raw(done_raw));
            }
            return Err(OwnerError::Rejected);
        }

        let mut state = lock(&completion.state);
        loop {
            if let Some(status) = state.done_status {
                if status == 0 {
                    return Err(OwnerError::NativeDidNotEnterTask);
                }
                if let Some(result) = state.result.take() {
                    return result?
                        .downcast::<T>()
                        .map(|value| *value)
                        .map_err(|_| OwnerError::BadResultType);
                }
            }
            let now = Instant::now();
            if now >= deadline {
                drop(state);
                self.fail_closed();
                return Err(OwnerError::TimedOut);
            }
            let (next, timeout) = completion
                .ready
                .wait_timeout(state, deadline.saturating_duration_since(now))
                .unwrap_or_else(|poison| poison.into_inner());
            state = next;
            if timeout.timed_out() && state.done_status.is_none() {
                drop(state);
                self.fail_closed();
                return Err(OwnerError::TimedOut);
            }
        }
    }

    pub(super) fn close_admission(&self, timeout: Duration) -> Result<(), OwnerError> {
        // A graceful shutdown closes ordinary admission without quarantining
        // the kit. A drain timeout below is the fail-closed condition.
        lock(&self.inner.admission).closed = true;
        let deadline = Instant::now() + timeout;
        let mut admission = lock(&self.inner.admission);
        loop {
            if admission.pending == 0 {
                return Ok(());
            }
            let now = Instant::now();
            if now >= deadline {
                drop(admission);
                self.quarantine();
                return Err(OwnerError::TimedOut);
            }
            let (next, timed) = self
                .inner
                .drained
                .wait_timeout(admission, deadline.saturating_duration_since(now))
                .unwrap_or_else(|poison| poison.into_inner());
            admission = next;
            if timed.timed_out() && admission.pending != 0 {
                drop(admission);
                self.quarantine();
                return Err(OwnerError::TimedOut);
            }
        }
    }

    pub(super) fn wait_drained(&self, timeout: Duration) -> Result<(), OwnerError> {
        let deadline = Instant::now() + timeout;
        let mut admission = lock(&self.inner.admission);
        loop {
            if admission.pending == 0
                && admission.submitting == 0
                && admission.accepted_not_done == 0
            {
                return Ok(());
            }
            let now = Instant::now();
            if now >= deadline {
                drop(admission);
                self.quarantine();
                return Err(OwnerError::TimedOut);
            }
            let (next, timed) = self
                .inner
                .drained
                .wait_timeout(admission, deadline.saturating_duration_since(now))
                .unwrap_or_else(|poison| poison.into_inner());
            admission = next;
            if timed.timed_out()
                && (admission.pending != 0
                    || admission.submitting != 0
                    || admission.accepted_not_done != 0)
            {
                drop(admission);
                self.quarantine();
                return Err(OwnerError::TimedOut);
            }
        }
    }

    pub(super) fn native_kit(&self) -> *mut c_void {
        self.inner.kit.load(Ordering::Acquire)
    }

    pub(super) fn on_owner_thread(&self) -> bool {
        CURRENT_OWNER.with(|current| current.get() == Arc::as_ptr(&self.inner) as usize)
    }

    pub(super) fn is_destroyed(&self) -> bool {
        self.inner.destroyed.load(Ordering::Acquire)
    }

    pub(super) fn is_quarantined(&self) -> bool {
        self.inner.quarantined.load(Ordering::Acquire)
    }

    /// Destroys the kit after callback/document disposal and successful
    /// non-owner joinThreads. A return from this call is the quiet boundary.
    pub(super) fn destroy_after_join(&self) -> Result<(), OwnerError> {
        if CURRENT_OWNER.with(|current| current.get() == Arc::as_ptr(&self.inner) as usize) {
            return Err(OwnerError::Closed);
        }
        self.close_admission(self.inner.deadline)?;
        self.wait_drained(self.inner.deadline)?;
        let kit = self.native_kit();
        if kit.is_null() {
            return if self.is_destroyed() {
                Ok(())
            } else {
                Err(OwnerError::Closed)
            };
        }
        unsafe { (self.inner.api.destroy)(kit) };
        self.inner.kit.store(ptr::null_mut(), Ordering::Release);
        self.inner.destroyed.store(true, Ordering::Release);
        Ok(())
    }

    pub(super) fn quarantine(&self) {
        self.inner.quarantined.store(true, Ordering::Release);
        lock(&self.inner.admission).closed = true;
        let mut pins = PROCESS_PINS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if !pins.iter().any(|pin| Arc::ptr_eq(pin, &self.inner)) {
            pins.push(Arc::clone(&self.inner));
        }
    }

    fn admit(&self, internal: bool) -> Result<(), OwnerError> {
        let mut admission = lock(&self.inner.admission);
        if self.is_quarantined() {
            return Err(OwnerError::Closed);
        }
        if admission.closed && !internal {
            return Err(OwnerError::Closed);
        }
        if self.is_destroyed() {
            return Err(OwnerError::Closed);
        }
        admission.pending += 1;
        Ok(())
    }

    fn finish_inline(&self) {
        let mut admission = lock(&self.inner.admission);
        admission.pending -= 1;
        self.inner.drained.notify_all();
    }

    fn fail_closed(&self) {
        self.quarantine();
    }

    #[cfg(test)]
    fn counts(&self) -> (bool, usize, usize, usize) {
        let admission = lock(&self.inner.admission);
        (
            admission.closed,
            admission.pending,
            admission.submitting,
            admission.accepted_not_done,
        )
    }
}

fn is_current_owner(inner: &Inner) -> bool {
    let identity = inner as *const Inner as usize;
    if CURRENT_OWNER.with(|current| current.get() == identity) {
        return true;
    }
    unsafe { (inner.api.is_current)(inner.kit.load(Ordering::Acquire)) != 0 }
}

fn run_on_owner<T: Any + Send>(inner: &Inner, task: impl FnOnce() -> T) -> Result<T, OwnerError> {
    let identity = inner as *const Inner as usize;
    let previous = CURRENT_OWNER.with(|current| current.replace(identity));
    let result = catch_unwind(AssertUnwindSafe(task)).map_err(|_| OwnerError::TaskPanicked);
    CURRENT_OWNER.with(|current| current.set(previous));
    result
}

unsafe extern "C" fn task_trampoline(context: *mut c_void) {
    let context = unsafe { Arc::from_raw(context.cast::<TaskContext>()) };
    context.task_entered.store(true, Ordering::Release);
    let identity = Arc::as_ptr(&context.owner) as usize;
    let previous = CURRENT_OWNER.with(|current| current.replace(identity));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let task = lock(&context.task).take().expect("owner task runs once");
        task()
    }))
    .map_err(|_| OwnerError::TaskPanicked);
    CURRENT_OWNER.with(|current| current.set(previous));
    {
        let mut state = lock(&context.completion.state);
        state.result = Some(result);
    }
    context.completion.ready.notify_all();
}

unsafe extern "C" fn done_trampoline(context: *mut c_void, status: u32) {
    let context = unsafe { Arc::from_raw(context.cast::<DoneContext>()) };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if status == 0 && !context.task_entered.load(Ordering::Acquire) {
            unsafe { drop(Arc::from_raw(context.task_raw)) };
        }
        {
            let mut state = lock(&context.completion.state);
            state.done_status = Some(status);
        }
        {
            let mut admission = lock(&context.owner.admission);
            admission.accepted_not_done -= 1;
            admission.pending -= 1;
            context.owner.drained.notify_all();
        }
        context.completion.ready.notify_all();
        #[cfg(test)]
        if PANIC_DONE.swap(false, Ordering::SeqCst) {
            panic!("fake done panic");
        }
    }));
}

/// Copies and frees a malloc-family engine buffer while on its owner thread.
pub(super) unsafe fn copy_and_free(
    owner: &OwnerState,
    buffer: *mut c_void,
    length: usize,
) -> Option<Vec<u8>> {
    if buffer.is_null() {
        return None;
    }
    assert!(
        is_current_owner(&owner.inner),
        "native buffer copied off owner"
    );
    struct FreeOnDrop {
        ptr: *mut c_void,
        free: BufferFree,
    }
    impl Drop for FreeOnDrop {
        fn drop(&mut self) {
            unsafe { (self.free)(self.ptr) };
        }
    }
    let _free = FreeOnDrop {
        ptr: buffer,
        free: owner.inner.api.buffer_free,
    };
    Some(unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), length) }.to_vec())
}

pub(super) unsafe fn copy_c_string_and_free(
    owner: &OwnerState,
    buffer: *mut c_void,
) -> Option<String> {
    if buffer.is_null() {
        return None;
    }
    assert!(
        is_current_owner(&owner.inner),
        "native buffer copied off owner"
    );
    struct FreeOnDrop {
        ptr: *mut c_void,
        free: BufferFree,
    }
    impl Drop for FreeOnDrop {
        fn drop(&mut self) {
            unsafe { (self.free)(self.ptr) };
        }
    }
    let _free = FreeOnDrop {
        ptr: buffer,
        free: owner.inner.api.buffer_free,
    };
    let text = unsafe { std::ffi::CStr::from_ptr(buffer.cast()) }
        .to_string_lossy()
        .into_owned();
    Some(text)
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_LOCK: Mutex<()> = Mutex::new(());
    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
    static POST_COUNT: AtomicUsize = AtomicUsize::new(0);
    static MODE: Mutex<Mode> = Mutex::new(Mode::Reject);
    static SAVED: Mutex<Option<SavedCallbacks>> = Mutex::new(None);

    #[derive(Clone, Copy)]
    enum Mode {
        Reject,
        Early,
        Hold,
        PanicDone,
    }

    #[derive(Clone, Copy)]
    struct SavedCallbacks {
        task: OwnerTask,
        task_context: *mut c_void,
        done: OwnerDone,
        done_context: *mut c_void,
    }

    unsafe impl Send for SavedCallbacks {}

    unsafe extern "C" fn fake_is_current(_: *mut c_void) -> u32 {
        0
    }

    unsafe extern "C" fn fake_post(
        _: *mut c_void,
        task: OwnerTask,
        task_context: *mut c_void,
        done: OwnerDone,
        done_context: *mut c_void,
    ) -> u32 {
        POST_COUNT.fetch_add(1, Ordering::SeqCst);
        let mode = *lock(&MODE);
        match mode {
            Mode::Reject => 0,
            Mode::Early | Mode::PanicDone => {
                unsafe { task(task_context) };
                if matches!(mode, Mode::PanicDone) {
                    PANIC_DONE.store(true, Ordering::SeqCst);
                }
                unsafe { done(done_context, 1) };
                1
            }
            Mode::Hold => {
                *lock(&SAVED) = Some(SavedCallbacks {
                    task,
                    task_context,
                    done,
                    done_context,
                });
                1
            }
        }
    }

    unsafe extern "C" fn fake_free(ptr: *mut c_void) {
        unsafe { drop(Box::from_raw(ptr.cast::<u8>())) };
    }

    unsafe extern "C" fn fake_destroy(_: *mut c_void) {}

    fn owner(deadline: Duration) -> OwnerState {
        unsafe {
            OwnerState::new(
                ptr::null_mut(),
                NativeApi {
                    is_current: fake_is_current,
                    post: fake_post,
                    buffer_free: fake_free,
                    destroy: fake_destroy,
                },
                Arc::new(()),
                deadline,
            )
        }
    }

    struct TrackedDrop;
    impl Drop for TrackedDrop {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn release_saved() {
        if let Some(saved) = lock(&SAVED).take() {
            unsafe { (saved.task)(saved.task_context) };
            unsafe { (saved.done)(saved.done_context, 1) };
        }
    }

    #[test]
    fn rejected_post_reclaims_both_contexts_without_callbacks() {
        let _serial = lock(&TEST_LOCK);
        *lock(&MODE) = Mode::Reject;
        DROP_COUNT.store(0, Ordering::SeqCst);
        let owner = owner(Duration::from_millis(100));
        let tracked = TrackedDrop;
        let result = owner.invoke(move || {
            std::hint::black_box(&tracked);
            7usize
        });
        assert_eq!(result, Err(OwnerError::Rejected));
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1);
        assert_eq!(owner.counts(), (false, 0, 0, 0));
    }

    #[test]
    fn accepted_callbacks_may_complete_before_post_returns() {
        let _serial = lock(&TEST_LOCK);
        *lock(&MODE) = Mode::Early;
        let owner = owner(Duration::from_millis(100));
        assert_eq!(owner.invoke(|| 41u32), Ok(41));
        assert_eq!(owner.counts(), (false, 0, 0, 0));
    }

    #[test]
    fn graceful_admission_close_does_not_quarantine() {
        let _serial = lock(&TEST_LOCK);
        *lock(&MODE) = Mode::Early;
        let owner = owner(Duration::from_millis(100));
        assert_eq!(owner.close_admission(Duration::from_millis(100)), Ok(()));
        assert_eq!(owner.counts(), (true, 0, 0, 0));
        assert!(!owner.is_quarantined());
        assert_eq!(owner.invoke(|| 1usize), Err(OwnerError::Closed));
        assert_eq!(owner.invoke_internal(|| 2usize), Ok(2));
    }

    #[test]
    fn quarantine_rejects_internal_native_posts() {
        let _serial = lock(&TEST_LOCK);
        *lock(&MODE) = Mode::Early;
        let owner = owner(Duration::from_millis(100));
        owner.quarantine();
        let before = POST_COUNT.load(Ordering::SeqCst);
        assert_eq!(owner.invoke_internal(|| 3usize), Err(OwnerError::Closed));
        assert_eq!(POST_COUNT.load(Ordering::SeqCst), before);
    }

    #[test]
    fn timeout_closes_admission_and_late_callbacks_release_contexts() {
        let _serial = lock(&TEST_LOCK);
        *lock(&MODE) = Mode::Hold;
        DROP_COUNT.store(0, Ordering::SeqCst);
        let owner = owner(Duration::from_millis(10));
        let tracked = TrackedDrop;
        let result = owner.invoke(move || {
            std::hint::black_box(&tracked);
            9usize
        });
        assert_eq!(result, Err(OwnerError::TimedOut));
        assert!(owner.is_quarantined());
        assert_eq!(owner.counts(), (true, 1, 0, 1));
        assert_eq!(owner.invoke(|| 1usize), Err(OwnerError::Closed));
        release_saved();
        assert_eq!(owner.counts(), (true, 0, 0, 0));
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn task_panic_is_contained_and_reported() {
        let _serial = lock(&TEST_LOCK);
        *lock(&MODE) = Mode::Early;
        let owner = owner(Duration::from_millis(100));
        let result = owner.invoke(|| -> usize { panic!("fake task panic") });
        assert_eq!(result, Err(OwnerError::TaskPanicked));
    }

    #[test]
    fn done_panic_is_contained_at_ffi_boundary() {
        let _serial = lock(&TEST_LOCK);
        *lock(&MODE) = Mode::PanicDone;
        let owner = owner(Duration::from_millis(100));
        // The done trampoline catches its own unwind; the task's result stays
        // available to the host even if cleanup code is interrupted.
        assert_eq!(owner.invoke(|| 13usize), Ok(13));
    }
}
