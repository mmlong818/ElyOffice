//! The name documents sign new comments and changes with.

use std::io;

/// `author` as the settings give it, else the account's name.
pub fn named(author: &str) -> io::Result<String> {
    if !author.is_empty() {
        return Ok(author.to_string());
    }
    account()
}

/// The account's full name, else its login name.
pub fn account() -> io::Result<String> {
    let full = full_name()?;
    if !full.trim().is_empty() {
        return Ok(full.trim().to_string());
    }
    let key = if cfg!(windows) { "USERNAME" } else { "USER" };
    log::info!("user: no full name; the login name signs");
    std::env::var(key).map_err(|error| io::Error::other(format!("no {key}: {error}")))
}

#[cfg(target_os = "macos")]
fn full_name() -> io::Result<String> {
    Ok(objc2_foundation::NSFullUserName().to_string())
}

#[cfg(target_os = "linux")]
fn full_name() -> io::Result<String> {
    // The first comma field of the passwd comment.
    let entry = unsafe { libc::getpwuid(libc::getuid()) };
    if entry.is_null() {
        return Err(io::Error::last_os_error());
    }
    let gecos = unsafe { (*entry).pw_gecos };
    if gecos.is_null() {
        return Ok(String::new());
    }
    let text = unsafe { std::ffi::CStr::from_ptr(gecos) }.to_string_lossy();
    Ok(text.split(',').next().unwrap_or_default().to_string())
}

#[cfg(windows)]
fn full_name() -> io::Result<String> {
    use windows_sys::Win32::Security::Authentication::Identity::{GetUserNameExW, NameDisplay};
    let mut size = 0u32;
    unsafe { GetUserNameExW(NameDisplay, std::ptr::null_mut(), &mut size) };
    // Accounts outside a domain have no display name.
    if size == 0 {
        return Ok(String::new());
    }
    let mut buffer = vec![0u16; size as usize];
    if !unsafe { GetUserNameExW(NameDisplay, buffer.as_mut_ptr(), &mut size) } {
        return Err(io::Error::last_os_error());
    }
    Ok(String::from_utf16_lossy(&buffer[..size as usize]))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_set_name_signs_else_the_account() {
        assert_eq!(super::named("Ada").expect("a name"), "Ada");
        let account = super::named("").expect("a name");
        assert!(!account.is_empty());
        assert_eq!(account, super::account().expect("the account"));
    }
}
