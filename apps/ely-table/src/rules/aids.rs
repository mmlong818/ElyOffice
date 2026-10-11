//! The cell's aids: the choices menu, input help, list button.

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    menus::{ContextMenu, Menu as Popup, MenuItem as PopupItem},
    primitives::{IconName, raise},
    theme::{ActiveTheme as _, ControlSize, Elevation, Radius, TextSize},
};
use ely_lok::{LokView, sheet};
use ely_office::prelude::*;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement as _, Pixels, Point, Styled as _, div,
    px,
};

use crate::book::Book;

/// The open choices menu: tab, values, request.
pub struct Choices {
    pub(crate) tab: u64,
    pub(crate) values: Vec<String>,
    request: (u64, Point<Pixels>),
}

impl Book {
    /// A cell's validation list, as a menu below.
    pub(crate) fn offer_choices(
        &mut self,
        id: u64,
        (column, row): (u32, u32),
        cx: &mut Context<Self>,
    ) {
        self.ask_in(
            id,
            move |worker, doc| {
                let json = worker.values(doc, &sheet::cell_query(sheet::LIST, column, row))?;
                sheet::list_values(&json)
            },
            move |book, read, _, cx| {
                let values = match read {
                    Ok(values) => values,
                    Err(error) => return book.engine_failed(&error.to_string(), cx),
                };
                let Some(tab) = book.tab(id) else {
                    return;
                };
                let geometry = &tab.extra.geometry;
                let bottom = (geometry.columns.start(column), geometry.rows.start(row + 1));
                let at = tab.view.read(cx).window_point(bottom);
                let number = book
                    .choices
                    .as_ref()
                    .map_or(0, |choices| choices.request.0 + 1);
                book.choices = Some(Choices {
                    tab: id,
                    values,
                    request: (number, at),
                });
                cx.notify();
            },
            cx,
        );
    }

    /// The choices menu and the help of the current cell.
    pub(crate) fn cell_aids(
        &self,
        view: &Entity<LokView>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(tab) = self.active_tab() else {
            return Vec::new();
        };
        let mut aids = Vec::new();
        if let Some(choices) = self
            .choices
            .as_ref()
            .filter(|choices| choices.tab == tab.id)
        {
            let this = cx.entity();
            let menu = choices.values.iter().fold(Popup::new(), |menu, value| {
                let (this, value) = (this.clone(), value.clone());
                menu.item(PopupItem::new(value.clone()).on_click(move |_, cx| {
                    this.update(cx, |book, cx| book.run_active("cell.enter", &[&value], cx))
                }))
            });
            aids.push(
                ContextMenu::new("table-choices", menu)
                    .manual(Some(choices.request))
                    .into_any_element(),
            );
        }
        let read = view.read(cx);
        let (Some((at, rule)), Some((x, y, w, h))) = (&tab.extra.rule, read.cell().cursor) else {
            return aids;
        };
        if at.1 != read.cell().address {
            return aids;
        }
        if rule.kind == "list" && rule.dropdown {
            let corner = read.place((x + w, y));
            let (column, row) = (
                tab.extra.geometry.columns.at(x),
                tab.extra.geometry.rows.at(y),
            );
            let (id, size) = (
                tab.id,
                f32::from(read.place((x, y + h)).y - read.place((x, y)).y),
            );
            aids.push(
                div()
                    .absolute()
                    .left(corner.x)
                    .top(corner.y)
                    .child(
                        IconButton::new("table-choices-open", IconName::ChevronDown)
                            .variant(ButtonVariant::Ghost)
                            .size(ControlSize::Sm)
                            .tooltip(Book::text(cx, "table.rules.choose", &[]))
                            .on_click(cx.listener(move |book, _, _, cx| {
                                book.offer_choices(id, (column, row), cx)
                            })),
                    )
                    .h(px(size.max(16.0)))
                    .into_any_element(),
            );
        }
        if rule.show_input && !rule.input_message.is_empty() {
            let below = read.place((x, y + h));
            let theme = cx.theme();
            let title = (!rule.input_title.is_empty()).then(|| {
                div()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(rule.input_title.clone())
            });
            let note = div()
                .absolute()
                .left(below.x)
                .top(below.y)
                .max_w(theme.tooltip_max_width())
                .px_2()
                .py_1()
                .rounded(theme.radius(Radius::Md))
                .bg(theme.colors.tooltip_bg)
                .text_color(theme.colors.tooltip_fg)
                .text_size(theme.text_size(TextSize::Sm))
                .shadow(theme.elevation(Elevation::Floating))
                .children(title)
                .child(rule.input_message.clone());
            aids.push(raise("table-input-help", note).into_any_element());
        }
        aids
    }
}
