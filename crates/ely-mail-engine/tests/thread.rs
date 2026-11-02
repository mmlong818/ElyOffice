use ely_mail_engine::{Fetched, Key, MailboxId, MessageId, Role, Store};

/// `links` is the In-Reply-To then References header.
fn mail(id: Option<&str>, links: (&str, &str), subject: &str, day: u32) -> Vec<u8> {
    let mut head = format!(
        "From: Ann <ann@example.com>\r\nSubject: {subject}\r\n\
         Date: {day} Oct 2026 09:00:00 +0000\r\n"
    );
    if let Some(id) = id {
        head.push_str(&format!("Message-ID: <{id}>\r\n"));
    }
    if !links.0.is_empty() {
        head.push_str(&format!("In-Reply-To: {}\r\n", links.0));
    }
    if !links.1.is_empty() {
        head.push_str(&format!("References: {}\r\n", links.1));
    }
    format!("{head}Content-Type: text/plain\r\n\r\nday {day}\r\n").into_bytes()
}

struct Boxes {
    _dir: tempfile::TempDir,
    store: Store,
    inbox: MailboxId,
    sent: MailboxId,
    junk: MailboxId,
    other: MailboxId,
    uid: u32,
}

fn boxes() -> Boxes {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let ann = store
        .add_account(&fake::account("ann@example.com"))
        .unwrap();
    let bob = store
        .add_account(&fake::account("bob@example.com"))
        .unwrap();
    let [inbox, sent, junk] = store
        .set_folders(
            ann,
            &[
                ("INBOX", Some(Role::Inbox)),
                ("Sent", Some(Role::Sent)),
                ("Spam", Some(Role::Junk)),
            ],
        )
        .unwrap()
        .try_into()
        .unwrap();
    let [other] = store
        .set_folders(bob, &[("INBOX", Some(Role::Inbox))])
        .unwrap()
        .try_into()
        .unwrap();
    Boxes {
        _dir: dir,
        store,
        inbox,
        sent,
        junk,
        other,
        uid: 0,
    }
}

impl Boxes {
    fn put(&mut self, mailbox: MailboxId, fetched: Fetched) -> MessageId {
        self.uid += 1;
        let key = Key::Imap {
            uidvalidity: 1,
            uid: self.uid,
        };
        self.store.put(mailbox, &key, &[], fetched).unwrap()
    }

    fn whole(&mut self, mailbox: MailboxId, bytes: &[u8]) -> MessageId {
        self.put(mailbox, Fetched::Whole(bytes))
    }

    fn thread(&self, id: MessageId) -> Vec<MessageId> {
        let thread = self.store.thread(id).unwrap();
        thread.into_iter().map(|each| each.id).collect()
    }
}

#[test]
fn replies_across_folders_join_oldest_first() {
    let mut b = boxes();
    let reply = mail(Some("b@x"), ("<a@x>", "<a@x>"), "Re: plan", 2);
    let reply = b.whole(b.sent, &reply);
    let last = mail(Some("c@x"), ("", "<a@x> <b@x>"), "Re: plan", 3);
    let last = b.whole(b.inbox, &last);
    let first = b.whole(b.inbox, &mail(Some("a@x"), ("", ""), "plan", 1));
    for each in [first, reply, last] {
        assert_eq!(b.thread(each), [first, reply, last]);
    }
}

#[test]
fn the_same_subject_alone_joins_nothing() {
    let mut b = boxes();
    let one = b.whole(b.inbox, &mail(Some("a@x"), ("", ""), "lunch", 1));
    let two = b.whole(b.inbox, &mail(Some("b@x"), ("", ""), "lunch", 2));
    let none = b.whole(b.inbox, &mail(None, ("", ""), "lunch", 3));
    assert_eq!(b.thread(one), [one]);
    assert_eq!(b.thread(two), [two]);
    assert_eq!(b.thread(none), [none]);
}

#[test]
fn answers_to_a_missing_message_join_through_it() {
    let mut b = boxes();
    let one = b.whole(b.inbox, &mail(Some("b@x"), ("<gone@x>", ""), "Re: q", 1));
    let two = b.whole(b.inbox, &mail(None, ("", "<gone@x>"), "Re: q", 2));
    assert_eq!(b.thread(one), [one, two]);
    assert_eq!(b.thread(two), [one, two]);
}

#[test]
fn junk_and_other_accounts_stay_out_unless_picked() {
    let mut b = boxes();
    let first = b.whole(b.inbox, &mail(Some("a@x"), ("", ""), "plan", 1));
    let spam = b.whole(b.junk, &mail(Some("s@x"), ("<a@x>", ""), "Re: plan", 2));
    let elsewhere = b.whole(b.other, &mail(Some("o@x"), ("<a@x>", ""), "Re: plan", 3));
    assert_eq!(b.thread(first), [first]);
    assert_eq!(b.thread(spam), [first, spam]);
    assert_eq!(b.thread(elsewhere), [elsewhere]);
}

#[test]
fn a_message_held_twice_shows_once() {
    let mut b = boxes();
    let bytes = mail(Some("a@x"), ("", ""), "plan", 1);
    let end = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    let head = b.put(b.inbox, Fetched::Headers(&bytes[..end]));
    let whole = b.whole(b.sent, &bytes);
    let reply = b.whole(b.inbox, &mail(Some("b@x"), ("<a@x>", ""), "Re: plan", 2));
    assert_eq!(b.thread(reply), [whole, reply], "the copy with a body");
    assert_eq!(b.thread(head), [head, reply], "the picked copy");
}

#[test]
fn a_removed_message_leaves_its_thread() {
    let mut b = boxes();
    let first = b.whole(b.inbox, &mail(Some("a@x"), ("", ""), "plan", 1));
    let middle = b.whole(b.inbox, &mail(Some("b@x"), ("<a@x>", ""), "Re: plan", 2));
    let last = b.whole(b.inbox, &mail(Some("c@x"), ("<b@x>", ""), "Re: plan", 3));
    b.store.remove(middle).unwrap();
    assert_eq!(b.thread(first), [first]);
    assert_eq!(b.thread(last), [last]);
}

#[test]
fn a_message_written_again_takes_its_new_links() {
    let mut b = boxes();
    let first = b.whole(b.inbox, &mail(Some("a@x"), ("", ""), "plan", 1));
    let key = Key::Imap {
        uidvalidity: 1,
        uid: 99,
    };
    let bytes = mail(Some("b@x"), ("<a@x>", ""), "Re: plan", 2);
    let reply = b
        .store
        .put(b.inbox, &key, &[], Fetched::Whole(&bytes))
        .unwrap();
    let bytes = mail(Some("b@x"), ("", ""), "Re: plan", 2);
    b.store
        .put(b.inbox, &key, &[], Fetched::Whole(&bytes))
        .unwrap();
    assert_eq!(b.thread(first), [first]);
    assert_eq!(b.thread(reply), [reply]);
}
