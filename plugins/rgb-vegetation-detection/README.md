# RGB Vegetation Detection (Module 4)

Plugin for the Aerial Analytics Platform, written in Rust in a functional style
(pure calculation functions, `Result`-based errors, side effects only at the I/O edges).
It follows the Module 2 plugin contract (`plugins/template`, `plugins/mock_rust`) and
supports the ExG / ExR / VARI vegetation indices.

## Files

| File                     | Purpose                                                                                                                                                    |
| :----------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `manifest.json`          | Plugin identity, binary runtime (`bin/main`), timeout                                                                                                      |
| `parameters.json`        | User-configurable parameters (`method`, `threshold_value`)                                                                                                 |
| `inputs.json`            | Input requirements (single image, JPEG/PNG/TIFF, optional AOI)                                                                                             |
| `outputs.json`           | Metrics (`green_coverage`, `mean_green`, `std_green`, `max_green`, `method_used`, `threshold_used`) and artifacts (`vegetation_mask`, `detection_overlay`) |
| `execution_payload.json` | Sample payload delivered by Module 2                                                                                                                       |
| `execution_result.json`  | Sample result written by the plugin                                                                                                                        |
| `Cargo.toml`             | Rust package (binaries `main` and `experiment`)                                                                                                            |
| `src/lib.rs`             | Core pipeline (index, threshold, stats, overlay, bounds, orchestrator)                                                                                     |
| `src/main.rs`            | Entrypoint (`--healthcheck`, `--input`/`--output`)                                                                                                         |
| `src/bin/experiment.rs`  | Mask experiment tool (outside the plugin contract)                                                                                                         |

## Building the Binary

### Linux / macOS

```bash
cargo build --release --manifest-path plugins/rgb-vegetation-detection/Cargo.toml
mkdir -p plugins/rgb-vegetation-detection/bin
cp plugins/rgb-vegetation-detection/target/release/main plugins/rgb-vegetation-detection/bin/
```

### Windows (PowerShell)

```powershell
cargo build --release --manifest-path plugins\rgb-vegetation-detection\Cargo.toml
if (!(Test-Path plugins\rgb-vegetation-detection\bin)) { New-Item -ItemType Directory -Path plugins\rgb-vegetation-detection\bin }
Copy-Item plugins\rgb-vegetation-detection\target\release\main.exe plugins\rgb-vegetation-detection\bin\
```

## Public functions (Module 2 interface)

1. `./plugins/rgb-vegetation-detection/bin/main --healthcheck` prints a healthy/unhealthy JSON and exits 0/1.
2. `./plugins/rgb-vegetation-detection/bin/main --input <payload.json> --output <result.json>` runs the analysis,
   prints a completion line to stdout, and writes the result file. On error it writes `status: "failure"` and exits 1.

## Test locally

Run from the repository root:

```bash
cargo test --manifest-path plugins/rgb-vegetation-detection/Cargo.toml

./plugins/rgb-vegetation-detection/bin/main --healthcheck

./plugins/rgb-vegetation-detection/bin/main \
  --input plugins/rgb-vegetation-detection/execution_payload.json \
  --output /tmp/test_result.json

check-jsonschema --schemafile schemas/plugin.schema.json plugins/rgb-vegetation-detection/manifest.json
check-jsonschema --schemafile schemas/parameters.schema.json plugins/rgb-vegetation-detection/parameters.json
check-jsonschema --schemafile schemas/inputs.schema.json plugins/rgb-vegetation-detection/inputs.json
check-jsonschema --schemafile schemas/outputs.schema.json plugins/rgb-vegetation-detection/outputs.json
check-jsonschema --schemafile schemas/execution_payload.schema.json plugins/rgb-vegetation-detection/execution_payload.json
check-jsonschema --schemafile schemas/execution_result.schema.json plugins/rgb-vegetation-detection/execution_result.json
```
