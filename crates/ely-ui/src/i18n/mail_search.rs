//! ElyMail's search of the mail held here.

pub(super) const EN: &[(&str, &str)] = &[
    ("mail.search", "Search mail"),
    ("mail.search.in", "Search in"),
    ("mail.search.all", "Anywhere"),
    ("mail.search.from", "From"),
    ("mail.search.to", "To"),
    ("mail.search.subject", "Subject"),
    ("mail.search.body", "Text"),
    ("mail.search.files", "Attachment names"),
    ("mail.search.any_time", "Any date"),
    (
        "mail.search.local",
        "Searched mail kept on this computer, in every account.",
    ),
    (
        "mail.search.headers",
        "Searched mail kept on this computer, in every account. {count} have only their headers here, so their text was not searched.",
    ),
    ("mail.search.none", "No messages match"),
];

pub(super) const ZH_CN: &[(&str, &str)] = &[
    ("mail.search", "搜索邮件"),
    ("mail.search.in", "搜索范围"),
    ("mail.search.all", "全部"),
    ("mail.search.from", "发件人"),
    ("mail.search.to", "收件人"),
    ("mail.search.subject", "主题"),
    ("mail.search.body", "正文"),
    ("mail.search.files", "附件名"),
    ("mail.search.any_time", "不限日期"),
    ("mail.search.local", "已搜索存在本机的邮件，含全部账号。"),
    (
        "mail.search.headers",
        "已搜索存在本机的邮件，含全部账号。其中 {count} 封只存了邮件头，未搜索其正文。",
    ),
    ("mail.search.none", "没有相符的邮件"),
];
