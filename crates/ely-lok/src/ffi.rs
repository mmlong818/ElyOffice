use std::{
    any::Any,
    ffi::{CString, c_char, c_void},
    io,
    path::Path,
    sync::Arc,
};

/// The kit vtable: nSize, then 26 slots in header order.
#[repr(C)]
struct KitClass {
    n_size: usize,
    entries: [*mut c_void; 26],
}

/// The document vtable: nSize, then 81 slots in header order.
#[repr(C)]
struct DocClass {
    n_size: usize,
    entries: [*mut c_void; 81],
}

#[repr(C)]
struct Kit {
    class: *const KitClass,
}

#[repr(C)]
struct Doc {
    class: *const DocClass,
}

type Hook2 = unsafe extern "C" fn(*const i8, *const i8) -> *mut Kit;

/// A loaded LibreOfficeKit instance.
pub struct Lok {
    kit: *mut Kit,
    #[cfg(not(windows))]
    _lib: Library,
    #[cfg(windows)]
    _lib: Arc<Library>,
    #[cfg(windows)]
    owner: owner::OwnerState,
}

/// A LOK event callback (LibreOfficeKitCallback).
pub type Callback = unsafe extern "C" fn(i32, *const c_char, *mut c_void);

/// Documents move between the registry and handlers on one thread.
unsafe impl Send for Document {}

/// A loaded document inside one Lok.
pub struct Document {
    doc: *mut Doc,
    /// The kernel view the load made.
    view: i32,
    #[cfg(windows)]
    core: Arc<document::DocCore>,
}

/// Calls vtable slot `index` as `F`.
macro_rules! call {
    ($class:expr, $index:expr, $type:ty, $($arg:expr),*) => {{
        unsafe {
            let entry = (*$class).entries[$index];
            let f: $type = std::mem::transmute(entry);
            f($($arg),*)
        }
    }};
}

mod document;
#[cfg(windows)]
#[allow(dead_code)]
mod owner;
mod slideshow;
pub use document::{Raw, close_window};

impl Lok {
    /// LOK_FEATURE_PART_IN_INVALIDATION_CALLBACK.
    pub const PART_IN_INVALIDATION: u64 = 1 << 2;
    /// LOK_FEATURE_NO_TILED_ANNOTATIONS: comments come as callbacks.
    pub const NO_TILED_ANNOTATIONS: u64 = 1 << 3;
    /// LOK_FEATURE_DOCUMENT_PASSWORD: the client answers.
    pub const DOCUMENT_PASSWORD: u64 = 1 << 0;

    fn invoke_kit<T: Any + Send>(
        &self,
        task: impl FnOnce(*mut Kit) -> T + Send + 'static,
    ) -> io::Result<T> {
        #[cfg(windows)]
        {
            let owner = self.owner.clone();
            let task_owner = owner.clone();
            owner
                .invoke(move || task(task_owner.native_kit().cast()))
                .map_err(owner_error)
        }
        #[cfg(not(windows))]
        {
            Ok(task(self.kit))
        }
    }

    #[cfg(windows)]
    pub(super) fn close_admission(&self, timeout: std::time::Duration) -> io::Result<()> {
        self.owner.close_admission(timeout).map_err(owner_error)
    }

    #[cfg(windows)]
    pub(super) fn wait_owner_drained(&self, timeout: std::time::Duration) -> io::Result<()> {
        self.owner.wait_drained(timeout).map_err(owner_error)
    }

    #[cfg(windows)]
    pub(super) fn quarantine(&self) {
        self.owner.quarantine();
    }

    #[cfg(windows)]
    pub(super) fn owner_failed_closed(&self) -> bool {
        self.owner.is_quarantined()
    }

    /// dlopens the LOK library and hooks with the profile URL.
    pub fn hook(install: &Path, profile_url: &str) -> io::Result<Self> {
        let install = std::fs::canonicalize(install)?;
        let install = install.as_path();
        let install_str = install.to_string_lossy().into_owned();
        let install_c = cstring(&install_str)?;
        let profile_c = cstring(profile_url)?;
        #[cfg(windows)]
        let lib = Arc::new(Library::open(install)?);
        #[cfg(not(windows))]
        let lib = Library::open(install)?;
        let hook: Hook2 = unsafe {
            lib.symbol(b"libreofficekit_hook_2\0")
                .or_else(|_| lib.symbol(b"libreofficekit_hook\0"))
                .map_err(|error| io::Error::new(io::ErrorKind::NotFound, error))?
        };
        #[cfg(windows)]
        let native_api = {
            use owner::{BufferFree, IsCurrent, Post};
            let is_current: IsCurrent = unsafe {
                lib.symbol(b"ely_lok_owner_is_current_v1\0")
                    .map_err(|error| io::Error::new(io::ErrorKind::NotFound, error))?
            };
            let post: Post = unsafe {
                lib.symbol(b"ely_lok_owner_post_v1\0")
                    .map_err(|error| io::Error::new(io::ErrorKind::NotFound, error))?
            };
            let buffer_free: BufferFree = unsafe {
                lib.symbol(b"ely_lok_buffer_free_v1\0")
                    .map_err(|error| io::Error::new(io::ErrorKind::NotFound, error))?
            };
            (is_current, post, buffer_free)
        };
        let kit = unsafe { hook(install_c.as_ptr(), profile_c.as_ptr()) };
        if kit.is_null() {
            return Err(io::Error::other(format!(
                "lok hook failed for {install_str}"
            )));
        }
        #[cfg(windows)]
        let lok = {
            type Destroy = unsafe extern "C" fn(*mut Kit);
            let class = unsafe { (*kit).class };
            let destroy: Destroy = unsafe { std::mem::transmute((*class).entries[0]) };
            let owner = unsafe {
                owner::OwnerState::new(
                    kit.cast(),
                    owner::NativeApi {
                        is_current: native_api.0,
                        post: native_api.1,
                        buffer_free: native_api.2,
                        destroy: std::mem::transmute::<Destroy, owner::KitDestroy>(destroy),
                    },
                    lib.clone(),
                    std::time::Duration::from_secs(30),
                )
            };
            Self {
                kit,
                _lib: lib,
                owner,
            }
        };
        #[cfg(not(windows))]
        let mut lok = Self { kit, _lib: lib };
        let features =
            Self::PART_IN_INVALIDATION | Self::NO_TILED_ANNOTATIONS | Self::DOCUMENT_PASSWORD;
        lok.invoke_kit(move |kit| {
            type F = unsafe extern "C" fn(*mut Kit, u64);
            call!((*kit).class, 7, F, kit, features);
        })?;
        log::info!("lok: hooked {}", install_str);
        Ok(lok)
    }

    /// The BuildId string of the hooked library.
    pub fn build_id(&self) -> io::Result<String> {
        #[cfg(windows)]
        let owner = self.owner.clone();
        self.invoke_kit(move |kit| {
            let class = unsafe { (*kit).class };
            type F = unsafe extern "C" fn(*mut Kit) -> *mut i8;
            let raw = call!(class, 9, F, kit);
            #[cfg(windows)]
            {
                unsafe { owner::copy_c_string_and_free(&owner, raw.cast()) }.unwrap_or_default()
            }
            #[cfg(not(windows))]
            {
                unsafe { std::ffi::CStr::from_ptr(raw) }
                    .to_string_lossy()
                    .into_owned()
            }
        })
    }

    /// Loads a document; failures surface (LOK-09).
    pub fn load(&self, url: &str, options: &str) -> io::Result<Document> {
        let url_c = cstring(url)?;
        let options_c = cstring(options)?;
        let loaded = self
            .invoke_kit(move |kit| {
                type F = unsafe extern "C" fn(*mut Kit, *const i8, *const i8) -> *mut Doc;
                let class = unsafe { (*kit).class };
                let doc = call!(class, 3, F, kit, url_c.as_ptr(), options_c.as_ptr());
                if doc.is_null() {
                    return Err(kit_error_raw(kit));
                }
                type View = unsafe extern "C" fn(*mut Doc) -> i32;
                let doc_class = unsafe { (*doc).class };
                let view = call!(doc_class, 28, View, doc);
                Ok((doc as usize, view))
            })?
            .map_err(io::Error::other)?;
        let (raw_doc, view) = loaded;
        let doc = raw_doc as *mut Doc;
        log::info!("lok: loaded into view {view}");
        #[cfg(windows)]
        {
            Ok(Document::from_owner(doc, view, self.owner.clone()))
        }
        #[cfg(not(windows))]
        {
            Ok(Document { doc, view })
        }
    }

    /// Registers the kit-wide callback.
    pub fn register_callback(&self, callback: Callback) -> io::Result<()> {
        self.invoke_kit(move |kit| {
            let class = unsafe { (*kit).class };
            type F = unsafe extern "C" fn(*mut Kit, Callback, *mut c_void);
            call!(class, 5, F, kit, callback, std::ptr::null_mut());
        })
    }

    /// Answers a password question; callback calls are owner-reentrant.
    pub fn set_password(&self, url: &str, password: Option<&str>) {
        let url_c = cstring(url).expect("a url without NUL");
        let password_c = password.map(|password| cstring(password).expect("no NUL"));
        let result = self.invoke_kit(move |kit| {
            let class = unsafe { (*kit).class };
            let password_ptr = password_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
            type F = unsafe extern "C" fn(*mut Kit, *const c_char, *const c_char);
            call!(class, 8, F, kit, url_c.as_ptr(), password_ptr);
        });
        if let Err(error) = result {
            log::warn!("lok: password callback answer failed: {error}");
        }
    }

    /// Sets a kit option such as `author`.
    pub fn set_option(&self, option: &str, value: &str) -> io::Result<()> {
        let (option, value) = (cstring(option)?, cstring(value)?);
        self.invoke_kit(move |kit| {
            let class = unsafe { (*kit).class };
            type F = unsafe extern "C" fn(*mut Kit, *const c_char, *const c_char);
            call!(class, 14, F, kit, option.as_ptr(), value.as_ptr());
        })
    }

    /// Waits for the kit's threads to finish (LOK-10). Called off the owner.
    pub fn join_threads(&self) -> bool {
        #[cfg(windows)]
        let kit = {
            assert!(!self.owner.on_owner_thread());
            self.owner.native_kit().cast::<Kit>()
        };
        #[cfg(not(windows))]
        let kit = self.kit;
        if kit.is_null() {
            return false;
        }
        let class = unsafe { (*kit).class };
        type F = unsafe extern "C" fn(*mut Kit) -> i32;
        call!(class, 20, F, kit) != 0
    }

    pub(super) fn destroy_after_join(&mut self) -> io::Result<()> {
        #[cfg(windows)]
        {
            self.owner.destroy_after_join().map_err(owner_error)?;
            self.kit = std::ptr::null_mut();
            Ok(())
        }
        #[cfg(not(windows))]
        {
            if !self.kit.is_null() {
                let class = unsafe { (*self.kit).class };
                type F = unsafe extern "C" fn(*mut Kit);
                call!(class, 0, F, self.kit);
                self.kit = std::ptr::null_mut();
            }
            Ok(())
        }
    }

    /// Runs a macro by URL, for the debug probe.
    pub fn run_macro(&self, url: &str) -> i32 {
        let url_c = cstring(url).expect("a url");
        self.invoke_kit(move |kit| {
            let class = unsafe { (*kit).class };
            type F = unsafe extern "C" fn(*mut Kit, *const i8) -> i32;
            call!(class, 10, F, kit, url_c.as_ptr())
        })
        .unwrap_or(-1)
    }

    /// Answers the kernel's "input waiting?" without a run-loop peek.
    pub fn register_any_input(&self, waiting: extern "C" fn(*mut c_void, i32) -> bool) {
        static DATA: u8 = 0;
        let class = unsafe { (*self.kit).class };
        type F =
            unsafe extern "C" fn(*mut Kit, extern "C" fn(*mut c_void, i32) -> bool, *mut c_void);
        let data = std::ptr::addr_of!(DATA).cast_mut().cast();
        call!(class, 24, F, self.kit, waiting, data);
    }

    /// Runs LOK's loop until kit exit; unipoll calls `poll`.
    pub fn run_loop(&self, poll: Option<Poll>, wake: Option<Wake>) {
        // VCL calls neither hook while their data is null.
        static DATA: u8 = 0;
        let data = std::ptr::addr_of!(DATA).cast_mut().cast();
        let class = unsafe { (*self.kit).class };
        type F = unsafe extern "C" fn(*mut Kit, Option<Poll>, Option<Wake>, *mut c_void);
        call!(class, 12, F, self.kit, poll, wake, data);
    }
}

fn kit_error_raw(kit: *mut Kit) -> String {
    let class = unsafe { (*kit).class };
    type Get = unsafe extern "C" fn(*mut Kit) -> *mut i8;
    type Free = unsafe extern "C" fn(*mut i8);
    let raw = call!(class, 2, Get, kit);
    if raw.is_null() {
        return "unknown LOK error".into();
    }
    let text = unsafe { std::ffi::CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned();
    call!(class, 4, Free, raw);
    text
}

/// The kernel library: every build here merges its libraries.
#[cfg(target_os = "macos")]
pub const KERNEL: &str = "libmergedlo.dylib";
#[cfg(target_os = "linux")]
pub const KERNEL: &str = "libmergedlo.so";
#[cfg(windows)]
pub const KERNEL: &str = "mergedlo.dll";

/// Waits up to its microseconds; above zero, work was done.
pub type Poll = extern "C" fn(*mut c_void, i32) -> i32;
/// The kernel has work for its loop.
pub type Wake = extern "C" fn(*mut c_void);

impl Drop for Lok {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            if !self.owner.is_destroyed() {
                // Native destruction is allowed only in the explicit worker
                // teardown after successful joinThreads.
                self.owner.quarantine();
            }
            return;
        }
        #[cfg(not(windows))]
        {
            if self.kit.is_null() {
                return;
            }
            let class = unsafe { (*self.kit).class };
            type F = unsafe extern "C" fn(*mut Kit);
            call!(class, 0, F, self.kit);
            log::info!("lok: destroyed");
        }
    }
}

fn owner_error(error: owner::OwnerError) -> io::Error {
    match error {
        owner::OwnerError::TimedOut => io::Error::new(io::ErrorKind::TimedOut, error),
        owner::OwnerError::Closed => io::Error::new(io::ErrorKind::BrokenPipe, error),
        _ => io::Error::other(error),
    }
}

/// A Rust string as a NUL-terminated C string.
fn cstring(text: &str) -> io::Result<CString> {
    CString::new(text)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the string holds a NUL"))
}

#[cfg(unix)]
struct Library {
    handle: *mut c_void,
}

#[cfg(unix)]
impl Library {
    /// dlopens the merged kernel from `install`.
    fn open(install: &Path) -> io::Result<Self> {
        use std::os::unix::ffi::OsStrExt;
        let path = install.join(KERNEL);
        let c = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the path holds a NUL"))?;
        let handle = unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_LAZY | libc::RTLD_GLOBAL) };
        if handle.is_null() {
            return Err(io::Error::other(format!("dlopen {}: {}", path.display(), dl_error())));
        }
        Ok(Self { handle })
    }

    /// dlsyms `name` from the library.
    unsafe fn symbol<F: Copy>(&self, name: &[u8]) -> Result<F, String> {
        let name = name.strip_suffix(b"\0").unwrap_or(name);
        let c = CString::new(name).map_err(|_| "bad symbol".to_string())?;
        unsafe { libc::dlerror() };
        let ptr = unsafe { libc::dlsym(self.handle, c.as_ptr()) };
        if ptr.is_null() {
            return Err(dl_error());
        }
        Ok(unsafe { std::mem::transmute_copy(&ptr) })
    }
}

#[cfg(unix)]
fn dl_error() -> String {
    let raw = unsafe { libc::dlerror() };
    if raw.is_null() {
        return "dlopen failed".into();
    }
    unsafe { std::ffi::CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(windows)]
struct Library {
    handle: isize,
}

#[cfg(windows)]
unsafe impl Send for Library {}
#[cfg(windows)]
unsafe impl Sync for Library {}

#[cfg(windows)]
impl Library {
    /// Loads the merged kernel first; the unmerged legacy library is a fallback only if absent.
    fn open(install: &Path) -> io::Result<Self> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::System::LibraryLoader::{
            LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LoadLibraryExW,
        };
        for name in [KERNEL, "sofficeapp.dll"] {
            let path = install.join(name);
            if !path.is_file() {
                continue;
            }
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let handle = unsafe {
                LoadLibraryExW(
                    wide.as_ptr(),
                    std::ptr::null_mut(),
                    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                )
            };
            if handle.is_null() {
                let error = io::Error::last_os_error();
                return Err(io::Error::other(format!(
                    "LoadLibrary {}: {error}",
                    path.display()
                )));
            }
            return Ok(Self {
                handle: handle as isize,
            });
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no LOK library under {}", install.display()),
        ))
    }

    /// Gets `name` from the library.
    unsafe fn symbol<F: Copy>(&self, name: &[u8]) -> Result<F, String> {
        use windows_sys::Win32::System::LibraryLoader::GetProcAddress;
        let name = name.strip_suffix(b"\0").unwrap_or(name);
        let c = CString::new(name).map_err(|_| "bad symbol".to_string())?;
        let ptr = unsafe { GetProcAddress(self.handle as _, c.as_ptr() as _) };
        let Some(ptr) = ptr else {
            return Err(format!("no symbol {}", String::from_utf8_lossy(name)));
        };
        Ok(unsafe { std::mem::transmute_copy(&ptr) })
    }
}

#[cfg(windows)]
impl Drop for Library {
    fn drop(&mut self) {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn FreeLibrary(module: *mut std::ffi::c_void) -> i32;
        }
        if self.handle != 0 {
            unsafe { FreeLibrary(self.handle as _) };
            self.handle = 0;
        }
    }
}
