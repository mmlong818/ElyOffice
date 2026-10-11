//! The Branches panel: branches, remote branches and tags (TREE-07).

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    feedback::ErrorView,
    git::{Branch, BranchList, GitTag, TagList},
    primitives::IconName,
    theme::ControlSize,
};
use gpui::{
    AnyElement, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
};

use crate::{
    branches::{Kind, Ref},
    changes::Changes,
    hist::when,
    tree::{Tree, text},
};

fn branch(found: &Ref) -> Branch {
    let (ahead, behind) = found.track.unwrap_or((0, 0));
    Branch {
        name: found.name.clone().into(),
        remote: found.kind == Kind::Remote,
        current: found.current,
        ahead,
        behind,
        subject: found.subject.clone().into(),
        when: found.at.map_or_else(|| "—".into(), when),
    }
}

fn tag(found: &Ref) -> GitTag {
    GitTag {
        name: found.name.clone().into(),
        message: found.message.clone().map(Into::into),
        commit: found.commit.clone().into(),
        when: found.at.map_or_else(|| "—".into(), when),
    }
}

pub fn branches(tree: &Entity<Tree>, cx: &mut Context<Changes>) -> AnyElement {
    let read = tree.read(cx);
    let Some(opened) = read.active() else {
        return div().into_any_element();
    };
    let refs = match &opened.refs {
        None => return div().into_any_element(),
        Some(Err(error)) => {
            return ErrorView::new(
                "tree-refs-error",
                IconName::TriangleAlert,
                text(cx, "tree.refs.unread", &[]),
            )
            .detail(error.clone())
            .into_any_element();
        }
        Some(Ok(refs)) => refs,
    };
    let picked = opened.picked_ref.clone();
    // A name shows picked only in its own kind's list.
    let picked_in = move |kind: Kind| {
        picked
            .clone()
            .filter(|(at, _)| *at == kind)
            .map(|(_, name)| name)
    };
    let pick = |kind: Kind| {
        let this = tree.downgrade();
        move |name: &gpui::SharedString, _: &mut gpui::Window, cx: &mut gpui::App| {
            let name = name.to_string();
            this.update(cx, |tree, cx| tree.pick_ref(kind, name, cx)).ok();
        }
    };
    let switch = |kind: Kind| {
        let this = tree.downgrade();
        move |name: &gpui::SharedString, _: &mut gpui::Window, cx: &mut gpui::App| {
            this.update(cx, |tree, cx| tree.switch(name, kind, cx)).ok();
        }
    };
    // Each delete asks first, as from the details.
    let delete = || {
        let this = tree.downgrade();
        move |name: &gpui::SharedString, _: &mut gpui::Window, cx: &mut gpui::App| {
            this.update(cx, |tree, cx| tree.delete_branch(name, false, cx))
                .ok();
        }
    };
    // Local branches, the current first, then remote ones.
    let mut local: Vec<&Ref> = refs.iter().filter(|r| r.kind == Kind::Local).collect();
    local.sort_by_key(|r| !r.current);
    let remote: Vec<&Ref> = refs.iter().filter(|r| r.kind == Kind::Remote).collect();
    let mut tags: Vec<&Ref> = refs.iter().filter(|r| r.kind == Kind::Tag).collect();
    tags.sort_by_key(|r| std::cmp::Reverse(r.at));
    let (new_branch, new_tag) = (tree.downgrade(), tree.downgrade());
    let actions = div()
        .flex()
        .gap_2()
        .p_2()
        .child(
            Button::new("tree-new-branch", text(cx, "tree.branch.new", &[]))
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, window, cx| {
                    new_branch
                        .update(cx, |tree, cx| tree.ask_branch(None, window, cx))
                        .ok();
                }),
        )
        .child(
            Button::new("tree-new-tag", text(cx, "tree.tag.new", &[]))
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, window, cx| {
                    new_tag
                        .update(cx, |tree, cx| tree.ask_tag(None, window, cx))
                        .ok();
                }),
        );
    let selected = |list: BranchList, kind: Kind| match picked_in(kind) {
        Some(name) => list.selected(name),
        None => list,
    };
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(actions)
        .child(
            div()
                .id("tree-refs-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px_2()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    selected(
                        BranchList::new("tree-local", local.iter().map(|r| branch(r))),
                        Kind::Local,
                    )
                    .on_pick(pick(Kind::Local))
                    .on_switch(switch(Kind::Local))
                    .on_delete(delete()),
                )
                .when(!remote.is_empty(), |list| {
                    list.child(
                        selected(
                            BranchList::new("tree-remote", remote.iter().map(|r| branch(r))),
                            Kind::Remote,
                        )
                        .on_pick(pick(Kind::Remote))
                        .on_switch(switch(Kind::Remote)),
                    )
                })
                .when(!tags.is_empty(), |list| {
                    let tags_list = TagList::new("tree-tags", tags.iter().map(|r| tag(r)))
                        .on_pick(pick(Kind::Tag));
                    list.child(match picked_in(Kind::Tag) {
                        Some(name) => tags_list.selected(name),
                        None => tags_list,
                    })
                }),
        )
        .into_any_element()
}

impl crate::tree::Tree {
    /// A branch or tag picked: it shows, nothing else.
    pub fn pick_ref(&mut self, kind: Kind, name: String, cx: &mut gpui::Context<Self>) {
        self.look = None;
        if let Some(opened) = self.active.and_then(|id| self.opened_mut(id)) {
            opened.picked_ref = Some((kind, name));
        }
        cx.notify();
    }
}
