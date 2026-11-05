# RGB Vegetation Condition Analysis (Module 6)

Plugin for the Aerial Analytics Platform. Assesses visible vegetation condition
from standard RGB drone imagery — no NIR or multispectral sensor needed — and
returns machine-readable masks, statistics and map layers.

Ported from the Module 6 research implementation (`Module-6-Experiment`,
`module6/` package) to Rust, keeping its pure-functional structure: every
calculation is a pure function, all I/O lives at the edges.

## Files

| File                     | Purpose                                                                                    |
| :----------------------- | :----------------------------------------------------------------------------------------- |
| `manifest.json`          | Plugin identity, binary runtime (`bin/main`), timeout                                      |
| `parameters.json`        | Thresholds (`exg_threshold`, VARI classes, yellowing hue, browning RBVI), grid, plots      |
| `inputs.json`            | Input requirements (single image, JPEG/PNG/TIFF, optional AOI)                             |
| `outputs.json`           | Metrics (VARI, colour stats, class shares, yellowing/browning) and artifacts               |
| `execution_payload.json` | Sample payload delivered by Module 2                                                       |
| `execution_result.json`  | Sample result written by the plugin                                                        |
| `Cargo.toml`             | Rust package (binary `main`)                                                               |
| `src/lib.rs`             | Pure pipeline (canopy mask, VARI, HSL stats, classes, chlorosis/necrosis, zonation, plots) |
| `src/main.rs`            | Entrypoint (`--healthcheck`, `--input`/`--output`)                                         |

## Building the Binary

### Linux / macOS

```bash
cargo build --release --manifest-path plugins/vegetation_condition_analysis/Cargo.toml
mkdir -p plugins/vegetation_condition_analysis/bin
cp plugins/vegetation_condition_analysis/target/release/main plugins/vegetation_condition_analysis/bin/
```

### Windows (PowerShell)

```powershell
cargo build --release --manifest-path plugins\vegetation_condition_analysis\Cargo.toml
if (!(Test-Path plugins\vegetation_condition_analysis\bin)) { New-Item -ItemType Directory plugins\vegetation_condition_analysis\bin }
Copy-Item plugins\vegetation_condition_analysis\target\release\main.exe plugins\vegetation_condition_analysis\bin\
```

## Public functions (Module 2 interface)

1. `./plugins/vegetation_condition_analysis/bin/main --healthcheck` prints a healthy JSON and exits 0.
2. `./plugins/vegetation_condition_analysis/bin/main --input <payload.json> --output <result.json>` runs the analysis and writes the result file. On error it writes `status: "failure"` and exits 1.

## Pipeline (mirrors the research modules M6.0–M6.5)

1. **Canopy isolation (M6.0 adapter)** — normalized chromatic coordinates, `ExG = 2g − r − b`, plus green dominance (`G > R`) and brightness bounds (`0.08 < L < 0.95`), with `mask_source = "file"` able to take the Module 4 mask instead.
2. **Green intensity (M6.1)** — `VARI = (G − R) / (G + R − B + ε)`, clamped to [−1, 1], zero outside the canopy.
3. **Colour characteristics (M6.2)** — RGB means/standard deviations and HSL hue/saturation means over canopy pixels only.
4. **Condition classes (M6.3)** — VARI ≥ `threshold_vigorous` → vigorous; ≥ `threshold_moderate` → moderate stress; otherwise severe stress. Shares and dominant class reported.
5. **Chlorosis (M6.4)** — canopy pixels with hue inside `[yellowing_hue_min, yellowing_hue_max]` and saturation above `yellowing_sat_min`.
6. **Necrosis (M6.5)** — canopy pixels with `RBVI = (R − B) / (R + B) > browning_min_rbvi` and `R > browning_r_g_ratio · G`.
7. **Zonation and plots** — `grid_size × grid_size` cells with per-cell condition shares, and optional per-plot summaries (`plots` parameter) ranked by mean VARI.

Artifacts: `condition_map` (class palette PNG), `yellowing_mask`, `browning_mask`, `condition_zonation` (GeoJSON), `plot_condition` (GeoJSON). Raster artifacts carry WGS84 bounds derived from GPS + altitude (assumed 60° HFOV) or the AOI, so the platform can publish them as map overlays.

## Tests

```bash
cargo test --manifest-path plugins/vegetation_condition_analysis/Cargo.toml
```
