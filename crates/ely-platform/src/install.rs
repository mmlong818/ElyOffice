//! Puts a downloaded release in place of the running app.

use std::{
    io,
    path::{Path, PathBuf},
};
#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};

/// The `.app` running this process; none in dev builds.
pub fn running_bundle() -> io::Result<Option<PathBuf>> {
    let exe = std::env::current_exe()?;
    Ok(bundle_of(&exe).map(Path::to_path_buf))
}

fn bundle_of(exe: &Path) -> Option<&Path> {
    exe.ancestors().find(|dir| dir.extension().is_some_and(|ext| ext == "app"))
}

/// Replaces `bundle` with the same-named app inside `dmg`.
#[cfg(target_os = "macos")]
pub fn install(dmg: &Path, bundle: &Path) -> io::Result<()> {
    let name = bundle.file_name().ok_or_else(|| io::Error::other("the app has no name"))?;
    let parent = bundle.parent().ok_or_else(|| io::Error::other("the app has no folder"))?;
    let mount = tempfile::tempdir()?;
    run(Command::new("hdiutil").args(["attach", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint"]).arg(mount.path()).arg(dmg))?;
    let copied = stage(&mount.path().join(name), parent, name);
    let detached = run(Command::new("hdiutil").args(["detach", "-force"]).arg(mount.path()));
    let staged = copied?;
    detached?;
    swap(&staged, bundle)
}

#[cfg(not(target_os = "macos"))]
pub fn install(_dmg: &Path, _bundle: &Path) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "installing updates is not built for this platform"))
}

/// Copies the new app beside the old, hidden.
#[cfg(target_os = "macos")]
fn stage(from: &Path, parent: &Path, name: &std::ffi::OsStr) -> io::Result<PathBuf> {
    if !from.is_dir() {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("the download holds no {}", name.to_string_lossy())));
    }
    let staged = parent.join(format!(".{}.new", name.to_string_lossy()));
    if staged.exists() {
        std::fs::remove_dir_all(&staged)?;
    }
    run(Command::new("ditto").arg(from).arg(&staged))?;
    Ok(staged)
}

/// Puts `staged` at `bundle`; on failure the old returns.
#[cfg(any(target_os = "macos", test))]
fn swap(staged: &Path, bundle: &Path) -> io::Result<()> {
    let name = bundle.file_name().ok_or_else(|| io::Error::other("the app has no name"))?;
    let old = bundle.with_file_name(format!(".{}.old", name.to_string_lossy()));
    if old.exists() {
        std::fs::remove_dir_all(&old)?;
    }
    std::fs::rename(bundle, &old)?;
    if let Err(error) = std::fs::rename(staged, bundle) {
        std::fs::rename(&old, bundle)?;
        return Err(error);
    }
    log::info!("install: {} replaced", bundle.display());
    // Installed: an old copy left behind is only untidy.
    if let Err(error) = std::fs::remove_dir_all(&old) {
        log::warn!("install: the old copy at {} stays: {error}", old.display());
    }
    Ok(())
}

/// Returns once process `pid` has ended.
#[cfg(unix)]
pub fn wait_for_exit(pid: u32) {
    while unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

#[cfg(not(unix))]
pub fn wait_for_exit(_pid: u32) {}

/// Opens `bundle` as a fresh launch.
#[cfg(target_os = "macos")]
pub fn reopen(bundle: &Path) -> io::Result<()> {
    run(Command::new("open").arg(bundle))
}

#[cfg(not(target_os = "macos"))]
pub fn reopen(_bundle: &Path) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "reopening after an update is not built for this platform"))
}

/// Runs `command`; a failure carries what it said.
#[cfg(target_os = "macos")]
fn run(command: &mut Command) -> io::Result<()> {
    let out = command.stdin(Stdio::null()).output()?;
    if out.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&out.stderr);
    Err(io::Error::other(format!("{:?} failed ({}): {}", command.get_program(), out.status, said.trim())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundle_is_the_nearest_app_folder() {
        let exe = Path::new("/Applications/ElyDoc.app/Contents/MacOS/ely-doc");
        assert_eq!(bundle_of(exe), Some(Path::new("/Applications/ElyDoc.app")));
        assert_eq!(bundle_of(Path::new("/repo/target/debug/ely-doc")), None);
    }

    #[test]
    fn a_swap_replaces_the_app_and_leaves_no_copy() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let (bundle, staged) = (dir.path().join("ElyDoc.app"), dir.path().join(".ElyDoc.app.new"));
        std::fs::create_dir_all(bundle.join("old")).expect("the old app");
        std::fs::create_dir_all(staged.join("new")).expect("the new app");
        swap(&staged, &bundle).expect("swaps");
        assert!(bundle.join("new").exists() && !bundle.join("old").exists());
        assert_eq!(std::fs::read_dir(dir.path()).expect("lists").count(), 1, "nothing else is left");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn an_old_copy_that_will_not_go_leaves_the_install_done() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let (bundle, staged) = (dir.path().join("ElyDoc.app"), dir.path().join(".ElyDoc.app.new"));
        std::fs::create_dir_all(&bundle).expect("the old app");
        std::fs::write(bundle.join("version"), "old").expect("marks it");
        std::fs::create_dir_all(&staged).expect("the new app");
        std::fs::write(staged.join("version"), "new").expect("marks it");
        run(Command::new("chflags").arg("uchg").arg(bundle.join("version"))).expect("locks a file");
        swap(&staged, &bundle).expect("installed although the old copy stays");
        assert_eq!(std::fs::read_to_string(bundle.join("version")).expect("reads"), "new");
        let old = dir.path().join(".ElyDoc.app.old");
        run(Command::new("chflags").arg("nouchg").arg(old.join("version"))).expect("unlocks");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_dmg_installs_over_the_app() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let source = dir.path().join("src");
        std::fs::create_dir_all(source.join("ElyDoc.app/Contents")).expect("the new app");
        std::fs::write(source.join("ElyDoc.app/Contents/version"), "new").expect("marks it");
        let dmg = dir.path().join("ElyDoc.dmg");
        run(Command::new("hdiutil").args(["create", "-quiet", "-fs", "HFS+", "-srcfolder"]).arg(&source).arg(&dmg)).expect("a dmg");
        let bundle = dir.path().join("Applications/ElyDoc.app");
        std::fs::create_dir_all(bundle.join("Contents")).expect("the old app");
        std::fs::write(bundle.join("Contents/version"), "old").expect("marks it");
        install(&dmg, &bundle).expect("installs");
        assert_eq!(std::fs::read_to_string(bundle.join("Contents/version")).expect("reads"), "new");
        let empty = dir.path().join("Applications/Other.app");
        assert!(install(&dmg, &empty).is_err(), "a dmg without that app is refused");
    }
}
