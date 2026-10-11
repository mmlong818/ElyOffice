use ely_gpui_component::theme::{ActiveTheme as _, Mode};
use ely_platform::{
    AppId,
    settings::{Appearance, Shared},
};
use gpui::TestAppContext;

use super::{changed, window::SettingsView};

fn installed(cx: &mut TestAppContext) {
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        super::install_defaults(cx);
        crate::theme::install(cx);
        crate::i18n::install(cx);
    });
}

#[gpui::test]
fn a_change_on_disk_turns_the_theme_at_once(cx: &mut TestAppContext) {
    installed(cx);
    let dark = Shared { appearance: Appearance::Dark, ..Shared::default() };
    cx.update(|cx| changed((dark, None), cx));
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(cx.global::<crate::theme::ThemeSettings>().appearance, Appearance::Dark);
        assert_eq!(cx.theme().mode(), Mode::Dark);
    });
    let light = Shared { appearance: Appearance::Light, ..Shared::default() };
    cx.update(|cx| changed((light, None), cx));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.theme().mode(), Mode::Light));
}

#[gpui::test]
fn each_app_shows_its_own_sections(cx: &mut TestAppContext) {
    installed(cx);
    for (app, keys) in [
        (AppId::Doc, &["general", "documents", "storage", "diagnostics"][..]),
        (AppId::Pdf, &["general", "documents", "storage", "diagnostics"]),
        (AppId::Tree, &["general", "git", "storage", "diagnostics"]),
        (AppId::Mark, &["general", "storage", "diagnostics"]),
    ] {
        // As each app does before its first window.
        cx.update(|cx| {
            let menus = crate::commands::menus(app, &crate::commands::OPENING, false, Vec::new(), cx);
            cx.set_menus(menus);
        });
        let window = cx.add_window(|window, cx| SettingsView::new(app, window, cx));
        let shown = window
            .update(cx, |view, _, cx| view.sections(cx).into_iter().map(|(key, ..)| key).collect::<Vec<_>>())
            .unwrap();
        assert_eq!(shown, keys, "{app:?}");
    }
}
