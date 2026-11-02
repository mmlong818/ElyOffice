#[derive(Default)]
pub(crate) struct Completion {
    pending: Option<(u64, Option<u64>)>,
    reported: u64,
}

impl Completion {
    pub(crate) fn pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn queued(&mut self, seq: u64) {
        self.pending = Some((seq, None));
    }

    pub(crate) fn reported(&mut self, revision: u64) {
        self.reported = revision;
        self.finish();
    }

    pub(crate) fn landed(&mut self, seq: u64, revision: Option<u64>) {
        if self.pending.is_some_and(|(waiting, _)| waiting == seq) {
            self.pending = revision.map(|revision| (seq, Some(revision)));
            self.finish();
        }
    }

    fn finish(&mut self) {
        // Inline callbacks and serialized output preserve completion revisions.
        if self
            .pending
            .is_some_and(|(_, revision)| revision.is_some_and(|revision| self.reported >= revision))
        {
            self.pending = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Completion;

    #[test]
    fn an_old_snapshot_cannot_reopen_an_accepted_menu() {
        let mut completion = Completion::default();
        completion.reported(10);
        completion.queued(7);
        completion.reported(10);
        assert!(completion.pending());
        completion.landed(7, Some(11));
        assert!(completion.pending());
        completion.reported(10);
        assert!(completion.pending());
        completion.reported(11);
        assert!(!completion.pending());
    }

    #[test]
    fn a_snapshot_before_the_reply_waits_for_the_reply() {
        let mut completion = Completion::default();
        completion.queued(7);
        completion.reported(11);
        assert!(completion.pending());
        completion.landed(7, Some(11));
        assert!(!completion.pending());
    }

    #[test]
    fn a_reply_before_the_snapshot_waits_for_the_snapshot() {
        let mut completion = Completion::default();
        completion.queued(7);
        completion.landed(7, Some(11));
        assert!(completion.pending());
        completion.reported(11);
        assert!(!completion.pending());
    }

    #[test]
    fn another_input_cannot_acknowledge_the_completion() {
        let mut completion = Completion::default();
        completion.queued(7);
        completion.reported(20);
        completion.landed(6, Some(20));
        assert!(completion.pending());
        completion.landed(8, None);
        assert!(completion.pending());
        completion.landed(7, Some(19));
        assert!(!completion.pending());
    }

    #[test]
    fn a_failed_completion_allows_retry() {
        let mut completion = Completion::default();
        completion.queued(7);
        completion.landed(7, None);
        assert!(!completion.pending());
        completion.queued(8);
        assert!(completion.pending());
    }

    #[test]
    fn the_next_completion_requires_its_own_snapshot() {
        let mut completion = Completion::default();
        completion.reported(11);
        completion.queued(7);
        completion.landed(7, Some(11));
        assert!(!completion.pending());
        completion.queued(8);
        completion.landed(8, Some(12));
        assert!(completion.pending());
        completion.reported(11);
        assert!(completion.pending());
        completion.reported(12);
        assert!(!completion.pending());
    }
}
