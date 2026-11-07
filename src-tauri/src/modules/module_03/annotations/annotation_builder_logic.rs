//! Berkas ini berisi logika fungsional murni pembentukan objek catatan anotasi peta (zero I/O).
//!
//! Komponen dan fungsi yang didefinisikan dalam berkas ini:
//! - `create_annotation`: Membentuk struct AnnotationFeature secara deterministik tanpa efek samping.
//!
//! ---
//! ### CATATAN ARSITEKTURAL & PANDUAN PENGEMBANGAN MENDATANG:
//!
//! 1. **Kondisi Implementasi Saat Ini (Frontend-Heavy):**
//!    Saat ini, fitur anotasi dikelola secara dominan di sisi frontend (`src/modules/module-03/`):
//!    - Perhitungan estimasi luas poligon (`calculateGeodesicAreaMeters`) dan panjang garis (`calculateLineLengthMeters`)
//!      dihitung secara dinamis (*on-the-fly*) di frontend (`geoJson.ts`), bukan disimpan di backend.
//!    - Validasi koordinat serta interaksi kanvas peta juga dilakukan di frontend (`useAnnotationModalForm.ts`).
//!    - Backend Rust pada submodul ini saat ini berperan murni sebagai lapisan persistensi disk (*storage layer*)
//!      ke berkas `map_storage/annotations/{id}.json`. Oleh karena itu, fungsi `create_annotation` di berkas ini
//!      saat ini hanya bertindak sebagai konstruktor/pabrik struct sederhana.
//!
//! 2. **Rekomendasi untuk Programmer Mendatang:**
//!    - **Opsi A (Penguatan Validasi & Perhitungan Otoritatif di Backend):**
//!      Jika ingin menyelaraskan arsitektur anotasi agar setara dengan AOI (`aoi_builder_logic` / `spherical_excess_logic`):
//!      - Tambahkan validasi koordinat WGS84 sebelum objek dibentuk (`validate_coordinate` / `validate_polygon`).
//!      - Lakukan perhitungan metrik geodesik resmi di backend menggunakan `crate::modules::module_03::geometry::calculate_area`
//!        dan simpan hasilnya ke dalam field properti entitas anotasi.
//!      - Tambahkan logika relasional otomatis (misal: mendeteksi secara spasial apakah koordinat anotasi beririsan
//!        dengan citra ortomosaik aktif untuk mengisi field `image_id`).
//!    - **Opsi B (Refactoring / Penyederhanaan Idiomatik Rust):**
//!      Jika backend tetap dipertahankan murni sebagai *dumb storage layer*, berkas `annotation_builder_logic.rs` ini
//!      sebaiknya dilebur dengan memindahkan fungsi ini menjadi *associated function* (`AnnotationFeature::new(...)`)
//!      langsung di dalam `annotation_entity_struct.rs`, guna mengeliminasi lapisan abstraksi berlebih (*over-engineering*).

use super::annotation_entity_struct::AnnotationFeature;
use crate::modules::module_03::types::geometry_type::SpatialGeometry;

/// Membentuk data anotasi secara fungsional murni.
pub fn create_annotation(
    id: String,
    text: String,
    geometry: SpatialGeometry,
    created_at: String,
) -> AnnotationFeature {
    AnnotationFeature {
        id,
        schema_version: 1,
        text,
        geometry,
        created_at,
        image_id: None,
    }
}
