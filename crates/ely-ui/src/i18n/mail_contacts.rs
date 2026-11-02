//! ElyMail's contacts.

pub(super) const EN: &[(&str, &str)] = &[
    ("mail.boxes.contacts", "Contacts"),
    ("mail.contacts.name", "Name"),
    ("mail.contacts.address", "Email address"),
    ("mail.contacts.add", "Add Contact"),
    ("mail.contacts.remove", "Remove contact"),
    ("mail.contacts.import", "Import vCard…"),
    ("mail.contacts.export", "Export vCard…"),
    ("mail.contacts.empty", "No contacts yet. People you write to are added here."),
    ("mail.contacts.unread", "The contacts could not be read: {error}"),
    ("mail.contacts.unsaved", "The contact was not saved: {error}"),
    (
        "mail.contacts.imported",
        "Imported: {added} added, {renamed} renamed, {unchanged} already here; {skipped} cards had no address",
    ),
    ("mail.contacts.unimported", "The vCard file was not imported: {error}"),
    ("mail.contacts.unexported", "The contacts were not exported: {error}"),
];

pub(super) const ZH_CN: &[(&str, &str)] = &[
    ("mail.boxes.contacts", "联系人"),
    ("mail.contacts.name", "姓名"),
    ("mail.contacts.address", "邮件地址"),
    ("mail.contacts.add", "添加联系人"),
    ("mail.contacts.remove", "删除联系人"),
    ("mail.contacts.import", "导入 vCard…"),
    ("mail.contacts.export", "导出 vCard…"),
    ("mail.contacts.empty", "还没有联系人。写过信的人会加在这里。"),
    ("mail.contacts.unread", "联系人读不出：{error}"),
    ("mail.contacts.unsaved", "联系人没能保存：{error}"),
    (
        "mail.contacts.imported",
        "已导入：新增 {added} 个，改名 {renamed} 个，已有 {unchanged} 个；{skipped} 张卡片没有地址",
    ),
    ("mail.contacts.unimported", "vCard 文件没能导入：{error}"),
    ("mail.contacts.unexported", "联系人没能导出：{error}"),
];
