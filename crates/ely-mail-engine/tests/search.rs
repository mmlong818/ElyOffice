use ely_mail_engine::{
    Change, Fetched, Field, Filter, Key, MailboxId, MessageId, Role, Search, Store,
};

/// Ann's inbox at UIDVALIDITY 1.
fn store() -> (tempfile::TempDir, Store, MailboxId) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let id = store
        .add_account(&fake::account("ann@example.com"))
        .unwrap();
    let [inbox] = store
        .set_folders(id, &[("INBOX", Some(Role::Inbox))])
        .unwrap()
        .try_into()
        .unwrap();
    store.open_mailbox(inbox, 1).unwrap();
    (dir, store, inbox)
}

fn mail(from: &str, to: &str, subject: &str, date: &str, body: &str) -> Vec<u8> {
    format!(
        "From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\nDate: {date}\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n"
    )
    .into_bytes()
}

fn put(store: &mut Store, inbox: MailboxId, uid: u32, bytes: &[u8]) -> MessageId {
    let key = Key::Imap {
        uidvalidity: 1,
        uid,
    };
    store.put(inbox, &key, &[], Fetched::Whole(bytes)).unwrap()
}

fn found(store: &Store, words: &str, field: Field) -> Vec<MessageId> {
    let search = Search {
        words: words.into(),
        field,
        ..Search::default()
    };
    let found = store.search(&search).unwrap();
    found.messages.into_iter().map(|each| each.id).collect()
}

const OCT: &str = "Mon, 19 Oct 2026 10:00:00 +0000";
const SEP: &str = "Fri, 18 Sep 2026 10:00:00 +0000";

#[test]
fn chinese_words_of_any_length_are_found() {
    let (_dir, mut store, inbox) = store();
    let one = put(
        &mut store,
        inbox,
        1,
        &mail("a@x.com", "b@x.com", "周报", OCT, "明天的会议改到下午"),
    );
    let two = put(
        &mut store,
        inbox,
        2,
        &mail("a@x.com", "b@x.com", "通知", OCT, "会场在三楼，议程另发"),
    );
    assert_eq!(found(&store, "会议", Field::All), [one]);
    assert_eq!(found(&store, "会", Field::All), [two, one]);
    assert_eq!(found(&store, "下午 会议", Field::Body), [one]);
    assert_eq!(found(&store, "周报", Field::Subject), [one]);
    assert_eq!(found(&store, "会议室", Field::All), []);
}

#[test]
fn latin_words_match_from_their_start_in_any_case() {
    let (_dir, mut store, inbox) = store();
    let one = put(
        &mut store,
        inbox,
        1,
        &mail("a@x.com", "b@x.com", "Café", OCT, "An Example here"),
    );
    assert_eq!(found(&store, "exam", Field::All), [one]);
    assert_eq!(found(&store, "EXAMPLE", Field::All), [one]);
    assert_eq!(found(&store, "cafe", Field::Subject), [one]);
    // Inside a word is not a match.
    assert_eq!(found(&store, "ample", Field::All), []);
    assert_eq!(found(&store, "\"here an\"", Field::All), []);
    assert_eq!(found(&store, "\"example here\"", Field::All), [one]);
}

#[test]
fn senders_and_recipients_are_apart() {
    let (_dir, mut store, inbox) = store();
    let from = put(
        &mut store,
        inbox,
        1,
        &mail("Bob Lee <bob@corp.com>", "ann@example.com", "a", OCT, "x"),
    );
    let to = put(
        &mut store,
        inbox,
        2,
        &mail("ann@example.com", "Bob Lee <bob@corp.com>", "b", OCT, "y"),
    );
    assert_eq!(found(&store, "bob", Field::From), [from]);
    assert_eq!(found(&store, "bob@corp", Field::To), [to]);
    assert_eq!(found(&store, "lee", Field::All), [to, from]);
    assert_eq!(found(&store, "bob", Field::Subject), []);
}

#[test]
fn attachment_names_are_searched() {
    let (_dir, mut store, inbox) = store();
    let bytes = b"From: a@x.com\r\nTo: b@x.com\r\nSubject: s\r\nDate: Mon, 19 Oct 2026 10:00:00 +0000\r\n\
        Content-Type: multipart/mixed; boundary=B\r\n\r\n--B\r\nContent-Type: text/plain\r\n\r\nhi\r\n\
        --B\r\nContent-Type: application/pdf\r\nContent-Disposition: attachment; filename=\"Q3-report.pdf\"\r\n\r\n%PDF\r\n--B--\r\n";
    let one = put(&mut store, inbox, 1, bytes);
    assert_eq!(found(&store, "report", Field::Files), [one]);
    assert_eq!(found(&store, "hi", Field::Files), []);
}

#[test]
fn dates_narrow_and_alone_list_a_span() {
    let (_dir, mut store, inbox) = store();
    let october = put(
        &mut store,
        inbox,
        1,
        &mail("a@x.com", "b@x.com", "plan", OCT, "x"),
    );
    put(
        &mut store,
        inbox,
        2,
        &mail("a@x.com", "b@x.com", "plan", SEP, "x"),
    );
    // 1 Oct 2026 to 1 Nov 2026, UTC.
    let dates = Some((1_790_812_800, 1_793_491_200));
    let words = Search {
        words: "plan".into(),
        dates,
        ..Search::default()
    };
    let ids = |search: &Search| -> Vec<MessageId> {
        let found = store.search(search).unwrap();
        found.messages.into_iter().map(|each| each.id).collect()
    };
    assert_eq!(ids(&words), [october]);
    let alone = Search {
        dates,
        ..Search::default()
    };
    assert_eq!(ids(&alone), [october]);
    assert!(store.search(&Search::default()).is_err());
}

#[test]
fn bodies_not_held_are_counted_not_searched() {
    let (_dir, mut store, inbox) = store();
    let key = Key::Imap {
        uidvalidity: 1,
        uid: 1,
    };
    let headers = mail("a@x.com", "b@x.com", "budget", OCT, "");
    let head = store
        .put(inbox, &key, &[], Fetched::Headers(&headers))
        .unwrap();
    let whole = put(
        &mut store,
        inbox,
        2,
        &mail("a@x.com", "b@x.com", "other", OCT, "budget"),
    );
    let search = Search {
        words: "budget".into(),
        ..Search::default()
    };
    let found = store.search(&search).unwrap();
    let ids: Vec<_> = found.messages.iter().map(|each| each.id).collect();
    assert_eq!(ids, [whole, head]);
    assert_eq!(found.unsearched, 1);
    let subject = Search {
        field: Field::Subject,
        ..search
    };
    assert_eq!(store.search(&subject).unwrap().unsearched, 0);
}

#[test]
fn filters_and_hidden_deletes_apply() {
    let (_dir, mut store, inbox) = store();
    let one = put(
        &mut store,
        inbox,
        1,
        &mail("a@x.com", "b@x.com", "s", OCT, "lunch"),
    );
    let two = put(
        &mut store,
        inbox,
        2,
        &mail("a@x.com", "b@x.com", "s", OCT, "lunch"),
    );
    store.change(&[one], Change::Flagged(true)).unwrap();
    let flagged = Search {
        words: "lunch".into(),
        filter: Filter {
            flagged: true,
            ..Filter::default()
        },
        ..Search::default()
    };
    let found = store.search(&flagged).unwrap();
    assert_eq!(
        found
            .messages
            .iter()
            .map(|each| each.id)
            .collect::<Vec<_>>(),
        [one]
    );
    store
        .change(&[two], Change::Delete { erase: true })
        .unwrap();
    assert_eq!(found_all(&store, "lunch"), [one]);
}

fn found_all(store: &Store, words: &str) -> Vec<MessageId> {
    found(store, words, Field::All)
}

#[test]
fn punctuation_alone_is_not_a_search() {
    let (_dir, mut store, inbox) = store();
    put(
        &mut store,
        inbox,
        1,
        &mail("a@x.com", "b@x.com", "s", OCT, "x"),
    );
    let search = Search {
        words: "\" * - ( ) :".into(),
        ..Search::default()
    };
    assert!(store.search(&search).is_err());
    // Query syntax in words is only words.
    assert_eq!(found_all(&store, "x OR NOT\" y"), []);
}

const MIXED: &str = "From: a@x.com\r\nSubject: s\r\nDate: Mon, 19 Oct 2026 10:00:00 +0000\r\n\
    Content-Type: multipart/mixed; boundary=b\r\n\r\n\
    --b\r\nContent-Type: text/plain\r\n\r\nFirst section\r\n\
    --b\r\nContent-Type: text/html\r\n\r\n<p>The meeting is tomorrow</p>\r\n--b--\r\n";

const ALTERNATIVE: &str = "From: a@x.com\r\nSubject: s\r\nDate: Mon, 19 Oct 2026 10:00:00 +0000\r\n\
    Content-Type: multipart/alternative; boundary=b\r\n\r\n\
    --b\r\nContent-Type: text/plain\r\n\r\nplainonly words\r\n\
    --b\r\nContent-Type: text/html\r\n\r\n<p>htmlonly words</p>\r\n--b--\r\n";

#[test]
fn plain_and_html_bodies_are_both_searched() {
    let (_dir, mut store, inbox) = store();
    let mixed = put(&mut store, inbox, 1, MIXED.as_bytes());
    let either = put(&mut store, inbox, 2, ALTERNATIVE.as_bytes());
    let all =
        ["meeting", "first", "htmlonly", "plainonly"].map(|word| found(&store, word, Field::Body));
    assert_eq!(all, [vec![mixed], vec![mixed], vec![either], vec![either]]);
}
