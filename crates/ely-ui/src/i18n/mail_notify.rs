//! ElyMail's notices of new mail, and its files.

pub(super) const EN: &[(&str, &str)] = &[
    ("mail.notify.title", "New mail"),
    ("mail.notify.body.one", "From {senders}"),
    ("mail.notify.body", "{count} new messages from {senders}"),
    ("mail.notify.more", "{senders} and others"),
    ("mail.file.downloading", "Downloading the message with its files…"),
    ("mail.file.download_failed", "The files did not download: {error}"),
    ("mail.file.unread", "The file could not be read: {error}"),
    ("mail.file.unsaved", "The file was not saved: {error}"),
    ("mail.file.saved", "Saved {name}"),
    ("mail.file.unopened", "The file did not open: {error}"),
    ("mail.file.unsafe", "This kind of file can run a program, so it is not opened here. Save it, and open it yourself if you trust its sender."),
];

pub(super) const ZH_CN: &[(&str, &str)] = &[
    ("mail.notify.title", "新邮件"),
    ("mail.notify.body.one", "来自 {senders}"),
    ("mail.notify.body", "{count} 封新邮件，来自 {senders}"),
    ("mail.notify.more", "{senders} 等"),
    ("mail.file.downloading", "正在下载这封邮件及其附件…"),
    ("mail.file.download_failed", "附件没能下载：{error}"),
    ("mail.file.unread", "附件读不出：{error}"),
    ("mail.file.unsaved", "附件没能保存：{error}"),
    ("mail.file.saved", "已保存 {name}"),
    ("mail.file.unopened", "附件没能打开：{error}"),
    ("mail.file.unsafe", "这类文件可以运行程序，这里不直接打开。请先另存，信任发件人再自行打开。"),
];
