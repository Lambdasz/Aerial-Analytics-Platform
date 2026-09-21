# RGB Vegetation Detection (Module 4)

Plugin skeleton for the Aerial Analytics Platform. It follows the Module 2 plugin
contract (`plugins/template`, `plugins/mock`). Detection is simulated for now; the
real ExG/VARI/GLI pipeline replaces `simulate_detection()` in `main.py`.

## Files

| File                     | Purpose                                                                                                       |
| :----------------------- | :------------------------------------------------------------------------------------------------------------ |
| `manifest.json`          | Plugin identity, runtime, timeout                                                                             |
| `parameters.json`        | User-configurable parameters (`index_type`, `threshold`, `export_mask`)                                       |
| `inputs.json`            | Input requirements (single image, JPEG/PNG/TIFF, optional AOI)                                                |
| `outputs.json`           | Metrics (`vegetation_coverage_pct`, `total_pixels_analyzed`) and artifacts (`mask_image`, `geojson_boundary`) |
| `execution_payload.json` | Sample payload delivered by Module 2                                                                          |
| `execution_result.json`  | Sample result written by the plugin                                                                           |
| `main.py`                | Entrypoint (`--healthcheck`, `--input`/`--output`)                                                            |

## Public functions (Module 2 interface)

1. `python main.py --healthcheck` prints a healthy/unhealthy JSON and exits 0/1.
2. `python main.py --input <payload.json> --output <result.json>` runs the analysis,
   prints `PROGRESS: {"job_id", "percent", "stage"}` lines to stdout, and writes the
   result file. On error it writes `status: "failure"` and exits 1.

## Test locally

Run from the repository root:

```bash
python plugins/rgb-vegetation-detection/main.py --healthcheck

python plugins/rgb-vegetation-detection/main.py \
  --input plugins/rgb-vegetation-detection/execution_payload.json \
  --output /tmp/test_result.json

check-jsonschema --schemafile schemas/plugin.schema.json plugins/rgb-vegetation-detection/manifest.json
check-jsonschema --schemafile schemas/parameters.schema.json plugins/rgb-vegetation-detection/parameters.json
check-jsonschema --schemafile schemas/inputs.schema.json plugins/rgb-vegetation-detection/inputs.json
check-jsonschema --schemafile schemas/outputs.schema.json plugins/rgb-vegetation-detection/outputs.json
check-jsonschema --schemafile schemas/execution_payload.schema.json plugins/rgb-vegetation-detection/execution_payload.json
check-jsonschema --schemafile schemas/execution_result.schema.json /tmp/test_result.json
```
