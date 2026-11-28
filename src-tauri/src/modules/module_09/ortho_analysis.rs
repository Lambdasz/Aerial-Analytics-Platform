//! Plot Analytics Orchestration & Sliced Metric Reducer (Module 9).
//!
//! Bertanggung jawab mengatur eksekusi analisis geospasial untuk kumpulan Plot
//! dari citra Orthomosaic (`map_storage`), mengeksekusi plugin per slice secara sekuensial,
//! serta mereduksi dan mengagregasikan metrik antar-slice ke tabel `plot_indicator`.
//!
//! Gaya penulisan mengikuti NFR-9.1.4–9.1.7 (`src/modules/module-09/REQUIREMENTS.md`):
//! percabangan dengan `match`, tanpa binding mutable, tanpa loop imperatif atas
//! koleksi. Iterasi sekuensial yang melibatkan `await` ditulis sebagai rekursi
//! (`Box::pin`) yang membawa akumulator secara *by value*.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Emitter;

use crate::modules::module_03::ortho_source::clipper::clip_orthomosaic_for_plot;
use crate::modules::module_03::ortho_source::store::get_ortho_image;
use crate::modules::module_09::appended;
use crate::modules::module_09::indicators::{normalise_indicators, save_indicators};
use crate::modules::module_09::plots::{get_plot, polygon_from_geometry, PlotRecord};
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

/// Cara sebuah metrik digabung antar-slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricKind {
    /// Hitungan / jumlah piksel: dijumlahkan.
    Count,
    /// Rasio / persentase: dirata-rata (terbobot bila memungkinkan).
    Ratio,
}

/// Pure: klasifikasi metrik dari namanya.
fn metric_kind(key: &str) -> MetricKind {
    let lower = key.to_lowercase();
    let mentions = |words: &[&str]| words.iter().any(|word| lower.contains(word));
    match (
        mentions(&["count", "pixel", "total", "trees"]),
        mentions(&["pct", "percent", "ratio"]),
    ) {
        (true, false) => MetricKind::Count,
        _ => MetricKind::Ratio,
    }
}

/// Pure: pembulatan 4 desimal.
fn round4(value: f64) -> f64 {
    (value * 10000.0).round() / 10000.0
}

/// Pure: seluruh key unik dari semua slice, urut sesuai kemunculan pertama.
fn unique_keys(slice_metrics: &[Value]) -> Vec<String> {
    let all: Vec<&String> = slice_metrics
        .iter()
        .filter_map(Value::as_object)
        .flat_map(|map| map.keys())
        .collect();
    all.iter()
        .enumerate()
        .filter(|(index, key)| !all[..*index].contains(key))
        .map(|(_, key)| (*key).clone())
        .collect()
}

/// Pure: bobot slice = `sampled_pixels` (atau `total_pixels`), minimal 1.
fn slice_weight(metrics: &Value) -> f64 {
    metrics
        .get("sampled_pixels")
        .or_else(|| metrics.get("total_pixels"))
        .and_then(Value::as_f64)
        .unwrap_or(1.0)
        .max(1.0)
}

/// Pure: rata-rata terbobot bila setiap slice punya nilai, selain itu rata-rata biasa.
fn ratio_mean(numbers: &[f64], weights: &[f64], total_weight: f64) -> f64 {
    match total_weight > 0.0 && numbers.len() == weights.len() {
        true => {
            numbers
                .iter()
                .zip(weights)
                .map(|(value, weight)| value * weight)
                .sum::<f64>()
                / total_weight
        }
        false => numbers.iter().sum::<f64>() / numbers.len() as f64,
    }
}

/// Pure: nilai gabungan satu key, atau `None` bila tidak ada nilai yang bisa digabung.
fn aggregate_key(
    key: &str,
    slice_metrics: &[Value],
    weights: &[f64],
    total_weight: f64,
) -> Option<Value> {
    let values = || slice_metrics.iter().filter_map(|metrics| metrics.get(key));
    let numbers: Vec<f64> = values().filter_map(Value::as_f64).collect();
    let text = values()
        .filter_map(Value::as_str)
        .find(|text| !text.is_empty());
    match (numbers.is_empty(), metric_kind(key)) {
        (true, _) => text.map(|text| json!(text)),
        (false, MetricKind::Count) => Some(json!(numbers.iter().sum::<f64>().round() as i64)),
        (false, MetricKind::Ratio) => {
            Some(json!(round4(ratio_mean(&numbers, weights, total_weight))))
        }
    }
}

/// Pure: gabungan beberapa slice (minimal dua).
fn aggregate_many(slice_metrics: &[Value]) -> Value {
    let weights: Vec<f64> = slice_metrics.iter().map(slice_weight).collect();
    let total_weight: f64 = weights.iter().sum();

    let aggregated: serde_json::Map<String, Value> = unique_keys(slice_metrics)
        .into_iter()
        .filter_map(|key| {
            aggregate_key(&key, slice_metrics, &weights, total_weight).map(|value| (key, value))
        })
        .collect();

    // Rekalibrasi persentase bila piksel vegetasi & piksel sampel tersedia.
    let vegetation = aggregated.get("vegetation_pixels").and_then(Value::as_f64);
    let sampled = aggregated
        .get("sampled_pixels")
        .or_else(|| aggregated.get("total_pixels"))
        .and_then(Value::as_f64);
    let recomputed = vegetation
        .zip(sampled)
        .filter(|(_, sample)| *sample > 0.0)
        .map(|(veg, sample)| round4((veg / sample) * 100.0));

    Value::Object(
        aggregated
            .into_iter()
            .map(|(key, value)| match (key.as_str(), recomputed) {
                ("coverage_percentage" | "vegetation_coverage_pct", Some(pct)) => (key, json!(pct)),
                _ => (key, value),
            })
            .collect(),
    )
}

/// Pure: Menggabungkan (*aggregate/reduce*) hasil metrik dari beberapa slice menjadi satu metrik plot utuh.
///
/// **Aturan Agregasi**:
/// 1. Jika slice tunggal (`slice_metrics.len() == 1`), metrik dikembalikan langsung tanpa modifikasi.
/// 2. Metrik bertipe count/piksel (misal `sampled_pixels`, `vegetation_pixels`, `tree_count`) dijumlahkan (`sum`).
/// 3. Metrik bertipe rasio/persentase (misal `coverage_percentage`, `vegetation_coverage_pct`)
///    dihitung sebagai rata-rata terbobot (*weighted average*) menggunakan `sampled_pixels` atau `total_pixels` jika tersedia.
/// 4. Jika jumlah piksel vegetasi dan sampel diketahui, persentase coverage dihitung ulang secara deterministik.
pub fn aggregate_slice_metrics(slice_metrics: &[Value]) -> Value {
    match slice_metrics {
        [] => json!({}),
        [single] => single.clone(),
        many => aggregate_many(many),
    }
}

/// Pure: status job → hasil akhir, atau `None` bila job masih berjalan.
fn job_verdict(
    job_id: &str,
    tracker: &ActiveJobTracker,
) -> Option<Result<StandardJobResultDto, CommandError>> {
    match tracker.jobs.get(job_id) {
        None => Some(Err(CommandError {
            code: "JOB_NOT_FOUND".to_string(),
            message: format!("ID pekerjaan '{}' tidak terdaftar", job_id),
        })),
        Some(record) => match &record.status {
            JobStatusDto::Completed { .. } => {
                Some(record.result.clone().ok_or_else(|| CommandError {
                    code: "RESULT_MISSING".to_string(),
                    message: format!("Hasil pekerjaan '{}' tidak ditemukan", job_id),
                }))
            }
            JobStatusDto::Failed { error } => Some(Err(CommandError {
                code: "JOB_FAILED".to_string(),
                message: error.clone(),
            })),
            JobStatusDto::Aborted => Some(Err(CommandError {
                code: "JOB_ABORTED".to_string(),
                message: "Pekerjaan plugin dibatalkan oleh pengguna".to_string(),
            })),
            JobStatusDto::Running { .. } | JobStatusDto::Queued => None,
        },
    }
}

/// Pure: event progres untuk satu slice.
fn slice_progress(
    plot_idx: usize,
    total_plots: usize,
    plot_name: &str,
    slice_idx: usize,
    total_slices: usize,
) -> PlotAnalysisProgressEvent {
    let overall_percent =
        ((plot_idx as f64 + (slice_idx as f64 / total_slices as f64)) / total_plots as f64) * 100.0;
    PlotAnalysisProgressEvent {
        stage: "running_plugin".to_string(),
        plot_index: plot_idx + 1,
        total_plots,
        plot_name: plot_name.to_string(),
        slice_index: slice_idx + 1,
        total_slices,
        percent: (overall_percent * 10.0).round() / 10.0,
        message: format!(
            "Menganalisis Plot '{}': Memproses slice {} dari {}",
            plot_name,
            slice_idx + 1,
            total_slices
        ),
    }
}

/// Status akhir satu plot.
#[derive(Debug, Clone, PartialEq)]
enum PlotStatus {
    /// Indikator tersimpan.
    Stored,
    /// Tidak ada slice yang menghasilkan metrik; tidak dihitung berhasil maupun gagal.
    Skipped,
    /// Gagal, beserta alasannya (sudah memuat nama plot).
    Failed(String),
}

/// Hasil analisis satu plot: jumlah slice yang selesai dan statusnya.
#[derive(Debug, Clone, PartialEq)]
struct PlotRun {
    slices_processed: usize,
    status: PlotStatus,
}

/// Akumulator hasil seluruh plot.
#[derive(Debug, Clone, Default, PartialEq)]
struct Tally {
    successful_plots: usize,
    failed_plots: Vec<String>,
    total_slices_processed: usize,
}

impl Tally {
    /// Pure: akumulator baru setelah satu plot selesai.
    fn record(self, run: PlotRun) -> Self {
        let total_slices_processed = self.total_slices_processed + run.slices_processed;
        match run.status {
            PlotStatus::Stored => Tally {
                successful_plots: self.successful_plots + 1,
                total_slices_processed,
                ..self
            },
            PlotStatus::Skipped => Tally {
                total_slices_processed,
                ..self
            },
            PlotStatus::Failed(reason) => Tally {
                failed_plots: appended(self.failed_plots, reason),
                total_slices_processed,
                ..self
            },
        }
    }

    fn into_outcome(self) -> PlotAnalysisOutcome {
        PlotAnalysisOutcome {
            successful_plots: self.successful_plots,
            failed_plots: self.failed_plots,
            total_slices_processed: self.total_slices_processed,
        }
    }
}

// ============================================================================
// 2. IMPERATIVE SHELL (Orchestration, Plugin Execution, DB Persistence)
// ============================================================================

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

fn lock_poisoned() -> CommandError {
    CommandError {
        code: "STATE_ERROR".to_string(),
        message: "Database lock poisoned".to_string(),
    }
}

/// Menunggu penyelesaian satu pekerjaan plugin dengan polling read-lock pada ActiveJobTracker.
pub async fn wait_for_job_completion(
    job_id: &str,
    jobs: &tokio::sync::RwLock<ActiveJobTracker>,
    poll_interval: Duration,
    timeout: Duration,
) -> Result<StandardJobResultDto, CommandError> {
    let start_time = tokio::time::Instant::now();

    loop {
        let verdict = match start_time.elapsed() > timeout {
            true => Some(Err(CommandError {
                code: "TIMEOUT".to_string(),
                message: format!(
                    "Pekerjaan plugin '{}' melebihi batas waktu eksekusi",
                    job_id
                ),
            })),
            false => job_verdict(job_id, &*jobs.read().await),
        };
        match verdict {
            Some(outcome) => return outcome,
            None => tokio::time::sleep(poll_interval).await,
        }
    }
}

/// Konteks bersama satu run analisis (hanya dibaca).
struct ShellContext<'a> {
    app: &'a tauri::AppHandle,
    state: &'a tauri::State<'a, crate::AppState>,
    payload: &'a AnalysePlotsOrthoPayload,
    staging: &'a Path,
    total_plots: usize,
}

/// Menjalankan plugin untuk satu slice dan mengembalikan metriknya.
async fn run_slice(
    ctx: &ShellContext<'_>,
    plot: &PlotRecord,
    plot_idx: usize,
    slice_idx: usize,
    total_slices: usize,
    slice_path: &Path,
) -> Result<Value, String> {
    let _ = ctx.app.emit(
        "plot-analysis-progress",
        &slice_progress(
            plot_idx,
            ctx.total_plots,
            &plot.name,
            slice_idx,
            total_slices,
        ),
    );

    let request = StartJobRequestDto {
        plugin_id: ctx.payload.plugin_id.clone(),
        session_id: None,
        target_image_path: slice_path.to_string_lossy().to_string(),
        mime_type: "image/png".to_string(),
        aoi: None, // Citra sudah dipotong pas dengan batas plot
        parameters: ctx.payload.parameters.clone(),
        metadata: None,
    };

    let handle = start_plugin_job(request, ctx.app.clone(), ctx.state.clone())
        .await
        .map_err(|e| format!("{}: Gagal memulai job plugin: {}", plot.name, e.message))?;

    let job_result = wait_for_job_completion(
        &handle.job_id,
        &ctx.state.jobs,
        Duration::from_millis(250),
        Duration::from_secs(120),
    )
    .await
    .map_err(|e| format!("{}: Plugin error: {}", plot.name, e.message))?;

    // Hapus berkas slice yang selesai untuk membersihkan disk seketika
    let _ = std::fs::remove_file(slice_path);
    Ok(json!(job_result.metrics))
}

/// Metrik slice yang selesai dan alasan berhenti (bila ada slice yang gagal).
struct SliceRun {
    metrics: Vec<Value>,
    failure: Option<String>,
}

/// Eksekusi plugin per slice secara sekuensial (menjaga RAM O(1)); berhenti di slice gagal pertama.
fn run_slices<'a>(
    ctx: &'a ShellContext<'a>,
    plot: &'a PlotRecord,
    plot_idx: usize,
    total_slices: usize,
    remaining: &'a [PathBuf],
    done: Vec<Value>,
) -> BoxFuture<'a, SliceRun> {
    Box::pin(async move {
        match remaining.split_first() {
            None => SliceRun {
                metrics: done,
                failure: None,
            },
            Some((slice_path, rest)) => {
                match run_slice(ctx, plot, plot_idx, done.len(), total_slices, slice_path).await {
                    Ok(metrics) => {
                        run_slices(
                            ctx,
                            plot,
                            plot_idx,
                            total_slices,
                            rest,
                            appended(done, metrics),
                        )
                        .await
                    }
                    Err(reason) => SliceRun {
                        metrics: done,
                        failure: Some(reason),
                    },
                }
            }
        }
    })
}

/// Potong orthomosaic mengikuti batas plot.
fn clip_plot(ctx: &ShellContext<'_>, plot: &PlotRecord) -> Result<Vec<PathBuf>, String> {
    let polygon =
        polygon_from_geometry(&plot.geometry).map_err(|e| format!("{}: {}", plot.name, e))?;
    let exterior_coords = polygon
        .first()
        .ok_or_else(|| format!("{}: Poligon kosong", plot.name))?;
    clip_orthomosaic_for_plot(
        &ctx.payload.ortho_id,
        exterior_coords,
        ctx.staging,
        &plot.id,
    )
    .map_err(|e| format!("{}: {}", plot.name, e))
}

/// Simpan metrik gabungan satu plot ke tabel `plot_indicator`.
fn store_plot_metrics(
    ctx: &ShellContext<'_>,
    plot: &PlotRecord,
    slice_metrics: &[Value],
) -> Result<PlotStatus, CommandError> {
    let indicator_inputs = normalise_indicators(&aggregate_slice_metrics(slice_metrics));
    let conn = ctx.state.db.lock().map_err(|_| lock_poisoned())?;
    Ok(
        match save_indicators(
            &conn,
            &plot.id,
            &ctx.payload.plugin_id,
            None,
            &indicator_inputs,
        ) {
            Ok(_) => PlotStatus::Stored,
            Err(e) => {
                PlotStatus::Failed(format!("{}: Gagal menyimpan indikator: {}", plot.name, e))
            }
        },
    )
}

/// Analisis satu plot: clip → slice → plugin → agregasi → simpan.
async fn analyse_plot(
    ctx: &ShellContext<'_>,
    plot_idx: usize,
    plot: &PlotRecord,
) -> Result<PlotRun, CommandError> {
    match clip_plot(ctx, plot) {
        Err(reason) => Ok(PlotRun {
            slices_processed: 0,
            status: PlotStatus::Failed(reason),
        }),
        Ok(slice_paths) => {
            let run = run_slices(
                ctx,
                plot,
                plot_idx,
                slice_paths.len(),
                &slice_paths,
                Vec::new(),
            )
            .await;
            let status = match (run.failure, run.metrics.is_empty()) {
                (Some(reason), _) => PlotStatus::Failed(reason),
                (None, true) => PlotStatus::Skipped,
                (None, false) => store_plot_metrics(ctx, plot, &run.metrics)?,
            };
            Ok(PlotRun {
                slices_processed: run.metrics.len(),
                status,
            })
        }
    }
}

/// Analisis plot secara berurutan; akumulator dibawa *by value* ke langkah berikutnya.
fn analyse_remaining<'a>(
    ctx: &'a ShellContext<'a>,
    remaining: &'a [PlotRecord],
    plot_idx: usize,
    tally: Tally,
) -> BoxFuture<'a, Result<Tally, CommandError>> {
    Box::pin(async move {
        match remaining.split_first() {
            None => Ok(tally),
            Some((plot, rest)) => {
                let run = analyse_plot(ctx, plot_idx, plot).await?;
                analyse_remaining(ctx, rest, plot_idx + 1, tally.record(run)).await
            }
        }
    })
}

/// Muat plot yang diminta; ID yang tidak ditemukan dilewati.
fn load_plots(
    state: &tauri::State<'_, crate::AppState>,
    plot_ids: &[String],
) -> Result<Vec<PlotRecord>, CommandError> {
    let conn = state.db.lock().map_err(|_| lock_poisoned())?;
    plot_ids
        .iter()
        .map(|plot_id| get_plot(&conn, plot_id))
        .collect::<Result<Vec<_>, _>>()
        .map(|found| found.into_iter().flatten().collect())
        .map_err(|e| CommandError {
            code: "PLOT_ERROR".to_string(),
            message: e.to_string(),
        })
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

    ortho_meta.bounds.as_ref().ok_or_else(|| CommandError {
        code: "MISSING_BOUNDS".to_string(),
        message: format!(
            "Citra orthomosaic '{}' tidak memiliki georeferensi bounds",
            payload.ortho_id
        ),
    })?;

    // 2. Muat data plot dari SQLite
    let plots = Some(load_plots(state, &payload.plot_ids)?)
        .filter(|plots| !plots.is_empty())
        .ok_or_else(|| CommandError {
            code: "NO_PLOTS".to_string(),
            message: "Tidak ada plot yang valid untuk dianalisis".to_string(),
        })?;

    // 3. Buat folder staging sementara RAII untuk menampung potongan slice
    let staging_temp = tempfile::Builder::new()
        .prefix("ortho_plots_")
        .tempdir()
        .map_err(|e| CommandError {
            code: "IO_ERROR".to_string(),
            message: format!("Gagal membuat direktori staging plot: {}", e),
        })?;

    let ctx = ShellContext {
        app,
        state,
        payload: &payload,
        staging: staging_temp.path(),
        total_plots: plots.len(),
    };

    // 4. Analisis setiap plot secara berurutan
    let tally = analyse_remaining(&ctx, &plots, 0, Tally::default()).await?;

    // Emit event selesai
    let _ = app.emit(
        "plot-analysis-progress",
        &PlotAnalysisProgressEvent {
            stage: "completed".to_string(),
            plot_index: plots.len(),
            total_plots: plots.len(),
            plot_name: "".to_string(),
            slice_index: 0,
            total_slices: 0,
            percent: 100.0,
            message: "Analisis plot orthomosaic selesai".to_string(),
        },
    );

    Ok(tally.into_outcome())
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
