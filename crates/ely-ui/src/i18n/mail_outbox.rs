//! ElyMail's outbox and how sends end.

pub(super) const EN: &[(&str, &str)] = &[
    ("mail.boxes.outbox", "Outbox"),
    ("mail.outbox.pick", "Pick a message to see where it stands"),
    ("mail.outbox.state", "Status"),
    ("mail.outbox.queued_at", "Queued"),
    ("mail.outbox.tried_at", "Last tried"),
    ("mail.outbox.message_id", "Message-ID"),
    ("mail.outbox.queued", "Queued"),
    ("mail.outbox.waiting", "Waiting for the server: {error}"),
    ("mail.outbox.sending", "Sending"),
    (
        "mail.outbox.accepted",
        "Accepted by the server; its copy is being saved in Sent",
    ),
    (
        "mail.outbox.unfiled",
        "Accepted by the server; its copy was not saved in Sent: {error}",
    ),
    (
        "mail.outbox.filing_later",
        "Accepted by the server; its copy for Sent waits for the server: {error}. It is tried again at the next sync.",
    ),
    ("mail.outbox.failed", "Failed: {error}"),
    ("mail.outbox.unsure", "Outcome unknown: {error}"),
    (
        "mail.outbox.stopped",
        "Outcome unknown: ElyMail stopped while sending it",
    ),
    ("mail.outbox.cancelled", "Cancelled"),
    (
        "mail.outbox.unsure_help",
        "The server may have this message. ElyMail does not send it again by itself. \
         Check Sent or ask a recipient, then remove it here or send it again.",
    ),
    (
        "mail.outbox.accepted_help",
        "Accepted means the server took it, not that it was delivered or read.",
    ),
    ("mail.outbox.retry", "Try Again"),
    ("mail.outbox.send", "Send"),
    ("mail.outbox.send_again", "Send Again Anyway"),
    ("mail.outbox.cancel", "Cancel Sending"),
    ("mail.outbox.edit", "Edit"),
    ("mail.outbox.forget", "Remove"),
    ("mail.outbox.file_again", "Save the Copy Again"),
    ("mail.outbox.sent", "The server accepted the message"),
    (
        "mail.outbox.offline",
        "The server was not reached. The message waits in the Outbox and is tried again \
         at the next sync, or when you choose Try Again there. It can be cancelled there.",
    ),
    (
        "mail.outbox.failed_toast",
        "Not sent: {error}. The message stays in the Outbox.",
    ),
    (
        "mail.outbox.unsure_toast",
        "The server may have the message but did not confirm it: {error}. \
         It is not sent again by itself; see the Outbox.",
    ),
    (
        "mail.outbox.unfiled_toast",
        "Sent, but its copy was not saved in Sent: {error}. Only the copy is tried again.",
    ),
    (
        "mail.outbox.unfiled_later",
        "Sent, but the server for its Sent copy was not reached: {error}. The copy is tried again at the next sync; the message is not sent again.",
    ),
    (
        "mail.outbox.no_sent_folder",
        "the account lists no Sent folder",
    ),
    (
        "mail.outbox.unread",
        "The Outbox could not be read: {error}",
    ),
    ("mail.outbox.unsaved", "The Outbox was not updated: {error}"),
    (
        "mail.outbox.unqueued",
        "The message was not put in the Outbox: {error}",
    ),
    ("mail.outbox.copy", "Save a copy in Sent"),
    (
        "mail.outbox.copy_note",
        "Off by default here: this provider files sent mail itself.",
    ),
    (
        "mail.outbox.unsaved_choice",
        "The Sent copy choice was not saved: {error}",
    ),
    ("mail.outbox.files_size", "Attachments: {size}"),
    (
        "mail.outbox.too_large",
        "About {size} in all; {address}'s server takes at most {limit}",
    ),
    ("mail.outbox.quit.title", "Quit with mail unsent?"),
    (
        "mail.outbox.quit.message",
        "{n} messages in the Outbox are not done yet. ElyMail does not send them after \
         it quits; it tries again the next time it opens.",
    ),
    (
        "mail.outbox.quit.message.one",
        "1 message in the Outbox is not done yet. ElyMail does not send it after it \
         quits; it tries again the next time it opens.",
    ),
    (
        "mail.outbox.quit.sending",
        "One is being sent right now; quitting may leave its outcome unknown.",
    ),
    ("mail.outbox.quit.go", "Quit"),
];

pub(super) const ZH_CN: &[(&str, &str)] = &[
    ("mail.boxes.outbox", "发件箱"),
    ("mail.outbox.pick", "选一封邮件查看其状态"),
    ("mail.outbox.state", "状态"),
    ("mail.outbox.queued_at", "排入时间"),
    ("mail.outbox.tried_at", "上次尝试"),
    ("mail.outbox.message_id", "Message-ID"),
    ("mail.outbox.queued", "排队中"),
    ("mail.outbox.waiting", "等待服务器：{error}"),
    ("mail.outbox.sending", "正在发送"),
    (
        "mail.outbox.accepted",
        "服务器已接受；正在把副本存到“已发送”",
    ),
    (
        "mail.outbox.unfiled",
        "服务器已接受；副本没有存到“已发送”：{error}",
    ),
    (
        "mail.outbox.filing_later",
        "服务器已接受；存到“已发送”的副本在等服务器：{error}。下次同步时再存。",
    ),
    ("mail.outbox.failed", "失败：{error}"),
    ("mail.outbox.unsure", "结果未知：{error}"),
    ("mail.outbox.stopped", "结果未知：发送途中 ElyMail 停止了"),
    ("mail.outbox.cancelled", "已取消"),
    (
        "mail.outbox.unsure_help",
        "服务器可能已收下这封邮件。ElyMail 不会自行再发。请先查看“已发送”或问收件人，再在此移除，或者再发一次。",
    ),
    (
        "mail.outbox.accepted_help",
        "已接受只说明服务器收下了，不等于已送达或已读。",
    ),
    ("mail.outbox.retry", "重试"),
    ("mail.outbox.send", "发送"),
    ("mail.outbox.send_again", "仍然再发一次"),
    ("mail.outbox.cancel", "取消发送"),
    ("mail.outbox.edit", "改写"),
    ("mail.outbox.forget", "移除"),
    ("mail.outbox.file_again", "再存一次副本"),
    ("mail.outbox.sent", "服务器已接受这封邮件"),
    (
        "mail.outbox.offline",
        "没连上服务器。邮件留在发件箱，下次同步时或在发件箱里点“重试”时再发；也可以在那里取消。",
    ),
    (
        "mail.outbox.failed_toast",
        "未发送：{error}。邮件留在发件箱。",
    ),
    (
        "mail.outbox.unsure_toast",
        "服务器可能已收下，但没有确认：{error}。不会自行再发，请到发件箱查看。",
    ),
    (
        "mail.outbox.unfiled_toast",
        "已发出，但副本没有存到“已发送”：{error}。只会再存副本，不会再发邮件。",
    ),
    (
        "mail.outbox.unfiled_later",
        "已发出，但存“已发送”副本时没连上服务器：{error}。下次同步时再存副本，不会再发邮件。",
    ),
    ("mail.outbox.no_sent_folder", "该账号没有列出“已发送”文件夹"),
    ("mail.outbox.unread", "发件箱读不出：{error}"),
    ("mail.outbox.unsaved", "发件箱没有更新：{error}"),
    ("mail.outbox.unqueued", "邮件没有放进发件箱：{error}"),
    ("mail.outbox.copy", "在“已发送”里存一份副本"),
    (
        "mail.outbox.copy_note",
        "此处默认关闭：这家服务商会自己把发出的邮件存进“已发送”。",
    ),
    ("mail.outbox.unsaved_choice", "副本的设置没有保存：{error}"),
    ("mail.outbox.files_size", "附件共 {size}"),
    (
        "mail.outbox.too_large",
        "整封约 {size}，{address} 的服务器最多收 {limit}",
    ),
    ("mail.outbox.quit.title", "还有邮件没发出，要退出吗？"),
    (
        "mail.outbox.quit.message",
        "发件箱里有 {n} 封邮件还没处理完。ElyMail 退出后不会发送，下次打开时再试。",
    ),
    (
        "mail.outbox.quit.message.one",
        "发件箱里有 1 封邮件还没处理完。ElyMail 退出后不会发送，下次打开时再试。",
    ),
    (
        "mail.outbox.quit.sending",
        "其中一封正在发送；现在退出，它的结果可能无从得知。",
    ),
    ("mail.outbox.quit.go", "退出"),
];
