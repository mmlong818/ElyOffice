//! Document events: tiles, cursor, state, dialogs, a11y (LOK-03..08).

use std::{
    collections::{HashMap, HashSet, VecDeque},
    ffi::{CStr, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Condvar, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use crate::ffi::Document;

mod cell;
mod parse;
pub use cell::{Cell, Function, Mark, Request};
use cell::{functions_of, marks_of, request_of, usage_of};
pub use parse::A11y;
use parse::{
    Dialog, Unreadable, a11y_update, dialog_of, fonts_of, formula_text, found_of, rect_of,
    result_of, state_of, tabs_of,
};

/// LOK_CALLBACK kinds Ely reads.
const INVALIDATE_TILES: i32 = 0;
const CURSOR: i32 = 1;
const SELECTION: i32 = 2;
const CURSOR_VISIBLE: i32 = 5;
const GRAPHIC: i32 = 6;
const STATE: i32 = 8;
const RULER: i32 = 35;
const WINDOW: i32 = 36;
const JSDIALOG: i32 = 46;
const RESULT: i32 = 16;
const A11Y_FOCUS: i32 = 62;
const A11Y_CARET: i32 = 63;
const A11Y_SELECTION: i32 = 64;
const A11Y_CELL: i32 = 67;
const A11Y_EDITING: i32 = 68;
const A11Y_OBJECT: i32 = 69;
const REDLINES: i32 = 30;
const REDLINE: i32 = 31;
const COMMENT: i32 = 32;
const NOT_FOUND: i32 = 12;
const FOUND: i32 = 15;
const MENU: i32 = 23;
const FONTS: i32 = 57;
const SIZE_CHANGED: i32 = 13;
const SET_PART: i32 = 14;
const CELL_CURSOR: i32 = 17;
const CELL_FORMULA: i32 = 19;
const HEADER: i32 = 33;
const GEOMETRY: i32 = 50;
const CELL_ADDRESS: i32 = 34;
const REFERENCE_MARKS: i32 = 45;
const FUNCTION_LIST: i32 = 47;
const TOOLTIP: i32 = 71;

/// A mixed state; kernel values never hold NUL.
pub const MIXED: &str = "\u{0}mixed";

/// A rectangle in twips: x, y, width, height.
pub type Rect = (i32, i32, i32, i32);

/// One drained batch of document events.
#[derive(Clone, Debug, Default)]
pub struct Events {
    /// Bumps on every event below.
    pub seq: u64,
    /// The document's revision when the batch was sent.
    pub revision: u64,
    /// Bumps on tile invalidations only.
    pub generation: u64,
    pub all: bool,
    pub rects: Vec<Rect>,
    pub cursor: Option<Rect>,
    pub cursor_visible: bool,
    pub selection: Vec<Rect>,
    /// The selected object's frame, if any.
    pub graphic: Option<Rect>,
    /// The selected image is in crop mode.
    pub cropping: bool,
    /// Bumps on comment and tracked-change events.
    pub review: u64,
    /// Bumps on each search outcome.
    pub searched: u64,
    /// Bumps when the document's size changes.
    pub resized: u64,
    /// The last search's matches; empty when none.
    pub found: Vec<Rect>,
    /// A context menu the kernel offered, as JSON.
    pub menu: Option<String>,
    /// Kernel messages Ely could not read.
    pub faults: Vec<String>,
    /// Font families the document names and the system lacks.
    pub fonts: Vec<String>,
    /// Kernel states changed since the last drain (LOK-05).
    pub states: Vec<(String, String)>,
    /// Dialogs the kernel opened and Ely closed (LOK-08).
    pub blocked: Vec<String>,
    pub a11y: A11y,
    /// The paragraph's own tab stops, in 1/100 mm.
    pub tabs: Vec<i32>,
    pub cell: Cell,
}

#[derive(Default)]
struct Slot {
    batch: Events,
    closed: HashSet<u32>,
    titles: HashMap<u32, String>,
    /// Every state the kernel reported, for the worker's checks.
    known: HashMap<String, String>,
    /// Monotonic sequence of every RESULT callback, including malformed ones.
    #[cfg(windows)]
    result_seq: u64,
    /// The one completion currently awaited by a host-side dispatch.
    #[cfg(windows)]
    pending_result: Option<PendingResult>,
    /// Compatibility receipt for commands outside the bounded consumer wait set.
    legacy_result: Option<(String, bool)>,
    /// A consumer completion arrived without an active dispatch and cannot be attributed.
    #[cfg(windows)]
    orphan_result: bool,
    /// The seq last pushed to the client.
    pushed: u64,
    /// The client zoom last set: tile pixels, tile twips.
    zoom: (i32, i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(windows)]
struct ResultReceipt {
    seq: u64,
    command: Option<String>,
    success: Option<bool>,
}

#[derive(Debug)]
#[cfg(windows)]
struct PendingResult {
    expected: String,
    after_seq: u64,
    first: Option<ResultReceipt>,
    duplicate: bool,
}

/// A result wait failed in a way that makes later native callbacks unsafe to attribute.
#[derive(Debug)]
#[cfg(windows)]
pub struct ResultWaitError(String);

#[cfg(windows)]
impl std::fmt::Display for ResultWaitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(windows)]
impl std::error::Error for ResultWaitError {}

#[cfg(windows)]
pub fn is_result_consumer(uno: &str) -> bool {
    use crate::commands::Check;
    crate::commands::all().any(|command| {
        command.uno == uno && matches!(command.check, Check::Result | Check::Finished)
    })
}

/// Whether this command has a callback path that can be safely awaited.
#[cfg(windows)]
pub fn waits_for_result(uno: &str) -> bool {
    is_result_consumer(uno) && uno != ".uno:TransformDialog"
}

#[cfg(windows)]
fn bypassed_result_name(uno: &str) -> bool {
    is_known_nonconsumer(uno) || uno == ".uno:TransformDialog"
}

#[cfg(windows)]
fn is_known_nonconsumer(uno: &str) -> bool {
    use crate::commands::Check;
    let mut found = false;
    let mut consumer = false;
    for command in crate::commands::all().filter(|command| command.uno == uno) {
        found = true;
        consumer |= matches!(command.check, Check::Result | Check::Finished);
    }
    found && !consumer
}

impl Slot {
    /// The batch so far; drains the one-shot parts.
    fn drain(&mut self) -> Events {
        let batch = self.batch.clone();
        self.batch.all = false;
        self.batch.rects.clear();
        self.batch.states.clear();
        self.batch.blocked.clear();
        self.batch.menu = None;
        self.batch.faults.clear();
        batch
    }
}

fn slots() -> &'static Mutex<HashMap<u64, Slot>> {
    static SLOTS: OnceLock<Mutex<HashMap<u64, Slot>>> = OnceLock::new();
    SLOTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn signal() -> &'static Condvar {
    static SIGNAL: OnceLock<Condvar> = OnceLock::new();
    SIGNAL.get_or_init(Condvar::new)
}

const KINDS: [i32; 33] = [
    REFERENCE_MARKS,
    FUNCTION_LIST,
    TOOLTIP,
    GEOMETRY,
    SIZE_CHANGED,
    SET_PART,
    CELL_CURSOR,
    CELL_FORMULA,
    HEADER,
    CELL_ADDRESS,
    FONTS,
    NOT_FOUND,
    FOUND,
    MENU,
    REDLINES,
    REDLINE,
    COMMENT,
    RULER,
    INVALIDATE_TILES,
    CURSOR,
    SELECTION,
    CURSOR_VISIBLE,
    GRAPHIC,
    STATE,
    RESULT,
    WINDOW,
    JSDIALOG,
    A11Y_FOCUS,
    A11Y_CARET,
    A11Y_SELECTION,
    A11Y_CELL,
    A11Y_EDITING,
    A11Y_OBJECT,
];

unsafe extern "C" fn on_event(kind: i32, payload: *const c_char, data: *mut c_void) {
    #[cfg(windows)]
    {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            if !KINDS.contains(&kind) || data.is_null() {
                return;
            }
            let (id, raw) = unsafe { &*data.cast::<(u64, crate::ffi::Raw)>() };
            let text = if payload.is_null() {
                String::new()
            } else {
                unsafe { CStr::from_ptr(payload) }
                    .to_string_lossy()
                    .into_owned()
            };
            let callback_seq = RAW_CALLBACK_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
            let enqueued_at = Instant::now();
            raw_queue()
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push_back((*id, kind, text, raw.clone(), callback_seq, enqueued_at));
            push_signal().notify_all();
        }));
        return;
    }
    #[cfg(not(windows))]
    on_event_direct(kind, payload, data)
}

static RAW_CALLBACK_SEQ: AtomicU64 = AtomicU64::new(0);

/// Number of callbacks copied into the host queue so far.
pub fn callback_sequence() -> u64 {
    RAW_CALLBACK_SEQ.load(Ordering::Acquire)
}

/// Writes opt-in Windows result diagnostics outside the native callback.
pub fn trace_select_all(message: &str) {
    let Some(path) = std::env::var_os("ELY_LOK_TRACE_SELECT_ALL_FILE") else {
        return;
    };
    let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    use std::io::Write as _;
    let _ = writeln!(file, "{message}");
}

#[cfg(not(windows))]
unsafe fn on_event_direct(kind: i32, payload: *const c_char, data: *mut c_void) {
    if !KINDS.contains(&kind) || data.is_null() {
        return;
    }
    let (id, raw) = unsafe { *data.cast::<(u64, crate::ffi::Raw)>() };
    let text = if payload.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(payload) }
            .to_string_lossy()
            .into_owned()
    };
    let close = {
        let mut slots = slots().lock().expect("the events lock");
        let Some(slot) = slots.get_mut(&id) else {
            return;
        };
        let close = record(slot, kind, &text);
        slot.batch.seq += 1;
        close
    };
    if let Some(window) = close {
        // Outside the lock: closing calls back in.
        crate::ffi::close_window(raw, window);
    }
    signal().notify_all();
}

#[cfg(windows)]
fn raw_queue() -> &'static Mutex<VecDeque<(u64, i32, String, crate::ffi::Raw, u64, Instant)>> {
    static QUEUE: OnceLock<
        Mutex<VecDeque<(u64, i32, String, crate::ffi::Raw, u64, Instant)>>,
    > = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg(windows)]
fn push_signal() -> &'static Condvar {
    static PUSH_SIGNAL: OnceLock<Condvar> = OnceLock::new();
    PUSH_SIGNAL.get_or_init(Condvar::new)
}

#[cfg(windows)]
fn callback_pins() -> &'static Mutex<Vec<Box<(u64, crate::ffi::Raw)>>> {
    static PINS: OnceLock<Mutex<Vec<Box<(u64, crate::ffi::Raw)>>>> = OnceLock::new();
    PINS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Releases callback userdata only after native kit destruction returned.
#[cfg(windows)]
pub fn quiet_after_kit_destroy() {
    raw_queue()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clear();
    callback_pins()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clear();
}

/// A rectangle payload; an unreadable one is a fault.
fn rect_in(batch: &mut Events, what: &str, text: &str) -> Option<Rect> {
    rect_of(text).unwrap_or_else(|Unreadable| {
        batch.faults.push(format!("an unreadable {what}: {text}"));
        None
    })
}

/// Folds one callback in; returns a dialog to close.
fn record(slot: &mut Slot, kind: i32, text: &str) -> Option<u32> {
    let batch = &mut slot.batch;
    match kind {
        INVALIDATE_TILES => {
            batch.generation += 1;
            match rect_in(batch, "invalidation", text) {
                Some(rect) => batch.rects.push(rect),
                None => batch.all = true,
            }
        }
        CURSOR => batch.cursor = rect_in(batch, "cursor", text),
        SELECTION => {
            let parts: Vec<&str> = text.split(';').collect();
            batch.selection = parts
                .into_iter()
                .filter_map(|part| rect_in(batch, "selection", part))
                .collect();
        }
        CURSOR_VISIBLE => batch.cursor_visible = text == "true",
        GRAPHIC => {
            batch.graphic = rect_in(batch, "graphic selection", text);
            batch.cropping = batch.graphic.is_some() && text.contains("\"isCropMode\": true");
        }
        STATE => {
            if let Some(state) = state_of(text) {
                slot.known.insert(state.0.clone(), state.1.clone());
                batch.states.retain(|(name, _)| *name != state.0);
                batch.states.push(state);
            }
        }
        RESULT => record_result(slot, text),
        RULER => batch.tabs = tabs_of(text),
        REDLINES | REDLINE | COMMENT => batch.review += 1,
        NOT_FOUND => {
            batch.searched += 1;
            batch.found.clear();
        }
        FOUND => match found_of(text) {
            Some(found) => {
                batch.searched += 1;
                batch.found = found;
            }
            None => batch
                .faults
                .push(format!("an unreadable search result: {text}")),
        },
        MENU => batch.menu = Some(text.to_string()),
        CELL_CURSOR => batch.cell.cursor = rect_in(batch, "cell cursor", text),
        CELL_FORMULA => batch.cell.formula = text.to_string(),
        CELL_ADDRESS => batch.cell.address = text.to_string(),
        REFERENCE_MARKS => match marks_of(text) {
            Some(marks) => batch.cell.marks = marks,
            None => batch.faults.push(format!("unreadable marks: {text}")),
        },
        FUNCTION_LIST => match functions_of(text) {
            Some(functions) => {
                // "hidetip" also ends the usage hint.
                if functions.is_empty() {
                    batch.cell.usage.clear();
                }
                batch.cell.functions = functions;
            }
            None => batch.faults.push(format!("unreadable functions: {text}")),
        },
        TOOLTIP => {
            if let Some(usage) = usage_of(text) {
                batch.cell.usage = usage;
            } else if let Some(request) = request_of(text) {
                log::info!("lok: the kernel asks for {}", request.kind());
                batch.cell.requests += 1;
                batch.cell.request = Some(request);
            }
        }
        SET_PART => batch.cell.sheets += 1,
        SIZE_CHANGED => {
            batch.cell.sheets += 1;
            batch.resized += 1;
        }
        HEADER | GEOMETRY => batch.cell.headers += 1,
        FONTS => match fonts_of(text) {
            Some(fonts) => batch.fonts = fonts,
            None => batch
                .faults
                .push(format!("an unreadable font list: {text}")),
        },
        JSDIALOG if formula_text(text).is_some() => {
            // Calc's input line holds the current cell.
            batch.a11y.text = formula_text(text).expect("checked above");
        }
        WINDOW | JSDIALOG => match dialog_of(kind == JSDIALOG, text)? {
            Dialog::Titled { window, title } => {
                slot.titles.insert(window, title);
            }
            Dialog::Opened { window, title } => {
                if !slot.closed.insert(window) {
                    return None;
                }
                let title = title
                    .or_else(|| slot.titles.get(&window).cloned())
                    .unwrap_or_else(|| format!("window {window}"));
                log::warn!("lok: closed kernel dialog {window} ({title})");
                batch.blocked.push(title);
                return Some(window);
            }
        },
        _ => a11y_update(&mut batch.a11y, kind, text),
    }
    None
}

#[cfg(windows)]
fn result_payload(text: &str, seq: u64) -> ResultReceipt {
    let json: serde_json::Value = match serde_json::from_str(text) {
        Ok(json) => json,
        Err(_) => return ResultReceipt { seq, command: None, success: None },
    };
    ResultReceipt {
        seq,
        command: json.get("commandName").and_then(serde_json::Value::as_str).map(str::to_owned),
        success: json.get("success").and_then(serde_json::Value::as_bool),
    }
}

#[cfg(windows)]
fn record_result(slot: &mut Slot, text: &str) {
    slot.result_seq = slot.result_seq.wrapping_add(1);
    let receipt = result_payload(text, slot.result_seq);
    if receipt.command.as_deref().is_some_and(bypassed_result_name) {
        slot.legacy_result = result_of(text);
        return;
    }
    if let Some(pending) = slot.pending_result.as_mut() {
        if pending.first.is_some() {
            pending.duplicate = true;
            return;
        }
        pending.first = Some(receipt);
    } else if let Some(command) = receipt.command.as_deref() {
        if bypassed_result_name(command) {
            slot.legacy_result = result_of(text);
        } else {
            slot.orphan_result = true;
        }
    } else {
        slot.orphan_result = true;
    }
}

#[cfg(not(windows))]
fn record_result(slot: &mut Slot, text: &str) {
    slot.legacy_result = result_of(text);
}

/// Registers the trampoline on `doc` (LOK-03).
pub fn watch(id: u64, doc: &Document) -> std::io::Result<()> {
    let mut slot = Slot::default();
    slot.batch.cursor_visible = true;
    slots().lock().expect("the events lock").insert(id, slot);
    doc.initialize_for_rendering();
    #[cfg(windows)]
    {
        let mut pin = Box::new((id, doc.raw()));
        let data = (&mut *pin as *mut (u64, crate::ffi::Raw)).cast();
        callback_pins()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(pin);
        if let Err(error) = doc.set_callback(Some(on_event), data) {
            slots()
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&id);
            return Err(error);
        }
    }
    #[cfg(not(windows))]
    {
        let data = Box::into_raw(Box::new((id, doc.raw())));
        if let Err(error) = doc.set_callback(Some(on_event), data) {
            slots()
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&id);
            return Err(error);
        }
    }
    Ok(())
}

/// Stops the trampoline and drops the slot.
pub fn unwatch(id: u64, doc: &Document) -> std::io::Result<()> {
    #[cfg(windows)]
    doc.unregister_for_teardown()?;
    #[cfg(not(windows))]
    doc.set_callback(None, std::ptr::null_mut())?;
    slots().lock().expect("the events lock").remove(&id);
    signal().notify_all();
    notify_pusher();
    Ok(())
}

/// Stops every watch before the documents die.
pub fn stop_all(docs: &HashMap<u64, Document>) -> std::io::Result<()> {
    for doc in docs.values() {
        #[cfg(windows)]
        doc.unregister_for_teardown()?;
        #[cfg(not(windows))]
        doc.set_callback(None, std::ptr::null_mut())?;
    }
    slots().lock().expect("the events lock").clear();
    signal().notify_all();
    notify_pusher();
    Ok(())
}

/// The current generation of `id`.
pub fn generation(id: u64) -> u64 {
    slots()
        .lock()
        .expect("the events lock")
        .get(&id)
        .map_or(0, |slot| slot.batch.generation)
}

/// Blocks until documents have unpushed events.
pub fn wait_unpushed() -> bool {
    loop {
        #[cfg(windows)]
        process_raw_events();
        if STOP_PUSH.load(Ordering::Acquire) {
            return false;
        }
        #[cfg(windows)]
        {
            if slots()
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .values()
                .any(|slot| slot.batch.seq > slot.pushed)
            {
                return true;
            }
            let queue = raw_queue().lock().unwrap_or_else(|p| p.into_inner());
            if !queue.is_empty() {
                continue;
            }
            let _queue = push_signal().wait(queue).unwrap_or_else(|p| p.into_inner());
            continue;
        }
        #[cfg(not(windows))]
        {
            let slots = slots().lock().unwrap_or_else(|p| p.into_inner());
            if slots.values().any(|slot| slot.batch.seq > slot.pushed) {
                return true;
            }
            let _guard = signal().wait(slots).unwrap_or_else(|p| p.into_inner());
        }
    }
}

/// Wakes the pusher so its owned thread can exit and be joined.
pub fn stop_push_thread() {
    STOP_PUSH.store(true, Ordering::Release);
    signal().notify_all();
    notify_pusher();
}

fn notify_pusher() {
    #[cfg(windows)]
    {
        let _queue = raw_queue().lock().unwrap_or_else(|p| p.into_inner());
        push_signal().notify_all();
    }
}

static STOP_PUSH: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
fn process_raw_events() {
    loop {
        let item = raw_queue()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pop_front();
        let Some((id, kind, text, raw, callback_seq, enqueued_at)) = item else {
            break;
        };
        if kind == RESULT && std::env::var_os("ELY_LOK_TRACE_SELECT_ALL_FILE").is_some() {
            trace_select_all(&format!(
                "result-host id={id} callback_seq={callback_seq} queue_delay_us={} parsed={:?} raw={text}",
                enqueued_at.elapsed().as_micros(),
                result_of(&text),
            ));
        }
        let close = {
            let mut state = slots().lock().unwrap_or_else(|p| p.into_inner());
            let Some(slot) = state.get_mut(&id) else {
                continue;
            };
            let close = record(slot, kind, &text);
            slot.batch.seq += 1;
            close
        };
        if let Some(window) = close {
            crate::ffi::close_window(raw, window);
        }
        signal().notify_all();
    }
}

/// Drains the documents with unpushed events.
pub fn unpushed() -> Vec<(u64, Events)> {
    let mut slots = slots().lock().expect("the events lock");
    slots
        .iter_mut()
        .filter(|(_, slot)| slot.batch.seq > slot.pushed)
        .map(|(id, slot)| {
            slot.pushed = slot.batch.seq;
            (*id, slot.drain())
        })
        .collect()
}

/// Begins the only result-consuming dispatch allowed for this document.
#[cfg(windows)]
pub fn begin_result(id: u64, expected: &str) -> std::io::Result<()> {
    let mut all = slots().lock().unwrap_or_else(|p| p.into_inner());
    let slot = all.get_mut(&id).ok_or_else(|| result_wait_error("document event watch ended"))?;
    if slot.pending_result.is_some() {
        return Err(result_wait_error("a command completion is already pending"));
    }
    if slot.orphan_result {
        return Err(result_wait_error("an earlier command completion could not be attributed"));
    }
    slot.pending_result = Some(PendingResult {
        expected: expected.to_owned(),
        after_seq: slot.result_seq,
        first: None,
        duplicate: false,
    });
    Ok(())
}

/// Clears the compatibility receipt for an unbounded callback path.
pub fn clear_result(id: u64) {
    if let Some(slot) = slots().lock().unwrap_or_else(|p| p.into_inner()).get_mut(&id) {
        slot.legacy_result = None;
    }
}

/// Takes the compatibility receipt used by legacy non-waiting Finished checks.
pub fn take_result(id: u64) -> Option<(String, bool)> {
    slots()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get_mut(&id)?
        .legacy_result
        .take()
}

/// Waits for and retires the completion belonging to `expected`.
/// Timeout deliberately leaves the pending record in place until worker termination.
#[cfg(windows)]
pub fn wait_result(
    id: u64,
    expected: &str,
    timeout: Duration,
) -> std::io::Result<(Option<bool>, u64)> {
    let deadline = Instant::now() + timeout;
    let mut all = slots().lock().unwrap_or_else(|p| p.into_inner());
    loop {
        let Some(slot) = all.get_mut(&id) else {
            return Err(result_wait_error("document event watch ended while awaiting completion"));
        };
        let Some(pending) = slot.pending_result.as_ref() else {
            return Err(result_wait_error("the command completion wait was cancelled"));
        };
        if pending.expected != expected {
            return Err(result_wait_error("the pending command completion changed"));
        }
        if let Some(receipt) = pending.first.clone() {
            if pending.duplicate {
                return Err(result_wait_error("multiple terminal command completions arrived"));
            }
            if receipt.seq <= pending.after_seq || receipt.command.as_deref() != Some(expected) {
                return Err(result_wait_error(&format!(
                    "completion was not attributable to {expected}: {receipt:?}"
                )));
            }
            slot.pending_result = None;
            return Ok((receipt.success, receipt.seq));
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(result_wait_error(&format!("timed out awaiting {expected}")));
        }
        let (next, timed) = signal()
            .wait_timeout(all, deadline - now)
            .unwrap_or_else(|p| p.into_inner());
        all = next;
        if timed.timed_out() {
            let has_result = all
                .get(&id)
                .and_then(|slot| slot.pending_result.as_ref())
                .is_some_and(|pending| pending.first.is_some());
            if !has_result {
                return Err(result_wait_error(&format!("timed out awaiting {expected}")));
            }
        }
    }
}

#[cfg(windows)]
fn result_wait_error(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, ResultWaitError(message.to_owned()))
}

/// The bridged a11y text now.
pub fn a11y_text(id: u64) -> Option<String> {
    Some(
        slots()
            .lock()
            .expect("the events lock")
            .get(&id)?
            .batch
            .a11y
            .text
            .clone(),
    )
}

/// Waits for the a11y text to leave `before`.
pub fn wait_a11y_change(id: u64, before: &str, timeout: Duration) -> Option<String> {
    let deadline = Instant::now() + timeout;
    let mut slots = slots().lock().expect("the events lock");
    loop {
        let text = slots.get(&id)?.batch.a11y.text.clone();
        let now = Instant::now();
        if text != before || now >= deadline {
            return Some(text);
        }
        slots = signal()
            .wait_timeout(slots, deadline - now)
            .expect("the events wait")
            .0;
    }
}

/// Waits for `uno` to read `want`; returns the last state.
pub fn wait_state(id: u64, uno: &str, want: &str, timeout: Duration) -> Option<String> {
    let deadline = Instant::now() + timeout;
    let mut slots = slots().lock().expect("the events lock");
    loop {
        let state = slots.get(&id)?.known.get(uno).cloned();
        let now = Instant::now();
        if state.as_deref() == Some(want) || now >= deadline {
            return state;
        }
        slots = signal()
            .wait_timeout(slots, deadline - now)
            .expect("the events wait")
            .0;
    }
}

/// Waits until the selected image is in crop mode.
pub fn wait_cropping(id: u64, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    let mut slots = slots().lock().expect("the events lock");
    loop {
        let cropping = slots.get(&id).is_some_and(|slot| slot.batch.cropping);
        let now = Instant::now();
        if cropping || now >= deadline {
            return cropping;
        }
        slots = signal()
            .wait_timeout(slots, deadline - now)
            .expect("the events wait")
            .0;
    }
}

/// The client zoom last set on `id`, in tile units.
pub fn zoom(id: u64) -> (i32, i32) {
    slots().lock().expect("the events lock").get(&id).map_or((0, 0), |slot| slot.zoom)
}

/// A new client zoom resizes the document unannounced (Calc).
pub fn rescaled(id: u64, zoom: (i32, i32)) {
    if let Some(slot) = slots().lock().expect("the events lock").get_mut(&id)
        && slot.zoom != zoom
    {
        slot.zoom = zoom;
        slot.batch.resized += 1;
        slot.batch.seq += 1;
    }
    signal().notify_all();
}

/// Seeds the a11y text from the kernel's snapshot (§7 tail).
pub fn seed_a11y(id: u64, paragraph: &str) {
    if let Some(slot) = slots().lock().expect("the events lock").get_mut(&id) {
        a11y_update(&mut slot.batch.a11y, A11Y_FOCUS, paragraph);
        slot.batch.seq += 1;
    }
    signal().notify_all();
    notify_pusher();
}

#[cfg(all(test, windows))]
mod result_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn fresh_id() -> u64 {
        static NEXT: AtomicU64 = AtomicU64::new(0xE100_0000);
        NEXT.fetch_add(1, Ordering::Relaxed)
    }

    fn install() -> u64 {
        let id = fresh_id();
        slots().lock().unwrap().insert(id, Slot::default());
        id
    }

    fn inject(id: u64, payload: &str) {
        let mut all = slots().lock().unwrap();
        record_result(all.get_mut(&id).unwrap(), payload);
        signal().notify_all();
    }

    fn remove(id: u64) {
        slots().lock().unwrap().remove(&id);
    }

    #[test]
    fn registry_classifies_aliases_that_share_a_consumer_uno() {
        use crate::commands::Check;
        let mut by_uno = HashMap::<&str, (bool, bool)>::new();
        for command in crate::commands::all() {
            let entry = by_uno.entry(command.uno).or_default();
            if matches!(command.check, Check::Result | Check::Finished) {
                entry.0 = true;
            } else {
                entry.1 = true;
            }
        }
        for (uno, (consumer, _also_nonconsumer)) in by_uno {
            assert_eq!(is_result_consumer(uno), consumer, "{uno}");
            assert_eq!(waits_for_result(uno), consumer && uno != ".uno:TransformDialog", "{uno}");
            if waits_for_result(uno) {
                assert!(!_also_nonconsumer, "wait set intersects a nonconsumer: {uno}");
            }
            assert_eq!(is_known_nonconsumer(uno), !consumer, "{uno}");
        }
        assert!(is_result_consumer(".uno:SelectAll"));
        assert!(is_result_consumer(".uno:TransformDialog"));
        assert!(!waits_for_result(".uno:TransformDialog"));
    }

    #[test]
    fn early_receipt_is_latched_and_nonconsumer_result_is_bypassed() {
        let id = install();
        begin_result(id, ".uno:SelectAll").unwrap();
        inject(id, r#"{"commandName":".uno:Undo","success":true}"#);
        inject(id, r#"{"commandName":".uno:SelectAll","success":true}"#);
        assert_eq!(wait_result(id, ".uno:SelectAll", Duration::from_millis(10)).unwrap(), (Some(true), 2));
        remove(id);
    }

    #[test]
    fn transform_callback_does_not_claim_or_replace_a_select_all_pending_result() {
        let id = install();
        begin_result(id, ".uno:SelectAll").unwrap();
        inject(id, r#"{"commandName":".uno:TransformDialog","success":false}"#);
        assert_eq!(take_result(id), Some((".uno:TransformDialog".to_owned(), false)));
        inject(id, r#"{"commandName":".uno:SelectAll","success":true}"#);
        assert_eq!(wait_result(id, ".uno:SelectAll", Duration::from_millis(10)).unwrap(), (Some(true), 2));
        remove(id);
    }

    #[test]
    fn a_transform_with_no_callback_keeps_legacy_no_completion_behavior() {
        let id = install();
        clear_result(id);
        assert_eq!(take_result(id), None);
        remove(id);
    }

    #[test]
    fn delayed_receipt_wakes_waiter_and_unknown_is_a_terminal_receipt() {
        let id = install();
        begin_result(id, ".uno:SelectAll").unwrap();
        let waiter = std::thread::spawn(move || wait_result(id, ".uno:SelectAll", Duration::from_secs(1)));
        inject(id, r#"{"commandName":".uno:SelectAll"}"#);
        assert_eq!(waiter.join().unwrap().unwrap(), (None, 1));
        remove(id);
    }

    #[test]
    fn first_terminal_is_never_overwritten_and_timeout_keeps_pending() {
        let id = install();
        begin_result(id, ".uno:SelectAll").unwrap();
        inject(id, r#"{"commandName":".uno:SelectAll","success":false}"#);
        inject(id, r#"{"commandName":".uno:SelectAll","success":true}"#);
        assert!(wait_result(id, ".uno:SelectAll", Duration::from_millis(10)).is_err());
        remove(id);

        let id = install();
        begin_result(id, ".uno:SelectAll").unwrap();
        let error = wait_result(id, ".uno:SelectAll", Duration::from_millis(1)).unwrap_err();
        assert!(error.get_ref().unwrap().is::<ResultWaitError>());
        assert!(slots().lock().unwrap()[&id].pending_result.is_some());
        assert!(begin_result(id, ".uno:SelectAll").is_err());
        remove(id);
    }

    #[test]
    fn malformed_or_wrong_consumer_receipts_are_fatal_to_attribute() {
        for payload in [
            r#"{"success":true}"#,
            r#"{"commandName":".uno:Unknown","success":true}"#,
        ] {
            let id = install();
            begin_result(id, ".uno:SelectAll").unwrap();
            inject(id, payload);
            assert!(wait_result(id, ".uno:SelectAll", Duration::from_millis(10)).is_err());
            remove(id);
        }
    }
}
