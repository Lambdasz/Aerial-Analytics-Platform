//! Konstanta terpusat untuk batas ukuran citra, streaming TIFF, dan pratinjau Module 3.

/// Batas maksimal ukuran berkas citra Opsi A (JPG/PNG): 50 MB (52.428.800 bytes).
pub const MAX_IMAGE_FILE_SIZE_BYTES: u64 = 50 * 1024 * 1024;

/// Batas maksimal ukuran berkas citra Opsi B (TIFF/GeoTIFF): 250 MB (262.144.000 bytes).
pub const MAX_TIFF_FILE_SIZE_BYTES: u64 = 250 * 1024 * 1024;

/// Batas maksimal ukuran per chunk/strip TIFF uncompressed untuk proteksi memori: 30 MB.
pub const MAX_TIFF_STRIP_SIZE_BYTES: u64 = 30 * 1024 * 1024;

/// Batas total megapiksel citra Opsi A (144 Megapiksel).
pub const MAX_OPTION_A_PIXELS: u64 = 144_000_000;

/// Batas total megapiksel citra Opsi B (500 Megapiksel).
pub const MAX_OPTION_B_PIXELS: u64 = 500_000_000;

/// Batas maksimal dimensi citra Opsi A (JPG/PNG) per sisi: 16.384 piksel.
pub const MAX_IMAGE_DIMENSION_PX: u32 = 16_384;

/// Batas maksimal resolusi JPEG progressive: 40 Megapiksel (40.000.000 piksel).
pub const MAX_PROGRESSIVE_JPEG_PIXELS: u64 = 40_000_000;

/// Batas maksimal dimensi citra Opsi B (TIFF/GeoTIFF) per sisi: 25.000 piksel.
pub const MAX_TIFF_DIMENSION_PX: u32 = 25_000;

/// Target resolusi maksimal untuk display (sisi terpanjang): 4.096 piksel.
pub const DISPLAY_TARGET_DIMENSION_PX: u32 = 4_096;

/// Alias untuk kompatibilitas ke belakang.
pub const PREVIEW_TARGET_DIMENSION_PX: u32 = DISPLAY_TARGET_DIMENSION_PX;

/// Batas resolusi minimal saat adaptasi kompresi display: 1.024 piksel.
pub const DISPLAY_MIN_DIMENSION_PX: u32 = 1_024;

/// Batas maksimal ukuran berkas display (display.jpg/display.png): 5 MB (5.242.880 bytes).
pub const MAX_DISPLAY_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024;

/// Jumlah maksimal riwayat job yang disimpan di map_storage/.jobs/
pub const MAX_JOB_HISTORY_RETENTION: usize = 20;
