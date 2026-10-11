#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod attach;
mod autosave;
mod compare;
mod desk;
mod engine;
mod export;
mod finding;
mod folder;
mod keys;
mod look;
mod outside;
mod page;
mod paper;
mod places;
mod rename;
mod restore;
mod side;
mod text;
mod vcs;
mod view;

use ely_platform::AppId;

fn main() {
    // CEF runs its helper processes from this same binary.
    if let Some(code) = ely_web_content::subprocess() {
        std::process::exit(code);
    }
    ely_ui::run(AppId::Mark, desk::open);
}
