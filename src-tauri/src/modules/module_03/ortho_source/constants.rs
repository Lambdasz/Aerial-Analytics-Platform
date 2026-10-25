//! Konstanta terpusat untuk batas ukuran citra dan pratinjau Module 3.

/// Batas maksimal ukuran berkas citra orthomosaic JPG/PNG: 50 MB (52.428.800 bytes).
pub const MAX_IMAGE_FILE_SIZE_BYTES: u64 = 50 * 1024 * 1024;

/// Batas maksimal dimensi citra per sisi: 16.384 piksel.
pub const MAX_IMAGE_DIMENSION_PX: u32 = 16_384;

/// Target resolusi maksimal untuk pratinjau display.jpg (sisi terpanjang): 2.048 piksel.
pub const PREVIEW_TARGET_DIMENSION_PX: u32 = 2_048;

/// Batas maksimal ukuran berkas pratinjau display (display.jpg/display.png): 5 MB (5.242.880 bytes).
pub const MAX_DISPLAY_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024;
