//! The table group: rows, columns, cells, borders, flow (DOC-07).

use ely_gpui_component::{buttons::Button, data_display::PropertyGroup, theme::ControlSize};
use ely_office::panel::{button, color_state, palette, row, send, toggle};
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, div};

use super::Props;
use crate::docs::Docs;

impl Props {
    pub(super) fn table_group(&self, cx: &mut Context<Self>) -> PropertyGroup {
        let tip = |key: &str| Docs::text(cx, key, &[]);
        let why = "doc.why.selection";
        PropertyGroup::new(tip("doc.props.table"))
            .row(
                tip("doc.props.rows"),
                div()
                    .debug_selector(|| "doc-props-table".into())
                    .child(row([
                        button(self, "table.row_above", "doc.props.above_row", why, cx),
                        button(self, "table.row_below", "doc.props.below_row", why, cx),
                        button(self, "table.delete_rows", "doc.props.delete", why, cx),
                    ])),
            )
            .row(
                tip("doc.props.columns"),
                row([
                    button(self, "table.column_left", "doc.props.left_column", why, cx),
                    button(
                        self,
                        "table.column_right",
                        "doc.props.right_column",
                        why,
                        cx,
                    ),
                    button(self, "table.delete_columns", "doc.props.delete", why, cx),
                ]),
            )
            .row(
                tip("doc.props.cells"),
                row([
                    button(self, "table.merge", "doc.props.merge", "doc.why.merge", cx),
                    self.steps(
                        "doc-props-split-columns",
                        "doc.props.split_columns",
                        &[("table.split", &["2", "false"])],
                        cx,
                    ),
                    self.steps(
                        "doc-props-split-rows",
                        "doc.props.split_rows",
                        &[("table.split", &["2", "true"])],
                        cx,
                    ),
                ]),
            )
            .row(
                tip("doc.props.cell_size"),
                row([
                    self.steps(
                        "doc-props-even-columns",
                        "doc.props.even_columns",
                        &[("table.select", &[]), ("table.even_columns", &[])],
                        cx,
                    ),
                    self.steps(
                        "doc-props-even-rows",
                        "doc.props.even_rows",
                        &[("table.select", &[]), ("table.even_rows", &[])],
                        cx,
                    ),
                    self.steps(
                        "doc-props-fit-rows",
                        "doc.props.fit_rows",
                        &[("table.select", &[]), ("table.optimal_height", &[])],
                        cx,
                    ),
                ]),
            )
            .row(
                tip("doc.props.borders"),
                row([
                    button(self, "table.borders_all", "doc.props.borders_all", why, cx),
                    button(
                        self,
                        "table.borders_none",
                        "doc.props.borders_none",
                        why,
                        cx,
                    ),
                ]),
            )
            .row(
                tip("doc.props.shading"),
                palette(
                    &cx.entity(),
                    "table.shading",
                    (color_state(self, ".uno:TableCellBackgroundColor", cx), "office.props.automatic"),
                    None,
                    cx,
                ),
            )
            .row(
                tip("doc.props.heading"),
                row([toggle(
                    self,
                    "table.repeat_heading",
                    (None, &tip("doc.props.repeat")),
                    ("", "doc.why.heading"),
                    cx,
                )]),
            )
            .row(
                tip("doc.props.row_break"),
                row([
                    self.steps(
                        "doc-props-row-breaks",
                        "doc.props.may_break",
                        &[("table.row_split", &["true"])],
                        cx,
                    ),
                    self.steps(
                        "doc-props-row-whole",
                        "doc.props.keep_whole",
                        &[("table.row_split", &["false"])],
                        cx,
                    ),
                ]),
            )
            .row(
                tip("doc.props.table"),
                row([button(
                    self,
                    "table.delete",
                    "doc.props.delete_table",
                    why,
                    cx,
                )]),
            )
    }

    /// A button that runs `commands` in order.
    fn steps(
        &self,
        id: &'static str,
        key: &str,
        commands: &'static [(&'static str, &'static [&'static str])],
        cx: &Context<Self>,
    ) -> AnyElement {
        let this = cx.entity();
        Button::new(SharedString::from(id), Docs::text(cx, key, &[]))
            .size(ControlSize::Sm)
            .on_click(move |_, _, cx| {
                for (command, values) in commands {
                    send(&this, command, values, cx);
                }
            })
            .into_any_element()
    }
}
