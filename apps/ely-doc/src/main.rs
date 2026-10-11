// Release builds open no console on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod actions;
mod chrome;
mod compare;
mod docs;
mod files;
mod insert;
mod merge;
mod navigate;
mod output;
mod props;
mod refs;
mod review;
mod spell;

use ely_platform::AppId;

fn main() {
    ely_ui::run(AppId::Doc, docs::open);
}

#[cfg(test)]
mod tests;
