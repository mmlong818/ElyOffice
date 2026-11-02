//! A function the kernel offers, taken in input order (TABLE-03).

use std::io;

use super::with_doc;
use crate::ipc::{Pack, Unpack};

/// Takes an offered function, else posts the key.
pub(super) fn complete(unpack: &mut Unpack) -> io::Result<Vec<u8>> {
    let id = unpack.u64()?;
    let name = match unpack.u64()? {
        0 => None,
        _ => Some(unpack.str()?),
    };
    let (char_code, key_code) = (unpack.i64()?, unpack.i64()?);
    // No key: a click; else Enter (1280) or Tab (1282).
    if !matches!(key_code, 0 | 1280 | 1282) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a completion takes Enter or Tab",
        ));
    }
    // Earlier input's callbacks already landed here.
    let offered = crate::events::offered(id);
    let take = match name {
        Some(name) if offered.contains(&name) => Some(name),
        // As the menu orders them: shortest first.
        _ if key_code != 0 => offered.into_iter().min_by_key(String::len),
        _ => None,
    };
    if let Some(name) = take {
        log::info!("lok-worker: completes a function");
        with_doc(id, |doc| doc.complete_function(&name))??;
        return Ok(Pack::new().u64(1).bytes());
    }
    if key_code != 0 {
        with_doc(id, |doc| {
            doc.post_key(0, char_code as i32, key_code as i32);
            doc.post_key(1, char_code as i32, key_code as i32);
        })?;
    }
    Ok(Pack::new().u64(0).bytes())
}
