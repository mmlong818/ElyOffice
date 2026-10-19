//! Each app's build script: its icon as resource 1 on Windows.

use std::{env, fs, path::PathBuf};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let app = env::var("CARGO_PKG_NAME").expect("cargo names the package");
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo names the folder"));
    let icon = root.join("../../assets/icons").join(format!("{app}.ico"));
    let icon =
        fs::canonicalize(&icon).unwrap_or_else(|error| panic!("{}: {error}", icon.display()));
    println!("cargo:rerun-if-changed={}", icon.display());
    // gpui's window icon is resource 1.
    let rc = PathBuf::from(env::var("OUT_DIR").expect("cargo gives OUT_DIR")).join("icon.rc");
    let path = icon
        .to_str()
        .expect("a UTF-8 path")
        .trim_start_matches(r"\\?\")
        .replace('\\', "/");
    fs::write(&rc, format!("1 ICON \"{path}\"\n")).expect("the resource script is written");
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_optional()
        .expect("the icon resource compiles");
}
