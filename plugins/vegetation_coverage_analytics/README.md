# Vegetation Coverage Analytics (Module 5)

Plugin for the Aerial Analytics Platform. Given a vegetation mask — either
derived from the input image with an RGB index (like the Module 4 plugin) or
read from a mask file the Module 4 run produced — it quantifies how much of the
area is covered by vegetation, classifies that coverage, splits the image into a
zonation grid, and compares coverage between plots.

Written in Rust with pure calculation functions and `Result`-based errors; I/O
is confined to the file-reading and artifact-writing edges.

## Files

| File                     | Purpose                                                                                  |
| :----------------------- | :--------------------------------------------------------------------------------------- |
| `manifest.json`          | Plugin identity, binary runtime (`bin/main`), timeout                                    |
| `parameters.json`        | User-configurable parameters (`mask_source`, `method`, `grid_size`, thresholds, `plots`) |
| `inputs.json`            | Input requirements (single image, JPEG/PNG/TIFF, optional AOI)                           |
| `outputs.json`           | Metrics (coverage, area, classification, plot comparison) and artifacts                  |
| `execution_payload.json` | Sample payload delivered by Module 2                                                     |
| `execution_result.json`  | Sample result written by the plugin                                                      |
| `Cargo.toml`             | Rust package (binary `main`)                                                             |
| `src/lib.rs`             | Pure pipeline (index, threshold, coverage, zonation, plots, GeoJSON) plus I/O edges      |
| `src/main.rs`            | Entrypoint (`--healthcheck`, `--input`/`--output`)                                       |

## Building the Binary

### Linux / macOS

```bash
cargo build --release --manifest-path plugins/vegetation_coverage_analytics/Cargo.toml
mkdir -p plugins/vegetation_coverage_analytics/bin
cp plugins/vegetation_coverage_analytics/target/release/main plugins/vegetation_coverage_analytics/bin/
```

### Windows (PowerShell)

```powershell
cargo build --release --manifest-path plugins\vegetation_coverage_analytics\Cargo.toml
if (!(Test-Path plugins\vegetation_coverage_analytics\bin)) { New-Item -ItemType Directory plugins\vegetation_coverage_analytics\bin }
Copy-Item plugins\vegetation_coverage_analytics\target\release\main.exe plugins\vegetation_coverage_analytics\bin\
```

## Public functions (Module 2 interface)

1. `./plugins/vegetation_coverage_analytics/bin/main --healthcheck` prints a healthy JSON and exits 0.
2. `./plugins/vegetation_coverage_analytics/bin/main --input <payload.json> --output <result.json>` runs the analysis, prints a completion line, and writes the result file. On error it writes `status: "failure"` and exits 1.

## Pipeline

1. **Mask acquisition** — `mask_source = "auto"` computes ExG/ExR/VARI and thresholds it (threshold defaults per index); `mask_source = "file"` reads a mask PNG (`mask_path`), white pixels = vegetation.
2. **Georeferencing** — GSD comes from `gsd_m`, else altitude + 35 mm-equivalent focal length, else altitude with an assumed 60° horizontal field of view. Image bounds come from the AOI, else the GPS point plus GSD. Without a GSD the run still reports percentages, with a `warning` status and no area metrics.
3. **Coverage** — counts vegetation pixels over the whole mask, or only inside the AOI.
4. **Classification** — `low` when coverage is at or below `low_threshold`, `medium` up to `medium_threshold`, otherwise `high`.
5. **Zonation** — `grid_size × grid_size` cells with integer boundaries (no pixels lost on uneven dimensions), written as a GeoJSON polygon layer with per-cell coverage and class.
6. **Plots** — each `plots` entry (`{"plot_id": "...", "polygon": [[lon, lat], ...]}`) is rasterized by pixel center with a ray-casting test, and coverage/area/class are reported per plot; `compare_plots` then returns the average, the highest and lowest plot, and a full ranking.
7. **Artifacts** — `coverage_zonation` (GeoJSON), `plot_coverage` (GeoJSON, when plots are supplied) and `vegetation_mask` (PNG echo of the mask used), each with georeferenced bounds when available.

## Tests

```bash
cargo test --manifest-path plugins/vegetation_coverage_analytics/Cargo.toml
```
