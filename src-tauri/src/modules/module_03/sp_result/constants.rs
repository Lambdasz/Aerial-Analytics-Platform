//! Konstanta terpusat untuk hasil pengukuran spasial (sp_result) Module 3.

/// Batas maksimal ukuran berkas foto hasil pengukuran: 10 MB (10.485.760 bytes).
pub const MAX_RESULT_PHOTO_SIZE_BYTES: usize = 10 * 1024 * 1024;

/// Batas maksimal ukuran berkas JSON result.json: 64 KB (65.536 bytes).
pub const MAX_RESULT_JSON_SIZE_BYTES: usize = 64 * 1024;
