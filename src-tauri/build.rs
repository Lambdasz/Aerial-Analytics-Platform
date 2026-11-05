//! Tauri build script — generates the code required by the Tauri runtime.

fn main() {
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let syslibs = manifest_dir.join("../target/syslibs");
    if syslibs.is_dir() {
        println!("cargo:rustc-link-search=native={}", syslibs.display());
    }
    tauri_build::build()
}
