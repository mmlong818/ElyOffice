//! Web pictures, trusted senders and header blocks, per letter.

use std::{rc::Rc, thread};

use ely_gpui_component::documents::HeldPicture;
use ely_mail_engine::{MessageId, fetch};
use gpui::{AppContext as _, Context, SharedString};

use crate::{
    letter::{self, Words},
    mail::Mail,
    reading::{Each, Open, Shown, words},
};

/// Web pictures come down this many at once.
const FETCHERS: usize = 4;

impl Mail {
    /// The shown letters, oldest first.
    pub fn letters_mut(&mut self) -> &mut [Each] {
        match &mut self.reading.shown {
            Some(Shown {
                thread: Some(Ok(thread)),
                ..
            }) => &mut thread.letters,
            _ => &mut [],
        }
    }

    fn open_mut(&mut self, id: MessageId) -> Option<&mut Open> {
        let each = self
            .letters_mut()
            .iter_mut()
            .find(|each| each.stored.id == id);
        each.and_then(|each| each.open.as_mut().ok())
    }

    /// Fetches a letter's web pictures, once.
    pub fn load_pictures(&mut self, id: MessageId, cx: &mut Context<Self>) {
        let read = self.reading.read;
        let Some(open) = self.open_mut(id) else {
            return;
        };
        let Some(urls) = open.body().map(|body| body.remote.clone()) else {
            return;
        };
        if open.fetching || urls.is_empty() {
            return;
        }
        open.fetching = true;
        log::info!(
            "mail: message {}: fetching {} web pictures",
            id.0,
            urls.len()
        );
        let words = words(cx);
        let fetched = cx.background_spawn(async move { fetch_all(urls, &words) });
        cx.spawn(async move |this, cx| {
            let fetched = fetched.await;
            this.update(cx, |mail, cx| mail.pictures_fetched(read, id, fetched, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }

    fn pictures_fetched(
        &mut self,
        read: u64,
        id: MessageId,
        fetched: Fetched,
        cx: &mut Context<Self>,
    ) {
        if self.reading.read != read {
            return log::info!("mail: pictures for read {read} came after a newer read");
        }
        let Some(open) = self.open_mut(id).filter(|open| open.fetching) else {
            return log::info!(
                "mail: message {} was read again; its pictures dropped",
                id.0
            );
        };
        let failed = fetched.iter().filter(|(_, got)| got.is_err()).count();
        log::info!(
            "mail: message {}: {} pictures came, {failed} did not",
            id.0,
            fetched.len() - failed
        );
        let mut pictures = (*open.pictures).clone();
        pictures.extend(fetched);
        open.pictures = Rc::new(pictures);
        cx.notify();
    }

    /// Always loads, or no longer loads, the letter's sender's pictures.
    pub fn trust_sender(&mut self, id: MessageId, trusted: bool, cx: &mut Context<Self>) {
        let each = self.letters_mut().iter().find(|each| each.stored.id == id);
        let Some(sender) = each.and_then(Each::sender).map(str::to_owned) else {
            return;
        };
        let from = sender.clone();
        self.reading.trusting += 1;
        self.with_store(
            cx,
            move |store| store.set_trusted(&from, trusted),
            move |mail, done, cx| {
                mail.reading.trusting -= 1;
                match done {
                    Ok(()) => mail.trusted(&sender, trusted, cx),
                    Err(error) => mail.failed("mail.error.trust", &error, cx),
                }
                // Builds planned before the write may hold old trust.
                mail.read_letter(cx);
            },
        );
    }

    fn trusted(&mut self, sender: &str, trusted: bool, cx: &mut Context<Self>) {
        let mut same = Vec::new();
        for each in self.letters_mut() {
            let theirs = each
                .sender()
                .is_some_and(|it| it.eq_ignore_ascii_case(sender));
            if let (true, Ok(open)) = (theirs, &mut each.open) {
                open.trusted = trusted;
                same.push(each.stored.id);
            }
        }
        if trusted {
            for id in same {
                self.load_pictures(id, cx);
            }
        }
    }

    pub fn toggle_headers(&mut self, id: MessageId, cx: &mut Context<Self>) {
        if let Some(each) = self
            .letters_mut()
            .iter_mut()
            .find(|each| each.stored.id == id)
        {
            each.headers = !each.headers;
            cx.notify();
        }
    }
}

type Fetched = Vec<(String, Result<HeldPicture, SharedString>)>;

/// Blocking fetches, a few at once; never logs addresses.
fn fetch_all(urls: Vec<String>, words: &Words) -> Fetched {
    let chunk = urls.len().div_ceil(FETCHERS);
    thread::scope(|scope| {
        let workers: Vec<_> = urls
            .chunks(chunk)
            .map(|urls| {
                scope.spawn(move || {
                    let got = urls.iter().map(|url| {
                        let picture = fetch::picture(url)
                            .map_err(|err| SharedString::from(err.to_string()))
                            .and_then(|(kind, bytes)| letter::held(&kind, bytes, words));
                        (url.clone(), picture)
                    });
                    got.collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("a picture fetch panicked"))
            .collect()
    })
}
