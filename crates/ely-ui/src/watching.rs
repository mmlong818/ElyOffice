use notify::EventKind;
use notify::event::{AccessKind, AccessMode};

/// A read; Linux reports those, macOS does not.
pub fn only_read(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Access(access) if *access != AccessKind::Close(AccessMode::Write))
}

#[cfg(test)]
mod tests {
    use notify::EventKind;
    use notify::event::{AccessKind, AccessMode, DataChange, ModifyKind};

    use super::only_read;

    #[test]
    fn opens_and_reads_are_reads_and_a_written_close_is_a_change() {
        for read in [
            AccessKind::Open(AccessMode::Any),
            AccessKind::Read,
            AccessKind::Close(AccessMode::Read),
            AccessKind::Any,
        ] {
            assert!(only_read(&EventKind::Access(read)), "{read:?}");
        }
        assert!(!only_read(&EventKind::Access(AccessKind::Close(AccessMode::Write))));
        assert!(!only_read(&EventKind::Modify(ModifyKind::Data(DataChange::Any))));
    }
}
