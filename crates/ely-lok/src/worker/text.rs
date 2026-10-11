//! CSV in and out, from typed fields only (TABLE-12, 13).

use std::io;

use super::{close, load, with_doc};
use crate::ffi::Lok;
use crate::ipc::Unpack;

/// Loads a text file from typed CSV options.
pub(super) fn import(lok: &Lok, unpack: &mut Unpack) -> io::Result<u64> {
    use crate::sheet::csv::{COLUMNS, ENCODINGS, Import};
    let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidInput, format!("a bad {what}"));
    let url = unpack.str()?;
    let character = |code: u64| u32::try_from(code).ok().and_then(char::from_u32);
    let separator = character(unpack.u64()?).ok_or_else(|| bad("separator"))?;
    let quote = match unpack.u64()? {
        0 => None,
        code => Some(character(code).ok_or_else(|| bad("quote"))?),
    };
    let pick = |at: u64, len| usize::try_from(at).ok().filter(|at| *at < len);
    let encoding =
        ENCODINGS[pick(unpack.u64()?, ENCODINGS.len()).ok_or_else(|| bad("encoding"))?];
    let columns = (0..unpack.u64()?)
        .map(|_| {
            let at = pick(unpack.u64()?, COLUMNS.len()).ok_or_else(|| bad("column"))?;
            Ok(COLUMNS[at])
        })
        .collect::<io::Result<Vec<_>>>()?;
    let how = Import {
        separator,
        quote,
        encoding,
        columns,
    };
    let filter = format!("FilterName=Text - txt - csv (StarCalc),{}", how.filter());
    let id = load(lok, &url, String::new(), &filter)?;
    // A .txt name must not land in Writer.
    let kind = with_doc(id, |doc| doc.kind())?;
    if kind != 1 {
        close(id)?;
        return Err(io::Error::other(format!(
            "the text loaded as kind {kind}, not a sheet"
        )));
    }
    Ok(id)
}

/// Writes the shown sheet as text from a typed separator.
pub(super) fn export(unpack: &mut Unpack) -> io::Result<()> {
    let (id, url) = (unpack.u64()?, unpack.str()?);
    let separator = u32::try_from(unpack.u64()?)
        .ok()
        .and_then(char::from_u32)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "a bad separator"))?;
    let options = crate::sheet::csv::export(separator);
    with_doc(id, |doc| doc.save_as(&url, "csv", &options))??;
    log::info!("lok-worker: exported text split by {separator:?}");
    Ok(())
}
