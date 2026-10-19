//! Recently opened paths, kept in a JSON file.

use std::{
    io,
    path::{Path, PathBuf},
};

/// How many paths the list keeps.
const KEEP: usize = 20;

/// Recently opened paths, newest first.
pub fn recent(file: &Path) -> io::Result<Vec<PathBuf>> {
    let listed: Vec<PathBuf> = match std::fs::read(file) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| io::Error::other(format!("{}: {error}", file.display())))?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    // Older builds wrote `\\?\` paths.
    let mut list = Vec::with_capacity(listed.len());
    for path in listed {
        let plain = dunce::simplified(&path).to_path_buf();
        if !list.contains(&plain) {
            list.push(plain);
        }
    }
    Ok(list)
}

/// Takes `root` out of the list.
pub fn forget(file: &Path, root: &Path) -> io::Result<()> {
    let mut list = recent(file)?;
    list.retain(|known| known != root);
    crate::save_atomic(
        file,
        &serde_json::to_vec(&list).map_err(io::Error::other)?,
        0,
    )
    .map(drop)
}

/// Empties the list.
pub fn clear(file: &Path) -> io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::save_atomic(file, b"[]", 0).map(drop)
}

/// Puts `root` first in the list and writes it.
pub fn remember(file: &Path, root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut list = recent(file)?;
    list.retain(|known| known != root);
    list.insert(0, root.to_path_buf());
    list.truncate(KEEP);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::save_atomic(
        file,
        &serde_json::to_vec(&list).map_err(io::Error::other)?,
        0,
    )?;
    Ok(list)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{clear, forget, recent, remember};

    #[test]
    fn the_list_puts_the_newest_first_and_keeps_twenty() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let file = dir.path().join("data/recent.json");
        assert!(recent(&file).expect("none yet").is_empty());
        for n in 0..25 {
            remember(&file, Path::new(&format!("/r/{n}"))).expect("remembers");
        }
        let list = remember(&file, Path::new("/r/10")).expect("remembers");
        assert_eq!(list.len(), 20);
        assert_eq!(list[..2], [PathBuf::from("/r/10"), PathBuf::from("/r/24")]);
        forget(&file, Path::new("/r/10")).expect("forgets");
        assert_eq!(recent(&file).expect("reads")[0], PathBuf::from("/r/24"));
        #[cfg(windows)]
        {
            let old = [r"\\?\C:\r\old", r"C:\r\old", r"\\?\C:\r\new"];
            std::fs::write(&file, serde_json::to_vec(&old).expect("json")).expect("writes");
            let plain = [PathBuf::from(r"C:\r\old"), PathBuf::from(r"C:\r\new")];
            assert_eq!(recent(&file).expect("reads"), plain);
        }
        clear(&file).expect("clears");
        assert!(recent(&file).expect("reads").is_empty());
        std::fs::write(&file, "not json").expect("writes");
        assert!(
            remember(&file, Path::new("/r/1")).is_err(),
            "a broken list is not overwritten"
        );
    }
}
