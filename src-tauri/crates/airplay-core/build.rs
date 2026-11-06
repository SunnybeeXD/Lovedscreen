use std::path::{Path, PathBuf};
use std::process::Command;

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        if p.extension().is_some_and(|e| e == "c") {
            out.push(p);
        }
    }
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), to).unwrap();
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let upstream = manifest.join("../../../vendor/UxPlay/lib");
    let patches = manifest.join("patches");
    println!("cargo:rerun-if-changed={}", patches.display());
    println!("cargo:rerun-if-changed={}", upstream.display());
    println!("cargo:rerun-if-changed=build.rs");

    let lib = out.join("uxplay-lib");
    if lib.exists() {
        std::fs::remove_dir_all(&lib).unwrap();
    }
    copy_dir(&upstream, &lib);
    let mut patch_files: Vec<_> = std::fs::read_dir(&patches)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "patch"))
        .collect();
    patch_files.sort();
    for p in patch_files {
        let status = Command::new("git")
            .args(["apply", "--whitespace=nowarn"])
            .arg(&p)
            .current_dir(&lib)
            .status()
            .expect("git is required to apply patches");
        assert!(status.success(), "patch failed: {}", p.display());
    }

    let windows = std::env::var("CARGO_CFG_WINDOWS").is_ok();
    let plist = pkg_config::Config::new()
        .atleast_version("2.0")
        .statik(windows)
        .probe("libplist-2.0")
        .unwrap();
    let openssl = pkg_config::Config::new()
        .atleast_version("1.1.1")
        .statik(windows)
        .probe("openssl")
        .unwrap();

    let mut files = Vec::new();
    collect(&lib, &mut files);
    collect(&lib.join("playfair"), &mut files);
    collect(&lib.join("llhttp"), &mut files);
    collect(&lib.join("mdnsd"), &mut files);

    let mut b = cc::Build::new();
    b.files(&files)
        .include(&lib)
        .include(lib.join("playfair"))
        .include(lib.join("llhttp"))
        .opt_level_str("s")
        .pic(true)
        .warnings(false)
        .define("STANDALONE", None)
        .define("__STDC_CONSTANT_MACROS", None)
        .define("__STDC_LIMIT_MACROS", None)
        .define("TARGET_POSIX", None)
        .define("_REENTRANT", None)
        .define("_LARGEFILE64_SOURCE", None)
        .define("_FILE_OFFSET_BITS", "64")
        .define("PIC", None)
        .define("NOHOLD", None)
        .define("OPENSSL_API_COMPAT", "0x10101000L");
    if windows {
        b.define("_WIN32", None);
    } else {
        b.define("_LINUX", None);
    }
    let mut parts = plist.version.split('.').map(|x| x.parse::<u32>().unwrap_or(0));
    let pv = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    if pv >= (2, 1) {
        b.define("PLIST_210", None);
    }
    if pv >= (2, 3) {
        b.define("PLIST_230", None);
    }
    for p in plist.include_paths.iter().chain(openssl.include_paths.iter()) {
        b.include(p);
    }
    b.compile("uxplay");

    if windows {
        for l in ["wsock32", "iphlpapi", "ws2_32"] {
            println!("cargo:rustc-link-lib={l}");
        }
    } else {
        println!("cargo:rustc-link-lib=pthread");
    }
}
