#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod analysis;
mod bars;
mod book;
mod cells;
mod data;
mod dialog;
mod figures;
mod format;
mod formulas;
mod grid;
mod output;
mod page;
mod protect;
mod rules;
mod sheets;
mod template;
mod text;

use ely_platform::AppId;

fn main() {
    ely_ui::run(AppId::Table, book::open);
}

#[cfg(test)]
mod tests;
