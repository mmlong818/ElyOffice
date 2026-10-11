//! Branches, tags, operations under way, conflicts (C13.4).

use std::path::Path;

use super::{committed, git, sh};
use ely_gpui_component::git::{MARKER, RegionKind, conflicts};

use crate::{
    branches::{self, Kind},
    conflict::{self, Stages},
    ops::{self, Op},
    work::{self, Write},
};

/// A merge of `side` that conflicts on a.txt.
pub(crate) fn conflicted(dir: &Path) -> (std::path::PathBuf, String) {
    let root = committed(dir);
    for (key, value) in [
        ("user.name", "Ely"),
        ("user.email", "ely@example.com"),
        ("commit.gpgsign", "false"),
    ] {
        sh(&root, &["config", key, value]);
    }
    let main = sh(&root, &["branch", "--show-current"]).trim().to_string();
    sh(&root, &["checkout", "-q", "-b", "side"]);
    std::fs::write(root.join("a.txt"), "one\nside\n").expect("writes");
    sh(&root, &["commit", "-qam", "side change"]);
    sh(&root, &["checkout", "-q", &main]);
    std::fs::write(root.join("a.txt"), "one\nmain\n").expect("writes");
    sh(&root, &["commit", "-qam", "main change"]);
    (root, main)
}

#[test]
fn refs_read_branches_remotes_and_both_kinds_of_tag() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let source = committed(&dir.path().join("source"));
    let into = dir.path().join("copy");
    git()
        .clone_repo(source.to_str().expect("utf-8"), &into)
        .expect("starts")
        .follow(|_| {})
        .expect("clones");
    sh(&into, &["branch", "topic"]);
    sh(&into, &["tag", "light"]);
    sh(&into, &["tag", "-a", "heavy", "-m", "Release notes"]);
    std::fs::write(into.join("a.txt"), "ahead\n").expect("writes");
    sh(&into, &["commit", "-qam", "ahead"]);
    let refs = branches::read(&git(), &into).expect("reads");
    let find = |name: &str| {
        refs.iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{name} in {refs:?}"))
    };
    let main = sh(&into, &["branch", "--show-current"]).trim().to_string();
    assert!(find(&main).current && find(&main).kind == Kind::Local);
    assert_eq!(find(&main).track, Some((1, 0)), "one ahead of its upstream");
    assert_eq!(find(&format!("origin/{main}")).kind, Kind::Remote);
    assert!(
        !refs
            .iter()
            .any(|r| r.name == "origin/HEAD" || r.name == "origin"),
        "origin/HEAD is no branch"
    );
    assert_eq!(find("light").message, None);
    let heavy = find("heavy");
    assert_eq!(
        (heavy.message.as_deref(), heavy.subject.as_str()),
        (Some("Release notes"), "first")
    );
    assert_eq!(
        heavy.commit,
        find("light").commit,
        "an annotated tag names its commit"
    );
}

#[test]
fn operations_under_way_name_their_sides() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (root, main) = conflicted(dir.path());
    let doing = |root: &Path| ops::doing(&git(), root, &root.join(".git")).expect("reads");
    assert_eq!(doing(&root), None);
    assert!(git().run(&root, &["merge", "side"]).is_err());
    let merge = doing(&root).expect("a merge");
    assert_eq!(merge.op, Op::Merge);
    assert_eq!(merge.ours, main);
    assert!(merge.theirs.ends_with("side change"), "{}", merge.theirs);
    sh(&root, &["merge", "--abort"]);
    sh(&root, &["checkout", "-q", "side"]);
    assert!(git().run(&root, &["rebase", &main]).is_err());
    let rebase = doing(&root).expect("a rebase");
    assert_eq!(rebase.op, Op::Rebase);
    assert!(
        rebase.ours.ends_with("main change") && rebase.theirs.ends_with("side change"),
        "{rebase:?}"
    );
    sh(&root, &["rebase", "--abort"]);
    let side = sh(&root, &["rev-parse", "side"]).trim().to_string();
    sh(&root, &["checkout", "-q", &main]);
    assert!(git().run(&root, &["cherry-pick", &side]).is_err());
    assert_eq!(doing(&root).expect("a pick").op, Op::CherryPick);
}

#[test]
fn a_conflict_reads_its_stages_and_gits_own_blocks() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (root, _) = conflicted(dir.path());
    assert!(git().run(&root, &["merge", "side"]).is_err());
    let Stages::Text {
        sides,
        work,
        regions,
        ..
    } = conflict::read(&git(), &root, "a.txt").expect("reads")
    else {
        panic!("a text conflict")
    };
    assert_eq!(
        sides,
        [
            Some("one\ntwo\n".into()),
            Some("one\nmain\n".into()),
            Some("one\nside\n".into())
        ]
    );
    assert!(work.contains("<<<<<<<"));
    let conflict = regions
        .iter()
        .find(|region| region.kind == RegionKind::Conflict)
        .expect("a block");
    assert_eq!(
        (conflict.ours.clone(), conflict.base.clone()),
        (vec!["main\n".to_string()], vec!["two\n".to_string()]),
        "base from Git's diff3"
    );
}

#[test]
fn near_conflicts_stay_one_block_as_git_wrote_them() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = committed(dir.path());
    for (key, value) in [
        ("user.name", "Ely"),
        ("user.email", "ely@example.com"),
        ("commit.gpgsign", "false"),
    ] {
        sh(&root, &["config", key, value]);
    }
    std::fs::write(root.join("a.txt"), "first\na\nunchanged\nb\nlast\n").expect("writes");
    sh(&root, &["commit", "-qam", "base"]);
    let main = sh(&root, &["branch", "--show-current"]).trim().to_string();
    sh(&root, &["checkout", "-q", "-b", "side"]);
    std::fs::write(
        root.join("a.txt"),
        "first\nTHEIRS a\nunchanged\nTHEIRS b\nlast\n",
    )
    .expect("writes");
    sh(&root, &["commit", "-qam", "theirs"]);
    sh(&root, &["checkout", "-q", &main]);
    std::fs::write(
        root.join("a.txt"),
        "first\nOURS a\nunchanged\nOURS b\nlast\n",
    )
    .expect("writes");
    sh(&root, &["commit", "-qam", "ours"]);
    assert!(git().run(&root, &["merge", "side"]).is_err());
    let stages = conflict::read(&git(), &root, "a.txt").expect("reads");
    let merging = conflict::Merging::new(stages).expect("text");
    let markers = conflicts(&merging.saved, MARKER).len();
    assert_eq!(
        merging.takes.len(),
        markers,
        "the view's blocks are Git's blocks"
    );
}

#[cfg(unix)]
#[test]
fn links_and_bytes_conflict_whole_and_a_save_spares_a_link() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = committed(dir.path());
    for (key, value) in [
        ("user.name", "Ely"),
        ("user.email", "ely@example.com"),
        ("commit.gpgsign", "false"),
    ] {
        sh(&root, &["config", key, value]);
    }
    std::fs::write(root.join("target"), "keep target\n").expect("writes");
    symlink("base-target", root.join("link")).expect("links");
    std::fs::write(root.join("blob.bin"), b"\0base\xff").expect("writes");
    sh(&root, &["add", "-A"]);
    sh(&root, &["commit", "-qm", "base"]);
    let main = sh(&root, &["branch", "--show-current"]).trim().to_string();
    sh(&root, &["checkout", "-q", "-b", "side"]);
    std::fs::remove_file(root.join("link")).expect("removes");
    symlink("theirs-target", root.join("link")).expect("links");
    std::fs::write(root.join("blob.bin"), b"\0theirs\xff").expect("writes");
    sh(&root, &["commit", "-qam", "theirs"]);
    sh(&root, &["checkout", "-q", &main]);
    std::fs::remove_file(root.join("link")).expect("removes");
    symlink("target", root.join("link")).expect("links");
    std::fs::write(root.join("blob.bin"), b"\0ours\xff").expect("writes");
    sh(&root, &["commit", "-qam", "ours"]);
    assert!(git().run(&root, &["merge", "side"]).is_err());
    for path in ["link", "blob.bin"] {
        assert_eq!(
            conflict::read(&git(), &root, path).expect("reads"),
            Stages::Whole { present: [true; 3] },
            "{path}"
        );
    }
    let save = Write::Save {
        path: "link".into(),
        text: "overwritten".into(),
    };
    assert!(
        work::run(&git(), &root, &save, &save.args(true)).is_err(),
        "no save through a link"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("target")).expect("reads"),
        "keep target\n"
    );
}

#[test]
fn a_branch_and_a_tag_of_one_name_keep_their_own_names() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = committed(dir.path());
    sh(&root, &["branch", "topic"]);
    sh(&root, &["tag", "topic"]);
    let refs = branches::read(&git(), &root).expect("reads");
    let named: Vec<(&str, &str)> = refs
        .iter()
        .filter(|r| r.name == "topic")
        .map(|r| (r.name.as_str(), r.full.as_str()))
        .collect();
    assert_eq!(
        named,
        [("topic", "refs/heads/topic"), ("topic", "refs/tags/topic")]
    );
    sh(&root, &["switch", "-q", "topic"]);
    assert_eq!(sh(&root, &["branch", "--show-current"]).trim(), "topic");
}

/// Two branches that both change `a.txt` from `base` on.
fn both_change(
    dir: &Path,
    base: &str,
    ours: &str,
    theirs: &str,
    setup: &[(&str, &str)],
) -> std::path::PathBuf {
    let root = committed(dir);
    for (key, value) in [
        ("user.name", "Ely"),
        ("user.email", "ely@example.com"),
        ("commit.gpgsign", "false"),
    ]
    .iter()
    .chain(setup)
    {
        sh(&root, &["config", key, value]);
    }
    std::fs::write(root.join("a.txt"), base).expect("writes");
    sh(&root, &["add", "-A"]);
    sh(&root, &["commit", "-qm", "base"]);
    let main = sh(&root, &["branch", "--show-current"]).trim().to_string();
    sh(&root, &["checkout", "-q", "-b", "side"]);
    std::fs::write(root.join("a.txt"), theirs).expect("writes");
    sh(&root, &["commit", "-qam", "theirs"]);
    sh(&root, &["checkout", "-q", &main]);
    std::fs::write(root.join("a.txt"), ours).expect("writes");
    sh(&root, &["commit", "-qam", "ours"]);
    assert!(git().run(&root, &["merge", "side"]).is_err(), "a conflict");
    root
}

#[test]
fn markers_follow_conflict_marker_size_and_keep_lookalike_lines() {
    for size in [5usize, 10] {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::create_dir_all(dir.path().join("work")).expect("a folder");
        let root = committed(dir.path());
        std::fs::write(
            root.join(".gitattributes"),
            format!("a.txt conflict-marker-size={size}\n"),
        )
        .expect("writes");
        let root = {
            sh(&root, &["add", ".gitattributes"]);
            sh(&root, &["commit", "-qm", "attributes"]);
            root
        };
        for (key, value) in [
            ("user.name", "Ely"),
            ("user.email", "ely@example.com"),
            ("commit.gpgsign", "false"),
        ] {
            sh(&root, &["config", key, value]);
        }
        let main = sh(&root, &["branch", "--show-current"]).trim().to_string();
        sh(&root, &["checkout", "-q", "-b", "side"]);
        std::fs::write(
            root.join("a.txt"),
            "theirs-top\n======= literal\nTHEIRS-END\n",
        )
        .expect("writes");
        sh(&root, &["commit", "-qam", "theirs"]);
        sh(&root, &["checkout", "-q", &main]);
        std::fs::write(root.join("a.txt"), "ours\n").expect("writes");
        sh(&root, &["commit", "-qam", "ours"]);
        assert!(git().run(&root, &["merge", "side"]).is_err());
        let merging =
            conflict::Merging::new(conflict::read(&git(), &root, "a.txt").expect("reads"))
                .expect("text");
        assert_eq!(
            (merging.size(), merging.takes.len()),
            (size, 1),
            "width {size}"
        );
        let settled = ely_gpui_component::git::resolve(
            &merging.saved,
            0,
            ely_gpui_component::git::Take::Theirs,
            size,
        );
        assert_eq!(
            settled.as_deref(),
            Some("theirs-top\n======= literal\nTHEIRS-END\n")
        );
    }
}

#[test]
fn nul_bytes_that_are_utf8_still_conflict_whole() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = both_change(
        dir.path(),
        "one\0base\n",
        "one\0ours\n",
        "one\0theirs\n",
        &[],
    );
    assert_eq!(
        conflict::read(&git(), &root, "a.txt").expect("reads"),
        Stages::Whole { present: [true; 3] }
    );
}

#[test]
fn each_block_keeps_its_own_base() {
    let same: String = (1..=10).map(|n| format!("same {n}\n")).collect();
    let base = format!("base-first\n{same}");
    let ours = format!("OURS first\n{same}ours insert\n");
    let theirs = format!("THEIRS first\n{same}theirs insert\n");
    for style in ["merge", "diff3"] {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = both_change(
            dir.path(),
            &base,
            &ours,
            &theirs,
            &[("merge.conflictStyle", style)],
        );
        let Stages::Text { regions, .. } = conflict::read(&git(), &root, "a.txt").expect("reads")
        else {
            panic!("text")
        };
        let bases: Vec<Vec<String>> = regions
            .into_iter()
            .filter(|r| r.kind == RegionKind::Conflict)
            .map(|r| r.base)
            .collect();
        assert_eq!(
            bases,
            [vec!["base-first\n".to_string()], Vec::new()],
            "{style}"
        );
    }
}

#[test]
fn a_remote_branch_and_a_tag_of_one_name_switch_apart() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = committed(dir.path());
    sh(&root, &["update-ref", "refs/remotes/origin/topic", "HEAD"]);
    sh(&root, &["tag", "origin/topic"]);
    sh(&root, &["config", "remote.origin.url", "/nowhere"]);
    sh(
        &root,
        &[
            "config",
            "remote.origin.fetch",
            "+refs/heads/*:refs/remotes/origin/*",
        ],
    );
    let write = Write::Git(vec![
        "switch".into(),
        "--track".into(),
        "refs/remotes/origin/topic".into(),
    ]);
    work::run(&git(), &root, &write, &write.args(true)).expect("switches");
    assert_eq!(sh(&root, &["branch", "--show-current"]).trim(), "topic");
}

#[test]
fn marker_sizes_git_cannot_read_mean_seven() {
    for value in ["0", "-1", "abc", "10abc"] {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = both_change(dir.path(), "base\n", "ours\n", "theirs\n", &[]);
        std::fs::write(
            root.join(".gitattributes"),
            format!("a.txt conflict-marker-size={value}\n"),
        )
        .expect("writes");
        let merging =
            conflict::Merging::new(conflict::read(&git(), &root, "a.txt").expect("reads"))
                .expect("text");
        assert_eq!(
            (merging.size(), merging.takes.len()),
            (MARKER, 1),
            "{value}"
        );
    }
}

#[test]
fn a_tag_on_a_blob_has_no_date_and_spoils_no_list() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = committed(dir.path());
    sh(&root, &["tag", "blobtag", "HEAD:a.txt"]);
    let refs = crate::branches::read(&git(), &root).expect("the list reads");
    let tag = refs
        .iter()
        .find(|r| r.name == "blobtag")
        .expect("the tag is listed");
    assert_eq!(tag.at, None);
    assert!(
        refs.iter()
            .filter(|r| r.name != "blobtag")
            .all(|r| r.at.is_some())
    );
}
