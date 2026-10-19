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
pub mod wheel;

pub use opening::{launch_files, on_open};
#[cfg(any(test, feature = "test-support"))]
pub use opening::install_no_launch_files;
pub use run::run;
pub use shell::{Root, Shell, blank, document_root, root, set_document_menus, tool_root};
pub use tasks::{LongTask, TaskState};
