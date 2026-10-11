// Release builds open no console on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use ely_platform::AppId;

fn main() {
    ely_ui::run(AppId::Mail, ely_ui::blank);
}
