use std::{
    any::Any,
    ffi::{c_char, c_void},
    io,
    sync::{
        Arc,
        atomic::{AtomicPtr, AtomicU8, Ordering},
    },
};

use super::{Callback, Doc, Document, cstring};

#[cfg(not(windows))]
unsafe extern "C" {
    fn free(ptr: *mut c_void);
}

#[cfg(windows)]
const LIVE: u8 = 0;
#[cfg(windows)]
const DISPOSING: u8 = 1;
#[cfg(windows)]
const DISPOSED: u8 = 2;

#[cfg(windows)]
pub(super) struct DocCore {
    pub(super) doc: AtomicPtr<Doc>,
    pub(super) state: AtomicU8,
    pub(super) owner: super::owner::OwnerState,
}

#[cfg(windows)]
unsafe impl Send for DocCore {}
#[cfg(windows)]
unsafe impl Sync for DocCore {}

#[cfg(windows)]
impl Clone for Document {
    fn clone(&self) -> Self {
        Self {
            doc: self.doc,
            view: self.view,
            core: Arc::clone(&self.core),
        }
    }
}

impl Document {
    #[cfg(windows)]
    pub(super) fn from_owner(doc: *mut Doc, view: i32, owner: super::owner::OwnerState) -> Self {
        Self {
            doc,
            view,
            core: Arc::new(DocCore {
                doc: AtomicPtr::new(doc),
                state: AtomicU8::new(LIVE),
                owner,
            }),
        }
    }

    pub(super) fn run<T: Any + Send>(
        &self,
        task: impl FnOnce(*mut Doc) -> T + Send + 'static,
    ) -> io::Result<T> {
        #[cfg(windows)]
        {
            let core = Arc::clone(&self.core);
            let owner = core.owner.clone();
            owner
                .invoke(move || {
                    if core.state.load(Ordering::Acquire) != LIVE {
                        return Err(io::Error::new(
                            io::ErrorKind::NotFound,
                            "document is disposing",
                        ));
                    }
                    let doc = core.doc.load(Ordering::Acquire);
                    if doc.is_null() {
                        return Err(io::Error::new(
                            io::ErrorKind::NotFound,
                            "document is disposed",
                        ));
                    }
                    Ok(task(doc))
                })
                .map_err(super::owner_error)?
        }
        #[cfg(not(windows))]
        {
            Ok(task(self.doc))
        }
    }

    #[cfg(windows)]
    fn run_internal<T: Any + Send>(
        &self,
        task: impl FnOnce(*mut Doc) -> T + Send + 'static,
    ) -> io::Result<T> {
        let core = Arc::clone(&self.core);
        let owner = core.owner.clone();
        owner
            .invoke_internal(move || {
                let state = core.state.load(Ordering::Acquire);
                if state == DISPOSED {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "document is disposed",
                    ));
                }
                let doc = core.doc.load(Ordering::Acquire);
                if doc.is_null() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "document is disposed",
                    ));
                }
                Ok(task(doc))
            })
            .map_err(super::owner_error)?
    }

    /// Makes this document's view the kernel's current one.
    pub fn activate(&self) {
        let view = self.view;
        let _ = self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, i32);
            call!(class, 27, F, doc, view);
        });
    }

    /// Page count or area in LOK twips; zeros when unset.
    pub fn size(&self) -> (i64, i64) {
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, *mut i64, *mut i64);
            let (mut width, mut height) = (0i64, 0i64);
            call!(class, 11, F, doc, &mut width, &mut height);
            (width, height)
        })
        .unwrap_or_default()
    }

    /// Paints the document into `buffer` at (x, y) twips.
    pub fn paint_rect(&self, buffer: &mut [u8], pixels: (i32, i32), twips: (i32, i32, i32, i32)) {
        let mut owned = buffer.to_vec();
        let (pw, ph) = pixels;
        let (x, y, w, h) = twips;
        if let Ok(painted) = self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, *mut u8, i32, i32, i32, i32, i32, i32);
            call!(class, 9, F, doc, owned.as_mut_ptr(), pw, ph, x, y, w, h);
            owned
        }) {
            buffer.copy_from_slice(&painted);
        }
    }

    /// Paints part `part` whole; `mode` 1 paints masters.
    pub fn paint_part(&self, buffer: &mut [u8], (part, mode): (i32, i32), pixels: (i32, i32)) {
        let mut owned = buffer.to_vec();
        let (pw, ph) = pixels;
        if let Ok(painted) = self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type Size = unsafe extern "C" fn(*mut Doc, *mut i64, *mut i64);
            type Paint =
                unsafe extern "C" fn(*mut Doc, *mut u8, i32, i32, i32, i32, i32, i32, i32, i32);
            let (mut width, mut height) = (0i64, 0i64);
            call!(class, 11, Size, doc, &mut width, &mut height);
            call!(
                class,
                32,
                Paint,
                doc,
                owned.as_mut_ptr(),
                part,
                mode,
                pw,
                ph,
                0,
                0,
                width as i32,
                height as i32
            );
            owned
        }) {
            buffer.copy_from_slice(&painted);
        }
    }

    /// Initializes rendering so callbacks flow (LOK-03).
    pub fn initialize_for_rendering(&self) {
        let _ = self.run(|doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, *const c_char);
            call!(class, 12, F, doc, std::ptr::null());
        });
    }

    /// Registers `callback` with `data` (LOK-03).
    pub fn set_callback(&self, callback: Option<Callback>, data: *mut c_void) -> io::Result<()> {
        let data = data as usize;
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, Option<Callback>, *mut c_void);
            call!(class, 13, F, doc, callback, data as *mut c_void);
        })
        .map(|_| ())
    }

    #[cfg(windows)]
    pub(crate) fn unregister_for_teardown(&self) -> io::Result<()> {
        self.run_internal(|doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, Option<Callback>, *mut c_void);
            call!(class, 13, F, doc, None, std::ptr::null_mut());
        })
        .map(|_| ())
    }

    /// Paints the document into `buffer` at `width` x `height` (R-01).
    pub fn paint_tile(&self, buffer: &mut [u8], width: i32, height: i32) {
        let mut owned = buffer.to_vec();
        if let Ok(painted) = self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, *mut u8, i32, i32, i32, i32, i32, i32);
            call!(
                class,
                9,
                F,
                doc,
                owned.as_mut_ptr(),
                width,
                height,
                0,
                0,
                width * 15,
                height * 15
            );
            owned
        }) {
            buffer.copy_from_slice(&painted);
        }
    }

    /// Runs a UNO command with JSON `args` (LOK-08).
    pub fn post_uno(&self, command: &str, args: Option<&str>) -> io::Result<()> {
        self.dispatch(command, args, false)
    }

    /// Runs a UNO command and asks for its completion result.
    pub fn post_uno_notify(&self, command: &str, args: Option<&str>) -> io::Result<()> {
        self.dispatch(command, args, true)
    }

    fn dispatch(&self, command: &str, args: Option<&str>, notify: bool) -> io::Result<()> {
        let command = cstring(command)?;
        let args = args.map(cstring).transpose()?;
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, *const c_char, *const c_char, bool);
            let args = args.as_ref().map_or(std::ptr::null(), |args| args.as_ptr());
            call!(class, 16, F, doc, command.as_ptr(), args, notify);
        })?;
        Ok(())
    }

    /// Posts a key event: 0 input, 1 up.
    pub fn post_key(&self, event_type: i32, char_code: i32, key_code: i32) {
        let _ = self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, i32, i32, i32);
            call!(class, 14, F, doc, event_type, char_code, key_code);
        });
    }

    /// Posts a mouse event: 0 down, 1 up, 2 move.
    pub fn post_mouse(&self, event: [i32; 6]) {
        let _ = self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, i32, i32, i32, i32, i32, i32);
            let [kind, x, y, count, buttons, modifier] = event;
            call!(class, 15, F, doc, kind, x, y, count, buttons, modifier);
        });
    }

    /// Moves a selected object's handle: 0 start, 1 end.
    pub fn set_graphic_selection(&self, kind: i32, x: i32, y: i32) {
        let _ = self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, i32, i32, i32);
            call!(class, 20, F, doc, kind, x, y);
        });
    }

    /// Posts IME text: 0 composing, 2 commit (LOK-07).
    pub fn post_text(&self, event_type: i32, text: &str) -> io::Result<()> {
        let text = cstring(text)?;
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, u32, i32, *const c_char);
            call!(class, 40, F, doc, 0, event_type, text.as_ptr());
        })?;
        Ok(())
    }

    /// LOK_DOCTYPE: 0 text, 1 spreadsheet, 2 presentation.
    pub fn kind(&self) -> i32 {
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc) -> i32;
            call!(class, 2, F, doc)
        })
        .unwrap_or(-1)
    }

    /// LOK_SELTYPE: 0 none, 1 text, 3 complex.
    pub fn selection_type(&self) -> i32 {
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc) -> i32;
            call!(class, 54, F, doc)
        })
        .unwrap_or(0)
    }

    /// The view clipboard in `mimes`; empty ones skipped (LOK-06).
    pub fn clipboard(&self, mimes: &[&str]) -> io::Result<Vec<(String, Vec<u8>)>> {
        let owned = mimes
            .iter()
            .map(|mime| cstring(mime))
            .collect::<io::Result<Vec<_>>>()?;
        #[cfg(windows)]
        let owner = self.core.owner.clone();
        self.run(move |doc| {
            type F = unsafe extern "C" fn(
                *mut Doc,
                *const *const c_char,
                *mut usize,
                *mut *mut *mut c_char,
                *mut *mut usize,
                *mut *mut *mut c_char,
            ) -> i32;
            let class = unsafe { (*doc).class };
            let mut list: Vec<*const c_char> = owned.iter().map(|mime| mime.as_ptr()).collect();
            list.push(std::ptr::null());
            let (mut count, mut names, mut sizes, mut streams) = (
                0usize,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            let ok = call!(
                class,
                52,
                F,
                doc,
                list.as_ptr(),
                &mut count,
                &mut names,
                &mut sizes,
                &mut streams
            );
            let mut found = Vec::new();
            for i in 0..count {
                let (name, size, stream) =
                    unsafe { (*names.add(i), *sizes.add(i), *streams.add(i)) };
                #[cfg(windows)]
                {
                    let mime = unsafe { super::owner::copy_c_string_and_free(&owner, name.cast()) }
                        .unwrap_or_default();
                    let data = unsafe { super::owner::copy_and_free(&owner, stream.cast(), size) }
                        .unwrap_or_default();
                    if ok != 0 && size > 0 && !stream.is_null() {
                        found.push((mime, data));
                    }
                }
                #[cfg(not(windows))]
                {
                    if ok != 0 && size > 0 && !stream.is_null() {
                        let mime = unsafe { std::ffi::CStr::from_ptr(name) }
                            .to_string_lossy()
                            .into_owned();
                        let data = unsafe { std::slice::from_raw_parts(stream.cast::<u8>(), size) }
                            .to_vec();
                        found.push((mime, data));
                    }
                    unsafe {
                        free(name.cast());
                        free(stream.cast());
                    }
                }
            }
            #[cfg(windows)]
            {
                unsafe {
                    super::owner::copy_and_free(
                        &owner,
                        names.cast(),
                        count * std::mem::size_of::<*mut c_char>(),
                    );
                }
                unsafe {
                    super::owner::copy_and_free(
                        &owner,
                        sizes.cast(),
                        count * std::mem::size_of::<usize>(),
                    );
                }
                unsafe {
                    super::owner::copy_and_free(
                        &owner,
                        streams.cast(),
                        count * std::mem::size_of::<*mut c_char>(),
                    );
                }
            }
            #[cfg(not(windows))]
            unsafe {
                free(names.cast());
                free(sizes.cast());
                free(streams.cast());
            }
            if ok == 0 {
                Err(io::Error::other("getClipboard failed"))
            } else {
                Ok(found)
            }
        })?
    }

    /// The JSON LOK keeps for `command` (getCommandValues).
    pub fn command_values(&self, command: &str) -> io::Result<String> {
        let command = cstring(command)?;
        #[cfg(windows)]
        let owner = self.core.owner.clone();
        self.run(move |doc| {
            type F = unsafe extern "C" fn(*mut Doc, *const c_char) -> *mut c_char;
            let class = unsafe { (*doc).class };
            let raw = call!(class, 22, F, doc, command.as_ptr());
            #[cfg(windows)]
            let text = unsafe { super::owner::copy_c_string_and_free(&owner, raw.cast()) };
            #[cfg(not(windows))]
            let text = owned(raw);
            text.ok_or_else(|| io::Error::other("no command values"))
        })?
    }

    /// Turns on the a11y callbacks for the current view.
    pub fn enable_a11y(&self) {
        let _ = self.run(|doc| {
            let class = unsafe { (*doc).class };
            type View = unsafe extern "C" fn(*mut Doc) -> i32;
            type F = unsafe extern "C" fn(*mut Doc, i32, bool);
            let view = call!(class, 28, View, doc);
            call!(class, 69, F, doc, view, true);
        });
    }

    /// Tells the kernel a tile's size in pixels and twips.
    pub fn set_client_zoom(&self, pixels: i32, twips: i32) {
        let _ = self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, i32, i32, i32, i32);
            call!(class, 23, F, doc, pixels, pixels, twips, twips);
        });
    }

    /// Tells the kernel the area the client shows, in twips.
    pub fn set_visible_area(&self, x: i32, y: i32, width: i32, height: i32) {
        let _ = self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, i32, i32, i32, i32);
            call!(class, 24, F, doc, x, y, width, height);
        });
    }

    /// The selection as plain text; Calc joins cells by tabs.
    pub fn selection_text(&self) -> io::Result<String> {
        #[cfg(windows)]
        let owner = self.core.owner.clone();
        self.run(move |doc| {
            type F = unsafe extern "C" fn(*mut Doc, *const c_char, *mut *mut c_char) -> *mut c_char;
            let class = unsafe { (*doc).class };
            let mime = cstring("text/plain;charset=utf-8").expect("constant MIME");
            let mut used: *mut c_char = std::ptr::null_mut();
            let raw = call!(class, 18, F, doc, mime.as_ptr(), &mut used);
            #[cfg(windows)]
            let (text, _) = unsafe {
                (
                    super::owner::copy_c_string_and_free(&owner, raw.cast()),
                    super::owner::copy_c_string_and_free(&owner, used.cast()),
                )
            };
            #[cfg(not(windows))]
            let (text, _) = (owned(raw), owned(used));
            text.ok_or_else(|| io::Error::other("the kernel gave no selection text"))
        })?
    }

    /// Completes the function name being typed (TABLE-03).
    pub fn complete_function(&self, name: &str) -> io::Result<()> {
        let name = cstring(name)?;
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, *const c_char);
            call!(class, 59, F, doc, name.as_ptr());
        })?;
        Ok(())
    }

    /// How many parts: sheets or slides.
    pub fn parts(&self) -> i32 {
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc) -> i32;
            call!(class, 3, F, doc)
        })
        .unwrap_or(0)
    }
    /// The part the view shows, from 0.
    pub fn part(&self) -> i32 {
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc) -> i32;
            call!(class, 5, F, doc)
        })
        .unwrap_or(0)
    }
    /// Shows part `part`.
    pub fn set_part(&self, part: i32) {
        let _ = self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, i32);
            call!(class, 6, F, doc, part);
        });
    }

    /// The name of part `part`.
    pub fn part_name(&self, part: i32) -> io::Result<String> {
        #[cfg(windows)]
        let owner = self.core.owner.clone();
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, i32) -> *mut c_char;
            let raw = call!(class, 7, F, doc, part);
            #[cfg(windows)]
            let text = unsafe { super::owner::copy_c_string_and_free(&owner, raw.cast()) };
            #[cfg(not(windows))]
            let text = owned(raw);
            text.ok_or_else(|| io::Error::other(format!("part {part} has no name")))
        })?
    }
    /// LOK's JSON about part `part`: visible, protected.
    pub fn part_info(&self, part: i32) -> io::Result<String> {
        #[cfg(windows)]
        let owner = self.core.owner.clone();
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, i32) -> *mut c_char;
            let raw = call!(class, 41, F, doc, part);
            #[cfg(windows)]
            let text = unsafe { super::owner::copy_c_string_and_free(&owner, raw.cast()) };
            #[cfg(not(windows))]
            let text = owned(raw);
            text.ok_or_else(|| io::Error::other(format!("part {part} has no info")))
        })?
    }
    /// Page rectangles as "x, y, w, h; ...".
    pub fn page_rects(&self) -> io::Result<String> {
        #[cfg(windows)]
        let owner = self.core.owner.clone();
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc) -> *mut c_char;
            let raw = call!(class, 4, F, doc);
            #[cfg(windows)]
            let text = unsafe { super::owner::copy_c_string_and_free(&owner, raw.cast()) };
            #[cfg(not(windows))]
            let text = owned(raw);
            Ok(text.unwrap_or_default())
        })?
    }
    /// The focused paragraph as LOK's a11y JSON.
    pub fn a11y_paragraph(&self) -> io::Result<String> {
        #[cfg(windows)]
        let owner = self.core.owner.clone();
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc) -> *mut c_char;
            let raw = call!(class, 70, F, doc);
            #[cfg(windows)]
            let text = unsafe { super::owner::copy_c_string_and_free(&owner, raw.cast()) };
            #[cfg(not(windows))]
            let text = owned(raw);
            text.ok_or_else(|| io::Error::other("no focused paragraph"))
        })?
    }

    /// Puts one `mime` entry on the view clipboard (LOK-06).
    pub fn set_clipboard(&self, mime: &str, data: &[u8]) -> io::Result<()> {
        let mime = cstring(mime)?;
        let data = data.to_vec();
        self.run(move |doc| {
            type F = unsafe extern "C" fn(
                *mut Doc,
                usize,
                *const *const c_char,
                *const usize,
                *const *const c_char,
            ) -> i32;
            let class = unsafe { (*doc).class };
            let size = data.len();
            let data_ptr = data.as_ptr().cast::<c_char>();
            let mime_ptr = mime.as_ptr();
            let ok = call!(class, 53, F, doc, 1, &mime_ptr, &size, &data_ptr);
            if ok == 0 {
                Err(io::Error::other("setClipboard failed"))
            } else {
                Ok(())
            }
        })??;
        Ok(())
    }

    /// Saves to `url` as `format`; false means LOK reported failure.
    pub fn save_as(&self, url: &str, format: &str, options: &str) -> io::Result<()> {
        let url = cstring(url)?;
        let format = cstring(format)?;
        let options = cstring(options)?;
        self.run(move |doc| {
            let class = unsafe { (*doc).class };
            type F = unsafe extern "C" fn(*mut Doc, *const i8, *const i8, *const i8) -> i32;
            let ok = call!(
                class,
                1,
                F,
                doc,
                url.as_ptr(),
                format.as_ptr(),
                options.as_ptr()
            );
            if ok == 0 {
                Err(io::Error::other("saveAs failed"))
            } else {
                Ok(())
            }
        })??;
        Ok(())
    }

    #[cfg(windows)]
    pub(super) fn begin_dispose(&self) {
        let _ =
            self.core
                .state
                .compare_exchange(LIVE, DISPOSING, Ordering::AcqRel, Ordering::Acquire);
    }

    #[cfg(windows)]
    pub(crate) fn dispose_on_owner(&self) -> io::Result<()> {
        self.begin_dispose();
        let core = Arc::clone(&self.core);
        let owner = core.owner.clone();
        let result = owner
            .invoke_internal(move || {
                if core.state.load(Ordering::Acquire) == DISPOSED {
                    return Ok(());
                }
                let doc = core.doc.load(Ordering::Acquire);
                if doc.is_null() {
                    core.state.store(DISPOSED, Ordering::Release);
                    return Ok(());
                }
                let class = unsafe { (*doc).class };
                type SetCallback = unsafe extern "C" fn(*mut Doc, Option<Callback>, *mut c_void);
                type Destroy = unsafe extern "C" fn(*mut Doc);
                call!(class, 13, SetCallback, doc, None, std::ptr::null_mut());
                call!(class, 0, Destroy, doc);
                core.doc.store(std::ptr::null_mut(), Ordering::Release);
                core.state.store(DISPOSED, Ordering::Release);
                Ok(())
            })
            .map_err(super::owner_error)?;
        if result.is_err() {
            self.core.owner.quarantine();
        }
        result
    }
}

impl Drop for Document {
    fn drop(&mut self) {
        #[cfg(not(windows))]
        {
            if self.doc.is_null() {
                return;
            }
            let class = unsafe { (*self.doc).class };
            type F = unsafe extern "C" fn(*mut Doc);
            call!(class, 0, F, self.doc);
        }
    }
}

/// A document handle for callbacks. Windows handles retain the owner core.
#[cfg(not(windows))]
#[derive(Clone, Copy)]
pub struct Raw(*mut Doc);

#[cfg(windows)]
#[derive(Clone)]
pub struct Raw(Arc<DocCore>);

impl Document {
    pub fn raw(&self) -> Raw {
        #[cfg(windows)]
        {
            Raw(Arc::clone(&self.core))
        }
        #[cfg(not(windows))]
        {
            Raw(self.doc)
        }
    }
}

/// Closes kernel window `window` of document `doc` (LOK-08).
pub fn close_window(doc: Raw, window: u32) {
    #[cfg(windows)]
    {
        let core = Arc::clone(&doc.0);
        let owner = core.owner.clone();
        let _ = owner.invoke(move || {
            if core.state.load(Ordering::Acquire) != LIVE {
                return;
            }
            let raw = core.doc.load(Ordering::Acquire);
            if raw.is_null() {
                return;
            }
            let class = unsafe { (*raw).class };
            type F = unsafe extern "C" fn(*mut Doc, u32, i32, *const c_char);
            call!(class, 36, F, raw, window, 0, std::ptr::null());
        });
    }
    #[cfg(not(windows))]
    {
        let doc = doc.0;
        let class = unsafe { (*doc).class };
        type F = unsafe extern "C" fn(*mut Doc, u32, i32, *const c_char);
        call!(class, 36, F, doc, window, 0, std::ptr::null());
    }
}

/// Takes a string LOK allocated; None for null.
#[cfg(not(windows))]
fn owned(raw: *mut c_char) -> Option<String> {
    if raw.is_null() {
        return None;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned();
    unsafe { free(raw.cast()) };
    Some(text)
}
