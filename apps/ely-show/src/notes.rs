//! The speaker notes under the slide (SHOW-08).

use ely_gpui_component::{
    feedback::InlineMessage,
    forms::{Input, InputEvent, TextInput},
    primitives::Severity,
    theme::ActiveTheme as _,
};
use ely_office::prelude::*;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement as _,
    Styled as _, Window, div, px,
};

use crate::deck::Deck;

/// The notes field and its last tab, slide, text.
#[derive(Default)]
pub struct NotesField {
    field: Option<Entity<TextInput>>,
    shown: Option<(u64, usize, String)>,
    /// The state the shown slide number was read in.
    pub(crate) at: Option<ely_lok::Doc>,
    _events: Option<gpui::Subscription>,
}

impl Deck {
    fn notes_field(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<TextInput> {
        if let Some(field) = &self.notes.field {
            return field.clone();
        }
        let (label, hint) = (
            Deck::text(cx, "show.notes.label", &[]),
            Deck::text(cx, "show.notes.hint", &[]),
        );
        let field = cx.new(|cx| {
            TextInput::new(window, cx)
                .multi_line(3, 5)
                .label(label)
                .placeholder(hint)
        });
        let events = cx.subscribe(&field, |deck, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Blur) {
                deck.commit_notes(cx)
            }
        });
        self.notes = NotesField {
            field: Some(field.clone()),
            shown: None,
            at: None,
            _events: Some(events),
        };
        field
    }

    /// Sends typed notes to the slide they came from.
    pub(crate) fn commit_notes(&mut self, cx: &mut Context<Self>) {
        let (Some(field), Some((tab, part, shown))) = (&self.notes.field, &self.notes.shown) else {
            return;
        };
        let (tab, part, text) = (*tab, *part, field.read(cx).text().to_string());
        if &text == shown {
            return;
        }
        let Some(at) = self.notes.at.clone() else {
            return log::warn!("show: notes for slide {part} have no read state");
        };
        log::info!("show: notes for slide {part} changed");
        // `shown` stays the kernel's until a read confirms the send.
        self.run_on(tab, &at, "notes.set", &[&text, &part.to_string()], cx);
        self.force_reread(tab, cx);
    }

    /// The notes field, showing the read notes unless typing.
    pub(crate) fn notes_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<TextInput>> {
        let tab = self.active_tab()?;
        let notes = tab.extra.notes.clone()?;
        let (id, part) = (tab.id, notes.part?);
        let field = self.notes_field(window, cx);
        let typing = field.focus_handle(cx).is_focused(window);
        // Typing keeps the state the slide number was read in.
        if !typing {
            self.notes.at = self.read_at(id, cx);
        }
        let read = (id, part, notes.text.clone());
        if !typing && self.notes.shown.as_ref() != Some(&read) {
            field.update(cx, |field, cx| field.set_text(notes.text, cx));
            self.notes.shown = Some(read);
        } else if let Some(shown) = self.notes.shown.as_mut().filter(|shown| (shown.0, shown.1) == (id, part)) {
            // A read while typing moves the base, not the draft.
            shown.2 = notes.text;
        }
        Some(field)
    }

    /// The notes pane: the field, or why it cannot edit.
    pub fn notes_pane(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let notes = self.active_tab()?.extra.notes.clone()?;
        let pane = div()
            .h(px(120.0))
            .flex_none()
            .p_2()
            .border_t_1()
            .border_color(cx.theme().colors.border);
        if !notes.editable {
            let why = Deck::text(cx, "show.notes.none", &[]);
            return Some(
                pane.child(InlineMessage::new(Severity::Info, why))
                    .into_any_element(),
            );
        }
        if notes.rich {
            let why = Deck::text(cx, "show.notes.rich", &[]);
            let text = div()
                .text_color(cx.theme().colors.fg_muted)
                .child(notes.text);
            let column = div()
                .flex()
                .flex_col()
                .gap_1()
                .child(InlineMessage::new(Severity::Info, why))
                .child(text);
            return Some(pane.child(column).into_any_element());
        }
        let field = self.notes_input(window, cx)?;
        Some(pane.child(Input::new(&field)).into_any_element())
    }
}
