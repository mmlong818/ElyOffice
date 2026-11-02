//! Formula entry: reference marks and function hints (TABLE-03).

use ely_gpui_component::{
    editor::{CompletionItem, CompletionMenu, Signature, SignatureHelp, SymbolKind, completions},
    primitives::raise,
};
use gpui::{
    AnyElement, Bounds, Context, Hsla, InteractiveElement, IntoElement, Keystroke, Modifiers,
    ParentElement, Pixels, anchored, div, rgb,
};

use super::{
    LokView,
    input::{Input, vcl_key},
};

mod completion;
pub(super) use completion::Completion;

impl LokView {
    /// The sheet shown, which reference marks follow.
    pub fn set_part(&mut self, part: usize) {
        self.part = part;
    }

    /// The shown sheet's reference marks, in window pixels.
    pub(super) fn marks(&self) -> Vec<(Bounds<Pixels>, Hsla)> {
        self.cell
            .marks
            .iter()
            .filter(|mark| mark.part == self.part)
            .map(|mark| (self.screen_rect(mark.rect), rgb(mark.color).into()))
            .collect()
    }

    /// The offered functions, in the order the menu shows.
    fn offered(&self) -> Vec<CompletionItem> {
        let items: Vec<CompletionItem> = self
            .cell
            .functions
            .iter()
            .map(|function| {
                CompletionItem::new(function.name.clone(), SymbolKind::Function)
                    .detail(function.signature.clone())
                    .docs(function.description.clone())
            })
            .collect();
        completions(&items, "")
            .into_iter()
            .map(|(item, _)| item)
            .collect()
    }

    /// Up and Down walk the offered functions.
    pub(super) fn walk(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        let count = self.cell.functions.len();
        if count == 0 || self.completion.pending() || keystroke.modifiers != Modifiers::none() {
            return false;
        }
        self.picked = match keystroke.key.as_str() {
            "down" => (self.picked + 1) % count,
            "up" => (self.picked + count - 1) % count,
            _ => return false,
        };
        cx.notify();
        true
    }

    /// Enter or Tab: the kernel's function then, else the key.
    pub(super) fn complete_or_press(&mut self, keystroke: &Keystroke) {
        let key = vcl_key(keystroke).expect("Enter and Tab are vcl keys");
        let name = (!self.completion.pending())
            .then(|| self.offered().into_iter().nth(self.picked))
            .flatten()
            .map(|item| item.label.to_string());
        self.take(name, Some(key));
    }

    /// A clicked function, if the kernel still offers it then.
    fn complete_function(&mut self, name: &str) {
        if !self.completion.pending() {
            self.take(Some(name.to_string()), None);
        }
    }

    /// Queues the choice in order with typing; the worker decides.
    fn take(&mut self, name: Option<String>, key: Option<(u32, i32)>) {
        self.send(Input::Complete { name, key });
        self.completion.queued(self.sent);
        self.picked = 0;
    }

    /// The offered functions, else the call's usage, under the cell.
    pub(super) fn hints(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.completion.pending() {
            return None;
        }
        let cell = self.cell.cursor?;
        if !self.is_visible(cell) {
            return None;
        }
        let cell = self.screen_rect(cell);
        let hint = if self.cell.functions.is_empty() {
            SignatureHelp::new("lok-view-usage", [usage(&self.cell.usage)?]).into_any_element()
        } else {
            let this = cx.entity();
            let offered = self.offered();
            CompletionMenu::new("lok-view-functions", offered)
                .selected(self.picked)
                .on_pick(move |item, _, cx| {
                    this.update(cx, |view, cx| {
                        view.complete_function(&item.label);
                        cx.notify();
                    })
                })
                .into_any_element()
        };
        let place = anchored()
            .position(cell.bottom_left())
            .snap_to_window()
            .child(div().block_mouse_except_scroll().child(hint));
        Some(raise("lok-view-hints", place).into_any_element())
    }
}

/// The kernel's usage text as a signature.
fn usage(text: &str) -> Option<Signature> {
    let (call, docs) = text.split_once(" : ").unwrap_or((text, ""));
    let at = call.find('►')?;
    let label = call.replacen('►', "", 1);
    let rest = &label[at..];
    let end = at + rest.find([',', ';', ')']).unwrap_or(rest.len());
    Some(Signature {
        label: label.clone().into(),
        params: std::iter::once(at..end).collect(),
        docs: (!docs.is_empty()).then(|| docs.to_string().into()),
    })
}

#[cfg(test)]
mod tests {
    use super::usage;

    #[test]
    fn the_kernel_usage_reads_as_a_signature() {
        let signature = usage("SUM(► Number 1, Number 2) : The numbers.").expect("a usage");
        assert_eq!(signature.label.as_ref(), "SUM( Number 1, Number 2)");
        assert_eq!(&signature.label[signature.params[0].clone()], " Number 1");
        assert_eq!(signature.docs.as_deref(), Some("The numbers."));
        assert!(usage("").is_none());
    }
}
