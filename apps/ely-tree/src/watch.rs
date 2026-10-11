//! Rereads after outside changes (TREE-02, §13.3).

use std::time::Duration;

use futures::{StreamExt as _, channel::mpsc};
use gpui::Context;
use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

use crate::{repo::Repo, tree::Tree};

/// Bursts, such as a checkout, settle into one read.
const SETTLE: Duration = Duration::from_millis(250);

/// Watches work tree and Git directory until dropped.
pub fn start(id: u64, repo: &Repo, cx: &mut Context<Tree>) -> notify::Result<RecommendedWatcher> {
    let (sender, mut changes) = mpsc::unbounded();
    let root = repo.root.display().to_string();
    let mut watcher =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
            // Git's reads, ours included, are no change.
            Ok(event) if ely_ui::watching::only_read(&event.kind) => {}
            Ok(_) => {
                let _ = sender.unbounded_send(());
            }
            Err(error) => log::warn!("tree: watching {root} failed: {error}"),
        })?;
    let mut watched = vec![repo.root.clone()];
    // A linked worktree's Git directories lie elsewhere.
    for dir in [&repo.common_dir, &repo.git_dir] {
        if !watched.iter().any(|top| dir.starts_with(top)) {
            watched.push(dir.clone());
        }
    }
    for dir in &watched {
        watcher.watch(dir, RecursiveMode::Recursive)?;
    }
    cx.spawn(async move |this, cx| {
        while changes.next().await.is_some() {
            cx.background_executor().timer(SETTLE).await;
            while changes.try_recv().is_ok() {}
            if this.update(cx, |tree, cx| tree.read(id, cx)).is_err() {
                break;
            }
        }
    })
    .detach();
    log::info!("tree: watching {}", repo.root.display());
    Ok(watcher)
}
