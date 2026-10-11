pub mod commands;
pub mod i18n;
mod opening;
mod run;
pub mod settings;
mod shell;
pub mod tasks;
mod theme;
pub mod updates;
pub mod watching;

pub use opening::{launch_files, on_open};
pub use run::run;
pub use shell::{Root, Shell, blank, document_root, root, set_document_menus, tool_root};
pub use tasks::{LongTask, TaskState};
