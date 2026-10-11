//! An OS-level exclusive file lock, released with its holder.

use std::{fs::File, io, path::Path};

pub struct FileLock(File);

impl FileLock {
    /// The lock, or None while another process holds it.
    pub fn try_take(path: &Path) -> io::Result<Option<Self>> {
        let file = open(path)?;
        match lock(&file, false) {
            Ok(()) => Ok(Some(Self(file))),
            Err(error) if busy(&error) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// The lock, once another holder lets go.
    pub fn take(path: &Path) -> io::Result<Self> {
        let file = open(path)?;
        lock(&file, true)?;
        Ok(Self(file))
    }
}

fn open(path: &Path) -> io::Result<File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

#[cfg(unix)]
fn lock(file: &File, wait: bool) -> io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let how = if wait {
        libc::LOCK_EX
    } else {
        libc::LOCK_EX | libc::LOCK_NB
    };
    match unsafe { libc::flock(file.as_raw_fd(), how) } {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error()),
    }
}

#[cfg(unix)]
fn busy(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::WouldBlock
}

#[cfg(unix)]
impl Drop for FileLock {
    fn drop(&mut self) {
        use std::os::unix::io::AsRawFd;
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

#[cfg(windows)]
fn lock(file: &File, wait: bool) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx,
    };
    let flags = if wait {
        LOCKFILE_EXCLUSIVE_LOCK
    } else {
        LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY
    };
    let mut overlapped = unsafe { std::mem::zeroed() };
    match unsafe {
        LockFileEx(
            file.as_raw_handle(),
            flags,
            0,
            u32::MAX,
            u32::MAX,
            &mut overlapped,
        )
    } {
        0 => Err(io::Error::last_os_error()),
        _ => Ok(()),
    }
}

#[cfg(windows)]
fn busy(error: &io::Error) -> bool {
    const ERROR_LOCK_VIOLATION: i32 = 33;
    error.raw_os_error() == Some(ERROR_LOCK_VIOLATION)
}

#[cfg(windows)]
impl Drop for FileLock {
    fn drop(&mut self) {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::UnlockFileEx;
        let mut overlapped = unsafe { std::mem::zeroed() };
        unsafe {
            UnlockFileEx(
                self.0.as_raw_handle(),
                0,
                u32::MAX,
                u32::MAX,
                &mut overlapped,
            )
        };
    }
}

#[cfg(test)]
mod tests {
    use super::FileLock;

    #[test]
    fn a_held_lock_refuses_a_second_try_until_released() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("held.lock");
        let held = FileLock::take(&path).unwrap();
        assert!(
            FileLock::try_take(&path).unwrap().is_none(),
            "flock is per open file"
        );
        drop(held);
        assert!(FileLock::try_take(&path).unwrap().is_some());
    }
}
