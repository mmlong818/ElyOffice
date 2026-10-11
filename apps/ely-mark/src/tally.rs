//! Which page reports count, by page load and by version.

use std::collections::HashMap;

/// One page load's change counts and the renders it owes.
pub struct Tally<R> {
    load: u64,
    /// The newest change the page announced.
    changed: u64,
    /// The change whose text arrived.
    reported: u64,
    renders: HashMap<u64, R>,
    asked: u64,
    /// Why the page is down, until it loads again.
    down: Option<String>,
}

impl<R> Tally<R> {
    pub fn new() -> Self {
        Tally { load: 0, changed: 0, reported: 0, renders: HashMap::new(), asked: 0, down: None }
    }

    pub fn load(&self) -> u64 {
        self.load
    }

    pub fn changed(&self) -> u64 {
        self.changed
    }

    /// Every change's text has arrived.
    pub fn settled(&self) -> bool {
        self.reported == self.changed
    }

    /// A reload or refresh: older reports lapse, owed renders end.
    pub fn restart(&mut self) -> Vec<R> {
        self.load += 1;
        (self.changed, self.reported) = (0, 0);
        self.lapse()
    }

    /// A page loaded: it answers again.
    pub fn loaded(&mut self) -> Vec<R> {
        self.down = None;
        self.restart()
    }

    /// The page failed: owed and later renders end with `why`.
    pub fn fail(&mut self, why: String) -> Vec<R> {
        self.down = Some(why);
        self.lapse()
    }

    fn lapse(&mut self) -> Vec<R> {
        self.renders.drain().map(|(_, render)| render).collect()
    }

    /// Whether a report from page load `generation` still counts.
    pub fn current(&self, generation: Option<u64>) -> bool {
        generation.is_none_or(|load| load == self.load)
    }

    pub fn change(&mut self, version: u64) {
        self.changed = self.changed.max(version);
    }

    /// Takes text `version` if it is the newest.
    pub fn text(&mut self, version: u64) -> bool {
        let newest = version >= self.reported;
        if newest {
            self.reported = version;
        }
        newest
    }

    /// Owes `render`; its number goes to the page.
    pub fn ask(&mut self, render: R) -> Result<u64, (R, String)> {
        if let Some(why) = &self.down {
            return Err((render, why.clone()));
        }
        self.asked += 1;
        self.renders.insert(self.asked, render);
        Ok(self.asked)
    }

    pub fn answered(&mut self, ask: u64) -> Option<R> {
        self.renders.remove(&ask)
    }
}

#[cfg(test)]
mod tests {
    use super::Tally;

    #[test]
    fn a_key_from_before_a_refresh_leaves_it_settled() {
        let mut tally = Tally::<()>::new();
        let old = tally.load();
        // The old page counted two versions.
        tally.change(2);
        assert!(tally.text(2));
        tally.restart();
        // The renderer's change for the old text lands late.
        if tally.current(Some(old)) {
            tally.change(3);
        }
        assert!(tally.settled(), "an old change does not count");
        assert_eq!(tally.changed(), 0, "versions start over");
        // The refreshed page's own edits count again.
        tally.change(1);
        assert!(!tally.settled());
        assert!(tally.text(1), "the new page's first text counts");
        assert!(tally.settled());
    }

    #[test]
    fn a_load_or_failure_ends_every_owed_render() {
        let mut tally = Tally::new();
        let (html, text) = (tally.ask("html").expect("up"), tally.ask("text").expect("up"));
        let mut owed = tally.loaded();
        owed.sort();
        assert_eq!(owed, ["html", "text"]);
        // Late answers find nobody waiting.
        assert_eq!((tally.answered(html), tally.answered(text)), (None, None));
        let pdf = tally.ask("pdf").expect("up");
        assert_eq!(tally.fail("denied".into()), ["pdf"]);
        assert_eq!(tally.answered(pdf), None);
    }

    #[test]
    fn a_failed_page_refuses_renders_until_it_loads() {
        let mut tally = Tally::new();
        tally.fail("denied".into());
        assert_eq!(tally.ask("html"), Err(("html", "denied".to_string())));
        // A refresh is no load: the page is still down.
        assert!(tally.restart().is_empty());
        assert_eq!(tally.ask("text"), Err(("text", "denied".to_string())));
        assert!(tally.loaded().is_empty());
        assert!(tally.ask("pdf").is_ok());
    }

    #[test]
    fn only_the_newest_text_counts() {
        let mut tally = Tally::<()>::new();
        tally.change(2);
        assert!(tally.text(2));
        assert!(!tally.text(1), "an older text is dropped");
        assert!(tally.settled());
    }
}
