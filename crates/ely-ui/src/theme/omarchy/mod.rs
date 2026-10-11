use std::{fs, io, path::Path};

use anyhow::{Context, Result};
use ely_gpui_component::theme::{Mode, Palette};
use futures::channel::mpsc::{self, UnboundedReceiver};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

mod colors;
mod palette;
#[cfg(test)]
mod tests;

/// An Omarchy theme as Ely's palette.
#[derive(Clone, Debug, PartialEq)]
pub struct OmarchyTheme {
    pub name: Option<String>,
    pub mode: Mode,
    pub palette: Palette,
    /// Hues the theme lacks, which Ely's own colors fill.
    pub standing_in: Vec<&'static str>,
}

/// Reads `<current>/theme/colors.toml`. None when Omarchy has not written one.
pub fn load(current: &Path) -> Result<Option<OmarchyTheme>> {
    let theme = current.join("theme");
    let file = theme.join("colors.toml");
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("cannot read {}", file.display())),
    };
    let name_file = current.join("theme.name");
    let name = match fs::read_to_string(&name_file) {
        Ok(name) => Some(name.trim().to_string()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error).with_context(|| format!("cannot read {}", name_file.display())),
    };
    parse(&text, name, theme.join("light.mode").exists())
        .with_context(|| format!("{} is not a usable theme", file.display()))
        .map(Some)
}

pub fn parse(text: &str, name: Option<String>, light_file: bool) -> Result<OmarchyTheme> {
    let colors = colors::read(text, light_file)?;
    let (palette, standing_in) = palette::build(&colors)?;
    Ok(OmarchyTheme {
        name,
        mode: colors.mode,
        palette,
        standing_in,
    })
}

/// One `()` per event under `current`; keep the watcher alive.
pub fn changes(current: &Path) -> Result<(RecommendedWatcher, UnboundedReceiver<()>)> {
    let (sender, receiver) = mpsc::unbounded();
    let mut watcher =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
            // Reads, ours included, are no change.
            Ok(event) if crate::watching::only_read(&event.kind) => {}
            Ok(_) => {
                let _ = sender.unbounded_send(());
            }
            Err(error) => log::warn!("theme: watching omarchy failed: {error}"),
        })
        .context("cannot start the file watcher")?;
    watcher
        .watch(current, RecursiveMode::NonRecursive)
        .with_context(|| format!("cannot watch {}", current.display()))?;
    Ok((watcher, receiver))
}
