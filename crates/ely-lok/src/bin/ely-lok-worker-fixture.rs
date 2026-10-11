use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
    thread,
    time::Duration,
};

fn main() -> io::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let profile = PathBuf::from(
        args.get(2)
            .ok_or_else(|| io::Error::other("missing profile"))?,
    );
    fs::write(
        profile.join("prebootstrap.pid"),
        std::process::id().to_string(),
    )?;
    #[cfg(windows)]
    ely_lok::worker_bootstrap()?;
    fs::write(profile.join("bootstrap.ok"), b"ok")?;
    let mode = fs::read_to_string(profile.join("mode.txt")).unwrap_or_default();
    if mode.trim() == "stall-startup" {
        fs::write(
            profile.join("startup-stalled"),
            b"token accepted; readiness withheld",
        )?;
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut count = 0u64;
    loop {
        let mut len = [0; 4];
        match input.read_exact(&mut len) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error),
        }
        let mut tag = [0; 1];
        let mut id = [0; 8];
        input.read_exact(&mut tag)?;
        input.read_exact(&mut id)?;
        if mode.trim() == "block-payload" && tag[0] == 250 {
            fs::write(profile.join("accepted-payload"), b"header accepted")?;
            loop {
                thread::sleep(Duration::from_secs(1));
            }
        }
        if mode.trim() == "stall-shutdown" && tag[0] == 6 {
            fs::write(profile.join("shutdown-accepted"), b"shutdown received")?;
            loop {
                thread::sleep(Duration::from_secs(1));
            }
        }
        let mut payload = vec![0; u32::from_le_bytes(len) as usize];
        input.read_exact(&mut payload)?;
        // The Windows readiness check sends BUILD_ID and VERIFY before the
        // fixture scenarios start counting ordinary client requests.
        if tag[0] != 7 {
            count += 1;
        }
        if tag[0] == 7 {
            ely_ipc::write_frame(&mut output, 100, u64::from_le_bytes(id), &ely_ipc::Pack::new().u64(1).bytes())?;
            continue;
        }
        if matches!(mode.trim(), "stall-second" | "fatal-second") && count == 2 {
            fs::write(profile.join(format!("accepted-{count}")), b"yes")?;
            if mode.trim() == "fatal-second" {
                std::process::exit(1);
            }
            loop {
                thread::sleep(Duration::from_secs(1));
            }
        }
        if mode.trim() == "delay-pair" && (2..=3).contains(&count) {
            fs::write(profile.join(format!("accepted-{count}")), b"yes")?;
            thread::sleep(Duration::from_millis(400));
        }
        let body = ely_ipc::Pack::new()
            .str(r#"{"ProductName":"LibreOffice","ProductVersion":"26.2","BuildId":"8221e31b3ac356a1623c672912a3d2b492f7e3d1"}"#)
            .bytes();
        let id = u64::from_le_bytes(id);
        ely_ipc::write_frame(&mut output, 100, id, &body)?;
        if tag[0] == 6 {
            return Ok(());
        }
    }
}
