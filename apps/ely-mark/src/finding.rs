//! Find and replace over the shown editor (MARK-06).

use ely_gpui_component::primitives::raise;
use ely_gpui_component::{
    editor::{FindOptions, FindWidget},
    forms::{InputEvent, TextInput},
};
use ely_office::prelude::*;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _,
    IntoElement as _, ParentElement as _, Styled as _, Subscription, Window, actions, div,
};
use serde_json::json;

use crate::desk::Desk;

actions!(mark, [Find]);

/// Where the shown editor's search stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub current: Option<usize>,
    pub total: usize,
    /// A pattern that does not compile.
    pub error: Option<String>,
}

pub struct Finder {
    query: Entity<TextInput>,
    with: Entity<TextInput>,
    options: FindOptions,
    replacing: bool,
    /// The tab last searched; another one is searched anew.
    tab: Option<u64>,
    _follow: Subscription,
}

impl Desk {
    /// Opens the find box, or takes focus to it.
    pub fn on_find(&mut self, _: &Find, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(finder) = &self.finder {
            let focus = finder.query.focus_handle(cx);
            return window.focus(&focus, cx);
        }
        let query = cx.new(|cx| TextInput::new(window, cx));
        let with = cx.new(|cx| TextInput::new(window, cx));
        cx.subscribe(&query, |desk, _, event: &InputEvent, cx| match event {
            InputEvent::Changed => desk.seek(cx),
            InputEvent::Submit => desk.step(true, cx),
            _ => {}
        })
        .detach();
        cx.subscribe(&with, |desk, _, event: &InputEvent, cx| match event {
            InputEvent::Changed => desk.seek(cx),
            InputEvent::Submit => desk.replace(false, cx),
            _ => {}
        })
        .detach();
        let focus = query.focus_handle(cx);
        let follow = cx.observe_self(|desk, cx| {
            let shown = desk.active_tab().map(|tab| tab.id);
            if desk
                .finder
                .as_ref()
                .is_some_and(|finder| finder.tab != shown)
            {
                desk.seek(cx);
            }
        });
        self.finder = Some(Finder {
            query,
            with,
            options: FindOptions::default(),
            replacing: false,
            tab: None,
            _follow: follow,
        });
        window.focus(&focus, cx);
        cx.notify();
    }

    /// The query and options as the page's search spec.
    fn spec(&self, cx: &Context<Self>) -> Option<serde_json::Value> {
        let finder = self.finder.as_ref()?;
        Some(json!({
            "search": finder.query.read(cx).text(),
            "replace": finder.with.read(cx).text(),
            "caseSensitive": finder.options.case,
            "regexp": finder.options.regex,
            "wholeWord": finder.options.word,
        }))
    }

    fn seek(&mut self, cx: &mut Context<Self>) {
        let shown = self.active_tab().map(|tab| tab.id);
        if let Some(finder) = self.finder.as_mut() {
            finder.tab = shown;
        }
        if let (Some(spec), Some(view)) = (self.spec(cx), self.active_view()) {
            view.update(cx, |view, cx| view.find(spec, cx));
        }
    }

    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let (Some(spec), Some(view)) = (self.spec(cx), self.active_view()) {
            view.update(cx, |view, cx| view.step(spec, forward, cx));
        }
    }

    fn replace(&mut self, all: bool, cx: &mut Context<Self>) {
        if let (Some(spec), Some(view)) = (self.spec(cx), self.active_view()) {
            view.update(cx, |view, cx| view.replace(spec, all, cx));
        }
    }

    fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finder = None;
        // Every tab's page holds its own highlights.
        let views: Vec<_> = self.core.tabs.iter().map(|tab| tab.view.clone()).collect();
        for view in views {
            view.update(cx, |view, cx| view.unfind(cx));
        }
        if let Some(view) = self.active_view() {
            let focus = view.read(cx).focus.clone();
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    pub fn find_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let finder = self.finder.as_ref()?;
        let found = self
            .active_view()
            .and_then(|view| view.read(cx).found.clone());
        let (current, total) = found
            .as_ref()
            .map_or((None, 0), |found| (found.current, found.total));
        let this = cx.entity();
        let widget = FindWidget::new("mark-find", &finder.query, current, total)
            .options(finder.options)
            .on_options({
                let this = this.clone();
                move |options, _, cx| {
                    this.update(cx, |desk, cx| {
                        if let Some(finder) = desk.finder.as_mut() {
                            finder.options = options;
                        }
                        desk.seek(cx);
                    })
                }
            })
            .on_step({
                let this = this.clone();
                move |forward, _, cx| this.update(cx, |desk, cx| desk.step(forward, cx))
            })
            .on_replace({
                let this = this.clone();
                move |all, _, cx| this.update(cx, |desk, cx| desk.replace(all, cx))
            })
            .on_toggle_replace({
                let this = this.clone();
                move |open, _, cx| {
                    this.update(cx, |desk, cx| {
                        if let Some(finder) = desk.finder.as_mut() {
                            finder.replacing = open;
                        }
                        cx.notify();
                    })
                }
            })
            .on_close(move |window, cx| this.update(cx, |desk, cx| desk.close_find(window, cx)));
        let widget = match finder.replacing {
            true => widget.replace(&finder.with),
            false => widget,
        };
        let widget = match found.and_then(|found| found.error) {
            Some(error) => widget.error(error),
            None => widget,
        };
        // Over the page's top right, as elsewhere.
        let place = div()
            .absolute()
            .top_2()
            .right_4()
            .block_mouse_except_scroll()
            .child(widget);
        Some(raise("mark-find-layer", place).into_any_element())
    }
}
