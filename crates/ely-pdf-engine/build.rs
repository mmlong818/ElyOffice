//! Builds the MuPDF shim into the worker binary only.

use std::path::{Path, PathBuf};

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo")).join("../..");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let mupdf = std::env::var_os("ELY_MUPDF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("engines/mupdf-build"));
    let mupdf_include = std::env::var_os("ELY_MUPDF_INCLUDE")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("reference/mupdf/include"));

    // One stamp for client and worker; stale workers are refused.
    let mut stamp: u64 = 0xcbf2_9ce4_8422_2325;
    for source in [
        "shim/ely_mupdf.c", "shim/ely_pages.c", "shim/ely_insert.c", "shim/ely_text.c",
        "shim/ely_write.c", "shim/ely_pen.c", "shim/ely_place.c", "shim/ely_annot.c",
        "shim/ely_form.c", "shim/ely_ocr.c", "shim/ely_outline.c", "shim/ely_meta.c",
        "shim/ely_redact.c", "shim/ely_alt.c", "shim/ely_clean.c", "shim/ely_secure.c",
        "shim/ely_sign.c", "shim/ely_output.c", "shim/ely_shim.h", "src/proto.rs",
        "src/bin/ely-pdf-worker/main.rs", "src/bin/ely-pdf-worker/ffi.rs",
        "src/bin/ely-pdf-worker/edit.rs", "src/bin/ely-pdf-worker/content.rs", "src/edit.rs",
        "src/content.rs", "src/marks.rs", "src/bin/ely-pdf-worker/marks.rs",
        "src/bin/ely-pdf-worker/ocr.rs", "src/bin/ely-pdf-worker/structure.rs",
        "src/bin/ely-pdf-worker/redact.rs", "src/redact.rs", "src/structure.rs", "src/ocr.rs",
        "src/bin/ely-pdf-worker/secure.rs", "src/secure.rs", "src/bin/ely-pdf-worker/sign.rs",
        "src/sign.rs", "src/bin/ely-pdf-worker/output.rs", "src/output.rs", "../ely-ipc/src/lib.rs",
    ] {
        println!("cargo:rerun-if-changed={source}");
        for byte in std::fs::read(source).expect("a source file") {
            stamp = (stamp ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    println!("cargo:rustc-env=ELY_PDF_BUILD={stamp:016x}");
    for name in ["ELY_MUPDF_DIR", "ELY_MUPDF_INCLUDE", "ELY_OPENSSL_DIR", "ELY_OPENSSL_INCLUDE", "ELY_OPENSSL_LIB_DIR"] {
        println!("cargo:rerun-if-env-changed={name}");
    }

    let libraries: Vec<&str> = if windows && mupdf.join("libthirdparty.lib").is_file() {
        vec![
            "libmupdf.lib", "libthirdparty.lib", "libresources.lib", "libextract.lib",
            "libharfbuzz.lib", "libmubarcode.lib", "libzxing.lib", "libpkcs7.lib",
            "libtesseract.lib", "libleptonica.lib",
        ]
    } else if windows {
        // Upstream's vcpkg/MSVC build packages third parties into libmupdf.lib.
        vec!["libmupdf.lib"]
    } else {
        vec!["libmupdf-pkcs7.a", "libmupdf.a", "libmupdf-third.a"]
    };
    for lib in &libraries {
        println!("cargo:rerun-if-changed={}", mupdf.join(lib).display());
        assert!(mupdf.join(lib).is_file(), "no {lib} in {} (run scripts/build-mupdf.sh or set ELY_MUPDF_DIR)", mupdf.display());
    }

    let openssl_include = openssl_include(&root, windows);
    let mut shim = cc::Build::new();
    for source in [
        "shim/ely_mupdf.c", "shim/ely_pages.c", "shim/ely_insert.c", "shim/ely_text.c",
        "shim/ely_write.c", "shim/ely_place.c", "shim/ely_annot.c", "shim/ely_form.c",
        "shim/ely_ocr.c", "shim/ely_outline.c", "shim/ely_meta.c", "shim/ely_redact.c",
        "shim/ely_alt.c", "shim/ely_clean.c", "shim/ely_secure.c", "shim/ely_sign.c",
        "shim/ely_output.c",
    ] {
        shim.file(source);
    }
    shim.include(&mupdf_include).include(openssl_include).warnings(true).cargo_metadata(false);
    if windows {
        shim.flag("/std:c11").static_crt(true);
    }
    shim.compile("ely_mupdf");

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("set by cargo"));
    let bin = "ely-pdf-worker";
    let shim_lib = if windows { "ely_mupdf.lib" } else { "libely_mupdf.a" };
    println!("cargo:rustc-link-arg-bin={bin}={}", out.join(shim_lib).display());
    for lib in &libraries {
        println!("cargo:rustc-link-arg-bin={bin}={}", mupdf.join(lib).display());
    }
    let crypto = if windows { "libcrypto.lib" } else { "libcrypto.a" };
    let openssl_lib = openssl_lib_dir(&root, windows);
    println!("cargo:rustc-link-arg-bin={bin}={}", openssl_lib.join(crypto).display());
    if windows {
        for lib in ["bcrypt.lib", "crypt32.lib", "advapi32.lib", "ws2_32.lib", "user32.lib"] {
            println!("cargo:rustc-link-arg-bin={bin}={lib}");
        }
        return;
    }
    let needs = std::process::Command::new("pkg-config")
        .args(["--static", "--libs-only-l", "libcrypto"])
        .output().expect("pkg-config, to find OpenSSL");
    assert!(needs.status.success(), "pkg-config knows no static OpenSSL");
    for lib in String::from_utf8(needs.stdout).expect("UTF-8 flags").split_whitespace().filter(|lib| *lib != "-lcrypto") {
        println!("cargo:rustc-link-arg-bin={bin}={lib}");
    }
    println!("cargo:rustc-link-arg-bin={bin}=-lm");
    let cxx = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") { "-lc++" } else { "-lstdc++" };
    println!("cargo:rustc-link-arg-bin={bin}={cxx}");
}

fn openssl_include(root: &Path, windows: bool) -> PathBuf {
    if let Some(path) = std::env::var_os("ELY_OPENSSL_INCLUDE") {
        return PathBuf::from(path);
    }
    if windows {
        return std::env::var_os("ELY_OPENSSL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("engines/vcpkg/installed/x64-windows-static"))
            .join("include");
    }
    PathBuf::from(openssl_pkg_config("includedir"))
}

fn openssl_lib_dir(root: &Path, windows: bool) -> PathBuf {
    if let Some(path) = std::env::var_os("ELY_OPENSSL_LIB_DIR") {
        return PathBuf::from(path);
    }
    if windows {
        return std::env::var_os("ELY_OPENSSL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("engines/vcpkg/installed/x64-windows-static"))
            .join("lib");
    }
    PathBuf::from(openssl_pkg_config("libdir"))
}

fn openssl_pkg_config(variable: &str) -> String {
    let out = std::process::Command::new("pkg-config")
        .args([&format!("--variable={variable}"), "libcrypto"])
        .output().expect("pkg-config, to find OpenSSL");
    assert!(out.status.success(), "pkg-config knows no libcrypto");
    String::from_utf8(out.stdout).expect("a UTF-8 path").trim().to_string()
}
