use super::*;

mod registry;
mod revert;

fn write(path: &Path, body: &str) {
    std::fs::write(path, body).expect("the fixture file writes");
}

fn saved(session: &mut Session, target: &Path) {
    let ticket = session.begin_save().expect("the save starts");
    write(target, "saved");
    let written = Fingerprint::of(target).expect("the written fingerprint");
    session
        .finish_save(ticket, target.to_path_buf(), written)
        .expect("the save lands");
}

#[test]
fn a_new_file_has_no_path_until_saved() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut session = Session::new_file();
    assert_eq!(session.path(), None);
    session.edit();
    assert_eq!(session.state(), SaveState::Dirty);
    let target = dir.path().join("草稿 01.md");
    saved(&mut session, &target);
    assert_eq!(session.path(), Some(target.as_path()));
    assert_eq!(session.state(), SaveState::Saved);
}

#[test]
fn an_old_ticket_never_marks_newer_edits_saved() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("r.txt");
    write(&target, "0");
    let mut session = Session::load(target.clone()).expect("the file loads");
    session.edit();
    let stale = session.begin_save().expect("the first save starts");
    session.edit();
    write(&target, "1");
    let written = Fingerprint::of(&target).expect("the written fingerprint");
    session
        .finish_save(stale, target.clone(), written)
        .expect("the stale save lands");
    assert_eq!(session.saved_revision(), 1);
    assert_eq!(session.revision(), 2);
    assert_eq!(session.state(), SaveState::Dirty);
    saved(&mut session, &target);
    assert_eq!(session.state(), SaveState::Saved);
}

#[test]
fn a_reload_kills_old_tickets() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("k.txt");
    write(&target, "0");
    let mut session = Session::load(target.clone()).expect("the file loads");
    session.edit();
    let stale = session.begin_save().expect("the save starts");
    session.reload().expect("the file reloads");
    session.edit();
    let written = Fingerprint::of(&target).expect("the fingerprint");
    assert_eq!(
        session.finish_save(stale, target, written),
        Err(StaleTicket)
    );
    assert_eq!(session.saved_revision(), 0);
    assert_eq!(session.state(), SaveState::Dirty);
}

#[test]
fn a_read_only_view_stays_read_only_through_reload() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("v.txt");
    write(&target, "0");
    let mut session = Session::read_only(target.clone()).expect("the view loads");
    write(&target, "1");
    session.external_modified();
    assert_eq!(session.state(), SaveState::ReadOnly);
    session.reload().expect("the view reloads");
    assert_eq!(session.state(), SaveState::ReadOnly);
    assert!(matches!(session.begin_save(), Err(SaveState::ReadOnly)));
}

#[test]
fn a_save_completion_keeps_a_newer_conflict() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("c.txt");
    write(&target, "0");
    let mut session = Session::load(target.clone()).expect("the file loads");
    session.edit();
    let ticket = session.begin_save().expect("the save starts");
    write(&target, "ours");
    let written = Fingerprint::of(&target).expect("our written fingerprint");
    write(&target, "external content, longer");
    session.external_modified();
    session
        .finish_save(ticket, target.clone(), written)
        .expect("our save lands");
    assert_eq!(session.state(), SaveState::ExternalModified);
    assert_eq!(session.saved_revision(), 1);
    assert_eq!(session.fingerprint(), Some(written));
    assert_ne!(
        session.fingerprint(),
        Fingerprint::of(&target).ok(),
        "the foreign fingerprint is never adopted"
    );
    session.reload().expect("the file reloads");
    assert_eq!(session.state(), SaveState::Saved);
}

#[test]
fn a_conflict_refuses_a_plain_save() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("m.txt");
    write(&target, "0");
    let mut session = Session::load(target.clone()).expect("the file loads");
    write(&target, "1");
    session.external_modified();
    assert!(matches!(
        session.begin_save(),
        Err(SaveState::ExternalModified)
    ));
    let ticket = session
        .begin_save_overwrite()
        .expect("the overwrite starts");
    write(&target, "kept local");
    let written = Fingerprint::of(&target).expect("the fingerprint");
    session
        .finish_save(ticket, target, written)
        .expect("the overwrite lands");
    assert_eq!(session.state(), SaveState::Saved);
}

#[test]
fn an_overwrite_keeps_the_access_mode_guards() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("g.txt");
    write(&target, "0");
    let mut view = Session::read_only(target.clone()).expect("the view loads");
    assert!(matches!(
        view.begin_save_overwrite(),
        Err(SaveState::ReadOnly)
    ));
    let mut loading = Session::loading(target);
    assert!(matches!(
        loading.begin_save_overwrite(),
        Err(SaveState::Loading)
    ));
}

#[test]
fn a_conflict_survives_edits_until_resolved() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("e.txt");
    write(&target, "0");
    let mut session = Session::load(target.clone()).expect("the file loads");
    write(&target, "1");
    session.external_modified();
    session.edit();
    assert_eq!(session.state(), SaveState::ExternalModified);
    assert!(matches!(
        session.begin_save(),
        Err(SaveState::ExternalModified)
    ));
    let ticket = session.begin_save_overwrite().expect("keep-local starts");
    write(&target, "kept");
    let written = Fingerprint::of(&target).expect("the fingerprint");
    session
        .finish_save(ticket, target, written)
        .expect("the overwrite lands");
    assert_eq!(session.state(), SaveState::Saved);
}

#[test]
fn a_conflict_survives_a_failed_save() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("s.txt");
    write(&target, "0");
    let mut session = Session::load(target.clone()).expect("the file loads");
    session.edit();
    let ticket = session.begin_save().expect("the save starts");
    write(&target, "external");
    session.external_modified();
    session.fail_save(ticket).expect("the failure lands");
    assert_eq!(session.state(), SaveState::SaveFailed);
    assert!(matches!(
        session.begin_save(),
        Err(SaveState::ExternalModified)
    ));
    session.reload().expect("the file reloads");
    assert_eq!(session.state(), SaveState::Saved);
}

#[test]
fn a_delayed_load_keeps_a_newer_conflict() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("d.txt");
    write(&target, "fourteen bytes");
    let loaded = Fingerprint::of(&target).expect("the loaded fingerprint");
    let mut session = Session::loading(target.clone());
    write(&target, "external content, much longer than before");
    session.external_modified();
    session.finish_load(loaded).expect("the load lands");
    assert_eq!(session.state(), SaveState::ExternalModified);
    assert_eq!(session.fingerprint(), Some(loaded));
}

#[test]
fn a_failed_save_keeps_the_unsaved_content() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("f.txt");
    write(&target, "0");
    let mut session = Session::load(target).expect("the file loads");
    session.edit();
    session.edit();
    let ticket = session.begin_save().expect("the save starts");
    session.fail_save(ticket).expect("the failure lands");
    assert_eq!(session.state(), SaveState::SaveFailed);
    assert_eq!(session.revision(), 2);
    let retry = session.begin_save();
    assert!(retry.is_ok());
}

#[test]
fn loading_transitions_cover_success_and_failure() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("big.docx");
    write(&target, "0");
    let mut session = Session::loading(target.clone());
    assert_eq!(session.state(), SaveState::Loading);
    assert!(matches!(session.begin_save(), Err(SaveState::Loading)));
    let loaded = Fingerprint::of(&target).expect("the loaded fingerprint");
    session.finish_load(loaded).expect("the load lands");
    assert_eq!(session.state(), SaveState::Saved);
    assert!(session.fingerprint().is_some());
    let mut failed = Session::loading(target);
    failed.fail_load().expect("the failure lands");
    assert_eq!(failed.state(), SaveState::LoadFailed);
    assert!(matches!(failed.begin_save(), Err(SaveState::LoadFailed)));
}

#[test]
fn a_running_task_refuses_another() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("t.txt");
    write(&target, "0");
    let mut loading = Session::loading(target.clone());
    write(&target, "external");
    loading.external_modified();
    assert!(matches!(
        loading.begin_save_overwrite(),
        Err(SaveState::Loading)
    ));

    let mut session = Session::load(target.clone()).expect("the file loads");
    session.edit();
    let first = session.begin_save().expect("the first save starts");
    write(&target, "external again");
    session.external_modified();
    session.edit();
    assert!(matches!(
        session.begin_save_overwrite(),
        Err(SaveState::Saving)
    ));
    write(&target, "first");
    let written = Fingerprint::of(&target).expect("the fingerprint");
    session
        .finish_save(first, target.clone(), written)
        .expect("the first save lands");
    assert_eq!(session.state(), SaveState::ExternalModified);
    let second = session.begin_save_overwrite().expect("keep-local starts");
    write(&target, "second");
    let written = Fingerprint::of(&target).expect("the fingerprint");
    session
        .finish_save(second, target, written)
        .expect("the second save lands");
    assert_eq!(session.state(), SaveState::Saved);
    assert_eq!(session.saved_revision(), session.revision());
}

#[test]
fn a_stale_completion_releases_the_save_slot() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("slot.txt");
    write(&target, "0");
    let mut session = Session::load(target.clone()).expect("the file loads");
    session.edit();
    let stale = session.begin_save().expect("the save starts");
    write(&target, "external");
    session.external_modified();
    session.reload().expect("the file reloads");
    let written = Fingerprint::of(&target).expect("the fingerprint");
    assert_eq!(
        session.finish_save(stale, target.clone(), written),
        Err(StaleTicket)
    );
    session.edit();
    let fresh = session.begin_save().expect("the slot released");
    write(&target, "fresh");
    let written = Fingerprint::of(&target).expect("the fingerprint");
    session
        .finish_save(fresh, target.clone(), written)
        .expect("the fresh save lands");
    assert_eq!(session.state(), SaveState::Saved);
    let written = Fingerprint::of(&target).expect("the fingerprint");
    assert_eq!(
        session.finish_save(fresh, target, written),
        Err(StaleTicket),
        "a second completion of one ticket is stale"
    );
}

#[test]
fn a_failed_load_never_saves_after_external_events() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("fl.txt");
    write(&target, "0");
    let mut session = Session::loading(target.clone());
    session.fail_load().expect("the failure lands");
    write(&target, "external");
    session.external_modified();
    assert!(matches!(
        session.begin_save_overwrite(),
        Err(SaveState::LoadFailed)
    ));
    assert!(matches!(session.begin_save(), Err(SaveState::LoadFailed)));
}

#[test]
fn a_duplicate_stale_completion_keeps_the_running_save() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("dup.txt");
    write(&target, "0");
    let mut session = Session::load(target.clone()).expect("the file loads");
    session.edit();
    let stale = session.begin_save().expect("the old save starts");
    session.reload().expect("the file reloads");
    let written = Fingerprint::of(&target).expect("the fingerprint");
    assert_eq!(
        session.finish_save(stale, target.clone(), written),
        Err(StaleTicket)
    );
    session.edit();
    let fresh = session.begin_save().expect("the new save starts");
    let written = Fingerprint::of(&target).expect("the fingerprint");
    assert_eq!(
        session.finish_save(stale, target.clone(), written),
        Err(StaleTicket),
        "a duplicate stale completion never frees the running save"
    );
    write(&target, "fresh");
    let written = Fingerprint::of(&target).expect("the fingerprint");
    session
        .finish_save(fresh, target, written)
        .expect("the running save lands");
    assert_eq!(session.state(), SaveState::Saved);
}

#[test]
fn a_failed_load_then_reload_recovers_saving() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("rec.txt");
    write(&target, "0");
    let mut session = Session::loading(target.clone());
    session.fail_load().expect("the failure lands");
    assert!(matches!(session.begin_save(), Err(SaveState::LoadFailed)));
    session.reload().expect("the file reloads");
    assert_eq!(session.state(), SaveState::Saved);
    session.edit();
    let ticket = session.begin_save().expect("the save starts after reload");
    write(&target, "edited");
    let written = Fingerprint::of(&target).expect("the fingerprint");
    session
        .finish_save(ticket, target, written)
        .expect("the save lands");
    assert_eq!(session.state(), SaveState::Saved);
}

#[test]
fn save_as_tasks_have_unique_identities() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let first = dir.path().join("first.md");
    let second = dir.path().join("second.md");
    write(&first, "0");
    let mut session = Session::load(first.clone()).expect("the file loads");
    session.edit();
    let one = session.begin_save().expect("the first save starts");
    write(&first, "1");
    let written = Fingerprint::of(&first).expect("the fingerprint");
    session
        .finish_save(one, first.clone(), written)
        .expect("the first save lands");
    let two = session.begin_save().expect("the save-as starts");
    let written = Fingerprint::of(&first).expect("the fingerprint");
    assert_eq!(
        session.finish_save(one, first, written),
        Err(StaleTicket),
        "a repeated old callback never frees the save-as task"
    );
    write(&second, "1");
    let written = Fingerprint::of(&second).expect("the fingerprint");
    session
        .finish_save(two, second.clone(), written)
        .expect("the save-as lands");
    assert_eq!(session.path(), Some(second.as_path()));
    assert_eq!(session.state(), SaveState::Saved);
}

#[test]
fn a_duplicate_failure_keeps_the_running_save() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("df.txt");
    write(&target, "0");
    let mut session = Session::load(target.clone()).expect("the file loads");
    session.edit();
    let old = session.begin_save().expect("task A starts");
    session.fail_save(old).expect("task A fails");
    session.edit();
    let running = session.begin_save().expect("task B starts");
    assert_eq!(
        session.fail_save(old),
        Err(StaleTicket),
        "a repeated old failure never frees task B"
    );
    assert_eq!(session.state(), SaveState::Saving);
    write(&target, "done");
    let written = Fingerprint::of(&target).expect("the fingerprint");
    session
        .finish_save(running, target, written)
        .expect("task B lands");
    assert_eq!(session.state(), SaveState::Saved);
}

#[test]
fn a_stale_failure_keeps_the_reloaded_state() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("sf.txt");
    write(&target, "0");
    let mut session = Session::load(target.clone()).expect("the file loads");
    session.edit();
    let stale = session.begin_save().expect("the save starts");
    write(&target, "external");
    session.external_modified();
    session.reload().expect("the file reloads");
    assert_eq!(session.fail_save(stale), Err(StaleTicket));
    assert_eq!(session.state(), SaveState::Saved);
    session.edit();
    let fresh = session.begin_save().expect("the slot released");
    write(&target, "fresh");
    let written = Fingerprint::of(&target).expect("the fingerprint");
    session
        .finish_save(fresh, target, written)
        .expect("the fresh save lands");
    assert_eq!(session.state(), SaveState::Saved);
}

#[test]
fn a_stale_load_callback_keeps_the_reloaded_state() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let target = dir.path().join("sl.txt");
    write(&target, "0");
    let old = Fingerprint::of(&target).expect("the old fingerprint");
    let mut session = Session::loading(target.clone());
    write(&target, "external, longer");
    session.external_modified();
    session.reload().expect("the file reloads");
    session.edit();
    assert_eq!(session.finish_load(old), Err(StaleTicket));
    assert_eq!(session.state(), SaveState::Dirty);
    assert_ne!(session.fingerprint(), Some(old));
    assert_eq!(session.fail_load(), Err(StaleTicket));
    assert_eq!(session.state(), SaveState::Dirty);
    let ticket = session.begin_save().expect("the save starts");
    write(&target, "current");
    let written = Fingerprint::of(&target).expect("the fingerprint");
    session
        .finish_save(ticket, target, written)
        .expect("the save lands");
    assert_eq!(session.state(), SaveState::Saved);
}

mod failed;

#[test]
fn a_replace_with_the_same_time_and_size_is_a_new_fingerprint() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (path, next) = (dir.path().join("a.md"), dir.path().join("a.md.new"));
    write(&path, "old text\n");
    let before = Fingerprint::of(&path).expect("the first fingerprint");
    write(&next, "new text\n");
    let file = std::fs::File::options().write(true).open(&next).expect("the new file");
    file.set_modified(before.modified).expect("the old time");
    drop(file);
    std::fs::rename(&next, &path).expect("the replace");
    let after = Fingerprint::of(&path).expect("the second fingerprint");
    assert_eq!((after.len, after.modified), (before.len, before.modified));
    assert_ne!(after, before, "the identity tells them apart");
}
