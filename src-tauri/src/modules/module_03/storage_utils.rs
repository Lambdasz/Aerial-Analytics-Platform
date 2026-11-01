//! Utilitas umum untuk penyimpanan atomic map_storage.

use std::fs;
use std::path::{Path, PathBuf};

/// Mengembalikan path root penyimpanan Modul 3.
///
/// Hierarki resolusi:
/// 1. Environment variable `AAP_STORAGE_DIR` (prioritas utama untuk testing / custom).
/// 2. Folder `map_storage` pada current dir atau parent dir.
/// 3. Default fallback ke `./map_storage`.
pub fn get_storage_root() -> PathBuf {
    if let Ok(custom_dir) = std::env::var("AAP_STORAGE_DIR") {
        let path = PathBuf::from(custom_dir);
        let _ = fs::create_dir_all(&path);
        return path;
    }

    // Jika berjalan dari dalam subfolder `src-tauri` (mis. saat `cargo run` / `tauri dev`),
    // selalu gunakan folder `map_storage` di root workspace (`../map_storage`).
    if Path::new("Cargo.toml").exists() && Path::new("../package.json").exists() {
        let path = PathBuf::from("../map_storage");
        let _ = fs::create_dir_all(&path);
        return path;
    }

    // Jika berjalan dari root workspace (mis. testing atau build script)
    if Path::new("package.json").exists() {
        let path = PathBuf::from("map_storage");
        let _ = fs::create_dir_all(&path);
        return path;
    }

    if Path::new("map_storage").is_dir() {
        PathBuf::from("map_storage")
    } else if Path::new("../map_storage").is_dir() {
        PathBuf::from("../map_storage")
    } else if Path::new("storage").is_dir() {
        PathBuf::from("storage")
    } else if Path::new("../storage").is_dir() {
        PathBuf::from("../storage")
    } else {
        let path = PathBuf::from("map_storage");
        let _ = fs::create_dir_all(&path);
        path
    }
}

/// Sanitasi string ID atau nama file agar aman digunakan pada filesystem.
pub fn sanitize_filename(id: &str) -> String {
    let sanitized: String = id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.trim().is_empty() {
        format!(
            "item_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        )
    } else {
        sanitized
    }
}

/// Menulis konten ke file secara atomik (.tmp lalu rename).
pub fn write_atomic(target_path: &Path, content: &str) -> Result<(), std::io::Error> {
    write_atomic_bytes(target_path, content.as_bytes())
}

/// Menulis bytes ke file secara atomik (.tmp lalu rename).
pub fn write_atomic_bytes(target_path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_path = target_path.with_extension(format!(
        "{}.tmp",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    fs::write(&temp_path, bytes)?;
    fs::rename(&temp_path, target_path)?;
    Ok(())
}
