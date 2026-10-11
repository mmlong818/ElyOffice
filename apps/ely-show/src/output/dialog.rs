//! The output dialog: what, which slides, how many a page.

use ely_gpui_component::{
    buttons::{Button, ButtonVariant, SegmentedControl},
    data_display::DescriptionList,
    forms::{Choice, NumberInput, Select},
    overlays::Dialog as Card,
    theme::ControlSize,
};
use gpui::{ParentElement as _, Styled as _, Window, div};

use super::{Kind, Output, Range};
use crate::deck::Deck;
use crate::dialog::Dialog;

/// A typed slide number on the deck; 0 means all.
fn slide_number(value: f64, last: u32) -> u32 {
    value.round().clamp(1.0, f64::from(last.max(1))) as u32
}

/// What goes out, which slides, how many a page.
pub fn output_dialog(
    this: gpui::Entity<Deck>,
    output: &Output,
    dismiss: impl Fn(&mut Window, &mut gpui::App) + 'static,
    cx: &gpui::App,
) -> Card {
    let text = |key: &str| Deck::text(cx, key, &[]);
    let change = |this: &gpui::Entity<Deck>, cx: &mut gpui::App, edit: &dyn Fn(&mut Output)| {
        this.update(cx, |deck, cx| {
            if let Some(Dialog::Output(output)) = &mut deck.dialog {
                edit(output);
            }
            cx.notify();
        })
    };
    let kinds: &[(Kind, &str)] = match output.print {
        true => &[(Kind::Slides, "slides"), (Kind::Handouts, "handouts")],
        false => &[(Kind::Slides, "pdf"), (Kind::Pictures, "pictures"), (Kind::Handouts, "handouts")],
    };
    let now = kinds.iter().find(|(kind, _)| *kind == output.kind).map_or("pdf", |(_, id)| id);
    let kind = {
        let (this, kinds) = (this.clone(), kinds.to_vec());
        kinds
            .iter()
            .fold(SegmentedControl::new("show-output-kind", now), |control, (_, id)| {
                control.segment(*id, text(&format!("show.output.{id}")), None)
            })
            .size(ControlSize::Sm)
            .on_change(move |value, _, cx| {
                if let Some((kind, _)) = kinds.iter().find(|(_, id)| *id == value.as_ref()) {
                    let kind = *kind;
                    change(&this, cx, &|output| output.kind = kind);
                }
            })
    };
    let which = match output.range {
        Range::All => "all",
        Range::Current => "current",
        Range::Slides(..) => "range",
    };
    let last = output.hidden.len() as u32;
    let range = {
        let this = this.clone();
        let control = SegmentedControl::new("show-output-range", which)
            .size(ControlSize::Sm)
            .segment("all", text("show.output.all"), None);
        // Master view has no current slide.
        let control = match output.current {
            Some(_) => control.segment("current", text("show.output.current"), None),
            None => control,
        };
        control
            .segment("range", text("show.output.range"), None)
            .on_change(move |value, _, cx| {
                let range = match value.as_ref() {
                    "all" => Range::All,
                    "current" => Range::Current,
                    _ => Range::Slides(1, last),
                };
                change(&this, cx, &|output| output.range = range);
            })
    };
    let bound = |id: &'static str, value: u32, first: bool| {
        // A typed number counts before blur commits it.
        let set = move |this: &gpui::Entity<Deck>, value: f64, cx: &mut gpui::App| {
            let value = slide_number(value, last);
            change(this, cx, &|output| {
                if let Range::Slides(from, to) = &mut output.range {
                    *(if first { from } else { to }) = value;
                }
            })
        };
        let (typed, committed) = (this.clone(), this.clone());
        NumberInput::new(id, f64::from(value))
            .label(text(if first { "show.output.from" } else { "show.output.to" }))
            .range(1.0, f64::from(last))
            .size(ControlSize::Sm)
            .on_change(move |value, _, cx| set(&typed, value, cx))
            .on_commit(move |value, _, cx| set(&committed, value, cx))
    };
    let bounds = match output.range {
        Range::Slides(from, to) => Some(
            div()
                .flex()
                .gap_2()
                .child(div().w_full().max_w_24().child(bound("show-output-from", from, true)))
                .child(div().w_full().max_w_24().child(bound("show-output-to", to, false))),
        ),
        _ => None,
    };
    let per = (output.kind == Kind::Handouts).then(|| {
        let this = this.clone();
        let choices = [1u8, 2, 3, 4, 6, 9].map(|n| Choice::new(n.to_string(), n.to_string()));
        Select::new("show-output-per", choices)
            .label(text("show.output.per"))
            .selected(output.per.to_string())
            .on_change(move |value, _, cx| {
                let per: u8 = value.parse().expect("listed numbers");
                change(&this, cx, &|output| output.per = per);
            })
    });
    let mut rows = DescriptionList::new()
        .item(text("show.output.what"), kind)
        .item(text("show.output.which"), div().flex().flex_col().gap_2().child(range).children(bounds));
    if let Some(per) = per {
        rows = rows.item(text("show.output.per"), per);
    }
    let (title, go) = match output.print {
        true => ("show.output.print_title", "show.output.print"),
        false => ("show.output.export_title", "show.output.export"),
    };
    let go = text(go);
    Card::new("show-output", text(title), dismiss)
        .child(rows)
        .action(ely_office::cancel(cx))
        .action(move |_| {
            Button::new("show-output-go", go)
                .variant(ButtonVariant::Primary)
                .on_click(move |_, _, cx| this.update(cx, |deck, cx| deck.output(cx)))
        })
}

#[cfg(test)]
mod tests {
    use super::slide_number;

    #[test]
    fn a_typed_number_stays_on_the_deck() {
        assert_eq!(slide_number(0.0, 5), 1, "0 would export every slide");
        assert_eq!(slide_number(-3.0, 5), 1);
        assert_eq!(slide_number(2.6, 5), 3);
        assert_eq!(slide_number(9.0, 5), 5);
    }
}
