use std::path::PathBuf;

use super::*;

mod analysis;
mod charts;
mod check;
mod commands;
mod format;
mod formula;
mod handouts;
mod independent;
pub(crate) mod input;
mod latency;
mod layout;
mod media;
mod motion;
mod output;
mod paint;
mod playlist;
mod print;
mod refs;
mod roundtrip;
mod rules;
mod sheet;
pub(crate) mod show;
mod slides;
#[cfg(all(windows, feature = "supervisor-test"))]
mod supervise;
mod tools;
mod workbook;
#[cfg(windows)]
mod windows_real_flow;

/// The pinned LibreOffice commit the build must report.
const PINNED_BUILD: &str = "8221e31";

/// The pinned build, or None when absent.
pub(crate) fn install() -> Option<PathBuf> {
    install_dir().ok()
}

/// A tiny flat-XML document; no zip needed.
pub(crate) const FODT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
 xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.text">
 <office:body><office:text><text:p>Hello 世界</text:p></office:text></office:body>
</office:document>"#;

/// A document whose button runs the Basic marker writer.
fn macro_fodt(marker: &std::path::Path) -> String {
    let marker = marker.display().to_string().replace('\\', "/");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
 xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
 xmlns:ooo="http://openoffice.org/2004/office"
 xmlns:form="urn:oasis:names:tc:opendocument:xmlns:form:1.0"
 xmlns:script="urn:oasis:names:tc:opendocument:xmlns:script:1.0"
 xmlns:xlink="http://www.w3.org/1999/xlink" office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.text">
 <office:scripts>
  <office:script script:language="ooo:Basic">
   <ooo:libraries>
    <ooo:library-embedded ooo:name="Standard">
     <ooo:module ooo:name="Module1">
      <ooo:source-code>Sub Sneaky&#x0A;  Open "{marker}" For Output As #1&#x0A;  Write #1, "ran"&#x0A;  Close #1&#x0A;End Sub</ooo:source-code>
     </ooo:module>
    </ooo:library-embedded>
   </ooo:libraries>
  </office:script>
 </office:scripts>
 <office:body>
  <office:text>
   <office:forms form:automatic-focus="false" form:apply-design-mode="false">
    <form:form>
     <form:button form:name="btn" form:control-implementation="ooo:com.sun.star.form.component.CommandButton" xml:id="btn1" form:id="btn1" form:label="Run">
      <office:event-listeners>
       <script:event-listener script:language="ooo:script" script:event-name="form:performaction" xlink:href="vnd.sun.star.script:Standard.Module1.Sneaky?language=Basic&amp;location=document" xlink:type="simple"/>
      </office:event-listeners>
     </form:button>
    </form:form>
   </office:forms>
   <text:p>Macro doc</text:p>
  </office:text>
 </office:body>
</office:document>"#
    )
}

/// Clicks (x, y) twips: left press and release.
pub(crate) fn click(worker: &mut Worker, doc: &Doc, x: i32, y: i32) {
    for kind in [0, 1] {
        let event = Mouse {
            kind,
            x,
            y,
            count: 1,
            buttons: 1,
            modifiers: 0,
        };
        worker.post_mouse(doc, event).expect("the click posts");
    }
}

/// Polls past `from` until `done` or `wait`; rects pile up.
pub(crate) fn poll_until(
    worker: &mut Worker,
    doc: &Doc,
    from: &Events,
    wait: std::time::Duration,
    done: impl Fn(&Events) -> bool,
) -> Events {
    let deadline = std::time::Instant::now() + wait;
    let mut seen = Events {
        rects: Vec::new(),
        all: false,
        states: Vec::new(),
        blocked: Vec::new(),
        ..from.clone()
    };
    loop {
        let batch = worker
            .poll_events(doc, seen.seq, 250)
            .expect("the poll returns");
        let mut rects = std::mem::take(&mut seen.rects);
        let mut states = std::mem::take(&mut seen.states);
        let mut blocked = std::mem::take(&mut seen.blocked);
        let all = seen.all || batch.all;
        rects.extend(batch.rects.iter().copied());
        states.extend(batch.states.iter().cloned());
        blocked.extend(batch.blocked.iter().cloned());
        seen = Events {
            rects,
            all,
            states,
            blocked,
            ..batch
        };
        if done(&seen) || std::time::Instant::now() >= deadline {
            return seen;
        }
    }
}

fn write(path: &Path, body: &str) {
    std::fs::write(path, body).expect("the fixture writes");
}

#[test]
fn the_worker_round_trips_a_document() {
    let Some(install) = install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let dir = tempfile::tempdir().expect("a temp dir");
    let profile = dir.path().join("配置 profile");
    let mut worker = Worker::spawn(&install, &profile).expect("the worker spawns");
    let build = worker.build_id().expect("the build id");
    assert!(build.contains(PINNED_BUILD), "the pinned build: {build}");

    let source = dir.path().join("note.fodt");
    write(&source, FODT);
    let url = format!("file://{}", source.display());
    let doc = worker.load(&url).expect("the document loads");
    let (width, height) = doc.size(&mut worker).expect("the size");
    assert!(width > 0 || height > 0, "a nonzero document size");

    let odt = dir.path().join("note.odt");
    doc.save_as(&mut worker, &format!("file://{}", odt.display()), "odt")
        .expect("the odt save");
    assert!(odt.try_exists().expect("the odt exists"));
    let doc2 = worker
        .load(&format!("file://{}", odt.display()))
        .expect("the odt reloads");
    let pdf = dir.path().join("note.pdf");
    doc2.save_as(&mut worker, &format!("file://{}", pdf.display()), "pdf")
        .expect("the pdf save");
    let head = std::fs::read(&pdf).expect("the pdf reads");
    assert!(head.starts_with(b"%PDF"), "a real PDF comes out");
    worker.close_doc(doc2).expect("the second doc closes");
    worker.close_doc(doc).expect("the first doc closes");
}

#[test]
fn a_missing_file_is_an_error_never_a_success() {
    let Some(install) = install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut worker = Worker::spawn(&install, &dir.path().join("profile")).expect("the worker");
    let missing = dir.path().join("gone.odt");
    let outcome = worker.load(&format!("file://{}", missing.display()));
    assert!(outcome.is_err(), "LOK-09: the failure is an error");
}

#[test]
fn a_failed_check_refuses_every_later_request() {
    let Some(install) = install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let dir = tempfile::tempdir().expect("a temp dir");
    let profile = dir.path().join("profile");
    // The self-check cannot write its sample over a folder.
    std::fs::create_dir_all(profile.join("lok-verify.fodt")).expect("the blocker");
    let mut worker = Worker::launch(&install, &profile).expect("the worker");
    assert!(worker.ready().is_err(), "the check fails");
    assert!(worker.ready().is_err(), "and stays failed");
    assert!(worker.load("private:factory/swriter").is_err(), "no document reaches it");
}

/// Initializes the profile fully, the CLI way (registers Basic).
fn seed_basic(install: &Path, profile: &Path) {
    let soffice = install.parent().expect("the app dir").join("MacOS/soffice");
    let url = ipc::file_url_of(profile);
    let status = std::process::Command::new(&soffice)
        .args(["--headless", "--norestore", "--terminate_after_init"])
        .arg(format!("-env:UserInstallation={url}"))
        .status()
        .expect("soffice runs");
    assert!(status.success(), "the profile initializes");
}

/// The macro module: writes its marker through the UCB.
fn module(marker: &Path) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE script:module PUBLIC \"-//OpenOffice.org//DTD OfficeDocument 1.0//EN\" \"module.dtd\">\n<script:module xmlns:script=\"http://openoffice.org/2000/script\" script:name=\"Module1\" script:language=\"StarBasic\">Sub Sneaky\n  oSFA = createUnoService(\"com.sun.star.ucb.SimpleFileAccess\")\n  oOut = oSFA.openFileWrite(\"{}\")\n  oOut.writeString(\"ran\")\n  oOut.closeOutput()\nEnd Sub\n</script:module>\n",
        marker.display().to_string().replace('\\', "/")
    )
}

const MACRO_URL: &str = "macro:///Standard.Module1.Sneaky";

/// 未验证: this backend never dispatches document events to Basic.
#[test]
fn a_macro_never_writes_its_marker() {
    let Some(install) = install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let dir = tempfile::tempdir().expect("a temp dir");
    let profile = dir.path().join("profile");
    let marker = dir.path().join("macro-ran.txt");
    seed_basic(&install, &profile);
    std::fs::write(
        profile.join("user/basic/Standard/Module1.xba"),
        module(&marker),
    )
    .expect("the module writes");
    let mut worker = Worker::spawn_dev(&install, &profile).expect("the worker");
    let source = dir.path().join("macro.fodt");
    std::fs::write(&source, macro_fodt(&marker)).expect("the macro fodt writes");
    let doc = worker
        .load(&format!("file://{}", source.display()))
        .expect("the document loads");
    std::thread::sleep(std::time::Duration::from_millis(800));
    assert!(
        !marker.try_exists().expect("the marker check"),
        "LOK-11: loading ran no macro"
    );
    let ran = worker.run_macro(MACRO_URL).expect("the macro channel");
    assert!(ran != 0, "the build runs the library macro when asked");
    assert!(
        marker.try_exists().expect("the marker check"),
        "the sample is real: it writes when run"
    );
    worker.close_doc(doc).expect("the document closes");
}

#[test]
fn a_production_worker_refuses_run_macro() {
    let Some(install) = install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut worker = Worker::spawn(&install, &dir.path().join("profile")).expect("the worker");
    let refused = worker.run_macro(MACRO_URL);
    assert!(
        refused.is_err(),
        "LOK-11: the production worker keeps the gate"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn an_unpatched_build_is_rejected() {
    let dmg = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../engines/LibreOffice_26.2.6_MacOS_aarch64.dmg");
    if !dmg.try_exists().expect("the dmg check") {
        eprintln!("SKIP: no official dmg");
        return;
    }
    let mount = tempfile::tempdir().expect("a temp dir");
    let attached = std::process::Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-mountpoint"])
        .arg(mount.path())
        .arg(&dmg)
        .status()
        .expect("hdiutil runs");
    assert!(attached.success(), "the official dmg mounts");
    let frameworks = mount.path().join("LibreOffice.app/Contents/Frameworks");
    let dir = tempfile::tempdir().expect("a temp dir");
    let outcome = Worker::spawn(&frameworks, &dir.path().join("profile"));
    assert!(outcome.is_err(), "the unpatched build must be rejected");
    let detached = std::process::Command::new("hdiutil")
        .args(["detach", "-quiet"])
        .arg(mount.path())
        .status()
        .expect("hdiutil detaches");
    assert!(detached.success());
}

#[test]
fn shutdown_leaves_no_locks_or_temp() {
    let Some(install) = install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let dir = tempfile::tempdir().expect("a temp dir");
    let profile = dir.path().join("profile");
    let mut worker = Worker::spawn(&install, &profile).expect("the worker");
    let source = dir.path().join("note.fodt");
    write(&source, FODT);
    #[cfg(windows)]
    let url = crate::file_url(&source);
    #[cfg(not(windows))]
    let url = format!("file://{}", source.display());
    let doc = worker.load(&url).expect("the document loads");
    let lock = dir.path().join(".~lock.note.fodt#");
    worker.close_doc(doc).expect("the document closes");
    worker.shutdown().expect("the worker shuts down");
    assert!(
        !lock.try_exists().expect("the doc lock check"),
        "no stale document lock"
    );
    assert!(
        !profile
            .join(".lock")
            .try_exists()
            .expect("the profile lock check"),
        "no profile lock"
    );
    let lu: Vec<_> = std::fs::read_dir(profile.join("tmp"))
        .expect("the temp dir lists")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("lu"))
        .collect();
    assert!(lu.is_empty(), "no kernel temp files: {lu:?}");
}

#[test]
fn each_load_is_its_own_session() {
    let Some(install) = install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut worker = Worker::spawn(&install, &dir.path().join("profile")).expect("the worker spawns");
    let source = dir.path().join("note.fodt");
    write(&source, FODT);
    let url = format!("file://{}", source.display());
    // Freed native documents reuse addresses; sessions never repeat.
    let mut old = Vec::new();
    for _ in 0..4 {
        let doc = worker.load(&url).expect("loads");
        assert!(old.iter().all(|seen: &Doc| seen.id != doc.id), "a session came back");
        worker.close_doc(doc.clone()).expect("closes");
        old.push(doc);
    }
    for doc in &old {
        assert!(doc.size(&mut worker).is_err(), "a closed session answered");
    }
    let doc = worker.load(&url).expect("loads again");
    worker.post_key(&doc, 'x' as u32, 0).expect("a key");
    let events = worker.poll_events(&doc, 0, 5_000).expect("events");
    assert!(events.seq > 0, "the new session's events flow");
    worker.close_doc(doc).expect("closes");
}

#[test]
fn a_document_belongs_to_its_worker() {
    let Some(install) = install() else {
        eprintln!("SKIP: no pinned LOK build");
        return;
    };
    let dir = tempfile::tempdir().expect("a temp dir");
    let source = dir.path().join("note.fodt");
    write(&source, FODT);
    let url = format!("file://{}", source.display());
    let mut first = Worker::spawn(&install, &dir.path().join("one")).expect("a worker");
    let mut second = Worker::spawn(&install, &dir.path().join("two")).expect("another worker");
    let theirs = first.load(&url).expect("loads");
    let ours = second.load(&url).expect("loads");
    // Another worker's handle never reaches these documents.
    assert!(theirs.size(&mut second).is_err(), "another worker's document answered");
    assert!(second.close_doc(theirs.clone()).is_err(), "another worker's document closed");
    assert!(ours.size(&mut second).is_ok(), "its own document stays");
    first.close_doc(theirs).expect("closes");
    second.close_doc(ours).expect("closes");
}

#[test]
fn a_worker_that_cannot_start_says_why() {
    if install().is_none() {
        eprintln!("SKIP: no pinned LOK build");
        return;
    }
    let dir = tempfile::tempdir().expect("a temp dir");
    let Err(error) = Worker::spawn(&dir.path().join("no-install"), &dir.path().join("profile")) else {
        panic!("a worker started without a kernel");
    };
    // The worker's own reason, not only a closed pipe.
    assert!(error.to_string().contains("lok-worker:"), "{error}");
}
