//! Read, flag, move, archive, delete, restore (C12.15).

use ely_gpui_component::{
    buttons::IconButton,
    menus::{DropdownMenu, Menu, MenuItem},
    overlays::ConfirmDialog,
    primitives::{IconName, Severity},
};
use ely_mail_engine::{AccountId, Change, LocalFolder, MessageId, Role, push::Dropped};
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Styled as _, Window, div,
};

use crate::{
    boxes::Fact,
    mail::{Mail, text},
};

pub type Offer = (&'static str, IconName, &'static str, Option<Change>);

/// Messages asked to go for good, and how many would.
pub struct Erasing {
    pub ids: Vec<MessageId>,
    pub count: usize,
}

impl Mail {
    /// The picked messages change here now, the server later.
    pub fn organize(&mut self, change: Change, cx: &mut Context<Self>) {
        let ids = self.listing.picked().iter().map(|fact| fact.id).collect();
        self.change(ids, change, cx);
    }

    fn change(&mut self, ids: Vec<MessageId>, change: Change, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return log::info!("mail: {change:?} with nothing picked");
        }
        self.with_store(
            cx,
            move |store| store.change(&ids, change),
            move |mail, done, cx| match done {
                Ok(accounts) => {
                    mail.read_mail(cx);
                    // Counted now, paused or not.
                    for id in accounts {
                        mail.read_synced(id, cx);
                        mail.push(id, cx);
                    }
                }
                Err(error) => mail.failed("mail.organize.failed", &error, cx),
            },
        );
    }

    /// Asks first when any would go for good.
    pub fn delete_picked(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<MessageId> = self.listing.picked().iter().map(|fact| fact.id).collect();
        if ids.is_empty() {
            return log::info!("mail: delete with nothing picked");
        }
        let asked = ids.clone();
        self.with_store(
            cx,
            move |store| store.forever(&asked),
            move |mail, read, cx| match read {
                Ok(0) => mail.change(ids, Change::Delete { erase: false }, cx),
                Ok(count) => mail.erasing = Some(Erasing { ids, count }),
                Err(error) => mail.failed("mail.organize.failed", &error, cx),
            },
        );
    }

    /// A star pressed in the list.
    pub fn star(&mut self, key: &SharedString, on: bool, cx: &mut Context<Self>) {
        match self.listing.facts.get(key) {
            Some(fact) => self.change(vec![fact.id], Change::Flagged(on), cx),
            None => log::error!("mail: a starred row is not listed"),
        }
    }

    /// Reading one message marks it read.
    pub fn read_on_open(&mut self, cx: &mut Context<Self>) {
        if let [fact] = self.listing.picked()[..]
            && !fact.seen
        {
            self.change(vec![fact.id], Change::Seen(true), cx);
        }
    }

    /// Changes let go, kept until told once.
    pub fn tell_dropped(&mut self, id: AccountId, cx: &mut Context<Self>) {
        self.with_store(
            cx,
            move |store| store.take_dropped(id),
            move |mail, taken, cx| match taken {
                Ok(dropped) => {
                    if let Some(words) = dropped_words(&dropped, cx) {
                        log::warn!("mail: {} changes let go", dropped.len());
                        mail.toast(words, Severity::Warning, cx);
                    }
                }
                Err(error) => mail.failed("mail.organize.failed", &error, cx),
            },
        );
    }

    /// Buttons for the picked messages; none when nothing is.
    pub fn actions(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let picked = self.listing.picked();
        if picked.is_empty() {
            return None;
        }
        let mut row = div()
            .debug_selector(|| "mail-actions".into())
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .py_1();
        for (id, icon, key, change) in self.offered(&picked) {
            let label = text(cx, key, &[]);
            row = row.child(self.button(id, icon, label, change, cx));
        }
        row = row.children(self.move_menu(&picked, cx));
        if let [one] = picked.as_slice() {
            let (this, id) = (cx.weak_entity(), one.id);
            let save = IconButton::new("mail-act-eml", IconName::Save)
                .tooltip(text(cx, "mail.eml.save", &[]))
                .on_click(move |_, _, cx| {
                    this.update(cx, |mail, cx| mail.save_eml(id, cx)).ok();
                });
            row = row.child(save);
        }
        Some(row.into_any_element())
    }

    /// Id, icon, label key and change; `None` deletes.
    pub fn offered(&self, picked: &[Fact]) -> Vec<Offer> {
        let unread = picked.iter().any(|fact| !fact.seen);
        let unflagged = picked.iter().any(|fact| !fact.flagged);
        let imap = picked.iter().all(|fact| !fact.pop);
        let read = if unread {
            "mail.act.read"
        } else {
            "mail.act.unread"
        };
        let flag = if unflagged {
            "mail.act.flag"
        } else {
            "mail.act.unflag"
        };
        let mut shown = vec![
            (
                "mail-act-read",
                IconName::Mail,
                read,
                Some(Change::Seen(unread)),
            ),
            (
                "mail-act-flag",
                IconName::Star,
                flag,
                Some(Change::Flagged(unflagged)),
            ),
        ];
        if imap && picked.iter().all(|fact| self.archivable(fact)) {
            let archive = Some(Change::Archive);
            shown.push((
                "mail-act-archive",
                IconName::Archive,
                "mail.act.archive",
                archive,
            ));
        }
        shown.push(("mail-act-delete", IconName::Trash2, "mail.act.delete", None));
        if imap && picked.iter().all(|fact| fact.role == Some(Role::Trash)) {
            let restore = Some(Change::Restore);
            shown.push((
                "mail-act-restore",
                IconName::Inbox,
                "mail.act.restore",
                restore,
            ));
        }
        shown
    }

    /// `None` deletes, asking first when for good.
    fn button(
        &self,
        id: &'static str,
        icon: IconName,
        label: SharedString,
        change: Option<Change>,
        cx: &mut Context<Self>,
    ) -> IconButton {
        let this = cx.weak_entity();
        IconButton::new(id, icon)
            .tooltip(label)
            .on_click(move |_, _, cx| {
                this.update(cx, |mail, cx| match change {
                    Some(change) => mail.organize(change, cx),
                    None => mail.delete_picked(cx),
                })
                .ok();
            })
    }

    /// An Archive, or Gmail's All Mail, not its own.
    fn archivable(&self, fact: &Fact) -> bool {
        let Some(syncing) = self.syncs.get(&fact.account) else {
            return false;
        };
        let folders = &syncing.folders;
        let target = (folders
            .iter()
            .find(|folder| folder.role == Some(Role::Archive)))
        .or_else(|| folders.iter().find(|folder| folder.role == Some(Role::All)));
        target.is_some_and(|target| target.id != fact.mailbox)
    }

    /// The picked messages' one IMAP account, and its other folders.
    pub fn move_targets(&self, picked: &[Fact]) -> Option<(AccountId, Vec<&LocalFolder>)> {
        let account = picked.first()?.account;
        if picked
            .iter()
            .any(|fact| fact.pop || fact.account != account)
        {
            return None;
        }
        let folders = &self.syncs.get(&account)?.folders;
        let targets = folders
            .iter()
            .filter(|folder| !folder.local && !picked.iter().all(|fact| fact.mailbox == folder.id))
            .collect();
        Some((account, targets))
    }

    fn move_menu(&self, picked: &[Fact], cx: &mut Context<Self>) -> Option<DropdownMenu> {
        let (_, targets) = self.move_targets(picked)?;
        let mut menu = Menu::new();
        for folder in targets {
            let (this, target) = (cx.weak_entity(), folder.id);
            let name = crate::side::folder_name(folder, cx);
            menu = menu.item(MenuItem::new(name).on_click(move |_, cx| {
                this.update(cx, |mail, cx| mail.organize(Change::Move(target), cx))
                    .ok();
            }));
        }
        let label = text(cx, "mail.act.move", &[]);
        Some(DropdownMenu::new("mail-act-move", label, menu).icon(IconName::Folder))
    }

    pub fn erase_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let erasing = self.erasing.as_ref()?;
        let n = erasing.count;
        let count = n.to_string();
        let args = [("n", count.as_str())];
        let (cancel, confirm) = (cx.weak_entity(), cx.weak_entity());
        let ids = erasing.ids.clone();
        let dialog = ConfirmDialog::new(
            "mail-erase-dialog",
            ely_ui::i18n::counted(cx, "mail.erase.title", n as u64, &args),
            text(cx, "mail.erase.message", &[]),
            move |_: &mut Window, cx: &mut App| {
                cancel
                    .update(cx, |mail, cx| {
                        mail.erasing = None;
                        cx.notify();
                    })
                    .ok();
            },
        )
        .confirm(text(cx, "mail.erase.go", &[]))
        .destructive()
        .on_confirm(move |_, cx| {
            let ids = ids.clone();
            confirm
                .update(cx, |mail, cx| {
                    mail.change(ids, Change::Delete { erase: true }, cx)
                })
                .ok();
        });
        Some(dialog.into_any_element())
    }
}

/// How many were let go, and the first reason.
pub fn dropped_words(dropped: &[Dropped], cx: &App) -> Option<SharedString> {
    let reason = match dropped.first()? {
        Dropped::Gone => text(cx, "mail.organize.gone", &[]),
        Dropped::Renumbered => text(cx, "mail.organize.renumbered", &[]),
        Dropped::Refused(words) => words.clone().into(),
        Dropped::Unsupported(name) => text(cx, "mail.organize.unsupported", &[("name", name)]),
        Dropped::Unplaced => text(cx, "mail.organize.unplaced", &[]),
        Dropped::Vanished(name) => text(cx, "mail.organize.vanished", &[("name", name)]),
    };
    let n = dropped.len();
    let count = n.to_string();
    let args = [("n", count.as_str()), ("reason", &reason)];
    Some(ely_ui::i18n::counted(
        cx,
        "mail.organize.dropped",
        n as u64,
        &args,
    ))
}
