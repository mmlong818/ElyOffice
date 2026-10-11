//! A newer release: offered, downloaded, installed after quitting.

use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::AtomicBool,
};

use ely_gpui_component::shell::{UpdateBanner, UpdateState};
use ely_platform::{
    AppId,
    update::{self, Check, Release},
};
use futures::{StreamExt as _, channel::mpsc};
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, Global, IntoElement, Render, Window, div,
};

/// The release server's newest release, listed by its API.
const LATEST: &str = "https://git.zacharyzhang.com/api/v1/repos/ZacharyZhang-NY/ElyOffice/releases/latest";
/// The argument that runs this binary as the installer.
const INSTALL: &str = "--ely-install-update";

#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Offered,
    Fetching(f32),
    Fetched(PathBuf),
}

pub struct Updates {
    app: AppId,
    bundle: PathBuf,
    offer: Option<(Release, Phase)>,
    dismissed: bool,
    /// The download to install once the app has quit.
    pending: Option<PathBuf>,
    /// Why the last install failed, for the window to say.
    pub note: Option<String>,
}

/// What an update says to the window.
pub enum UpdateNews {
    Failed(String),
    /// A check the user asked for found nothing newer.
    Latest,
}

impl EventEmitter<UpdateNews> for Updates {}

struct Updater(Entity<Updates>);

impl Global for Updater {}

/// The app's updater, if it runs from an installed bundle.
pub fn updater(cx: &App) -> Option<Entity<Updates>> {
    cx.try_global::<Updater>().map(|updater| updater.0.clone())
}

/// Checks now, as the user asked from the menu.
pub fn check_now(cx: &mut App) {
    if let Some(updates) = updater(cx) {
        updates.update(cx, |updates, cx| updates.check(true, cx));
    }
}

/// The quit an update asked for stopped: install nothing.
pub fn quit_stopped(cx: &mut App) {
    if let Some(updates) = updater(cx) {
        updates.update(cx, |updates, _| {
            if updates.pending.take().is_some() {
                log::info!("update: the quit stopped; nothing installs");
            }
        });
    }
}

/// Run as the installer: waits, installs, reopens; the exit code.
pub fn installer(app: AppId) -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    let [_, flag, dmg, bundle, pid] = args.as_slice() else {
        return None;
    };
    if flag != INSTALL {
        return None;
    }
    let Ok(pid) = pid.parse() else {
        eprintln!("update: {pid} is not a process number");
        return Some(2);
    };
    ely_platform::install::wait_for_exit(pid);
    let installed = ely_platform::install::install(Path::new(dmg), Path::new(bundle));
    if let Err(error) = &installed {
        // To the app's log; the reopened app shows it.
        eprintln!("update: the install failed: {error}");
        let note = ely_platform::data_dir(app)
            .map_err(io::Error::other)
            .and_then(|dir| {
                std::fs::create_dir_all(dir.join("updates"))?;
                std::fs::write(dir.join("updates/failed.txt"), error.to_string())
            });
        if let Err(lost) = note {
            eprintln!("update: the failure note was not written: {lost}");
        }
    }
    // Last: the note is written before the app reads it.
    if let Err(error) = ely_platform::install::reopen(Path::new(bundle)) {
        eprintln!("update: the app did not reopen: {error}");
        return Some(1);
    }
    Some(i32::from(installed.is_err()))
}

/// An installed app's updater; checks at launch if chosen.
pub(crate) fn install(app: AppId, cx: &mut App) {
    let bundle = match ely_platform::install::running_bundle() {
        Ok(Some(bundle)) => bundle,
        Ok(None) => return log::info!("update: a dev build; no check"),
        Err(error) => return log::warn!("update: the running app is not found: {error}"),
    };
    match update::install_policy() {
        Ok(update::InstallPolicy::SelfManaged) => {}
        Ok(update::InstallPolicy::PackageManager) => {
            return log::info!("update: the package manager updates this app");
        }
        Err(error) => return log::warn!("update: {error:#}"),
    }
    let note = ely_platform::data_dir(app)
        .ok()
        .map(|dir| dir.join("updates/failed.txt"));
    let note = note.and_then(|path| {
        std::fs::read_to_string(&path)
            .ok()
            .inspect(|_| drop(std::fs::remove_file(&path)))
    });
    let updates = cx.new(|cx| {
        cx.on_app_quit(|this: &mut Updates, _| {
            this.start_installer();
            async {}
        })
        .detach();
        Updates {
            app,
            bundle,
            offer: None,
            dismissed: false,
            pending: None,
            note,
        }
    });
    cx.set_global(Updater(updates.clone()));
    // Off by default: no request unless the user chose it.
    if crate::settings::current(cx).check_updates {
        updates.update(cx, |updates, cx| updates.check(false, cx));
    } else {
        log::info!("update: no check at launch; it is off in settings");
    }
}

/// The app's manifest in the newest release, or the test's.
fn manifest(app: AppId) -> anyhow::Result<String> {
    match std::env::var("ELY_UPDATE_MANIFEST") {
        Ok(url) => Ok(url),
        Err(_) => ely_platform::release::asset_url(LATEST, &format!("{}.json", app.slug())),
    }
}

impl Updates {
    /// Asks for a newer release; `asked` reports every outcome.
    fn check(&mut self, asked: bool, cx: &mut Context<Self>) {
        // A download under way or done stays the offer.
        if matches!(
            self.offer,
            Some((_, Phase::Fetching(_) | Phase::Fetched(_)))
        ) {
            log::info!("update: a release is already on its way");
            if asked {
                self.dismissed = false;
                cx.notify();
            }
            return;
        }
        let app = self.app;
        let current =
            semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("the build's version");
        // The lookup is a request too: off the window's thread.
        let checked = cx.background_spawn(async move { manifest(app).and_then(|url| update::check(&current, &url)) });
        cx.spawn(async move |this, cx| {
            let checked = checked.await;
            this.update(cx, |this, cx| match checked {
                Ok(Check::Available(_))
                    if matches!(
                        this.offer,
                        Some((_, Phase::Fetching(_) | Phase::Fetched(_)))
                    ) => {}
                Ok(Check::Available(release)) => {
                    (this.offer, this.dismissed) = (Some((release, Phase::Offered)), false);
                    cx.notify();
                }
                Ok(Check::Current) if asked => cx.emit(UpdateNews::Latest),
                // At launch a failure only logs; offline is normal.
                Err(error) if asked => cx.emit(UpdateNews::Failed(format!("{error:#}"))),
                Err(error) => log::warn!("update: the launch check failed: {error:#}"),
                Ok(Check::Current) => log::info!("update: nothing newer at launch"),
            })
            .ok();
        })
        .detach();
    }

    fn later(&mut self, cx: &mut Context<Self>) {
        self.dismissed = true;
        cx.notify();
    }

    fn download(&mut self, cx: &mut Context<Self>) {
        let Some((release, phase)) = self
            .offer
            .as_mut()
            .filter(|(_, phase)| *phase == Phase::Offered)
        else {
            return;
        };
        *phase = Phase::Fetching(0.0);
        cx.notify();
        let (release, app) = (release.clone(), self.app);
        let (sender, mut progress) = mpsc::unbounded();
        let fetching = release.clone();
        let fetched = cx.background_spawn(async move {
            let dir = ely_platform::data_dir(app)
                .map_err(|error| format!("{error:#}"))?
                .join("updates");
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|error| error.to_string())?;
            }
            std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
            let cancel = AtomicBool::new(false);
            let mut tell = |done: u64, total: Option<u64>| {
                let part = total
                    .filter(|total| *total > 0)
                    .map_or(0.0, |total| done as f32 / total as f32);
                sender.unbounded_send(part).ok();
            };
            let file = update::download(&fetching, &dir, &cancel, update::IDLE_TIMEOUT, &mut tell)
                .map_err(|error| format!("{error:#}"))?
                .ok_or("the download stopped")?;
            update::verify_file(&file, &fetching.sha256).map_err(|error| format!("{error:#}"))?;
            Ok::<PathBuf, String>(file)
        });
        cx.spawn(async move |this, cx| {
            while let Some(part) = progress.next().await {
                this.update(cx, |this, cx| this.fetching(part, cx)).ok();
            }
            let fetched = fetched.await;
            this.update(cx, |this, cx| this.fetched(&release, fetched, cx))
                .ok();
        })
        .detach();
    }

    fn fetching(&mut self, part: f32, cx: &mut Context<Self>) {
        if let Some((_, phase @ Phase::Fetching(_))) = self.offer.as_mut() {
            *phase = Phase::Fetching(part);
            cx.notify();
        }
    }

    /// A finished download counts only for the release it fetched.
    fn fetched(
        &mut self,
        release: &Release,
        fetched: Result<PathBuf, String>,
        cx: &mut Context<Self>,
    ) {
        let Some((offered, phase)) = self
            .offer
            .as_mut()
            .filter(|(offered, _)| offered == release)
        else {
            return log::warn!("update: a download for another release was dropped");
        };
        match fetched {
            Ok(file) => *phase = Phase::Fetched(file),
            Err(error) => {
                *phase = Phase::Offered;
                cx.emit(UpdateNews::Failed(error));
            }
        }
        log::info!("update: {} is {:?}", offered.version, phase);
        cx.notify();
    }

    /// Quits as usual: unsaved work is asked about first.
    fn restart(&mut self, cx: &mut Context<Self>) {
        let Some((_, Phase::Fetched(file))) = self.offer.as_ref() else {
            return;
        };
        self.pending = Some(file.clone());
        crate::commands::quit(cx);
    }

    /// At the real quit: the installer waits for this process.
    fn start_installer(&mut self) {
        let Some(file) = self.pending.take() else {
            return;
        };
        let started = std::env::current_exe().and_then(|exe| {
            let mut command = Command::new(exe);
            command
                .arg(INSTALL)
                .arg(&file)
                .arg(&self.bundle)
                .arg(std::process::id().to_string());
            // Its words go to the app's log.
            let log = ely_platform::logging::path()
                .map(|path| std::fs::OpenOptions::new().append(true).open(path));
            let said = match log {
                Some(file) => Stdio::from(file?),
                None => Stdio::null(),
            };
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(said);
            #[cfg(unix)]
            std::os::unix::process::CommandExt::process_group(&mut command, 0);
            command.spawn()
        });
        match started {
            Ok(_) => log::info!("update: installs after this quit"),
            Err(error) => log::error!("update: the installer did not start: {error}"),
        }
    }
}

impl Render for Updates {
    /// The bar under the title while a release is offered.
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let Some((release, phase)) = self.offer.as_ref().filter(|_| !self.dismissed) else {
            return div().into_any_element();
        };
        let state = match phase {
            Phase::Offered => UpdateState::Available,
            Phase::Fetching(done) => UpdateState::Downloading(*done),
            Phase::Fetched(_) => UpdateState::Ready,
        };
        let (download, restart, later) = (this.clone(), this.clone(), this);
        let banner = UpdateBanner::new("update-banner", release.version.to_string(), state)
            .on_download(move |_, cx| download.update(cx, |this, cx| this.download(cx)))
            .on_later(move |_, cx| later.update(cx, |this, cx| this.later(cx)));
        match phase {
            Phase::Fetched(_) => banner
                .on_restart(move |_, cx| restart.update(cx, |this, cx| this.restart(cx)))
                .into_any_element(),
            _ => banner.into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: &str, url: &str) -> Release {
        Release {
            version: semver::Version::parse(version).expect("a version"),
            url: url.into(),
            sha256: String::new(),
        }
    }

    #[gpui::test]
    fn a_check_brings_back_a_dismissed_download(cx: &mut gpui::TestAppContext) {
        let updates = cx.new(|_| Updates {
            app: AppId::Tree,
            bundle: PathBuf::from("/Applications/ElyTree.app"),
            offer: Some((
                release("9.0.0", "https://b"),
                Phase::Fetched(PathBuf::from("B.dmg")),
            )),
            dismissed: true,
            pending: None,
            note: None,
        });
        updates.update(cx, |updates, cx| updates.check(true, cx));
        updates.read_with(cx, |updates, _| {
            assert!(!updates.dismissed, "the bar shows again");
            assert_eq!(
                updates.offer.as_ref().map(|(_, phase)| phase.clone()),
                Some(Phase::Fetched(PathBuf::from("B.dmg")))
            );
        });
    }

    #[gpui::test]
    fn a_download_for_an_older_offer_is_dropped(cx: &mut gpui::TestAppContext) {
        let updates = cx.new(|_| Updates {
            app: AppId::Tree,
            bundle: PathBuf::from("/Applications/ElyTree.app"),
            offer: Some((release("9.0.0", "https://b"), Phase::Fetching(0.5))),
            dismissed: false,
            pending: None,
            note: None,
        });
        updates.update(cx, |updates, cx| {
            updates.fetched(
                &release("8.0.0", "https://a"),
                Ok(PathBuf::from("A.dmg")),
                cx,
            );
            assert_eq!(
                updates.offer.as_ref().map(|(_, phase)| phase.clone()),
                Some(Phase::Fetching(0.5))
            );
            updates.fetched(
                &release("9.0.0", "https://b"),
                Ok(PathBuf::from("B.dmg")),
                cx,
            );
            assert_eq!(
                updates.offer.as_ref().map(|(_, phase)| phase.clone()),
                Some(Phase::Fetched(PathBuf::from("B.dmg")))
            );
        });
    }
}
