# Plugin Specification Reference

This directory contains the canonical starter template for analytical plugins in the **Aerial Analytics Platform**.

Copy this folder (`plugins/template/`) to create a new plugin for:

- **Module 4**: RGB Vegetation Detection
- **Module 7**: Tree Detection & Counting
- **Module 8**: RGB Land-Cover Classification
- Any custom community/open-source analytical plugin

---

## Validating Your Manifest

All plugin manifests must conform to the platform schema defined in [`schemas/plugin.schema.json`](../../schemas/plugin.schema.json). Manifest validation is enforced using [`check-jsonschema`](https://check-jsonschema.readthedocs.io/).

Before submitting or testing your plugin, validate your `manifest.json`.

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
# In PowerShell or Command Prompt:
pip install check-jsonschema
```

**Alternative (via `pipx`):**

```powershell
# 1. Install pipx:
pip install pipx

# 2. Install check-jsonschema:
pipx install check-jsonschema

# 3. Ensure PATH is configured and restart your terminal:
pipx ensurepath
```

> **Note for Windows:** If `check-jsonschema` is not recognized after installing with pip, you can invoke it directly through Python:
>
> ```powershell
> python -m check_jsonschema --schemafile schemas\plugin.schema.json plugins\<your_plugin_folder>\manifest.json
> ```

---

### 2. Running Validation

Run the validator from the root of the repository:

#### Linux / macOS:

```bash
# Validate your plugin manifest:
check-jsonschema --schemafile schemas/plugin.schema.json plugins/<your_plugin_folder>/manifest.json

# Example: Validate mock manifest
check-jsonschema --schemafile schemas/plugin.schema.json plugins/mock/manifest.json
```

#### Windows (PowerShell):

```powershell
# Validate your plugin manifest:
check-jsonschema --schemafile schemas\plugin.schema.json plugins\<your_plugin_folder>\manifest.json

# Example: Validate mock manifest
check-jsonschema --schemafile schemas\plugin.schema.json plugins\mock\manifest.json
```

---

### 3. Understanding the Output

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

## Field Reference & Allowed Values

### 0. Root Fields

| Field              | Type     | Required | Allowed / Example Values                                            | Description                                                     |
| :----------------- | :------- | :------- | :------------------------------------------------------------------ | :-------------------------------------------------------------- |
| `$schema`          | `string` | No       | URI (e.g., `https://aerial-analytics.local/schemas/plugin.v1.json`) | JSON Schema URI used by IDEs and validators for autocompletion. |
| `manifest_version` | `number` | **Yes**  | `1`                                                                 | Schema specification version (incremented on breaking changes). |

### 1. `metadata`

| Field              | Type       | Required | Allowed / Example Values                                                                                                                    | Description                          |
| :----------------- | :--------- | :------- | :------------------------------------------------------------------------------------------------------------------------------------------ | :----------------------------------- |
| `id`               | `string`   | **Yes**  | Pattern: `^[a-z0-9-]+$` (e.g., `rgb-vegetation-exg`)                                                                                        | Unique slug for this plugin.         |
| `name`             | `string`   | **Yes**  | e.g., `Tree Detection & Counting`                                                                                                           | Display name in UI.                  |
| `version`          | `string`   | **Yes**  | SemVer (e.g., `1.0.0`)                                                                                                                      | Version of the plugin.               |
| `author`           | `string`   | **Yes**  | e.g., `PBL Team 2026`                                                                                                                       | Author or team name.                 |
| `license`          | `string`   | **Yes**  | `"MIT"`, `"Apache-2.0"`, `"GPL-3.0"`, `"BSD-3-Clause"`, `"Proprietary"`                                                                     | Software license.                    |
| `description`      | `string`   | **Yes**  | e.g., `"Extracts vegetation masks using RGB Excess Green index."`                                                                           | Summary of analytical capability.    |
| `category`         | `string`   | **Yes**  | `"vegetation"`, `"tree_detection"`, `"tree_counting"`, `"land_cover"`, `"temporal"`, `"elevation"`, `"analytics"`, `"utility"`, `"general"` | Domain category for UI grouping.     |
| `tags`             | `string[]` | No       | `["rgb", "vegetation", "indices"]`                                                                                                          | Search and filtering keywords.       |
| `min_core_version` | `string`   | No       | e.g., `"0.1.0"`                                                                                                                             | Minimum compatible platform version. |

### 2. `runtime`

| Field                | Type             | Required  | Allowed / Example Values         | Description                        |
| :------------------- | :--------------- | :-------- | :------------------------------- | :--------------------------------- |
| `type`               | `string`         | **Yes**   | `"python"`, `"binary"`, `"wasm"` | Runtime execution engine.          |
| `min_python_version` | `string`         | If Python | e.g., `"3.10"`                   | Minimum Python version required.   |
| `entrypoint`         | `string`         | **Yes**   | e.g., `"main.py"`                | Path relative to plugin directory. |
| `dependencies_file`  | `string \| null` | No        | e.g., `"requirements.txt"`       | Python dependencies file.          |
| `healthcheck_flag`   | `string`         | No        | e.g., `"--healthcheck"`          | CLI argument to verify setup.      |

### 3. `inputs`

| Field                  | Type             | Required | Allowed Values                                             | Description                        |
| :--------------------- | :--------------- | :------- | :--------------------------------------------------------- | :--------------------------------- |
| `granularity`          | `string`         | **Yes**  | `"single_image"`, `"image_pair"`, `"batch"`, `"session"`   | Analysis scope per run.            |
| `supported_mime_types` | `string[]`       | **Yes**  | `"image/jpeg"`, `"image/png"`, `"image/tiff"`              | Supported image formats.           |
| `requires_metadata`    | `string[]`       | **Yes**  | Subset of `["resolution", "gps", "altitude", "timestamp"]` | Mandatory image metadata.          |
| `optional_metadata`    | `string[]`       | No       | Subset of `["resolution", "gps", "altitude", "timestamp"]` | Optional image metadata.           |
| `supports_aoi`         | `boolean`        | **Yes**  | `true`, `false`                                            | Whether AOI clipping is supported. |
| `aoi_format`           | `string \| null` | If AOI   | `"geojson_polygon"`, `"geojson_bbox"`                      | Geometry format for AOI.           |

### 4. `parameters[]`

| Field                  | Type       | Required  | Allowed Values                                  | Description                           |
| :--------------------- | :--------- | :-------- | :---------------------------------------------- | :------------------------------------ |
| `name`                 | `string`   | **Yes**   | e.g., `threshold`                               | Variable key in `input.json`.         |
| `label`                | `string`   | **Yes**   | e.g., `Index Threshold`                         | Form label in UI.                     |
| `type`                 | `string`   | **Yes**   | `"select"`, `"number"`, `"boolean"`, `"string"` | Blueprint.js form control type.       |
| `options`              | `string[]` | If select | e.g., `["ExG", "VARI"]`                         | Dropdown choices.                     |
| `default`              | `any`      | **Yes**   | Value matching `type`                           | Initial default value.                |
| `min` / `max` / `step` | `number`   | If number | Numerical bounds and step                       | Slider / spinner constraints.         |
| `description`          | `string`   | No        | Help text / tooltip                             | Explains what the parameter controls. |

### 5. `outputs.metrics[]`

| Field         | Type     | Required | Allowed Values                                  | Description                    |
| :------------ | :------- | :------- | :---------------------------------------------- | :----------------------------- |
| `key`         | `string` | **Yes**  | e.g., `vegetation_coverage_pct`                 | Key in output JSON `metrics`.  |
| `label`       | `string` | **Yes**  | e.g., `Vegetation Coverage`                     | Card label in Dashboard.       |
| `type`        | `string` | **Yes**  | `"float"`, `"integer"`, `"string"`, `"boolean"` | Metric data type.              |
| `unit`        | `string` | No       | `"%"`, `"px"`, `"items"`, `"m²"`, `"ha"`        | Unit displayed next to number. |
| `description` | `string` | No       | Explanatory note                                | Tooltip explanation.           |

### 6. `outputs.artifacts[]`

| Field                        | Type      | Required | Allowed Values                                                          | Description                               |
| :--------------------------- | :-------- | :------- | :---------------------------------------------------------------------- | :---------------------------------------- |
| `key`                        | `string`  | **Yes**  | e.g., `mask_raster`                                                     | Key in output JSON `artifacts`.           |
| `label`                      | `string`  | **Yes**  | e.g., `Vegetation Mask`                                                 | Layer title in Map Explorer.              |
| `type`                       | `string`  | **Yes**  | `"raster"`, `"vector"`, `"table"`                                       | Artifact category.                        |
| `format`                     | `string`  | **Yes**  | `"image/png"`, `"image/tiff"`, `"application/geo+json"`, `"text/csv"`   | File format MIME.                         |
| `coordinate_system`          | `string`  | **Yes**  | `"pixel"`, `"crs"`                                                      | Image coordinate plane vs real-world CRS. |
| `optional`                   | `boolean` | **Yes**  | `true`, `false`                                                         | If artifact is optional.                  |
| `layer_hint.layer_type`      | `string`  | No       | `"raster_overlay"`, `"polygon_boundary"`, `"point_marker"`, `"heatmap"` | Visualization mode.                       |
| `layer_hint.default_opacity` | `number`  | No       | `0.0` - `1.0` (e.g., `0.6`)                                             | Initial overlay opacity.                  |
| `layer_hint.color_palette`   | `string`  | No       | `"Greens"`, `"Reds"`, `"Blues"`, `"Viridis"`, `"Jet"`                   | Raster colormap.                          |
| `layer_hint.stroke_color`    | `string`  | No       | Hex color code (e.g., `"#2185d0"`)                                      | Vector outline color.                     |
| `layer_hint.fill_opacity`    | `number`  | No       | `0.0` - `1.0` (e.g., `0.2`)                                             | Vector fill opacity.                      |

### 7. `execution`

| Field             | Type             | Required | Allowed Values                        | Description                         |
| :---------------- | :--------------- | :------- | :------------------------------------ | :---------------------------------- |
| `timeout_seconds` | `number`         | **Yes**  | e.g., `120`                           | Max seconds before killing process. |
| `concurrency`     | `string`         | No       | `"sequential"`, `"parallel"`          | Concurrency scheduling model.       |
| `gpu.support`     | `string`         | No       | `"none"`, `"optional"`, `"required"`  | Hardware acceleration preference.   |
| `gpu.driver`      | `string \| null` | No       | `"cuda"`, `"rocm"`, `"metal"`, `null` | Target GPU driver.                  |
