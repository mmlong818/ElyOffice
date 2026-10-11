//! LOK shared access: worker process, IPC and client (PRD §7).

mod client;
pub mod commands;
mod events;
mod ffi;
mod handshake;
pub mod history;
mod ipc;
pub mod review;
pub mod sheet;
pub mod show;
mod worker;

pub use client::{
    A11y, Cell, Doc, Events, Function, MIXED, Mark, Mouse, Pdf, Rect, Request, Sheet, Sheets, Tile,
    Worker, install_dir, is_stale, templates,
};
#[cfg(feature = "view")]
pub use view::{
    CONTEXT, Copy, Cut, Geom, Layout, LokError, LokView, Paste, Reported, TILE_TWIPS, ZOOM_STEPS,
    Zoomed, init, layout, paper, read_off, scheme,
};
#[cfg(feature = "view")]
mod view;
pub use ipc::STALE;
pub use ipc::file_url_of as file_url;
pub use ipc::path_of_file_url as file_path;
pub use worker::run as worker_run;

/// Waits for the Windows client to assign this process to its supervision job.
#[cfg(windows)]
pub fn worker_bootstrap() -> std::io::Result<()> {
    client::supervise::await_bootstrap()
}
