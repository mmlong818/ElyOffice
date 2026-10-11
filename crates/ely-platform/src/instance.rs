//! One running copy per app; two would wipe shared data.

use std::{fs::File, io, path::Path, sync::OnceLock};

use crate::AppId;

/// The lock this process holds while it runs.
static HELD: OnceLock<File> = OnceLock::new();

/// Takes `app`'s instance lock; a running copy refuses this one.
pub fn hold(app: AppId) -> io::Result<()> {
    let dir = crate::data_dir(app).map_err(io::Error::other)?;
    std::fs::create_dir_all(&dir)?;
    take(app, &dir)
}

fn take(app: AppId, dir: &Path) -> io::Result<()> {
    let file = std::fs::OpenOptions::new().write(true).create(true).truncate(false).open(dir.join("instance.lock"))?;
    match lock(&file)? {
        None => HELD.set(file).map_err(|_| io::Error::other("this process took its instance lock twice")),
        Some(holder) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} is already running ({holder}); open files in its window", app.name()),
        )),
    }
}

/// Locks `file`, or names its holder as the OS reports.
#[cfg(unix)]
fn lock(file: &File) -> io::Result<Option<String>> {
    use std::os::unix::io::AsRawFd;
    let fd = file.as_raw_fd();
    loop {
        let mut want: libc::flock = unsafe { std::mem::zeroed() };
        want.l_type = libc::F_WRLCK as _;
        want.l_whence = libc::SEEK_SET as _;
        if unsafe { libc::fcntl(fd, libc::F_SETLK, &want) } == 0 {
            return Ok(None);
        }
        let error = io::Error::last_os_error();
        if !matches!(error.raw_os_error(), Some(libc::EACCES | libc::EAGAIN)) {
            return Err(error);
        }
        let mut held = want;
        if unsafe { libc::fcntl(fd, libc::F_GETLK, &mut held) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // Let go between the two calls: try again.
        if i64::from(held.l_type) != i64::from(libc::F_UNLCK) {
            return Ok(Some(format!("process {}", held.l_pid)));
        }
    }
}

/// Windows names no holder; the lock alone refuses.
#[cfg(windows)]
fn lock(file: &File) -> io::Result<Option<String>> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx};
    const ERROR_LOCK_VIOLATION: i32 = 33;
    let flags = LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY;
    let mut overlapped = unsafe { std::mem::zeroed() };
    match unsafe { LockFileEx(file.as_raw_handle(), flags, 0, u32::MAX, u32::MAX, &mut overlapped) } {
        0 if io::Error::last_os_error().raw_os_error() == Some(ERROR_LOCK_VIOLATION) => Ok(Some("another copy holds its lock".into())),
        0 => Err(io::Error::last_os_error()),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_started_together_name_the_one_holder() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let exe = std::env::current_exe().expect("this test");
        let copies: Vec<_> = (0..6)
            .map(|_| {
                std::process::Command::new(&exe)
                    .args(["--exact", "instance::tests::one_copy", "--nocapture"])
                    .env("ELY_INSTANCE_DIR", dir.path())
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                    .expect("a copy")
            })
            .collect();
        let said: Vec<String> = copies
            .into_iter()
            .map(|copy| String::from_utf8_lossy(&copy.wait_with_output().expect("ends").stdout).into_owned())
            .collect();
        let holders: Vec<&String> = said.iter().filter(|out| out.contains("HELD")).collect();
        assert_eq!(holders.len(), 1, "{said:?}");
        let holder = holders[0].split("HELD ").nth(1).and_then(|rest| rest.split_whitespace().next()).expect("its pid");
        let refused: Vec<&String> = said.iter().filter(|out| out.contains("REFUSED")).collect();
        assert_eq!(refused.len(), 5, "{said:?}");
        // Unix names the holder; Windows cannot ask.
        let named = if cfg!(unix) { format!("(process {holder})") } else { "(another copy holds its lock)".into() };
        for out in refused {
            assert!(out.contains(&named), "{out}");
        }
    }

    /// One copy, in a process of its own.
    #[test]
    fn one_copy() {
        let Ok(dir) = std::env::var("ELY_INSTANCE_DIR") else {
            return;
        };
        match take(AppId::Doc, Path::new(&dir)) {
            Ok(()) => {
                println!("HELD {} ", std::process::id());
                std::thread::sleep(std::time::Duration::from_millis(1500));
            }
            Err(error) => println!("REFUSED {error}"),
        }
    }
}
