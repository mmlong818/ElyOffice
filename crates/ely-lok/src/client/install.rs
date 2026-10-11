//! Where the LOK build and the worker binary are.

use std::{
    io,
    path::{Path, PathBuf},
};

/// The kernel's bundled templates of `kind`, such as `presnt`.
pub fn templates(kind: &str) -> io::Result<Vec<PathBuf>> {
    let install = install_dir()?;
    let root = install
        .parent()
        .ok_or_else(|| io::Error::other("the install has no parent"))?;
    let common = match cfg!(target_os = "macos") {
        true => root.join("Resources/template/common"),
        false => root.join("share/template/common"),
    };
    let mut found = std::fs::read_dir(common.join(kind))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    found.sort();
    Ok(found)
}

/// The LOK build: `ELY_LOK_DIR`, the app bundle's, or engines/.
pub fn install_dir() -> io::Result<PathBuf> {
    if let Ok(dir) = std::env::var("ELY_LOK_DIR") {
        let path = PathBuf::from(dir);
        return if path.try_exists()? {
            Ok(path)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("ELY_LOK_DIR {} does not exist", path.display()),
            ))
        };
    }
    let exe = std::env::current_exe()?;
    let app_dir = exe.parent().unwrap_or(Path::new("."));
    #[cfg(target_os = "macos")]
    {
        let bundled = app_dir.join("../Frameworks");
        if bundled.join("libmergedlo.dylib").try_exists()? {
            return Ok(bundled);
        }
    }
    #[cfg(windows)]
    {
        let bundled = app_dir.join("engines/libreoffice/program");
        if bundled.join("mergedlo.dll").try_exists()? {
            return Ok(bundled);
        }
    }

    #[cfg(windows)]
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../engines/libreoffice/program");
    #[cfg(not(windows))]
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../engines/lo-src/instdir/LibreOffice.app/Contents/Frameworks");
    if dev.try_exists()? {
        return Ok(dev);
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no LOK build (set ELY_LOK_DIR)",
    ))
}

/// The worker binary: env override, else beside this exe.
pub(super) fn worker_bin() -> io::Result<PathBuf> {
    if let Ok(bin) = std::env::var("ELY_LOK_WORKER") {
        if bin.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "ELY_LOK_WORKER is empty",
            ));
        }
        let path = PathBuf::from(&bin);
        return if path.try_exists()? {
            Ok(path)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("ELY_LOK_WORKER {bin} does not exist"),
            ))
        };
    }
    let sibling = std::env::current_exe()?
        .parent()
        .unwrap_or(Path::new("."))
        .join(if cfg!(windows) {
            "ely-lok-worker.exe"
        } else {
            "ely-lok-worker"
        });
    if sibling.try_exists()? {
        return Ok(sibling);
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug")
        .join(if cfg!(windows) {
            "ely-lok-worker.exe"
        } else {
            "ely-lok-worker"
        });
    if dev.try_exists()? {
        return Ok(dev);
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no ely-lok-worker binary (set ELY_LOK_WORKER)",
    ))
}
