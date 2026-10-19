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

/// A main window's frame.
struct Main(gpui::FocusHandle);

impl gpui::Render for Main {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        crate::shell::Shell::new("main", &self.0)
    }
}

/// How far below the title bar the content starts.
fn below_title(cx: &mut TestAppContext, window: gpui::AnyWindowHandle) -> gpui::Pixels {
    let mut shown = gpui::VisualTestContext::from_window(window, cx);
    for _ in 0..3 {
        shown.update(|window, _| window.refresh());
        shown.run_until_parked();
    }
    let title = shown.debug_bounds("shell-title").expect("a title bar");
    shown.debug_bounds("shell-body").expect("a body").origin.y - title.bottom()
}

#[gpui::test]
fn only_the_main_window_carries_menus_and_updates(cx: &mut TestAppContext) {
    installed(cx);
    cx.update(|cx| {
        let menus = crate::commands::menus(AppId::Mark, &crate::commands::OPENING, false, Vec::new(), cx);
        cx.set_menus(menus);
    });
    let main = cx.add_window(|_, cx| Main(cx.focus_handle()));
    let menus = below_title(cx, main.into());
    // macOS keeps menus in the system bar.
    match cfg!(target_os = "macos") {
        true => assert_eq!(menus, gpui::px(0.0), "no menu row on macOS"),
        false => assert!(menus > gpui::px(0.0), "the main window shows the menus"),
    }
    cx.update(crate::updates::offer_for_tests);
    let main = cx.add_window(|_, cx| Main(cx.focus_handle()));
    assert!(below_title(cx, main.into()) > menus, "the main window shows the update");
    let settings = cx.add_window(|window, cx| SettingsView::new(AppId::Mark, window, cx));
    assert_eq!(below_title(cx, settings.into()), gpui::px(0.0), "settings repeats neither");
}
