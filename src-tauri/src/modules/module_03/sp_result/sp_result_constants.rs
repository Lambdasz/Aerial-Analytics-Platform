//! Berkas ini berisi konstanta terpusat untuk batasan ukuran hasil analitik spasial.
//!
//! Konstanta yang didefinisikan dalam berkas ini:
//! - `MAX_RESULT_PHOTO_SIZE_BYTES`: Batas maksimal ukuran foto hasil pengukuran (10 MB).
//! - `MAX_RESULT_JSON_SIZE_BYTES`: Batas maksimal ukuran berkas JSON result.json (64 KB).

/// Batas maksimal ukuran berkas foto hasil pengukuran: 10 MB (10.485.760 bytes).
pub const MAX_RESULT_PHOTO_SIZE_BYTES: usize = 10 * 1024 * 1024;

/// Batas maksimal ukuran berkas JSON result.json: 64 KB (65.536 bytes).
pub const MAX_RESULT_JSON_SIZE_BYTES: usize = 64 * 1024;
