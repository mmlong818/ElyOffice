//! ElyMail's read, flag, move, delete and their server queue.

pub(super) const EN: &[(&str, &str)] = &[
    ("mail.act.read", "Mark as Read"),
    ("mail.act.unread", "Mark as Unread"),
    ("mail.act.flag", "Flag"),
    ("mail.act.unflag", "Remove Flag"),
    ("mail.act.archive", "Archive"),
    ("mail.act.delete", "Delete"),
    ("mail.act.restore", "Move to Inbox"),
    ("mail.act.move", "Move To"),
    ("mail.organize.failed", "Not changed: {error}"),
    (
        "mail.organize.dropped",
        "{n} changes were not made on the server and are undone: {reason}",
    ),
    (
        "mail.organize.dropped.one",
        "A change was not made on the server and is undone: {reason}",
    ),
    (
        "mail.organize.gone",
        "the message is no longer on the server",
    ),
    (
        "mail.organize.renumbered",
        "the folder was rebuilt on the server",
    ),
    (
        "mail.organize.unsupported",
        "the server does not support {name}",
    ),
    (
        "mail.organize.unplaced",
        "the server moved the message without saying where",
    ),
    (
        "mail.organize.vanished",
        "the folder {name} is no longer on the server",
    ),
    ("mail.erase.title", "Delete {n} messages for good?"),
    ("mail.erase.title.one", "Delete this message for good?"),
    (
        "mail.erase.message",
        "They cannot be restored, here or on the server.",
    ),
    ("mail.erase.go", "Delete for Good"),
    ("mail.sync.waiting", "Changes"),
    (
        "mail.sync.waiting_value",
        "{n} changes made here wait for the server",
    ),
    (
        "mail.sync.waiting_value.one",
        "1 change made here waits for the server",
    ),
    ("mail.sync.waiting_none", "Every change is on the server"),
];

pub(super) const ZH_CN: &[(&str, &str)] = &[
    ("mail.act.read", "标为已读"),
    ("mail.act.unread", "标为未读"),
    ("mail.act.flag", "标记"),
    ("mail.act.unflag", "取消标记"),
    ("mail.act.archive", "归档"),
    ("mail.act.delete", "删除"),
    ("mail.act.restore", "移回收件箱"),
    ("mail.act.move", "移到"),
    ("mail.organize.failed", "未改动：{error}"),
    (
        "mail.organize.dropped",
        "{n} 项改动未能在服务器上完成，已撤回：{reason}",
    ),
    (
        "mail.organize.dropped.one",
        "1 项改动未能在服务器上完成，已撤回：{reason}",
    ),
    ("mail.organize.gone", "服务器上已没有这封邮件"),
    ("mail.organize.renumbered", "服务器重建了这个文件夹"),
    ("mail.organize.unsupported", "服务器不支持 {name}"),
    (
        "mail.organize.unplaced",
        "服务器移动了邮件，却未告知移到何处",
    ),
    ("mail.organize.vanished", "服务器上已没有文件夹 {name}"),
    ("mail.erase.title", "永久删除 {n} 封邮件？"),
    ("mail.erase.title.one", "永久删除这封邮件？"),
    ("mail.erase.message", "本机与服务器上都无法恢复。"),
    ("mail.erase.go", "永久删除"),
    ("mail.sync.waiting", "改动"),
    (
        "mail.sync.waiting_value",
        "本机的 {n} 项改动等待同步到服务器",
    ),
    (
        "mail.sync.waiting_value.one",
        "本机的 1 项改动等待同步到服务器",
    ),
    ("mail.sync.waiting_none", "改动都已同步到服务器"),
];
