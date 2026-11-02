//! Linux packages: which manager owns the app, and its command.

use std::{io, path::Path, process::Command};

use semver::Version;

/// The Linux package manager that installed the app.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Package {
    Deb,
    Rpm,
    Arch,
}

/// Whether this system's rpm lists `slug`.
pub(super) fn rpm_lists(slug: &str) -> io::Result<bool> {
    let status = Command::new("rpm").args(["-q", "--quiet", slug]).status()?;
    Ok(status.success())
}

impl Package {
    /// Reads the managers' own records under `root`.
    pub(super) fn of(root: &Path, slug: &str, rpm: &dyn Fn(&str) -> io::Result<bool>) -> io::Result<Self> {
        if root.join(format!("var/lib/dpkg/info/{slug}.list")).try_exists()? {
            return Ok(Self::Deb);
        }
        let pacman = root.join("var/lib/pacman/local");
        if pacman.try_exists()? {
            for entry in std::fs::read_dir(&pacman)? {
                let name = entry?.file_name();
                // pacman names it `<name>-<version>-<release>`.
                let version = name.to_str().and_then(|name| name.strip_prefix(slug)?.strip_prefix('-'));
                if version.is_some_and(|version| version.starts_with(|c: char| c.is_ascii_digit())) {
                    return Ok(Self::Arch);
                }
            }
        }
        let database = root.join("usr/lib/sysimage/rpm").try_exists()? || root.join("var/lib/rpm").try_exists()?;
        // A database alone says nothing of this app.
        if database && rpm(slug)? {
            return Ok(Self::Rpm);
        }
        Err(io::Error::new(io::ErrorKind::NotFound, format!("no package manager lists {slug}")))
    }

    /// The release's file for this manager.
    pub fn file(self, slug: &str, version: &Version) -> String {
        match self {
            Self::Deb => format!("{slug}_{version}-1_amd64.deb"),
            Self::Rpm => format!("{slug}-{version}-1.x86_64.rpm"),
            Self::Arch => format!("{slug}-{version}-1-x86_64.pkg.tar.zst"),
        }
    }

    /// The command that installs the downloaded file.
    pub fn command(self, slug: &str, version: &Version) -> String {
        let file = self.file(slug, version);
        match self {
            Self::Deb => format!("sudo apt install ./{file}"),
            Self::Rpm => format!("sudo dnf install ./{file}"),
            Self::Arch => format!("sudo pacman -U ./{file}"),
        }
    }
}
