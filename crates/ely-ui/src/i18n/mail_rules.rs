//! ElyMail's local rules.

pub(super) const EN: &[(&str, &str)] = &[
    ("mail.rules.field", "Look at"),
    ("mail.rules.from", "From"),
    ("mail.rules.to", "To or Cc"),
    ("mail.rules.subject", "Subject"),
    ("mail.rules.words", "Words to look for"),
    ("mail.rules.action", "Then"),
    ("mail.rules.flag", "flag it"),
    ("mail.rules.archive", "archive it"),
    ("mail.rules.move", "move it to {name}"),
    ("mail.rules.add", "Add Rule"),
    ("mail.rules.remove", "Remove rule"),
    ("mail.rules.said", "New mail whose {field} has “{words}”: {action}"),
    ("mail.rules.unsaved", "The rule was not saved: {error}"),
    ("mail.rules.unread", "The rules could not be read: {error}"),
    ("mail.rules.refused", "A rule could not act: {error}"),
];

pub(super) const ZH_CN: &[(&str, &str)] = &[
    ("mail.rules.field", "查看"),
    ("mail.rules.from", "发件人"),
    ("mail.rules.to", "收件人或抄送"),
    ("mail.rules.subject", "主题"),
    ("mail.rules.words", "要找的文字"),
    ("mail.rules.action", "然后"),
    ("mail.rules.flag", "加标记"),
    ("mail.rules.archive", "归档"),
    ("mail.rules.move", "移到 {name}"),
    ("mail.rules.add", "添加规则"),
    ("mail.rules.remove", "删除规则"),
    ("mail.rules.said", "{field}含“{words}”的新邮件：{action}"),
    ("mail.rules.unsaved", "规则没能保存：{error}"),
    ("mail.rules.unread", "规则读不出：{error}"),
    ("mail.rules.refused", "有规则没能执行：{error}"),
];
