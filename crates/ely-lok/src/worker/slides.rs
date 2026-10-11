use std::{
    collections::HashMap,
    io,
    sync::{LazyLock, Mutex, MutexGuard},
};

use super::with_doc;
use crate::ipc::{Pack, Unpack};

/// The most a slide's layers may take in memory.
const LAYER_MEMORY: usize = 512 * 1024 * 1024;

/// The most pixel bytes one reply carries.
const CHUNK: usize = 16 * 1024 * 1024;

/// Rendered layers by document; each leaves as it is read.
type Waiting = HashMap<u64, Vec<Option<Vec<u8>>>>;

fn waiting() -> MutexGuard<'static, Waiting> {
    static WAITING: LazyLock<Mutex<Waiting>> = LazyLock::new(Mutex::default);
    WAITING.lock().expect("the layers lock")
}

/// Drops document `id`'s waiting layers.
pub(super) fn forget(id: u64) {
    waiting().remove(&id);
}

/// Slide `part`'s layers, its pixels held back.
pub(super) fn slide_layers(unpack: &mut Unpack) -> io::Result<Vec<u8>> {
    let (id, part) = (unpack.u64()?, unpack.u64()?);
    let (width, height) = (unpack.u64()?, unpack.u64()?);
    let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidInput, what.to_string());
    let part = i32::try_from(part).map_err(|_| bad("bad part"))?;
    let pixels = (u32::try_from(width), u32::try_from(height));
    let (Ok(width), Ok(height)) = pixels else {
        return Err(bad("bad size"));
    };
    with_doc(id, |doc| {
        let info: serde_json::Value =
            serde_json::from_str(&doc.presentation_info()?).map_err(io::Error::other)?;
        let slide = usize::try_from(part).ok().and_then(|at| info["slides"].get(at));
        let hash = slide
            .and_then(|slide| slide["hash"].as_str())
            .ok_or_else(|| bad("no such slide"))?;
        let ((width, height), layers) = doc.slide_layers((part, hash), (width, height), LAYER_MEMORY)?;
        let mut body = Pack::new()
            .u64(u64::from(width))
            .u64(u64::from(height))
            .u64(layers.len() as u64);
        let mut pixels = Vec::with_capacity(layers.len());
        for layer in layers {
            body = body.str(&layer.json).u64(u64::from(layer.pixels.is_some()));
            pixels.push(layer.pixels);
        }
        waiting().insert(id, pixels);
        Ok(body.bytes())
    })?
}

/// Hands over layer `index` from byte `from`; read layers free.
pub(super) fn slide_layer(unpack: &mut Unpack) -> io::Result<Vec<u8>> {
    let (id, index, from) = (unpack.u64()?, unpack.u64()?, unpack.u64()?);
    let mut waiting = waiting();
    let Some((total, bytes)) = piece(waiting.get_mut(&id), index, from) else {
        // A request off the batch ends it.
        waiting.remove(&id);
        let what = format!("no layer {index} from byte {from} waits");
        return Err(io::Error::new(io::ErrorKind::InvalidInput, what));
    };
    if waiting.get(&id).is_some_and(|layers| layers.iter().all(Option::is_none)) {
        waiting.remove(&id);
    }
    Ok(Pack::new().u64(total).blob(&bytes).bytes())
}

/// Up to `CHUNK` bytes of a layer and its length.
fn piece(layers: Option<&mut Vec<Option<Vec<u8>>>>, index: u64, from: u64) -> Option<(u64, Vec<u8>)> {
    let slot = layers?.get_mut(usize::try_from(index).ok()?)?;
    let pixels = slot.as_ref()?;
    let from = usize::try_from(from).ok().filter(|from| *from < pixels.len() || *from == 0)?;
    let to = (from + CHUNK).min(pixels.len());
    let read = (pixels.len() as u64, pixels[from..to].to_vec());
    if to == pixels.len() {
        *slot = None;
    }
    Some(read)
}
