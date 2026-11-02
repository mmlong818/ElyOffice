//! The bar under the title while a release is offered.

use ely_gpui_component::{
    feedback::Banner,
    primitives::Severity,
    shell::{UpdateBanner, UpdateState},
};
use gpui::{ClipboardItem, Context, IntoElement, Render, Window, div};

use super::{Phase, Updates};

impl Render for Updates {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let Some((release, phase)) = self.offer.as_ref().filter(|_| !self.dismissed) else {
            return div().into_any_element();
        };
        // A package manager installs it; Ely names the command.
        if let Some(command) = self.command() {
            let version = release.version.to_string();
            let args = [("app", self.app.name()), ("version", version.as_str()), ("command", command.as_str())];
            let said = crate::i18n::text(cx, "updates.package", &args);
            let later = this.clone();
            return Banner::new("update-package", Severity::Info, said)
                .action(crate::i18n::text(cx, "updates.copy", &[]), move |_, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(command.clone()));
                })
                .on_dismiss(move |_, cx| later.update(cx, |this, cx| this.later(cx)))
                .into_any_element();
        }
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

/// An offered release, for window tests.
#[cfg(test)]
pub(crate) fn offer_for_tests(cx: &mut gpui::App) {
    use gpui::AppContext as _;
    let release = ely_platform::update::Release {
        version: semver::Version::new(9, 0, 0),
        url: "https://example.test/9.dmg".into(),
        sha256: String::new(),
    };
    let updates = cx.new(|_| Updates {
        app: ely_platform::AppId::Mark,
        installed: ely_platform::install::Installed::Bundle(Default::default()),
        offer: Some((release, Phase::Offered)),
        dismissed: false,
        pending: None,
        checked: None,
        note: None,
    });
    cx.set_global(super::Updater(updates));
}
