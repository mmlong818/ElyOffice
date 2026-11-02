//! PERF-06 (`--release --ignored`, ELY_MAILS); ELY_SEED seeds frame timing.


use std::time::Instant;

use ely_mail_engine::{Fetched, Field, Key, MailboxId, Role, Search, Store};

const LATIN: &[&str] = &[
    "the", "report", "meeting", "budget", "project", "review", "invoice", "travel", "plan",
    "update", "schedule", "design", "launch", "customer", "contract", "draft", "team", "office",
    "quarter", "release",
];
const HAN: &[&str] = &[
    "会议", "报告", "预算", "项目", "发票", "合同", "客户", "设计", "计划", "通知", "季度",
    "团队", "办公室", "出差", "审核",
];

/// A fixed sequence, the same on every run.
struct Seq(u64);

impl Seq {
    fn next(&mut self, below: usize) -> usize {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        ((self.0 >> 33) % below as u64) as usize
    }
}

fn body(seq: &mut Seq, n: usize) -> String {
    let mut out = String::new();
    for i in 0..200 {
        match seq.next(3) {
            0 => out.push_str(HAN[seq.next(HAN.len())]),
            _ => out.push_str(LATIN[seq.next(LATIN.len())]),
        }
        out.push(if i % 12 == 11 { '\n' } else { ' ' });
    }
    out.push_str(&format!("ticket{n}\n"));
    out
}

/// `count` messages, the same on every run.
fn fill(store: &mut Store, inbox: MailboxId, count: u32) {
    let mut seq = Seq(7);
    for uid in 1..=count {
        let sender = LATIN[seq.next(LATIN.len())];
        let bytes = format!(
            "From: {sender} <{sender}{}@example.com>\r\nTo: ann@example.com\r\n\
             Subject: {} {}\r\nDate: Mon, 19 Oct 2026 10:00:00 +0000\r\n\
             Content-Type: text/plain; charset=utf-8\r\n\r\n{}",
            uid % 500,
            LATIN[seq.next(LATIN.len())],
            HAN[seq.next(HAN.len())],
            body(&mut seq, uid as usize),
        );
        let key = Key::Imap {
            uidvalidity: 1,
            uid,
        };
        store.put(inbox, &key, &[], Fetched::Whole(bytes.as_bytes())).unwrap();
    }
}

#[test]
#[ignore]
fn a_hundred_thousand_messages_search_within_a_second() {
    let count: u32 = std::env::var("ELY_MAILS").map_or(100_000, |n| n.parse().unwrap());
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    let id = store.add_account(&fake::account("ann@example.com")).unwrap();
    let [inbox] = store
        .set_folders(id, &[("INBOX", Some(Role::Inbox))])
        .unwrap()
        .try_into()
        .unwrap();
    store.open_mailbox(inbox, 1).unwrap();
    let started = Instant::now();
    fill(&mut store, inbox, count);
    eprintln!("perf: {count} messages stored in {:?}", started.elapsed());
    let cases = [
        ("common Latin", "report", Field::All),
        ("common Han", "会议", Field::All),
        ("one Han", "会", Field::All),
        ("two words", "budget 预算", Field::All),
        ("rare", &format!("ticket{}", count / 2), Field::All),
        ("sender", "travel", Field::From),
        ("subject", "合同", Field::Subject),
    ];
    let mut worst = 0.0f64;
    for (name, words, field) in cases {
        let search = Search {
            words: words.into(),
            field,
            ..Search::default()
        };
        let mut times = Vec::new();
        let mut matched = 0;
        for _ in 0..5 {
            let at = Instant::now();
            matched = store.search(&search).unwrap().messages.len();
            times.push(at.elapsed().as_secs_f64());
        }
        times.sort_by(f64::total_cmp);
        eprintln!("perf: {name:12} {matched:6} found, median {:.3} s, max {:.3} s", times[2], times[4]);
        worst = worst.max(times[4]);
    }
    assert!(worst < 1.0, "a search took {worst:.3} s");
}

#[test]
#[ignore]
fn seeds_a_data_root_for_frame_timing() {
    let Ok(root) = std::env::var("ELY_SEED") else {
        return eprintln!("SKIP: no ELY_SEED");
    };
    let count: u32 = std::env::var("ELY_MAILS").map_or(20_000, |n| n.parse().unwrap());
    let mut store = Store::open(&std::path::Path::new(&root).join("ely-mail")).unwrap();
    // Nothing listens on its ports: syncs fail at once.
    let id = store.add_account(&fake::account("bench@example.com")).unwrap();
    let [inbox] = store
        .set_folders(id, &[("INBOX", Some(Role::Inbox))])
        .unwrap()
        .try_into()
        .unwrap();
    store.open_mailbox(inbox, 1).unwrap();
    fill(&mut store, inbox, count);
    eprintln!("perf: {count} messages seeded under {root}");
}
