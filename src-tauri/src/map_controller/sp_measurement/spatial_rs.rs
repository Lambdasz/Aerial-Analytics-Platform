//! # Spatial Result Dispatcher
//!
//! Module ini bertanggung jawab untuk memvalidasi dan mengirimkan data hasil
//! analisis spasial dari backend (seperti Modul 4/7/8) ke frontend melalui event Tauri.
#![allow(dead_code)]

// ... sisa kodenya (misal: use crate::map_controller...)
use crate::map_controller::sp_measurement::validation_input::MapControllerError;
use crate::map_controller::types::sp_measurement_type::LayerPayload;
use crate::map_controller::types::sp_measurement_type::MapLayerCommand;
use crate::map_controller::types::sp_measurement_type::SPATIAL_RESULT_EVENT;

use tauri::{AppHandle, Emitter};

/// Memvalidasi dan mengirimkan payload layer spasial ke frontend.
///
/// Fungsi ini akan mengeksekusi validasi geometri secara hierarkis pada `layer_payload`.
/// Jika data dinyatakan valid, fungsi akan membungkusnya ke dalam [`MapLayerCommand`]
/// dan memancarkannya sebagai event Tauri. Frontend yang mendengarkan event
/// [`SPATIAL_RESULT_EVENT`] akan menangkap data ini untuk dirender di atas peta Leaflet.
///
/// # Arguments
///
/// * `app_handle` - Referensi ke state aplikasi Tauri, digunakan untuk memancarkan event.
/// * `layer_payload` - Data berisi preferensi tampilan layer dan daftar hasil spasial.
///
/// # Errors
///
/// Mengembalikan [`MapControllerError`] dalam kondisi berikut:
/// * `InvalidGeometry`: Jika validasi bentuk koordinat gagal (mis. Polygon tidak tertutup).
/// * `InvalidGeometry`: Jika pengiriman event Tauri ke frontend gagal dieksekusi.
pub fn dispatch_spatial_result(
    app_handle: &AppHandle,
    layer_payload: LayerPayload,
) -> Result<(), MapControllerError> {
    layer_payload.validate()?;

    let command = MapLayerCommand {
        action: "load_spatial_result".to_string(),
        payload: layer_payload,
    };

    app_handle
        .emit(SPATIAL_RESULT_EVENT, command)
        .map_err(|e| MapControllerError::InvalidGeometry(format!("Gagal emit event: {}", e)))?;

    Ok(())
}

/// Kumpulan fungsi dummy pendukung testing dan development.
///
/// Module ini menyediakan data *mockup* untuk mempercepat pengembangan UI di frontend
/// tanpa perlu menunggu integrasi penuh dari plugin analitik di backend. Alur produksi
/// sesungguhnya tidak boleh bergantung pada command di dalam module ini.
pub mod dummy {
    use super::MapLayerCommand;

    /// Memuat dan mem-parsing data dummy hasil spasial yang di-embed saat *compile time*.
    ///
    /// Dipanggil langsung oleh frontend melalui Tauri invoke `"get_dummy_spatial_layers"`.
    /// Data diambil dari file `dummy_sp_result.json`.
    ///
    /// # Errors
    ///
    /// Mengembalikan `Err(String)` berisi pesan error jika struktur di dalam
    /// file JSON rusak atau tidak sesuai dengan skema [`MapLayerCommand`].
    #[tauri::command]
    pub fn get_dummy_spatial_layers() -> Result<Vec<MapLayerCommand>, String> {
        const DUMMY_JSON: &str = include_str!("dummy_sp_result.json");
        serde_json::from_str(DUMMY_JSON)
            .map_err(|e| format!("Gagal parsing dummy_spatial_results.json: {e}"))
    }
}
