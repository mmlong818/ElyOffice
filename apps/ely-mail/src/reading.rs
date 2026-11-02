//! The conversation open in the reading pane (C12.10, C12.11).

use std::{collections::HashMap, rc::Rc};

use ely_gpui_component::documents::{MarkdownBlock, Pictures};
use ely_mail_engine::{MessageId, Stored};
use gpui::{AppContext as _, Context};

use crate::{
    letter::{self, Body, Kept, Letter, Words},
    mail::{Mail, text},
};

/// A thread shows at most this many, the newest.
pub const SHOWN: usize = 50;

/// The pane's conversation; a newer read drops an older one.
#[derive(Default)]
pub struct Reading {
    pub read: u64,
    /// Store reads asked, checks of the shown one too.
    pub asked: u64,
    /// Trust writes not yet landed; builds wait them out.
    pub trusting: usize,
    pub shown: Option<Shown>,
}

impl Reading {
    /// The shown message's files, once its body is read.
    pub fn files(&self, id: MessageId) -> Option<&[(String, Option<u64>)]> {
        let Some(Shown { thread: Some(Ok(thread)), .. }) = &self.shown else {
            return None;
        };
        let each = thread.letters.iter().find(|each| each.stored.id == id)?;
        each.open.as_ref().ok()?.body().map(|body| body.files.as_slice())
    }
}

pub struct Shown {
    /// The message picked in the list.
    pub id: MessageId,
    /// None while it is read.
    pub thread: Option<Result<Thread, String>>,
}

pub struct Thread {
    /// Oldest first; the picked one among them.
    pub letters: Vec<Each>,
    /// Older messages left out.
    pub older: usize,
}

pub struct Each {
    pub stored: Stored,
    pub open: Result<Open, String>,
    /// The original headers are drawn.
    pub headers: bool,
}

impl Each {
    pub fn sender(&self) -> Option<&str> {
        self.stored.from.first().map(|from| from.email.as_str())
    }
}

/// What a message keeps; a change reads it again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    body: bool,
    missing: Vec<String>,
}

impl Version {
    pub fn of(stored: &Stored) -> Self {
        Self {
            body: stored.body,
            missing: stored.missing.clone(),
        }
    }
}

/// A letter as drawn, with what it may load.
pub struct Open {
    pub letter: Letter,
    pub blocks: Rc<[MarkdownBlock]>,
    pub pictures: Rc<Pictures>,
    /// The sender's pictures always load.
    pub trusted: bool,
    /// Web pictures were asked for.
    pub fetching: bool,
}

impl Open {
    pub fn body(&self) -> Option<&Body> {
        self.letter.body.as_ref()
    }
}

/// The members read, and how many older left out.
type Listed = (Vec<(MessageId, Version)>, usize);

/// A member kept as drawn, or read anew.
enum Member {
    Kept(MessageId),
    New(Box<(Stored, Result<Kept, String>, bool)>),
}

/// A letter and its blocks, or why not.
type Laid = Result<(Letter, Vec<MarkdownBlock>), String>;

enum Built {
    Kept(MessageId),
    New(Box<(Stored, Laid, bool)>),
}

pub fn words(cx: &gpui::App) -> Words {
    Words {
        broken: text(cx, "mail.picture.broken", &[]),
        waiting: text(cx, "mail.picture.waiting", &[]),
        not_picture: text(cx, "mail.picture.not_picture", &[]),
        svg: text(cx, "mail.picture.svg", &[]),
        unnamed: text(cx, "mail.file.unnamed", &[]),
    }
}

impl Mail {
    /// Reads the picked message's thread, or what changed in it.
    pub fn read_letter(&mut self, cx: &mut Context<Self>) {
        if self.open == crate::boxes::Open::Drafts {
            self.reading.shown = None;
            return;
        }
        let picked = self.listing.chosen.as_ref().map(|row| row.key.clone());
        let id = match picked.map(|key| key.parse::<i64>()) {
            Some(Ok(id)) => MessageId(id),
            Some(Err(_)) => return log::error!("mail: a listed key is no message id"),
            None => {
                self.reading.shown = None;
                return;
            }
        };
        let known: Option<Listed> = match &self.reading.shown {
            Some(Shown {
                id: shown,
                thread: Some(Ok(thread)),
            }) if *shown == id => Some(listed(thread)),
            _ => None,
        };
        if known.is_none() {
            self.reading.read += 1;
            self.reading.shown = Some(Shown { id, thread: None });
        }
        self.reading.asked += 1;
        let (read, asked) = (self.reading.read, self.reading.asked);
        let words = words(cx);
        self.with_store(
            cx,
            move |store| {
                let all = store.thread(id)?;
                let total = all.len();
                let newest = total.saturating_sub(SHOWN);
                let members: Vec<Stored> = (all.into_iter().enumerate())
                    .filter(|(ix, each)| *ix >= newest || each.id == id)
                    .map(|(_, each)| each)
                    .collect();
                let now: Listed = (
                    members.iter().map(|m| (m.id, Version::of(m))).collect(),
                    total - members.len(),
                );
                if known.as_ref() == Some(&now) {
                    return Ok(None);
                }
                let was: HashMap<_, _> = known.map(|known| known.0).into_iter().flatten().collect();
                let mut plan = Vec::new();
                for (member, (_, version)) in members.into_iter().zip(&now.0) {
                    if was.get(&member.id) == Some(version) {
                        plan.push(Member::Kept(member.id));
                        continue;
                    }
                    let kept = match store.original(member.id) {
                        Ok(Some(raw)) => Ok(Kept::Original(raw)),
                        Ok(None) => store.head(member.id).map(Kept::Head),
                        Err(err) => Err(err),
                    };
                    let trusted = match member.from.first() {
                        Some(from) => store.trusts(&from.email)?,
                        None => false,
                    };
                    let kept = kept.map_err(|err| err.to_string());
                    plan.push(Member::New(Box::new((member, kept, trusted))));
                }
                Ok(Some((plan, now.1)))
            },
            move |mail, got, cx| {
                if (mail.reading.read, mail.reading.asked) != (read, asked) {
                    return log::info!("mail: read {read} ended after a newer one");
                }
                let (plan, older) = match got {
                    Ok(Some(got)) => got,
                    Ok(None) => return,
                    Err(error) => return mail.thread_failed(read, error),
                };
                let built = cx.background_spawn(async move { build(plan, &words) });
                cx.spawn(async move |this, cx| {
                    let built = built.await;
                    this.update(cx, |mail, cx| {
                        mail.thread_built(read, asked, built, older, cx)
                    })
                    .ok();
                })
                .detach();
            },
        );
    }

    fn thread_failed(&mut self, read: u64, error: String) {
        log::warn!("mail: read {read} failed: {error}");
        if let Some(shown) = &mut self.reading.shown {
            shown.thread = Some(Err(error));
        }
    }

    /// Kept letters stay as drawn, pictures and all.
    fn thread_built(
        &mut self,
        read: u64,
        asked: u64,
        built: Vec<Built>,
        older: usize,
        cx: &mut Context<Self>,
    ) {
        if (self.reading.read, self.reading.asked) != (read, asked) {
            return log::info!("mail: thread {read} built after a newer read");
        }
        if self.reading.trusting > 0 {
            // Its trust may be stale; the write reads again.
            return log::info!("mail: thread {read} built during a trust change");
        }
        let Some(shown) = &mut self.reading.shown else {
            return;
        };
        let mut was: HashMap<MessageId, Each> = match shown.thread.take() {
            Some(Ok(thread)) => (thread.letters.into_iter())
                .map(|each| (each.stored.id, each))
                .collect(),
            _ => HashMap::new(),
        };
        let mut trusted = Vec::new();
        let letters: Vec<Each> = (built.into_iter())
            .map(|member| match member {
                // No other read landed since this one began.
                Built::Kept(id) => was.remove(&id).expect("a kept letter is still shown"),
                Built::New(new) => {
                    let (stored, letter, trust) = *new;
                    if trust && letter.is_ok() {
                        trusted.push(stored.id);
                    }
                    each(stored, letter, trust)
                }
            })
            .collect();
        log::info!(
            "mail: read {read}: {} letters in the thread, {older} older left out",
            letters.len()
        );
        shown.thread = Some(Ok(Thread { letters, older }));
        for id in trusted {
            self.load_pictures(id, cx);
        }
        cx.notify();
    }
}

/// Failed letters are left out, so they read again.
fn listed(thread: &Thread) -> Listed {
    let members = thread.letters.iter().filter(|each| each.open.is_ok());
    let members = members.map(|each| (each.stored.id, Version::of(&each.stored)));
    (members.collect(), thread.older)
}

/// Off the UI thread: new letters parsed and laid out.
fn build(plan: Vec<Member>, words: &Words) -> Vec<Built> {
    let built = plan.into_iter().map(|member| match member {
        Member::Kept(id) => Built::Kept(id),
        Member::New(new) => {
            let (stored, kept, trusted) = *new;
            let letter = kept.and_then(|kept| letter::letter(&stored, kept, words));
            let letter = letter.map(|mut letter| {
                let blocks = letter
                    .body
                    .as_mut()
                    .map(|body| std::mem::take(&mut body.blocks));
                (letter, blocks.unwrap_or_default())
            });
            Built::New(Box::new((stored, letter, trusted)))
        }
    });
    built.collect()
}

fn each(stored: Stored, letter: Laid, trusted: bool) -> Each {
    let open = letter.map(|(mut letter, blocks)| {
        let pictures = (letter.body.as_mut())
            .map(|body| std::mem::take(&mut body.pictures))
            .unwrap_or_default();
        if let Some(body) = &letter.body {
            log::info!(
                "mail: message {}: {} blocks, {} web pictures, {} trackers kept back",
                stored.id.0,
                blocks.len(),
                body.remote.len(),
                body.trackers
            );
        }
        Open {
            letter,
            blocks: blocks.into(),
            pictures: Rc::new(pictures),
            trusted,
            fetching: false,
        }
    });
    if let Err(error) = &open {
        log::warn!("mail: message {} could not be read: {error}", stored.id.0);
    }
    Each {
        stored,
        open,
        headers: false,
    }
}
