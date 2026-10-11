//! Starting another Ely app installed beside this one.

use std::{
    io,
    path::{Path, PathBuf},
    process::Command,
};

use crate::AppId;

/// Where `app` sits for an install laid out like `exe`'s.
fn beside(exe: &Path, app: AppId) -> Vec<PathBuf> {
    let file = format!("{}{}", app.slug(), std::env::consts::EXE_SUFFIX);
    let Some(dir) = exe.parent() else {
        return Vec::new();
    };
    let mut places = vec![dir.join(&file)];
    // macOS: Name.app/Contents/MacOS; Windows: Ely Office\Name.
    if let Some(bundle) = dir.parent().and_then(Path::parent) {
        let app_dir = format!("{}.app", app.name());
        if let Some(apps) = bundle.parent() {
            places.push(apps.join(app_dir).join("Contents/MacOS").join(&file));
        }
    }
    if let Some(suite) = dir.parent() {
        places.push(suite.join(app.name()).join(&file));
    }
    // Linux packages: /opt/ely-office/<slug>/bin.
    if let Some(opt) = dir.parent().and_then(Path::parent) {
        places.push(opt.join(app.slug()).join("bin").join(&file));
    }
    places
}

/// `app`'s binary: beside this one, else wherever macOS has it.
fn find(app: AppId) -> io::Result<Option<PathBuf>> {
    let exe = std::env::current_exe()?;
    let near = beside(&exe, app).into_iter().find(|place| place.is_file());
    Ok(near.or_else(|| registered(app)))
}

#[cfg(target_os = "macos")]
fn registered(app: AppId) -> Option<PathBuf> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;
    let id = NSString::from_str(&app.bundle_id());
    let url = NSWorkspace::sharedWorkspace().URLForApplicationWithBundleIdentifier(&id)?;
    let bundle = PathBuf::from(url.path()?.to_string());
    let binary = bundle.join("Contents/MacOS").join(app.slug());
    binary.is_file().then_some(binary)
}

#[cfg(not(target_os = "macos"))]
fn registered(_: AppId) -> Option<PathBuf> {
    None
}

/// Whether `app` can be started from here.
pub fn installed(app: AppId) -> bool {
    matches!(find(app), Ok(Some(_)))
}

/// The `.app` a macOS binary sits in.
fn bundle_of(binary: &Path) -> Option<&Path> {
    let bundle = binary.parent()?.parent()?.parent()?;
    (cfg!(target_os = "macos") && bundle.extension().is_some_and(|ext| ext == "app"))
        .then_some(bundle)
}

/// Starts `app` with `files`; a running one takes them.
pub fn open(app: AppId, files: &[PathBuf]) -> io::Result<()> {
    let found = find(app)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} is not installed", app.name()),
        )
    })?;
    // Launch Services hands files to a running app.
    if let Some(bundle) = bundle_of(&found) {
        let out = Command::new("open")
            .arg("-a")
            .arg(bundle)
            .args(files)
            .output()?;
        if !out.status.success() {
            let said = String::from_utf8_lossy(&out.stderr).trim().to_owned();
            return Err(io::Error::other(format!("{}: {said}", app.name())));
        }
        log::info!("apps: opened {} with {} files", app.name(), files.len());
        return Ok(());
    }
    let mut command = Command::new(&found);
    command.args(files);
    let child = fresh_signals(&mut command).spawn()?;
    log::info!("apps: started {} as {}", app.name(), child.id());
    Ok(())
}

/// The started app takes signals as from a shell.
#[cfg(unix)]
fn fresh_signals(command: &mut Command) -> &mut Command {
    use std::os::unix::process::CommandExt as _;
    // Chromium's ignored or blocked signals would carry over.
    unsafe {
        command.pre_exec(|| {
            for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP, libc::SIGQUIT] {
                libc::signal(signal, libc::SIG_DFL);
            }
            let mut none: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut none);
            libc::pthread_sigmask(libc::SIG_SETMASK, &none, std::ptr::null_mut());
            Ok(())
        })
    }
}

#[cfg(not(unix))]
fn fresh_signals(command: &mut Command) -> &mut Command {
    command
}

/// Opens `path` in the default app; a refusal says why.
pub fn open_default(path: &Path) -> io::Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(windows) {
        let mut start = Command::new("cmd");
        start.args(["/C", "start", ""]);
        start
    } else {
        Command::new("xdg-open")
    };
    let out = ely_ipc::windowless(&mut command).arg(path).output()?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        return Err(io::Error::other(said));
    }
    log::info!("apps: the system opened an output");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::AppId;

    #[cfg(target_os = "macos")]
    #[test]
    fn a_missing_output_says_so() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let error = super::open_default(&dir.path().join("gone.txt")).expect_err("no file");
        assert!(error.to_string().contains("does not exist"), "{error}");
    }

    #[test]
    fn each_install_layout_names_its_sibling() {
        let suffix = std::env::consts::EXE_SUFFIX;
        let places = super::beside(
            Path::new("/Apps/ElyTree.app/Contents/MacOS/ely-tree"),
            AppId::Mark,
        );
        assert!(places.contains(&PathBuf::from(format!(
            "/Apps/ElyMark.app/Contents/MacOS/ely-mark{suffix}"
        ))));
        let places = super::beside(Path::new("/target/debug/ely-tree"), AppId::Mark);
        assert_eq!(
            places[0],
            PathBuf::from(format!("/target/debug/ely-mark{suffix}"))
        );
        let places = super::beside(Path::new("/Ely Office/ElyTree/ely-tree.exe"), AppId::Mark);
        assert!(places.contains(&PathBuf::from(format!(
            "/Ely Office/ElyMark/ely-mark{suffix}"
        ))));
        let places = super::beside(Path::new("/opt/ely-office/ely-tree/bin/ely-tree"), AppId::Mark);
        assert!(places.contains(&PathBuf::from(format!(
            "/opt/ely-office/ely-mark/bin/ely-mark{suffix}"
        ))));
    }
}

#[cfg(all(test, unix))]
mod signals {
    use std::os::unix::process::ExitStatusExt as _;

    /// A starter that ignores and blocks SIGTERM, as Chromium may.
    #[test]
    fn a_started_app_still_stops_on_sigterm() {
        unsafe {
            libc::signal(libc::SIGTERM, libc::SIG_IGN);
            let mut term: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut term);
            libc::sigaddset(&mut term, libc::SIGTERM);
            libc::pthread_sigmask(libc::SIG_BLOCK, &term, std::ptr::null_mut());
        }
        let mut command = std::process::Command::new("/bin/sleep");
        command.arg("30");
        let mut child = super::fresh_signals(&mut command).spawn().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
        let status = child.wait().unwrap();
        unsafe { libc::signal(libc::SIGTERM, libc::SIG_DFL) };
        assert_eq!(status.signal(), Some(libc::SIGTERM), "{status:?}");
    }
}
