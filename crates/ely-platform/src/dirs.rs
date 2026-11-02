use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::BaseDirs;

use crate::AppId;

const DATA_ROOT_ENV: &str = "ELYOFFICE_DATA_DIR";

/// Everything an app stores lives here; uninstall removes only this.
pub fn data_dir(app: AppId) -> Result<PathBuf> {
    Ok(data_root()?.join(app.slug()))
}

/// The suite's selected data folder; each app uses its own subdirectory.
///
/// `ELYOFFICE_DATA_DIR` is the fork's historical explicit root. Otherwise,
/// use the v0.1.13 folder selection and its `ELY_DATA_ROOT` test override.
pub fn data_root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(DATA_ROOT_ENV) {
        return root_override(PathBuf::from(path));
    }
    root()
}

pub(crate) fn root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(DATA_ROOT_ENV) {
        return root_override(PathBuf::from(path));
    }
    Ok(crate::folder::read(&home()?)?.at)
}

/// The first folder, holding the pointer to the chosen one.
pub(crate) fn home() -> Result<PathBuf> {
    if let Some(root) = chosen_root(crate::env::value("ELY_DATA_ROOT"))? {
        return Ok(root);
    }
    let base = BaseDirs::new().context("no home directory")?;
    Ok(base.data_local_dir().join("ElyOffice"))
}

fn root_override(path: PathBuf) -> Result<PathBuf> {
    anyhow::ensure!(
        path.is_absolute(),
        "{DATA_ROOT_ENV} must be an absolute path: {}",
        path.display()
    );
    Ok(path)
}

/// Benchmarks only: a seeded folder, never the user's.
fn chosen_root(over: Option<String>) -> Result<Option<PathBuf>> {
    let Some(root) = over else { return Ok(None) };
    let root = PathBuf::from(root);
    anyhow::ensure!(root.is_absolute(), "ELY_DATA_ROOT is not absolute: {}", root.display());
    Ok(Some(root))
}

/// Omarchy's active theme directory; `ELY_OMARCHY_DIR` overrides it.
pub fn omarchy_current_dir() -> Option<PathBuf> {
    let home = BaseDirs::new()?.home_dir().to_path_buf();
    omarchy_dir(&home, crate::env::value("ELY_OMARCHY_DIR"))
}

/// Omarchy keeps its state under `~/.local/state`, whatever `XDG_STATE_HOME` says.
fn omarchy_dir(home: &Path, over: Option<String>) -> Option<PathBuf> {
    if let Some(dir) = over {
        return Some(PathBuf::from(dir));
    }
    let current = home.join(".local/state/omarchy/current");
    current.is_dir().then_some(current)
}

#[cfg(test)]
mod tests {
    use super::{chosen_root, data_dir, data_root, omarchy_dir, root_override};
    use crate::AppId;
    use std::path::PathBuf;

    #[test]
    fn each_app_has_its_own_directory_under_elyoffice() {
        let doc = data_dir(AppId::Doc).unwrap();
        let mail = data_dir(AppId::Mail).unwrap();

        assert!(doc.ends_with("ElyOffice/ely-doc"), "{}", doc.display());
        assert!(mail.ends_with("ElyOffice/ely-mail"), "{}", mail.display());
        assert_ne!(doc, mail);
    }

    #[test]
    fn explicit_root_requires_absolute_path_and_keeps_app_directories_separate() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("suite-data");
        assert!(root.is_absolute());
        let dirs: Vec<_> = AppId::ALL
            .into_iter()
            .map(|app| root.join(app.slug()))
            .collect();
        assert!(dirs.iter().all(|dir| dir.starts_with(&root)));
        for (i, dir) in dirs.iter().enumerate() {
            assert!(
                !dirs[..i].contains(dir),
                "duplicate app data path: {}",
                dir.display()
            );
        }
    }

    #[test]
    fn all_app_data_paths_use_the_absolute_environment_root_in_a_child_process() {
        const CHILD: &str = "ELYOFFICE_DATA_DIR_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let root = data_root().expect("the explicit suite data root");
            for app in AppId::ALL {
                let path = data_dir(app).expect("the app data path");
                assert_eq!(path, root.join(app.slug()));
                println!("{}={}", app.slug(), path.display());
            }
            return;
        }

        let temp = tempfile::tempdir().unwrap();
        let requested_root = temp.path().join("suite data root");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "dirs::tests::all_app_data_paths_use_the_absolute_environment_root_in_a_child_process",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env(super::DATA_ROOT_ENV, &requested_root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        for app in AppId::ALL {
            assert!(
                stdout.contains(&format!(
                    "{}={}",
                    app.slug(),
                    requested_root.join(app.slug()).display()
                )),
                "child output omitted app data path for {}: {stdout}",
                app.slug()
            );
        }
    }

    #[test]
    fn a_relative_environment_root_is_rejected_instead_of_falling_back() {
        const CHILD: &str = "ELYOFFICE_DATA_DIR_TEST_INVALID_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let error = data_root().unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("ELYOFFICE_DATA_DIR must be an absolute path")
            );
            return;
        }

        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "dirs::tests::a_relative_environment_root_is_rejected_instead_of_falling_back",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env(super::DATA_ROOT_ENV, "relative-profile")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let relative = PathBuf::from("relative-profile");
        assert!(!relative.is_absolute());
        let error = root_override(relative).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("ELYOFFICE_DATA_DIR must be an absolute path")
        );
    }

    #[test]
    fn a_data_root_override_must_be_absolute_and_wins() {
        let at = std::env::temp_dir().join("ely-bench");
        let chosen = chosen_root(Some(at.display().to_string())).unwrap();
        assert_eq!(chosen, Some(at));
        assert_eq!(chosen_root(None).unwrap(), None);
        let err = chosen_root(Some("relative".into())).unwrap_err();
        assert!(err.to_string().contains("not absolute"), "{err}");
    }

    #[test]
    fn omarchy_is_found_under_home_and_the_override_wins() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(omarchy_dir(home.path(), None), None);

        let current = home.path().join(".local/state/omarchy/current");
        std::fs::create_dir_all(&current).unwrap();
        assert_eq!(omarchy_dir(home.path(), None), Some(current));
        assert_eq!(
            omarchy_dir(home.path(), Some("/elsewhere".into())),
            Some("/elsewhere".into())
        );
    }
}
