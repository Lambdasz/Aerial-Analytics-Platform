//! # Spatial Validation and Error Handling
//!
//! File/module ini menyediakan implementasi pengecekan validitas tipe data
//! spasial sebelum dikirimkan ke frontend, serta definisi tipe error kustom.
#![allow(dead_code)]

use crate::map_controller::types::sp_measurement_type::LayerDisplayMode;
use crate::map_controller::types::sp_measurement_type::LayerPayload;
use crate::map_controller::types::sp_measurement_type::SpatialGeometry;
use crate::map_controller::types::sp_measurement_type::SpatialResult;
use thiserror::Error;

/// Tipe error kustom yang menampung segala kemungkinan kegagalan di dalam `map_controller`.
#[derive(Error, Debug)]
pub enum MapControllerError {
    /// Error yang terjadi ketika struktur data geometri (Point, Polygon)
    /// tidak memenuhi standar spesifikasi GeoJSON yang diwajibkan.
    #[error("geometri tidak valid: {0}")]
    InvalidGeometry(String),

    /// Error bawaan Rust yang terjadi saat ada kegagalan I/O (Input/Output).
    #[error("gagal memproses I/O: {0}")]
    Io(#[from] std::io::Error),
}

impl MapControllerError {
    /// Mengembalikan representasi kode string dari error.
    ///
    /// Kode ini sangat berguna saat dipetakan ke `CommandError` Tauri, sehingga
    /// kode frontend TypeScript bisa menggunakan `switch (error.code)` untuk
    /// menampilkan pesan spesifik ke pengguna.
    pub fn code(&self) -> String {
        match self {
            Self::InvalidGeometry(_) => "INVALID_GEOMETRY".to_string(),
            Self::Io(_) => "IO_ERROR".to_string(),
        }
    }
}

impl SpatialGeometry {
    /// Memvalidasi integritas koordinat geometri spasial.
    ///
    /// Pengecekan didasarkan pada spesifikasi standar GeoJSON:
    /// - **Point**: Wajib memiliki tepat 2 koordinat dengan urutan `[longitude, latitude]`.
    /// - **Polygon**: Wajib memiliki minimal satu *linear ring*. Tiap *ring* harus:
    ///   - Terdiri dari minimal 4 titik.
    ///   - Tertutup sempurna (titik pertama ekuivalen secara identik dengan titik terakhir).
    ///   - Tiap titik koordinat wajib memiliki ukuran `[longitude, latitude]`.
    ///
    /// # Errors
    ///
    /// Mengembalikan `Err(MapControllerError::InvalidGeometry)` berisi deskripsi detail
    /// ring/titik mana yang menyebabkan validasi gagal.
    pub fn validate(&self) -> Result<(), MapControllerError> {
        match self {
            SpatialGeometry::Point { coordinates } => {
                if coordinates.len() != 2 {
                    return Err(MapControllerError::InvalidGeometry(format!(
                        "Geometri Point membutuhkan tepat 2 koordinat [lon, lat], tapi mendapat {}",
                        coordinates.len()
                    )));
                }
            }
            SpatialGeometry::Polygon { coordinates } => {
                if coordinates.is_empty() {
                    return Err(MapControllerError::InvalidGeometry(
                        "Geometri Polygon kosong (tidak memiliki ring koordinat)".to_string(),
                    ));
                }

                for (ring_idx, ring) in coordinates.iter().enumerate() {
                    let ring: &Vec<Vec<f64>> = ring;
                    if ring.len() < 4 {
                        return Err(MapControllerError::InvalidGeometry(
                            format!("Ring {} pada Polygon minimal butuh 4 titik (termasuk titik penutup), mendapat {}", ring_idx, ring.len())
                        ));
                    }

                    let first_point = &ring[0];
                    let last_point = ring.last().unwrap();
                    if first_point != last_point {
                        return Err(MapControllerError::InvalidGeometry(
                            format!("Ring {} pada Polygon tidak tertutup (titik pertama {:?} tidak sama dengan titik terakhir {:?})", 
                            ring_idx, first_point, last_point)
                        ));
                    }

                    for (point_idx, point) in ring.iter().enumerate() {
                        if point.len() != 2 {
                            return Err(MapControllerError::InvalidGeometry(format!(
                                "Titik ke-{} pada ring {} membutuhkan tepat 2 koordinat [lon, lat]",
                                point_idx, ring_idx
                            )));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl SpatialResult {
    /// Mengeksekusi validasi pada satu objek hasil spasial.
    ///
    /// Fungsi ini memastikan bahwa komponen `geometry` di dalam hasil ini valid.
    /// Tambahkan validasi `properties` (mis. rentang *confidence*) ke dalam
    /// fungsi ini jika di masa depan diperlukan.
    pub fn validate(&self) -> Result<(), MapControllerError> {
        self.geometry.validate()?;
        Ok(())
    }
}

impl LayerPayload {
    /// Memvalidasi seluruh konfigurasi dan data sebelum diteruskan ke frontend.
    ///
    /// Alur pengecekan:
    /// 1. Menguji preferensi tampilan (`display_preference`). Jika menggunakan
    ///    `ImageOverlay`, nilai `opacity` harus aman (antara `0.0` sampai `1.0`).
    /// 2. Melakukan iterasi mendalam ke seluruh isi `spatial_results` untuk
    ///    memverifikasi geometri GeoJSON di dalamnya.
    ///
    /// # Errors
    ///
    /// Mengembalikan error pada validasi pertama yang gagal (fail-fast), sehingga
    /// payload yang cacat tidak akan pernah mencapai proses *emit* event.
    pub fn validate(&self) -> Result<(), MapControllerError> {
        if let LayerDisplayMode::ImageOverlay { opacity, .. } = &self.display_preference {
            if *opacity < 0.0 || *opacity > 1.0 {
                return Err(MapControllerError::InvalidGeometry(
                    "Opacity ImageOverlay harus di antara 0.0 hingga 1.0".to_string(),
                ));
            }
        }

        for result in &self.spatial_results {
            let result: &SpatialResult = result;
            result.validate()?;
        }
        Ok(())
    }
}
