use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=data/com.github.unrud.VideoDownloader.gschema.xml");
    println!("cargo:rerun-if-changed=po");
    println!("cargo:rerun-if-changed=src/window/view.rs");

    let out_dir = std::env::var("OUT_DIR").unwrap();

    // --- GSettings schema compilation ---
    let schema_dir = PathBuf::from(&out_dir).join("schemas");
    fs::create_dir_all(&schema_dir).unwrap();
    fs::copy(
        "data/com.github.unrud.VideoDownloader.gschema.xml",
        schema_dir.join("com.github.unrud.VideoDownloader.gschema.xml"),
    )
    .unwrap();
    let status = Command::new("glib-compile-schemas")
        .arg(&schema_dir)
        .status()
        .unwrap();
    assert!(status.success(), "glib-compile-schemas failed");
    println!("cargo:rustc-env=SCHEMA_DIR={}", schema_dir.display());

    // --- Gettext .po -> .mo compilation ---
    let locale_dir = PathBuf::from(&out_dir).join("locale");
    fs::create_dir_all(&locale_dir).unwrap();
    if std::path::Path::new("po").exists() {
        for entry in fs::read_dir("po").unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("po") {
                continue;
            }
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            if stem == "video-downloader" {
                continue; // template, skip
            }
            let lang_dir = locale_dir.join(&stem).join("LC_MESSAGES");
            fs::create_dir_all(&lang_dir).unwrap();
            let mo = lang_dir.join("video-downloader.mo");
            let status = Command::new("msgfmt")
                .arg("-o")
                .arg(&mo)
                .arg(&path)
                .status();
            // msgfmt failure is non-fatal (translation file may be fuzzy).
            let _ = status;
        }
    }
    println!("cargo:rustc-env=LOCALEDIR={}", locale_dir.display());
}
