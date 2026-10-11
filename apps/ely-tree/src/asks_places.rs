//! Dialogs for stashes, remotes, worktrees and pushes.

use ely_gpui_component::{
    buttons::Button,
    data_display::DescriptionList,
    forms::{Checkbox, Input},
    overlays::{ConfirmDialog, Dialog},
};
use gpui::{AnyElement, App, Context, IntoElement, ParentElement as _, Styled as _, Window, div};

use crate::{
    asks::{Ask, Choice},
    tree::{Tree, text},
    work::Write,
};

impl Tree {
    pub fn place_dialog(
        &self,
        dismiss: impl Fn(&mut Window, &mut App) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let this = cx.weak_entity();
        let label = |key: &str| text(cx, key, &[]);
        let tick = |id: &'static str, on: bool, key: &str, set: fn(&mut Ask, bool)| {
            let this = this.clone();
            Checkbox::new(id, on)
                .label(text(cx, key, &[]))
                .on_change(move |on, _, cx| {
                    this.update(cx, |tree, cx| {
                        if let Some(ask) = &mut tree.ask {
                            set(ask, on);
                            cx.notify();
                        }
                    })
                    .ok();
                })
                .into_any_element()
        };
        Some(match self.ask.as_ref()? {
            Ask::Stash {
                message, untracked, ..
            } => {
                let rows = DescriptionList::new().item(
                    label("tree.stash.message"),
                    div().w_full().child(Input::new(message)),
                );
                let extra = tick(
                    "tree-stash-untracked",
                    *untracked,
                    "tree.stash.untracked",
                    |ask, on| {
                        if let Ask::Stash { untracked, .. } = ask {
                            *untracked = on;
                        }
                    },
                );
                let (title, go) = (label("tree.stash.title"), label("tree.stash.go"));
                self.form("tree-stash", (title, go), rows, Some(extra), dismiss, cx)
            }
            Ask::Remote { old, name, url, .. } => {
                let rows = DescriptionList::new()
                    .item(
                        label("tree.remote.name"),
                        div().w_full().child(Input::new(name)),
                    )
                    .item(
                        label("tree.remote.url"),
                        div().w_full().child(Input::new(url)),
                    );
                let title = match old {
                    Some((was, _)) => text(cx, "tree.remote.edit", &[("name", was)]),
                    None => label("tree.remote.add"),
                };
                let go = label("tree.remote.save");
                self.form("tree-remote", (title, go), rows, None, dismiss, cx)
            }
            Ask::Worktree {
                path, branch, new, ..
            } => {
                let rows = DescriptionList::new().item(
                    label("tree.worktree.branch"),
                    div().w_full().child(Input::new(branch)),
                );
                let extra = tick("tree-worktree-new", *new, "tree.worktree.new", |ask, on| {
                    if let Ask::Worktree { new, .. } = ask {
                        *new = on;
                    }
                });
                let shown = path.display().to_string();
                let title = text(cx, "tree.worktree.title", &[("path", &shown)]);
                let go = label("tree.worktree.go");
                self.form("tree-worktree", (title, go), rows, Some(extra), dismiss, cx)
            }
            Ask::Force { repo, plan } => {
                let lost = match plan.lost.len() {
                    0 => label("tree.force.nothing"),
                    n => ely_ui::i18n::counted(
                        cx,
                        "tree.force.lost",
                        n as u64,
                        &[("n", &n.to_string()), ("commits", &plan.lost.join("\n"))],
                    ),
                };
                let message = text(
                    cx,
                    "tree.force.message",
                    &[
                        ("branch", &plan.branch),
                        ("remote", &plan.remote),
                        ("target", &plan.target),
                        ("lost", &lost),
                    ],
                );
                let (repo, write) = (*repo, Write::Remote(plan.args()));
                ConfirmDialog::new("tree-force", label("tree.force.title"), message, dismiss)
                    .confirm(label("tree.force.go"))
                    .destructive()
                    .on_confirm(move |_, cx| {
                        this.update(cx, |tree, cx| tree.write(repo, write.clone(), cx))
                            .ok();
                    })
                    .into_any_element()
            }
            Ask::Choose {
                repo,
                title,
                choices,
                then,
            } => {
                let buttons = choices.iter().enumerate().map(|(at, choice)| {
                    let (this, then, choice, repo) =
                        (this.clone(), then.clone(), choice.clone(), *repo);
                    Button::new(("tree-choice", at), choice.clone()).on_click(move |_, _, cx| {
                        this.update(cx, |tree, cx| {
                            tree.ask = None;
                            match &then {
                                Choice::Publish(branch) => tree.publish(repo, branch, &choice, cx),
                                Choice::Upstream(branch) => {
                                    tree.set_upstream(repo, branch, &choice, cx)
                                }
                            }
                            cx.notify();
                        })
                        .ok();
                    })
                });
                let cancel = label("tree.cancel");
                Dialog::new("tree-choose", title.clone(), dismiss)
                    .child(div().flex().flex_col().gap_2().children(buttons))
                    .action(move |close| {
                        Button::new("tree-choose-cancel", cancel)
                            .on_click(move |_, window, cx| close(window, cx))
                    })
                    .into_any_element()
            }
            _ => return None,
        })
    }
}
