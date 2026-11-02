//! ElyMail's drafts, here and on the server.

pub(super) const EN: &[(&str, &str)] = &[
    ("mail.boxes.drafts", "Drafts"),
    ("mail.drafts.pick", "Pick a draft to see it"),
    ("mail.drafts.to", "To"),
    ("mail.drafts.cc", "Cc"),
    ("mail.drafts.bcc", "Bcc"),
    ("mail.drafts.saved", "Saved"),
    ("mail.drafts.continue", "Continue Writing"),
    ("mail.drafts.delete", "Delete Draft…"),
    ("mail.drafts.kept", "The message is kept in Drafts"),
    ("mail.drafts.unkept", "The draft was not saved: {error}"),
    (
        "mail.drafts.undropped",
        "The draft was not deleted: {error}",
    ),
    ("mail.drafts.unread", "Drafts could not be read: {error}"),
    (
        "mail.drafts.up",
        "Also keep drafts in the server's Drafts folder",
    ),
    (
        "mail.drafts.unsaved_choice",
        "The drafts choice was not saved: {error}",
    ),
    (
        "mail.drafts.no_folder",
        "Kept on this computer. The server lists no Drafts folder yet.",
    ),
    (
        "mail.drafts.server_failed",
        "Kept on this computer. The server copy was not saved: {error}",
    ),
    (
        "mail.drafts.server_stale",
        "An older copy may remain in the server's Drafts: {error}",
    ),
];

pub(super) const ZH_CN: &[(&str, &str)] = &[
    ("mail.boxes.drafts", "草稿"),
    ("mail.drafts.pick", "选一份草稿查看"),
    ("mail.drafts.to", "收件人"),
    ("mail.drafts.cc", "抄送"),
    ("mail.drafts.bcc", "密送"),
    ("mail.drafts.saved", "保存于"),
    ("mail.drafts.continue", "继续写"),
    ("mail.drafts.delete", "删除草稿…"),
    ("mail.drafts.kept", "这封邮件存在草稿里"),
    ("mail.drafts.unkept", "草稿没有保存：{error}"),
    ("mail.drafts.undropped", "草稿没有删除：{error}"),
    ("mail.drafts.unread", "草稿读不出：{error}"),
    ("mail.drafts.up", "同时把草稿存到服务器的草稿箱"),
    ("mail.drafts.unsaved_choice", "草稿的设置没有保存：{error}"),
    (
        "mail.drafts.no_folder",
        "已存在本机。服务器还没有列出草稿箱。",
    ),
    (
        "mail.drafts.server_failed",
        "已存在本机。服务器上的副本没有存下：{error}",
    ),
    (
        "mail.drafts.server_stale",
        "服务器的草稿箱里可能仍留着旧的副本：{error}",
    ),
];
