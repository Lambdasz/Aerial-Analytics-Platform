//! Plot Analytics Orchestration & Sliced Metric Reducer (Module 9).
//!
//! Bertanggung jawab mengatur eksekusi analisis geospasial untuk kumpulan Plot
//! dari citra Orthomosaic (`map_storage`), mengeksekusi plugin per slice secara sekuensial,
//! serta mereduksi dan mengagregasikan metrik antar-slice ke tabel `plot_indicator`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Emitter;

use crate::modules::module_03::ortho_source::clipper::clip_orthomosaic_for_plot;
use crate::modules::module_03::ortho_source::store::get_ortho_image;
use crate::modules::module_09::indicators::{normalise_indicators, save_indicators};
use crate::modules::module_09::plots::{get_plot, polygon_from_geometry};
use crate::plugin_manager::error::CommandError;
use crate::plugin_manager::executor::{
    start_plugin_job, ActiveJobTracker, JobStatusDto, StartJobRequestDto,
};
use crate::plugin_manager::models::StandardJobResultDto;

/// Parameter request analisis plot berbasis orthomosaic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysePlotsOrthoPayload {
    pub ortho_id: String,
    pub plot_ids: Vec<String>,
    pub plugin_id: String,
    #[serde(default)]
    pub parameters: HashMap<String, Value>,
}

/// Hasil ringkasan akhir dari eksekusi analisis plot orthomosaic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotAnalysisOutcome {
    pub successful_plots: usize,
    pub failed_plots: Vec<String>,
    pub total_slices_processed: usize,
}

/// Event progres analisis plot yang dikirim ke UI via channel `plot-analysis-progress`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotAnalysisProgressEvent {
    pub stage: String,
    pub plot_index: usize,
    pub total_plots: usize,
    pub plot_name: String,
    pub slice_index: usize,
    pub total_slices: usize,
    pub percent: f64,
    pub message: String,
}

// ============================================================================
// 1. FUNCTIONAL CORE (Pure Functions, Zero I/O)
// ============================================================================

/// Pure: Menggabungkan (*aggregate/reduce*) hasil metrik dari beberapa slice menjadi satu metrik plot utuh.
///
/// **Aturan Agregasi**:
/// 1. Jika slice tunggal (`slice_metrics.len() == 1`), metrik dikembalikan langsung tanpa modifikasi.
/// 2. Metrik bertipe count/piksel (misal `sampled_pixels`, `vegetation_pixels`, `tree_count`) dijumlahkan (`sum`).
/// 3. Metrik bertipe rasio/persentase (misal `coverage_percentage`, `vegetation_coverage_pct`)
///    dihitung sebagai rata-rata terbobot (*weighted average*) menggunakan `sampled_pixels` atau `total_pixels` jika tersedia.
/// 4. Jika jumlah piksel vegetasi dan sampel diketahui, persentase coverage dihitung ulang secara deterministik.
pub fn aggregate_slice_metrics(slice_metrics: &[Value]) -> Value {
    if slice_metrics.is_empty() {
        return json!({});
    }
    if slice_metrics.len() == 1 {
        return slice_metrics[0].clone();
    }

    // Kumpulkan semua keys unik
    let mut all_keys = Vec::new();
    for metric_val in slice_metrics {
        if let Some(map) = metric_val.as_object() {
            for key in map.keys() {
                if !all_keys.contains(key) {
                    all_keys.push(key.clone());
                }
            }
        }
    }

    // Identifikasi apakah ada bobot piksel sampel per slice
    let mut slice_weights = Vec::with_capacity(slice_metrics.len());
    for metric_val in slice_metrics {
        let weight = metric_val
            .get("sampled_pixels")
            .or_else(|| metric_val.get("total_pixels"))
            .and_then(Value::as_f64)
            .unwrap_or(1.0)
            .max(1.0);
        slice_weights.push(weight);
    }
    let total_weight: f64 = slice_weights.iter().sum();

    let mut aggregated = serde_json::Map::new();

    for key in all_keys {
        let lower = key.to_lowercase();
        let is_count = (lower.contains("count")
            || lower.contains("pixel")
            || lower.contains("total")
            || lower.contains("trees"))
            && !lower.contains("pct")
            && !lower.contains("percent")
            && !lower.contains("ratio");

        // Kumpulkan nilai numerik
        let mut numeric_vals = Vec::new();
        let mut string_val = None;

        for metric_val in slice_metrics {
            if let Some(val) = metric_val.get(&key) {
                if let Some(n) = val.as_f64() {
                    numeric_vals.push(n);
                } else if let Some(s) = val.as_str() {
                    if string_val.is_none() && !s.is_empty() {
                        string_val = Some(s.to_string());
                    }
                }
            }
        }

        if !numeric_vals.is_empty() {
            if is_count {
                let sum: f64 = numeric_vals.iter().sum();
                aggregated.insert(key, json!(sum.round() as i64));
            } else if total_weight > 0.0 && numeric_vals.len() == slice_weights.len() {
                // Weighted average
                let mut weighted_sum = 0.0;
                for (v, w) in numeric_vals.iter().zip(slice_weights.iter()) {
                    weighted_sum += v * w;
                }
                let avg = weighted_sum / total_weight;
                aggregated.insert(key, json!((avg * 10000.0).round() / 10000.0));
            } else {
                // Simple arithmetic average
                let avg: f64 = numeric_vals.iter().sum::<f64>() / numeric_vals.len() as f64;
                aggregated.insert(key, json!((avg * 10000.0).round() / 10000.0));
            }
        } else if let Some(s) = string_val {
            aggregated.insert(key, json!(s));
        }
    }

    // Rekalibrasi persentase jika piksel vegetasi & piksel sampel tersedia
    let veg_px = aggregated.get("vegetation_pixels").and_then(Value::as_f64);
    let sample_px = aggregated
        .get("sampled_pixels")
        .or_else(|| aggregated.get("total_pixels"))
        .and_then(Value::as_f64);

    if let (Some(veg), Some(sample)) = (veg_px, sample_px) {
        if sample > 0.0 {
            let recomputed_pct = ((veg / sample) * 100.0 * 10000.0).round() / 10000.0;
            if aggregated.contains_key("coverage_percentage") {
                aggregated.insert("coverage_percentage".to_string(), json!(recomputed_pct));
            }
            if aggregated.contains_key("vegetation_coverage_pct") {
                aggregated.insert("vegetation_coverage_pct".to_string(), json!(recomputed_pct));
            }
        }
    }

    Value::Object(aggregated)
}

// ============================================================================
// 2. IMPERATIVE SHELL (Orchestration, Plugin Execution, DB Persistence)
// ============================================================================

/// Menunggu penyelesaian satu pekerjaan plugin dengan polling read-lock pada ActiveJobTracker.
pub async fn wait_for_job_completion(
    job_id: &str,
    jobs: &tokio::sync::RwLock<ActiveJobTracker>,
    poll_interval: Duration,
    timeout: Duration,
) -> Result<StandardJobResultDto, CommandError> {
    let start_time = tokio::time::Instant::now();

    loop {
        if start_time.elapsed() > timeout {
            return Err(CommandError {
                code: "TIMEOUT".to_string(),
                message: format!(
                    "Pekerjaan plugin '{}' melebihi batas waktu eksekusi",
                    job_id
                ),
            });
        }

        {
            let tracker = jobs.read().await;
            if let Some(record) = tracker.jobs.get(job_id) {
                match &record.status {
                    JobStatusDto::Completed { .. } => {
                        return record.result.clone().ok_or_else(|| CommandError {
                            code: "RESULT_MISSING".to_string(),
                            message: format!("Hasil pekerjaan '{}' tidak ditemukan", job_id),
                        });
                    }
                    JobStatusDto::Failed { error } => {
                        return Err(CommandError {
                            code: "JOB_FAILED".to_string(),
                            message: error.clone(),
                        });
                    }
                    JobStatusDto::Aborted => {
                        return Err(CommandError {
                            code: "JOB_ABORTED".to_string(),
                            message: "Pekerjaan plugin dibatalkan oleh pengguna".to_string(),
                        });
                    }
                    JobStatusDto::Running { .. } | JobStatusDto::Queued => {}
                }
            } else {
                return Err(CommandError {
                    code: "JOB_NOT_FOUND".to_string(),
                    message: format!("ID pekerjaan '{}' tidak terdaftar", job_id),
                });
            }
        }

        tokio::time::sleep(poll_interval).await;
    }
}

/// Orkestrator analisis plot langsung dari citra orthomosaic.
pub async fn analyse_plots_from_ortho_impl(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, crate::AppState>,
    payload: AnalysePlotsOrthoPayload,
) -> Result<PlotAnalysisOutcome, CommandError> {
    // 1. Verifikasi keberadaan citra orthomosaic di Modul 03
    let ortho_meta = get_ortho_image(&payload.ortho_id).map_err(|e| CommandError {
        code: "ORTHO_NOT_FOUND".to_string(),
        message: format!(
            "Citra orthomosaic '{}' tidak ditemukan: {}",
            payload.ortho_id, e
        ),
    })?;

    if ortho_meta.bounds.is_none() {
        return Err(CommandError {
            code: "MISSING_BOUNDS".to_string(),
            message: format!(
                "Citra orthomosaic '{}' tidak memiliki georeferensi bounds",
                payload.ortho_id
            ),
        });
    }

    // 2. Muat data plot dari SQLite
    let plots = {
        let conn = state.db.lock().map_err(|_| CommandError {
            code: "STATE_ERROR".to_string(),
            message: "Database lock poisoned".to_string(),
        })?;

        let mut loaded = Vec::with_capacity(payload.plot_ids.len());
        for plot_id in &payload.plot_ids {
            if let Some(plot) = get_plot(&conn, plot_id).map_err(|e| CommandError {
                code: "PLOT_ERROR".to_string(),
                message: e.to_string(),
            })? {
                loaded.push(plot);
            }
        }
        loaded
    };

    if plots.is_empty() {
        return Err(CommandError {
            code: "NO_PLOTS".to_string(),
            message: "Tidak ada plot yang valid untuk dianalisis".to_string(),
        });
    }

    // 3. Buat folder staging sementara RAII untuk menampung potongan slice
    let staging_temp = tempfile::Builder::new()
        .prefix("ortho_plots_")
        .tempdir()
        .map_err(|e| CommandError {
            code: "IO_ERROR".to_string(),
            message: format!("Gagal membuat direktori staging plot: {}", e),
        })?;

    let total_plots = plots.len();
    let mut successful_plots = 0;
    let mut failed_plots = Vec::new();
    let mut total_slices_processed = 0;

    for (plot_idx, plot) in plots.iter().enumerate() {
        // Parse poligon koordinat WGS84
        let polygon = match polygon_from_geometry(&plot.geometry) {
            Ok(poly) => poly,
            Err(e) => {
                failed_plots.push(format!("{}: {}", plot.name, e));
                continue;
            }
        };

        if polygon.is_empty() {
            failed_plots.push(format!("{}: Poligon kosong", plot.name));
            continue;
        }

        let exterior_coords = &polygon[0];

        // Lakukan pemotongan dan pembagian slice via Modul 03 Clipper
        let slice_paths: Vec<PathBuf> = match clip_orthomosaic_for_plot(
            &payload.ortho_id,
            exterior_coords,
            staging_temp.path(),
            &plot.id,
        ) {
            Ok(paths) => paths,
            Err(e) => {
                failed_plots.push(format!("{}: {}", plot.name, e));
                continue;
            }
        };

        let total_slices = slice_paths.len();
        let mut slice_metrics = Vec::with_capacity(total_slices);
        let mut plot_had_failure = false;

        // Eksekusi plugin per slice secara sekuensial (menjaga RAM O(1))
        for (slice_idx, slice_path) in slice_paths.iter().enumerate() {
            let overall_percent = ((plot_idx as f64 + (slice_idx as f64 / total_slices as f64))
                / total_plots as f64)
                * 100.0;

            let progress_evt = PlotAnalysisProgressEvent {
                stage: "running_plugin".to_string(),
                plot_index: plot_idx + 1,
                total_plots,
                plot_name: plot.name.clone(),
                slice_index: slice_idx + 1,
                total_slices,
                percent: (overall_percent * 10.0).round() / 10.0,
                message: format!(
                    "Menganalisis Plot '{}': Memproses slice {} dari {}",
                    plot.name,
                    slice_idx + 1,
                    total_slices
                ),
            };
            let _ = app.emit("plot-analysis-progress", &progress_evt);

            let request = StartJobRequestDto {
                plugin_id: payload.plugin_id.clone(),
                session_id: None,
                target_image_path: slice_path.to_string_lossy().to_string(),
                mime_type: "image/png".to_string(),
                aoi: None, // Citra sudah dipotong pas dengan batas plot
                parameters: payload.parameters.clone(),
                metadata: None,
            };

            let handle = match start_plugin_job(request, app.clone(), state.clone()).await {
                Ok(h) => h,
                Err(e) => {
                    failed_plots.push(format!(
                        "{}: Gagal memulai job plugin: {}",
                        plot.name, e.message
                    ));
                    plot_had_failure = true;
                    break;
                }
            };

            let job_result = match wait_for_job_completion(
                &handle.job_id,
                &state.jobs,
                Duration::from_millis(250),
                Duration::from_secs(120),
            )
            .await
            {
                Ok(res) => res,
                Err(e) => {
                    failed_plots.push(format!("{}: Plugin error: {}", plot.name, e.message));
                    plot_had_failure = true;
                    break;
                }
            };

            slice_metrics.push(json!(job_result.metrics));
            total_slices_processed += 1;

            // Hapus berkas slice yang selesai untuk membersihkan disk seketika
            let _ = std::fs::remove_file(slice_path);
        }

        if plot_had_failure || slice_metrics.is_empty() {
            continue;
        }

        // Agregasikan metrik seluruh slice menjadi 1 metrik plot utuh
        let final_metrics = aggregate_slice_metrics(&slice_metrics);
        let indicator_inputs = normalise_indicators(&final_metrics);

        // Simpan ke SQLite tabel `plot_indicator`
        let save_res = {
            let conn = state.db.lock().map_err(|_| CommandError {
                code: "STATE_ERROR".to_string(),
                message: "Database lock poisoned".to_string(),
            })?;
            save_indicators(&conn, &plot.id, &payload.plugin_id, None, &indicator_inputs)
        };

        match save_res {
            Ok(_) => successful_plots += 1,
            Err(e) => failed_plots.push(format!("{}: Gagal menyimpan indikator: {}", plot.name, e)),
        }
    }

    // Emit event selesai
    let _ = app.emit(
        "plot-analysis-progress",
        &PlotAnalysisProgressEvent {
            stage: "completed".to_string(),
            plot_index: total_plots,
            total_plots,
            plot_name: "".to_string(),
            slice_index: 0,
            total_slices: 0,
            percent: 100.0,
            message: "Analisis plot orthomosaic selesai".to_string(),
        },
    );

    Ok(PlotAnalysisOutcome {
        successful_plots,
        failed_plots,
        total_slices_processed,
    })
}

// ============================================================================
// 3. UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregate_slice_metrics_empty() {
        let res = aggregate_slice_metrics(&[]);
        assert_eq!(res, json!({}));
    }

    #[test]
    fn test_aggregate_slice_metrics_single_slice() {
        let slice = json!({
            "coverage_percentage": 42.5,
            "sampled_pixels": 1000,
            "vegetation_pixels": 425
        });
        let res = aggregate_slice_metrics(std::slice::from_ref(&slice));
        assert_eq!(res, slice);
    }

    #[test]
    fn test_aggregate_slice_metrics_weighted_average() {
        // Slice 1: 1000 piksel sampel, 400 piksel vegetasi (40%)
        let s1 = json!({
            "coverage_percentage": 40.0,
            "sampled_pixels": 1000,
            "vegetation_pixels": 400
        });
        // Slice 2: 3000 piksel sampel, 1800 piksel vegetasi (60%)
        let s2 = json!({
            "coverage_percentage": 60.0,
            "sampled_pixels": 3000,
            "vegetation_pixels": 1800
        });

        let res = aggregate_slice_metrics(&[s1, s2]);

        // Total piksel sampel: 4000
        // Total piksel vegetasi: 2200
        // Expected weighted coverage: (2200 / 4000) * 100.0 = 55.0%
        assert_eq!(res["sampled_pixels"], 4000);
        assert_eq!(res["vegetation_pixels"], 2200);
        assert_eq!(res["coverage_percentage"], 55.0);
    }

    #[test]
    fn test_aggregate_slice_metrics_tree_counts() {
        let s1 = json!({ "tree_count": 12, "classification": "dense" });
        let s2 = json!({ "tree_count": 8, "classification": "dense" });

        let res = aggregate_slice_metrics(&[s1, s2]);
        assert_eq!(res["tree_count"], 20);
        assert_eq!(res["classification"], "dense");
    }
}
