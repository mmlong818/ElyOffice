//! Branches, tags, merge, rebase, pick and revert (TREE-07, TREE-08).

use ely_gpui_component::{forms::TextInput, primitives::Severity};
use gpui::{AppContext as _, Context, SharedString, Window};

use crate::{
    asks::Ask,
    branches::{self, Kind},
    ops::{self, Op},
    tree::{Tree, text},
    work::Write,
};

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| word.to_string()).collect()
}

/// `-m 1`: a merge commit goes against its first parent.
fn mainline(parents: usize) -> Vec<String> {
    if parents > 1 {
        args(&["-m", "1"])
    } else {
        Vec::new()
    }
}

impl Tree {
    /// Rereads branches and tags; only the latest read lands.
    pub fn read_refs(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(git) = self.git.as_ref().ok().cloned() else {
            return;
        };
        let Some(opened) = self.opened_mut(id) else {
            return;
        };
        opened.refs_asks += 1;
        let (root, ask) = (opened.repo.root.clone(), opened.refs_asks);
        let task = cx.background_spawn(async move { branches::read(&git, &root) });
        cx.spawn(async move |this, cx| {
            let read = task.await.map_err(|error| error.to_string());
            this.update(cx, |tree, cx| {
                if let Some(opened) = tree.opened_mut(id).filter(|opened| opened.refs_asks == ask) {
                    opened.refs = Some(read);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Rereads what operation is under way.
    pub fn read_doing(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(git) = self.git.as_ref().ok().cloned() else {
            return;
        };
        let Some(opened) = self.opened_mut(id) else {
            return;
        };
        opened.doing_asks += 1;
        let (root, dir, ask) = (
            opened.repo.root.clone(),
            opened.repo.git_dir.clone(),
            opened.doing_asks,
        );
        let task = cx.background_spawn(async move { ops::doing(&git, &root, &dir) });
        cx.spawn(async move |this, cx| {
            let read = task.await.map_err(|error| error.to_string());
            if let Err(error) = &read {
                log::error!("tree: what Git is doing did not read: {error}");
            }
            this.update(cx, |tree, cx| {
                if let Some(opened) = tree
                    .opened_mut(id)
                    .filter(|opened| opened.doing_asks == ask)
                {
                    opened.doing = Some(read);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// A failure: a conflict stop says so, else Git's words.
    pub fn git_failed(&mut self, stopped: bool, error: String, cx: &mut Context<Self>) {
        if stopped {
            let words = text(cx, "tree.op.stopped", &[]);
            return self.toast(words, Severity::Warning, cx);
        }
        self.ask = Some(Ask::Failed {
            title: text(cx, "tree.op.failed", &[]),
            detail: error.into(),
        });
        cx.notify();
    }

    pub(crate) fn run_git(&mut self, words: Vec<String>, cx: &mut Context<Self>) {
        let Some(id) = self.active else { return };
        log::info!("tree: git {}", words.first().map_or("", String::as_str));
        self.write(id, Write::Git(words), cx);
    }

    /// Asks before a command; each yes counts once.
    pub(crate) fn confirm(
        &mut self,
        key: &str,
        name: &str,
        words: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.active else { return };
        self.ask = Some(Ask::Confirm {
            repo,
            title: text(cx, &format!("{key}.title"), &[("name", name)]),
            message: text(cx, &format!("{key}.message"), &[("name", name)]),
            go: text(cx, &format!("{key}.go"), &[]),
            write: Write::Git(words),
        });
        cx.notify();
    }

    pub fn switch(&mut self, name: &str, kind: Kind, cx: &mut Context<Self>) {
        match kind {
            // Track by full ref, never a same-named tag.
            Kind::Remote => self.run_git(
                args(&["switch", "--track", &format!("refs/remotes/{name}")]),
                cx,
            ),
            _ => self.run_git(args(&["switch", name]), cx),
        }
    }

    /// Asks which remote branch `branch` tracks.
    pub fn ask_upstream(&mut self, branch: &str, cx: &mut Context<Self>) {
        let choices: Vec<String> = match self.active().and_then(|opened| opened.refs.as_ref()) {
            Some(Ok(refs)) => refs
                .iter()
                .filter(|found| found.kind == Kind::Remote)
                .map(|found| found.name.clone())
                .collect(),
            _ => Vec::new(),
        };
        if choices.is_empty() {
            let words = text(cx, "tree.upstream.none", &[]);
            return self.toast(words, Severity::Warning, cx);
        }
        let Some(repo) = self.active else { return };
        self.ask = Some(Ask::Choose {
            repo,
            title: text(cx, "tree.upstream.title", &[("branch", branch)]),
            choices,
            then: crate::asks::Choice::Upstream(branch.to_string()),
        });
        cx.notify();
    }

    pub fn set_upstream(&mut self, repo: u64, branch: &str, remote: &str, cx: &mut Context<Self>) {
        let to = format!("--set-upstream-to=refs/remotes/{remote}");
        self.write(repo, Write::Git(args(&["branch", &to, "--", branch])), cx);
    }

    pub fn unset_upstream(&mut self, branch: &str, cx: &mut Context<Self>) {
        self.run_git(args(&["branch", "--unset-upstream", "--", branch]), cx);
    }

    pub fn merge(&mut self, name: &str, cx: &mut Context<Self>) {
        self.run_git(args(&["merge", name]), cx);
    }

    /// `onto` is the full ref; `name` is how it reads.
    pub fn rebase(&mut self, onto: &str, name: &str, cx: &mut Context<Self>) {
        self.confirm("tree.rebase", name, args(&["rebase", onto]), cx);
    }

    pub fn delete_branch(&mut self, name: &str, force: bool, cx: &mut Context<Self>) {
        log::info!("tree: delete of {name} asked, force {force}");
        let (key, flag) = if force {
            ("tree.force_delete", "-D")
        } else {
            ("tree.delete_branch", "-d")
        };
        self.confirm(key, name, args(&["branch", flag, name]), cx);
    }

    pub fn delete_tag(&mut self, name: &str, cx: &mut Context<Self>) {
        self.confirm("tree.delete_tag", name, args(&["tag", "-d", name]), cx);
    }

    pub fn cherry_pick(&mut self, commit: &str, parents: usize, cx: &mut Context<Self>) {
        let mut words = args(&["cherry-pick"]);
        words.extend(mainline(parents));
        words.push(commit.to_string());
        self.run_git(words, cx);
    }

    pub fn revert(&mut self, commit: &str, parents: usize, cx: &mut Context<Self>) {
        let mut words = args(&["revert"]);
        words.extend(mainline(parents));
        words.push(commit.to_string());
        self.run_git(words, cx);
    }

    pub fn op_continue(&mut self, op: Op, cx: &mut Context<Self>) {
        self.run_git(args(&[op.verb(), "--continue"]), cx);
    }

    /// Abort and skip throw work away; each asks.
    pub fn op_abort(&mut self, op: Op, cx: &mut Context<Self>) {
        let name = text(cx, &format!("tree.opname.{}", op.verb()), &[]);
        self.confirm("tree.abort", &name, args(&[op.verb(), "--abort"]), cx);
    }

    pub fn op_skip(&mut self, op: Op, cx: &mut Context<Self>) {
        let name = text(cx, &format!("tree.opname.{}", op.verb()), &[]);
        self.confirm("tree.skip", &name, args(&[op.verb(), "--skip"]), cx);
    }

    pub(crate) fn field_with(
        &self,
        value: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<TextInput> {
        let label = text(cx, key, &[]);
        let value = value.to_string();
        cx.new(|cx| {
            let mut field = TextInput::new(window, cx).label(label);
            field.set_text(value, cx);
            field
        })
    }

    /// A new branch at `at`, or at HEAD.
    pub fn ask_branch(&mut self, at: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.active else { return };
        let name = self.field_with("", "tree.branch.name", window, cx);
        self.ask = Some(Ask::NewBranch {
            repo,
            name,
            at,
            switch: true,
        });
        cx.notify();
    }

    pub fn ask_tag(&mut self, at: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.field_with("", "tree.tag.name", window, cx);
        let message = self.field_with("", "tree.tag.message", window, cx);
        let Some(repo) = self.active else { return };
        self.ask = Some(Ask::NewTag {
            repo,
            name,
            message,
            at,
        });
        cx.notify();
    }

    pub fn ask_rename(&mut self, old: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.active else { return };
        let name = self.field_with(old, "tree.branch.name", window, cx);
        self.ask = Some(Ask::Rename {
            repo,
            old: old.to_string(),
            name,
        });
        cx.notify();
    }

    /// The dialog's fields as arguments; empty names wait.
    pub fn answer(&mut self, cx: &mut Context<Self>) {
        let words = match &self.ask {
            Some(Ask::NewBranch {
                repo,
                name,
                at,
                switch,
            }) => {
                let name = name.read(cx).text().trim().to_string();
                let mut words = if *switch {
                    args(&["switch", "-c", &name])
                } else {
                    args(&["branch", &name])
                };
                words.extend(at.clone());
                (*repo, name, words)
            }
            Some(Ask::NewTag {
                repo,
                name,
                message,
                at,
            }) => {
                let (name, message) = (
                    name.read(cx).text().trim().to_string(),
                    message.read(cx).text().trim().to_string(),
                );
                let mut words = match message.is_empty() {
                    true => args(&["tag", &name]),
                    false => args(&["tag", "-a", &name, "-m", &message]),
                };
                words.extend(at.clone());
                (*repo, name, words)
            }
            Some(Ask::Rename { repo, old, name }) => {
                let name = name.read(cx).text().trim().to_string();
                (*repo, name.clone(), args(&["branch", "-m", old, &name]))
            }
            _ => return self.answer_places(cx),
        };
        let (repo, name, words) = words;
        if name.is_empty() {
            let words: SharedString = text(cx, "tree.name.empty", &[]);
            return self.toast(words, Severity::Warning, cx);
        }
        self.ask = None;
        self.write(repo, Write::Git(words), cx);
        cx.notify();
    }
}
