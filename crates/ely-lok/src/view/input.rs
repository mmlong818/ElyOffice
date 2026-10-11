//! Keys, mouse, IME and clipboard for LokView (LOK-06, LOK-07).

use std::{io, sync::Mutex, time::Instant};

use ely_platform::clipboard;
use gpui::{
    App, Context, KeyBinding, KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Window, actions,
};

use super::{LokView, clip, handles};
use crate::{Doc, Mouse, Rect, Worker};

actions!(lok_view, [Copy, Cut, Paste, NextField, PreviousField]);

/// The key context of every LokView.
pub const CONTEXT: &str = "LokView";

/// Binds the clipboard keys; call once at startup.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-c", Copy, Some(CONTEXT)),
        KeyBinding::new("secondary-x", Cut, Some(CONTEXT)),
        KeyBinding::new("secondary-v", Paste, Some(CONTEXT)),
        // The document owns Tab, as office apps do.
        KeyBinding::new("tab", NextField, Some(CONTEXT)),
        KeyBinding::new("shift-tab", PreviousField, Some(CONTEXT)),
    ]);
}

/// One queued request; the queue keeps input order.
pub(super) enum Input {
    Key {
        char_code: u32,
        key_code: i32,
    },
    Mouse(Mouse),
    Text {
        commit: bool,
        text: String,
    },
    Copy {
        cut: bool,
    },
    Paste,
    /// Values or formulas alone: `paste.values` or `paste.formulas`.
    PasteOnly(&'static str),
    /// A function the kernel offered, taken.
    Complete(String),
    /// The kernel's color scheme; not an edit.
    Theme(&'static str),
    /// Commands in order, from one read.
    Run {
        commands: Vec<(&'static str, Vec<String>)>,
        /// The document as read, for commands by number.
        at: Option<crate::Doc>,
    },
    /// A handle of the selected object, moved.
    Drag {
        from: (i32, i32),
        to: (i32, i32),
    },
    /// The area shown in twips, and the tile scale.
    Visible(Rect, (u32, i32)),
    /// Shows sheet or slide `part`, from 0.
    Part(usize),
}

impl Input {
    /// Whether the input may change the document.
    pub(super) fn edits(&self) -> bool {
        match self {
            // Arrows, Home, End, Page keys, Escape.
            Input::Key { key_code, .. } => !matches!(key_code & 0x0FFF, 1024..=1031 | 1281),
            Input::Mouse(_)
            | Input::Copy { cut: false }
            | Input::Visible(..)
            | Input::Part(_)
            | Input::Theme(_) => false,
            Input::Run { commands, .. } => commands.iter().any(|(id, _)| *id != "edit.select_all"),
            Input::Text { .. }
            | Input::Copy { cut: true }
            | Input::Paste
            | Input::PasteOnly(_)
            | Input::Drag { .. }
            | Input::Complete(_) => true,
        }
    }
}

/// vcl modifier bits.
const SHIFT: i32 = 0x1000;
const MOD1: i32 = 0x2000;
const MOD2: i32 = 0x4000;
const MOD3: i32 = 0x8000;

/// vcl bits for GPUI modifiers; macOS Cmd is MOD1.
fn vcl_modifiers(modifiers: &Modifiers) -> i32 {
    let mut bits = 0;
    if modifiers.shift {
        bits |= SHIFT;
    }
    if modifiers.alt {
        bits |= MOD2;
    }
    if cfg!(target_os = "macos") {
        if modifiers.platform {
            bits |= MOD1;
        }
        if modifiers.control {
            bits |= MOD3;
        }
    } else if modifiers.control {
        bits |= MOD1;
    }
    bits
}

/// A non-text keystroke as (char, vcl code).
pub(super) fn vcl_key(keystroke: &Keystroke) -> Option<(u32, i32)> {
    let modifiers = &keystroke.modifiers;
    let command =
        modifiers.control || modifiers.platform || (modifiers.alt && !cfg!(target_os = "macos"));
    let named = match keystroke.key.as_str() {
        "down" => Some(1024),
        "up" => Some(1025),
        "left" => Some(1026),
        "right" => Some(1027),
        "home" => Some(1028),
        "end" => Some(1029),
        "pageup" => Some(1030),
        "pagedown" => Some(1031),
        "enter" => Some(1280),
        "escape" => Some(1281),
        "tab" => Some(1282),
        "backspace" => Some(1283),
        "space" if command => Some(1284),
        "insert" => Some(1285),
        "delete" => Some(1286),
        key => key
            .strip_prefix('f')
            .and_then(|n| n.parse::<i32>().ok())
            .filter(|n| (1..=26).contains(n))
            .map(|n| 767 + n),
    };
    let code = match named {
        Some(code) => code,
        None if !command => return None,
        None => {
            let mut chars = keystroke.key.chars();
            match (chars.next(), chars.next()) {
                (Some(c @ 'a'..='z'), None) => 512 + (c as i32 - 'a' as i32),
                (Some(c @ '0'..='9'), None) => 256 + (c as i32 - '0' as i32),
                _ => return None,
            }
        }
    };
    Some((0, code | vcl_modifiers(modifiers)))
}

/// LOK mouse buttons: 1 left, 2 middle, 4 right.
fn lok_button(button: MouseButton) -> Option<i32> {
    match button {
        MouseButton::Left => Some(1),
        MouseButton::Middle => Some(2),
        MouseButton::Right => Some(4),
        _ => None,
    }
}

/// Runs one input; gives a copy's mark and the revision.
pub(super) fn run(worker: &Mutex<Worker>, doc: &Doc, input: Input) -> (io::Result<Option<String>>, Option<u64>) {
    let mut worker = worker.lock().expect("the worker lock");
    let counts = !matches!(input, Input::Visible(..));
    let copied = apply(&mut worker, doc, input);
    // A failed input may have moved the revision too.
    let now = match counts {
        true => worker.now(doc).inspect_err(|error| log::warn!("lok: revision unread: {error}")).ok().map(|now| now.rev),
        false => None,
    };
    (copied, now)
}

fn apply(worker: &mut Worker, doc: &Doc, input: Input) -> io::Result<Option<String>> {
    let done = match input {
        Input::Key {
            char_code,
            key_code,
        } => worker.post_key(doc, char_code, key_code),
        Input::Mouse(event) => worker.post_mouse(doc, event),
        Input::Drag { from, to } => worker.drag_graphic(doc, from, to),
        Input::Visible(area, scale) => doc.visible_area(worker, area, scale),
        Input::Part(part) => doc.show_sheet(worker, part),
        Input::Complete(name) => doc.complete_function(worker, &name),
        Input::Theme(scheme) => worker.run(doc, "view.theme", &[scheme]),
        Input::Text { commit, text } => worker.post_text(doc, commit, &text),
        Input::Copy { cut } => {
            return clip::copy_out(worker, doc, cut, |clip| {
                clipboard::write(clip).map_err(io::Error::other)
            });
        }
        Input::Run { commands, at } => {
            let values: Vec<Vec<&str>> =
                commands.iter().map(|(_, values)| values.iter().map(String::as_str).collect()).collect();
            let commands: Vec<(&str, &[&str])> =
                commands.iter().zip(&values).map(|((id, _), values)| (*id, values.as_slice())).collect();
            worker.run_all(at.as_ref().unwrap_or(doc), &commands)
        }
        Input::Paste => clip::paste(worker, doc),
        // The kernel's own cells: the app checked the clipboard.
        Input::PasteOnly(id) => worker.run(doc, id, &[]),
    };
    done.map(|()| None)
}

impl LokView {
    /// Queues `input` in order; edits mark the view pending.
    pub(super) fn send(&mut self, input: Input) {
        self.sent += 1;
        if input.edits() {
            self.last_edit = self.sent;
            self.clock.sent();
        }
        self.lane
            .inputs
            .unbounded_send((self.sent, input, Instant::now()))
            .expect("the input queue lives with the view");
    }

    /// Shows part `part` after the input queued before it.
    pub fn show_part(&mut self, part: usize) {
        log::info!("lok: shows part {part}");
        self.part = part;
        self.send(Input::Part(part));
    }

    /// Whether a possible edit is still queued.
    pub fn edit_pending(&self) -> bool {
        self.applied < self.last_edit
    }

    /// How many edits reached the kernel.
    pub fn edited(&self) -> u64 {
        self.edited
    }

    /// The sequence number of the last queued input.
    pub fn sent(&self) -> u64 {
        self.sent
    }

    /// Whether input `seq` reached the kernel.
    pub fn reached(&self, seq: u64) -> bool {
        self.applied >= seq
    }

    /// Whether every queued input reached the kernel.
    pub fn settled(&self) -> bool {
        self.applied >= self.sent
    }

    /// Posts a non-text key; text waits for the IME.
    pub(super) fn key_down(&mut self, keystroke: &Keystroke) -> bool {
        let Some((char_code, key_code)) = vcl_key(keystroke) else {
            return false;
        };
        self.send(Input::Key {
            char_code,
            key_code,
        });
        true
    }

    /// Posts a mouse event at a view position (LOK-04).
    pub(super) fn mouse(
        &mut self,
        kind: i32,
        position: gpui::Point<Pixels>,
        button: i32,
        count: usize,
        modifiers: &Modifiers,
    ) {
        // The kernel reads clicks in tile twips.
        let (x, y) = self.tile_point(position);
        self.send(Input::Mouse(Mouse {
            kind,
            x,
            y,
            count: count.max(1) as i32,
            buttons: button,
            modifiers: vcl_modifiers(modifiers),
        }));
    }

    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.walk(&event.keystroke, cx) || self.key_down(&event.keystroke) {
            cx.stop_propagation();
        }
    }

    pub(super) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(button) = lok_button(event.button) else {
            return;
        };
        window.focus(&self.focus, cx);
        if event.button == MouseButton::Left && self.grab(event.position) {
            cx.notify();
            return;
        }
        self.dragging = event.button == MouseButton::Left;
        self.mouse(
            0,
            event.position,
            button,
            event.click_count,
            &event.modifiers,
        );
    }

    pub(super) fn on_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(grip) = self.grip.take() {
            let to = self.doc_point(event.position);
            log::info!("lok: handle released");
            self.send(Input::Drag {
                from: grip.from,
                to,
            });
            cx.notify();
            return;
        }
        let Some(button) = lok_button(event.button) else {
            return;
        };
        if event.button == MouseButton::Left {
            self.dragging = false;
        }
        self.mouse(
            1,
            event.position,
            button,
            event.click_count,
            &event.modifiers,
        );
    }

    /// Ends a drag released outside the view.
    pub(super) fn on_mouse_up_out(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dragging || self.grip.is_some() {
            self.on_mouse_up(event, window, cx);
        }
    }

    pub(super) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.grip.is_some() {
            let to = self.doc_point(event.position);
            self.grip = self.grip.map(|grip| handles::Grip { to, ..grip });
            cx.notify();
            return;
        }
        if self.dragging && event.pressed_button == Some(MouseButton::Left) {
            self.mouse(2, event.position, 1, 1, &event.modifiers);
        }
    }

    pub(super) fn on_copy(&mut self, _: &Copy, _: &mut Window, _: &mut Context<Self>) {
        self.send(Input::Copy { cut: false });
    }

    pub(super) fn on_next_field(&mut self, _: &NextField, _: &mut Window, _: &mut Context<Self>) {
        if !self.complete() {
            self.key_down(&Keystroke::parse("tab").expect("a keystroke"));
        }
    }

    pub(super) fn on_previous_field(
        &mut self,
        _: &PreviousField,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        self.key_down(&Keystroke::parse("shift-tab").expect("a keystroke"));
    }

    pub(super) fn on_cut(&mut self, _: &Cut, _: &mut Window, _: &mut Context<Self>) {
        self.send(Input::Copy { cut: true });
    }

    pub(super) fn on_paste(&mut self, _: &Paste, _: &mut Window, _: &mut Context<Self>) {
        self.send(Input::Paste);
    }

    /// Pastes values or formulas alone (TABLE-08).
    pub fn paste_only(&mut self, id: &'static str) {
        self.send(Input::PasteOnly(id));
    }
}

#[cfg(test)]
mod tests;
