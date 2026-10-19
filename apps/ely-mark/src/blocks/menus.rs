use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use gpui::{
    App, Entity, KeyUpEvent, Keystroke, Menu, Modifiers, TestAppContext, VisualTestContext, point,
    px,
};

use super::tests::{current, document, opened, press};
use crate::{desk::Desk, view::Mode};

/// Selects `alpha` in the first block, opened.
fn select_alpha(desk: &Entity<Desk>, cx: &mut VisualTestContext) {
    let bounds = cx.debug_bounds("markdown-block-0").expect("a block");
    let at = point(bounds.left() + px(4.), bounds.top() + px(4.));
    cx.simulate_click(at, Modifiers::none());
    press(
        "home shift-right shift-right shift-right shift-right shift-right",
        cx,
    );
    let selected = document(desk, cx).read_with(cx, |document, cx| document.selected(cx));
    assert_eq!(selected, "alpha");
}

/// Runs `menu`'s row `row` from the title bar.
fn choose(menu: fn(&App) -> Menu, row: usize, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        cx.set_menus(vec![menu(cx)]);
        window.refresh();
    });
    cx.run_until_parked();
    let at = cx.update(|window, cx| {
        let rem = window.rem_size();
        let bar = cx.theme().titlebar_height().to_pixels(rem);
        let title = cx.theme().control_height(ControlSize::Sm).to_pixels(rem);
        point(px(12.), bar + title / 2.)
    });
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    press(&format!("{} enter", vec!["down"; row].join(" ")), cx);
    let enter = Keystroke::parse("enter").expect("a key");
    cx.simulate_event(KeyUpEvent { keystroke: enter });
    cx.run_until_parked();
}

fn edit_menu(cx: &App) -> Menu {
    let look = crate::look::Look::default();
    crate::keys::menus(&look, cx).remove(0)
}

#[gpui::test]
fn a_menu_formats_the_open_blocks_selection(cx: &mut TestAppContext) {
    let (desk, cx, _files) = opened("alpha beta\n", cx);
    select_alpha(&desk, cx);
    choose(super::format_menu, 1, cx);
    assert_eq!(current(&desk, cx), "**alpha** beta\n");
}

#[gpui::test]
fn a_menu_copies_the_open_blocks_selection(cx: &mut TestAppContext) {
    let (desk, cx, _files) = opened("alpha beta\n", cx);
    select_alpha(&desk, cx);
    // Copy as Markdown, after Copy as Plain Text.
    choose(edit_menu, 2, cx);
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some("alpha"));
}

const TABLED: &str = "before\n\n| A |\n| - |\n| 1 |\n\nafter\n";

#[gpui::test]
fn delete_table_from_the_menu_is_one_undo_step(cx: &mut TestAppContext) {
    let (desk, cx, _files) = opened(TABLED, cx);
    desk.update_in(cx, |desk, window, cx| desk.switch(Mode::Source, window, cx));
    let document = document(&desk, cx);
    document.update(cx, |document, cx| document.set_caret(4, 2, cx));
    choose(crate::tools::table_menu, 13, cx);
    assert_eq!(current(&desk, cx), "before\n\nafter\n");
    press("secondary-z", cx);
    assert_eq!(current(&desk, cx), TABLED);
}

#[gpui::test]
fn delete_table_in_an_open_block_keeps_keyboard_undo(cx: &mut TestAppContext) {
    let (desk, cx, _files) = opened(TABLED, cx);
    let bounds = cx.debug_bounds("markdown-block-1").expect("the table");
    let at = point(bounds.left() + px(4.), bounds.top() + px(4.));
    cx.simulate_click(at, Modifiers::none());
    choose(crate::tools::table_menu, 13, cx);
    assert_eq!(current(&desk, cx), "before\n\nafter\n");
    press("secondary-z", cx);
    assert_eq!(current(&desk, cx), TABLED);
}
