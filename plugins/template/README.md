# Plugin Specification Reference

This directory contains the canonical starter template for analytical plugins in the **Aerial Analytics Platform**.

To ensure modularity and separation of concerns, each plugin is structured across **4 domain specification contracts** and accompanied by **2 execution test fixtures**:

```text
plugins/<your_plugin>/
├── manifest.json          # Contract 1: Plugin identity, runtime engine, and domain pointers
├── parameters.json        # Contract 2: Dynamic UI parameter controls (for Blueprint.js forms)
├── inputs.json            # Contract 3: Input requirements contract (MIME types, metadata, AOI)
├── outputs.json           # Contract 4: Output specification contract (metrics, artifacts, layer hints)
├── execution_payload.json # Fixture 5: Sample execution payload delivered by host for testing
├── execution_result.json  # Fixture 6: Sample execution output result written by plugin
├── main.py                # Plugin Python entrypoint script
├── requirements.txt       # Python dependencies
└── README.md              # Plugin documentation
```

---

## Validating Your Manifest & Domain Files

All plugin files must conform to their corresponding platform schemas in [`schemas/`](../../schemas/). Validation is strictly enforced using [`check-jsonschema`](https://check-jsonschema.readthedocs.io/).

---

### 1. Installation

#### On Linux (Ubuntu / Debian)

**Recommended (via `pipx` to avoid PATH conflicts):**

```bash
# 1. Install pipx if not already installed:
sudo apt install pipx

# 2. Install check-jsonschema globally:
pipx install check-jsonschema

# 3. Ensure ~/.local/bin is added to your PATH:
pipx ensurepath
source ~/.bashrc
```

**Alternative (via `pip` / Conda):**

```bash
pip install check-jsonschema
```

---

#### On Windows (PowerShell / Command Prompt)

**Recommended (via `pip`):**

```powershell
pip install check-jsonschema
```

**Alternative (via `pipx`):**

```powershell
pip install pipx
pipx install check-jsonschema
pipx ensurepath
```

> **Note for Windows:** If `check-jsonschema` is not recognized after installing with pip, you can invoke it directly through Python:
>
> ```powershell
> python -m check_jsonschema --schemafile schemas\plugin.schema.json plugins\<your_plugin>\manifest.json
> ```

---

### 2. Running Validation

Run validation from the root of the repository for each domain file:

#### Linux / macOS:

```bash
# 1. Validate core manifest:
check-jsonschema --schemafile schemas/plugin.schema.json plugins/<your_plugin>/manifest.json

# 2. Validate parameters:
check-jsonschema --schemafile schemas/parameters.schema.json plugins/<your_plugin>/parameters.json

# 3. Validate inputs:
check-jsonschema --schemafile schemas/inputs.schema.json plugins/<your_plugin>/inputs.json

# 4. Validate outputs:
check-jsonschema --schemafile schemas/outputs.schema.json plugins/<your_plugin>/outputs.json

# 5. Validate test execution payload & result fixtures:
check-jsonschema --schemafile schemas/execution_payload.schema.json plugins/<your_plugin>/execution_payload.json
check-jsonschema --schemafile schemas/execution_result.schema.json plugins/<your_plugin>/execution_result.json
```

#### Windows (PowerShell):

```powershell
# 1. Validate core manifest:
check-jsonschema --schemafile schemas\plugin.schema.json plugins\<your_plugin>\manifest.json

# 2. Validate parameters:
check-jsonschema --schemafile schemas\parameters.schema.json plugins\<your_plugin>\parameters.json

# 3. Validate inputs:
check-jsonschema --schemafile schemas\inputs.schema.json plugins\<your_plugin>\inputs.json

# 4. Validate outputs:
check-jsonschema --schemafile schemas\outputs.schema.json plugins\<your_plugin>\outputs.json

# 5. Validate test execution payload & result fixtures:
check-jsonschema --schemafile schemas\execution_payload.schema.json plugins\<your_plugin>\execution_payload.json
check-jsonschema --schemafile schemas\execution_result.schema.json plugins\<your_plugin>\execution_result.json
```

---

### 3. Understanding Output

- **Success:**

  ```text
  ok -- validation done
  ```

  _(Exits with status code `0`)_

- **Failure:**
  ```text
  Schema validation errors were encountered.
    plugins/my_plugin/manifest.json::$: 'runtime' is a required property
  ```
  _(Exits with status code `1` and details the exact key, expected format, or missing property)_

---

## Field Reference (Contracts & Execution Fixtures)

### File 1: `manifest.json`

#### Root Fields

| Field              | Type     | Required | Allowed / Example Values                                | Description                         |
| :----------------- | :------- | :------- | :------------------------------------------------------ | :---------------------------------- |
| `$schema`          | `string` | No       | `https://aerial-analytics.local/schemas/plugin.v1.json` | JSON Schema URI for autocompletion. |
| `manifest_version` | `number` | **Yes**  | `1`                                                     | Manifest specification version.     |
| `parameters`       | `string` | **Yes**  | `"parameters.json"`                                     | Relative path to parameters file.   |
| `inputs`           | `string` | **Yes**  | `"inputs.json"`                                         | Relative path to inputs file.       |
| `outputs`          | `string` | **Yes**  | `"outputs.json"`                                        | Relative path to outputs file.      |

#### `metadata`

| Field              | Type       | Required | Allowed / Example Values                                                                                                                    | Description                          |
| :----------------- | :--------- | :------- | :------------------------------------------------------------------------------------------------------------------------------------------ | :----------------------------------- |
| `id`               | `string`   | **Yes**  | Pattern: `^[a-z0-9-_]+$` (e.g., `rgb-vegetation-exg`)                                                                                       | Unique plugin slug.                  |
| `name`             | `string`   | **Yes**  | e.g., `Tree Detection & Counting`                                                                                                           | Display name in UI.                  |
| `version`          | `string`   | **Yes**  | SemVer (e.g., `1.0.0`)                                                                                                                      | Version of the plugin.               |
| `author`           | `string`   | **Yes**  | e.g., `PBL Team 2026`                                                                                                                       | Author or team name.                 |
| `license`          | `string`   | **Yes**  | `"MIT"`, `"Apache-2.0"`, `"GPL-3.0"`, `"BSD-3-Clause"`, `"Proprietary"`                                                                     | Software license.                    |
| `description`      | `string`   | **Yes**  | e.g., `"Extracts vegetation masks using RGB Excess Green index."`                                                                           | Summary of capability.               |
| `category`         | `string`   | **Yes**  | `"vegetation"`, `"tree_detection"`, `"tree_counting"`, `"land_cover"`, `"temporal"`, `"elevation"`, `"analytics"`, `"utility"`, `"general"` | Domain category.                     |
| `tags`             | `string[]` | No       | `["rgb", "vegetation", "indices"]`                                                                                                          | Search and filtering keywords.       |
| `min_core_version` | `string`   | No       | e.g., `"0.1.0"`                                                                                                                             | Minimum compatible platform version. |

#### `runtime`

| Field                | Type             | Required  | Allowed / Example Values         | Description                        |
| :------------------- | :--------------- | :-------- | :------------------------------- | :--------------------------------- |
| `type`               | `string`         | **Yes**   | `"python"`, `"binary"`, `"wasm"` | Runtime execution engine.          |
| `min_python_version` | `string`         | If Python | e.g., `"3.10"`                   | Minimum Python version required.   |
| `entrypoint`         | `string`         | **Yes**   | e.g., `"main.py"`                | Path relative to plugin directory. |
| `dependencies_file`  | `string \| null` | No        | e.g., `"requirements.txt"`       | Python dependencies file.          |
| `healthcheck_flag`   | `string`         | No        | e.g., `"--healthcheck"`          | CLI argument to verify setup.      |

#### `execution`

| Field             | Type             | Required | Allowed Values                        | Description                         |
| :---------------- | :--------------- | :------- | :------------------------------------ | :---------------------------------- |
| `timeout_seconds` | `number`         | **Yes**  | e.g., `120`                           | Max seconds before killing process. |
| `concurrency`     | `string`         | No       | `"sequential"`, `"parallel"`          | Concurrency scheduling model.       |
| `gpu.support`     | `string`         | No       | `"none"`, `"optional"`, `"required"`  | Hardware acceleration preference.   |
| `gpu.driver`      | `string \| null` | No       | `"cuda"`, `"rocm"`, `"metal"`, `null` | Target GPU driver.                  |

---

### File 2: `parameters.json`

Schema URI: `https://aerial-analytics.local/schemas/parameters.v1.json`

| Field                  | Type       | Required  | Allowed Values                                  | Description                           |
| :--------------------- | :--------- | :-------- | :---------------------------------------------- | :------------------------------------ |
| `name`                 | `string`   | **Yes**   | Pattern: `^[a-z0-9_]+$` (e.g., `threshold`)     | Variable key in payload `parameters`. |
| `label`                | `string`   | **Yes**   | e.g., `Index Threshold`                         | Form label in UI.                     |
| `type`                 | `string`   | **Yes**   | `"select"`, `"number"`, `"boolean"`, `"string"` | Blueprint.js form control type.       |
| `options`              | `string[]` | If select | e.g., `["ExG", "VARI"]`                         | Dropdown choices.                     |
| `default`              | `any`      | **Yes**   | Value matching `type`                           | Initial default value.                |
| `min` / `max` / `step` | `number`   | If number | Numerical bounds and step                       | Slider / spinner constraints.         |
| `description`          | `string`   | No        | Help text / tooltip                             | Explains what the parameter controls. |

---

### File 3: `inputs.json`

Schema URI: `https://aerial-analytics.local/schemas/inputs.v1.json`

| Field                  | Type             | Required | Allowed Values                                             | Description                        |
| :--------------------- | :--------------- | :------- | :--------------------------------------------------------- | :--------------------------------- |
| `granularity`          | `string`         | **Yes**  | `"single_image"`, `"image_pair"`, `"batch"`, `"session"`   | Analysis scope per run.            |
| `supported_mime_types` | `string[]`       | **Yes**  | `"image/jpeg"`, `"image/png"`, `"image/tiff"`              | Supported image formats.           |
| `requires_metadata`    | `string[]`       | **Yes**  | Subset of `["resolution", "gps", "altitude", "timestamp"]` | Mandatory image metadata.          |
| `optional_metadata`    | `string[]`       | No       | Subset of `["resolution", "gps", "altitude", "timestamp"]` | Optional image metadata.           |
| `supports_aoi`         | `boolean`        | **Yes**  | `true`, `false`                                            | Whether AOI clipping is supported. |
| `aoi_format`           | `string \| null` | If AOI   | `"geojson_polygon"`, `"geojson_bbox"`                      | Geometry format for AOI.           |

---

### File 4: `outputs.json`

Schema URI: `https://aerial-analytics.local/schemas/outputs.v1.json`

#### `metrics[]`

| Field         | Type     | Required | Allowed Values                                            | Description                    |
| :------------ | :------- | :------- | :-------------------------------------------------------- | :----------------------------- |
| `key`         | `string` | **Yes**  | Pattern: `^[a-z0-9_]+$` (e.g., `vegetation_coverage_pct`) | Key in output JSON `metrics`.  |
| `label`       | `string` | **Yes**  | e.g., `Vegetation Coverage`                               | Card label in Dashboard.       |
| `type`        | `string` | **Yes**  | `"float"`, `"integer"`, `"string"`, `"boolean"`           | Metric data type.              |
| `unit`        | `string` | No       | `"%"`, `"px"`, `"items"`, `"m²"`, `"ha"`                  | Unit displayed next to number. |
| `description` | `string` | No       | Explanatory note                                          | Tooltip explanation.           |

#### `artifacts[]`

| Field                        | Type      | Required | Allowed Values                                                          | Description                     |
| :--------------------------- | :-------- | :------- | :---------------------------------------------------------------------- | :------------------------------ |
| `key`                        | `string`  | **Yes**  | Pattern: `^[a-z0-9_]+$` (e.g., `mask_raster`)                           | Key in output JSON `artifacts`. |
| `label`                      | `string`  | **Yes**  | e.g., `Vegetation Mask`                                                 | Layer title in Map Explorer.    |
| `type`                       | `string`  | **Yes**  | `"raster"`, `"vector"`, `"table"`                                       | Artifact category.              |
| `format`                     | `string`  | **Yes**  | `"image/png"`, `"image/tiff"`, `"application/geo+json"`, `"text/csv"`   | File format MIME.               |
| `coordinate_system`          | `string`  | **Yes**  | `"pixel"`, `"crs"`                                                      | Image plane vs real-world CRS.  |
| `optional`                   | `boolean` | **Yes**  | `true`, `false`                                                         | If artifact is optional.        |
| `layer_hint.layer_type`      | `string`  | No       | `"raster_overlay"`, `"polygon_boundary"`, `"point_marker"`, `"heatmap"` | Visualization mode.             |
| `layer_hint.default_opacity` | `number`  | No       | `0.0` - `1.0` (e.g., `0.6`)                                             | Initial overlay opacity.        |
| `layer_hint.color_palette`   | `string`  | No       | `"Greens"`, `"Reds"`, `"Blues"`, `"Viridis"`, `"Jet"`                   | Raster colormap.                |
| `layer_hint.stroke_color`    | `string`  | No       | Hex color code (e.g., `"#2185d0"`)                                      | Vector outline color.           |
| `layer_hint.fill_opacity`    | `number`  | No       | `0.0` - `1.0` (e.g., `0.2`)                                             | Vector fill opacity.            |

---

### File 5: `execution_payload.json`

Schema URI: `https://aerial-analytics.local/schemas/execution_payload.v1.json`

This file models the payload delivered by the platform supervisor into the plugin's execution runner.

| Field                 | Type     | Required | Allowed Values                                           | Description                                                       |
| :-------------------- | :------- | :------- | :------------------------------------------------------- | :---------------------------------------------------------------- |
| `execution_id`        | `string` | **Yes**  | e.g., `"exec_20260914_001"`                              | Unique analysis execution ID.                                     |
| `session_id`          | `string` | No       | e.g., `"session_20260914_01"`                            | Parent flight session / project ID.                               |
| `output_dir`          | `string` | **Yes**  | Absolute path to folder                                  | Sandboxed directory where artifacts must be written.              |
| `target.granularity`  | `string` | **Yes**  | `"single_image"`, `"image_pair"`, `"batch"`, `"session"` | Granularity matching `inputs.json`.                               |
| `target.image_path`   | `string` | **Yes**  | Absolute filesystem path                                 | Path to source aerial RGB image.                                  |
| `target.mime_type`    | `string` | **Yes**  | `"image/jpeg"`, `"image/png"`, `"image/tiff"`            | MIME type of source image.                                        |
| `target.metadata.gps` | `object` | No       | `{"lat": number, "lon": number}`                         | Decoded GPS coordinates in WGS84 decimal degrees.                 |
| `aoi`                 | `object` | No       | RFC 7946 GeoJSON Object                                  | Optional user-selected bounding geometry.                         |
| `parameters`          | `object` | **Yes**  | Key-value dictionary                                     | Values configured by the user corresponding to `parameters.json`. |

---

### File 6: `execution_result.json`

Schema URI: `https://aerial-analytics.local/schemas/execution_result.v1.json`

This file models the structured response written by the plugin upon execution completion.

| Field                       | Type             | Required | Allowed Values                                                        | Description                                                               |
| :-------------------------- | :--------------- | :------- | :-------------------------------------------------------------------- | :------------------------------------------------------------------------ |
| `execution_id`              | `string`         | **Yes**  | Matching `execution_id`                                               | Confirms output corresponds to request.                                   |
| `status`                    | `string`         | **Yes**  | `"success"`, `"failure"`, `"warning"`                                 | Exit status. Failure must not crash host.                                 |
| `execution_time_ms`         | `number`         | **Yes**  | Integer $\ge 0$                                                       | Processing duration measured in milliseconds.                             |
| `metrics`                   | `object`         | **Yes**  | Key-value dictionary matching `outputs.json`                          | Computed scalar metrics for dashboard and reporting.                      |
| `artifacts.<key>.file_path` | `string`         | **Yes**  | Absolute filesystem path                                              | Location of generated raster mask, GeoJSON, or CSV file.                  |
| `artifacts.<key>.format`    | `string`         | **Yes**  | `"image/png"`, `"image/tiff"`, `"application/geo+json"`, `"text/csv"` | File format MIME type.                                                    |
| `artifacts.<key>.bounds`    | `number[]`       | No       | `[min_lon, min_lat, max_lon, max_lat]`                                | Georeferenced bounding box (RFC 7946) for Map Explorer raster projection. |
| `error`                     | `string \| null` | No       | Diagnostic error string or `null`                                     | Diagnostic details if status is `failure` or `warning`.                   |
