# Module 7 Contract: Tree Detection & Counting

> For: the Module 2 (Plugin Manager), 3 (Map Explorer), 9 (Area & Plot), 10 (Temporal), and 11 (Dashboard) teams.
> Status: **v0.5.0**. Three real detectors are registered (`opencv_watershed` as the default,
> `deepforest`, and `deeplabv3plus`). The [To be agreed](#6-to-be-agreed) section is still waiting for
> answers from the teams involved. Field names in this document match the ones used in the code exactly.

This document describes what Module 7 **accepts** and **produces**. Other modules do not need to
read Module 7's Python code to use it.

## 1. How Module 7 is called

Module 7 does not expose functions that Rust or React call directly. The plugin is run by
Module 2 as a **separate process**, following the `plugins/template` template:

```text
python main.py --healthcheck                                  → exit 0 healthy / 1 unhealthy
python main.py --input <payload.yaml> --output <result.yaml>  → exit 0 finished / 1 failed
```

All data is exchanged through YAML files validated against the schemas in `schemas/`:

```text
M1 (image+metadata) ──┐
M3 (AOI) ─────────────┼─► M2 builds execution_payload.yaml ─► Module 7 ─► execution_result.yaml
                      │                                               ├─► <id>_tree_points.geojson
                      │                                               └─► <id>_plugin_output.json
                      └──────────────────── M3 / M9 / M10 / M11 read the results through M2
```

If the plugin fails (missing image, broken GeoJSON, and so on), it **still** writes
`result.yaml` with `status: "failure"` and a message in `error`, then exits with code 1.

## 2. Input (from Modules 1, 2, and 3)

Follows `schemas/execution_payload.schema.yaml`. Fields used by Module 7:

| Payload field                         | Required | Source    | Used for                                                                 |
| :------------------------------------ | :------: | :-------- | :----------------------------------------------------------------------- |
| `execution_id`                        |   yes    | M2        | artifact file names & the result's `execution_id`                        |
| `session_id`                          |    no    | M1/M2     | `project_id` in the result metadata                                      |
| `output_dir`                          |   yes    | M2        | folder the artifacts are written to                                      |
| `target.image_path`                   |   yes    | M1        | RGB image (`.jpg` `.jpeg` `.png` `.tif` `.tiff`)                         |
| `target.metadata.gps` `{lat, lon}`    | **yes**  | M1        | image center position (WGS84)                                            |
| `target.metadata.resolution` `[w, h]` | **yes**  | M1        | image size in pixels                                                     |
| `target.metadata.gsd_m`               |   no\*   | M1        | meters per pixel                                                         |
| `target.metadata.rotation_deg`        |    no    | M1        | clockwise rotation from north to the image's top edge, default `0`       |
| `target.metadata.image_id`            |    no    | M1        | image id; default: file name without extension                           |
| `aoi.geometry`                        |    no    | M3        | WGS84 GeoJSON `Polygon`/`MultiPolygon`; only trees inside it are counted |
| `parameters.*`                        |   yes    | M2 (form) | see the parameter table below                                            |

\* If `gsd_m` is missing, the `gsd_cm` parameter is used (default 3 cm/px).

**Parameters** (`parameters.yaml`, rendered by Module 2 as a form):

| `name`                  | Type        | Default            | Description                                               |
| :---------------------- | :---------- | :----------------- | :-------------------------------------------------------- |
| `confidence_threshold`  | number 0–1  | `0.5`              | detections below this value are discarded                 |
| `density_unit`          | select      | `per_ha`           | `per_ha` or `per_m2`                                      |
| `detector`              | select      | `opencv_watershed` | `opencv_watershed`, `deepforest`, `deeplabv3plus`, `stub` |
| `model_path`            | string      | `""`               | required for `deeplabv3plus`; optional for `deepforest`   |
| `backbone`              | select      | `resnet101`        | `resnet101` or `xception`; must match training            |
| `probability_threshold` | number 0–1  | `0.5`              | pixels above this value count as crown (DeepLabV3+)       |
| `iou_threshold`         | number 0–1  | `0.5`              | overlap limit for cross-tile deduplication                |
| `tile_size`             | number (px) | `1024`             | inference tile size                                       |
| `gsd_cm`                | number      | `3.0`              | fallback when the metadata has no `gsd_m`                 |

`opencv_watershed` does not read `model_path`, `backbone`, or `probability_threshold`. The
`deeplabv3plus` detector refuses to run without a checkpoint instead of returning 0 trees, so an
empty result is not confused with a plantation that really is empty.

`deepforest` uses the released model when `model_path` is empty, or a checkpoint produced by
`training/train_deepforest.py` when it is set. This detector downscales the photo by
`gsd_cm / 10` to match the model's training resolution, and always uses 400 px patches (it does
not read `tile_size`).

## 3. Output

### 3.1 `execution_result.yaml` (read by Module 2, then passed on to other modules)

`status`: `success`, `warning` (finished but with notes, e.g. the `stub` detector), or `failure`.

**Metrics** (`metrics`):

| Key                      | Type    | Meaning                                                                       |
| :----------------------- | :------ | :---------------------------------------------------------------------------- |
| `tree_count`             | integer | number of trees after the confidence filter, **inside the AOI** if one is set |
| `avg_confidence`         | float   | mean confidence of the counted trees (0–1)                                    |
| `removed_low_confidence` | integer | detections discarded for being below the threshold                            |
| `trees_outside_aoi`      | integer | detections outside the AOI (`0` without an AOI)                               |
| `analysis_area_m2`       | float   | AOI area, or the image footprint area without an AOI                          |
| `tree_density`           | float   | `tree_count / analysis_area_m2` in `tree_density_unit`                        |
| `tree_density_unit`      | string  | `per_ha` or `per_m2`                                                          |

**Artifacts** (`artifacts`):

| Key             | Format                 | Contents                                 |
| :-------------- | :--------------------- | :--------------------------------------- |
| `tree_points`   | `application/geo+json` | one point per tree (see 3.2)             |
| `plugin_output` | `application/json`     | the full `PluginOutput` result (see 3.3) |

### 3.2 `tree_points`: GeoJSON FeatureCollection

One `Feature` per counted tree. Coordinates are `[longitude, latitude]` in EPSG:4326 (RFC 7946).

```json
{
  "type": "FeatureCollection",
  "features": [
    {
      "type": "Feature",
      "id": "DJI_0042-deeplabv3plus-0001",
      "geometry": { "type": "Point", "coordinates": [116.8535, -1.2445] },
      "properties": {
        "detection_id": "DJI_0042-deeplabv3plus-0001",
        "confidence": 0.93,
        "area_m2": 18.4,
        "bbox_px": [1990.0, 1490.0, 2010.0, 1510.0]
      }
    }
  ]
}
```

- `detection_id` has the format `{image_id}-{model_name}-{4-digit number}`. Detections added
  manually through Detection Review start with `manual-`.
- `area_m2` is the estimated crown box area (`bbox pixel area × gsd_m²`).
- `bbox_px` = `[x_min, y_min, x_max, y_max]` in original image pixels, used to draw boxes over
  the photo.

### 3.3 `plugin_output`: full structure

Matches the `PluginOutput` model in the Module 7 progress report (`treedet/models.py`):

```json
{
  "status": "success",
  "detections": [
    {
      "detection_id": "...",
      "bbox": { "x_min": 0, "y_min": 0, "x_max": 0, "y_max": 0 },
      "confidence": 0.93,
      "center": { "latitude": -1.2445, "longitude": 116.8535 },
      "area_m2": 18.4
    }
  ],
  "statistics": {
    "total_trees": 42,
    "avg_confidence": 0.81,
    "counted_at": "2026-09-21T09:15:00+00:00",
    "density_value": 5.03,
    "density_unit": "per_ha",
    "area_m2": 83470.2,
    "removed_low_confidence": 7,
    "threshold_used": 0.5,
    "plot_id": "aoi",
    "trees_outside_area": 5
  },
  "geojson": "<string containing the tree_points above>",
  "metadata": {
    "plugin_version": "0.5.0",
    "model_name": "deeplabv3plus",
    "model_version": null,
    "executed_at": "2026-09-21T09:15:00+00:00",
    "device": "cpu",
    "image_id": "DJI_0042",
    "project_id": "session_20260921_01",
    "processing_time_s": 2.1,
    "warnings": []
  }
}
```

`PluginOutput` statuses are mapped to the official schema: `success → success`, `partial → warning`, `failed → failure`.

## 4. Per consuming module

| Module                | What it uses                                                                            | Notes                                                                                                                                                                                                                                                                                                                                        |
| :-------------------- | :-------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **M2** Plugin Manager | The CLI in section 1; `manifest.yaml`, `parameters.yaml`, `inputs.yaml`, `outputs.yaml` | all pass `check-jsonschema` validation against `schemas/`                                                                                                                                                                                                                                                                                    |
| **M3** Map Explorer   | the `tree_points` artifact                                                              | layer_hint `point_marker`. The M3 example in `docs/examples/spatial-result.example.json` uses the `spatial_results` format; M3 only needs to turn each `Feature` into `{source, geometry, properties}` with `source = "tree-detection-counting"`. `height_est_m` is **not** available because a single RGB image gives no height information |
| **M9** Area & Plot    | `tree_points` or `plugin_output.detections[].center`                                    | count per plot with point-in-polygon against M9's `Plot.rings`, or call `plot_summaries` with the full plot list. Alternative: M2 runs Module 7 with `aoi` = one plot, and `tree_count` = the trees in that plot                                                                                                                             |
| **M10** Temporal      | the `tree_count` metric from two executions (different dates)                           | compare only if `confidence_threshold`, `detector`, and the AOI are the same; `compare_plot_summaries` sums the differences per `plot_id`                                                                                                                                                                                                    |
| **M11** Dashboard     | all `metrics` + `plugin_output.statistics`                                              | labels & units are available in `outputs.yaml`                                                                                                                                                                                                                                                                                               |

## 5. Public functions inside the plugin

For fellow Python developers or testers, the `treedet` package exports the functions from the
Module 7 progress report. All of them are pure, except `detect_trees`.

| Feature                 | Function                                                                                   |
| :---------------------- | :----------------------------------------------------------------------------------------- |
| 1 Tree Detection        | `detect_trees(input_data, model_config) -> DetectionOutput`                                |
| 2 Tree Counting         | `count_trees(detections, image_id, counted_at) -> CountOutput`                             |
| 3 Confidence Filtering  | `filter_by_confidence(detections, threshold) -> FilteredOutput`                            |
| 4 Tree Location Mapping | `map_tree_locations(detections, image_metadata) -> LocationOutput`                         |
| 5 Area-Based Counting   | `count_trees_in_area(tree_points, plot_boundary) -> AreaCountOutput`                       |
| 6 Tree Density          | `calculate_tree_density(tree_count, area_m2, unit="per_ha") -> DensityOutput`              |
| 7 Detection Review      | `apply_review(detections, corrections) -> ReviewOutput`                                    |
| 8 Plugin Output         | `build_plugin_output(detections, count, density, locations, run_metadata) -> PluginOutput` |
| M9 connector            | `plot_summaries(tree_points, plot_boundaries, unit="per_ha") -> tuple[dict, ...]`          |
| M10 connector           | `compare_plot_summaries(before, after) -> tuple[dict, ...]`                                |
| Artifact reader         | `tree_points_of(tree_points_geojson) -> tuple[GeoCoordinate, ...]`                         |

Differences from the progress report, with the reasons:

- `manifest.yaml` uses the official `schemas/plugin.schema.yaml` format. That schema rejects the
  `input_schema`, `output_schema`, and `config` fields from the report's appendix, so their
  contents moved to `inputs.yaml`, `outputs.yaml`, and `parameters.yaml`.
- `count_trees` takes `image_id` and `counted_at` as arguments. Without them, `image_id` cannot be
  known when there are no detections, and reading the clock inside the function makes it impure.
- `TreeDetection.center` and `area_m2` are `null` before Feature 4 runs, because the detector only
  knows pixel positions.
- `image_id` and `project_id` are not in the official payload schema, so they are taken from
  `target.metadata.image_id` (or the file name) and `session_id`.
- The `partial`/`failed` statuses are mapped to `warning`/`failure` per the official result schema.

## 6. To be agreed

|  #  | Question                                                                                                                                                                             | With          | Module 7 proposal                                                                                                                  |
| :-: | :----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :------------ | :--------------------------------------------------------------------------------------------------------------------------------- |
|  1  | Can M1 fill `gsd_m`, `rotation_deg`, and `image_id` in `target.metadata`? GSD can be computed from relative altitude + focal length + sensor size; rotation from `gimbal_yaw_degree` | M1, M2        | yes, with field names exactly as in the input table                                                                                |
|  2  | Per-plot counting: aggregate points in M9, or have M2 run Module 7 per plot?                                                                                                         | M2, M9        | aggregate in M9 (one execution for all plots)                                                                                      |
|  3  | Detection Review needs a place to store user corrections and a way to send them to the plugin. This needs a new Tauri command in the core                                            | M2            | M2 adds a command to save corrections; Module 7 receives the correction file path through a parameter                              |
|  4  | Who converts the GeoJSON to M3's `spatial_results` format?                                                                                                                           | M3            | M3 (the plugin keeps emitting standard GeoJSON)                                                                                    |
|  5  | Which detection model to use (classic OpenCV or deep learning such as DeepLabV3+)?                                                                                                   | lecturer/team | decided on DeepLabV3+ with a ResNet101 or Xception encoder; `opencv_watershed` stays the default until a trained checkpoint exists |
|  6  | Should the plugin live in this repo (`plugins/tree_detection/`) or a separate repo? `CONTRIBUTING.md` says analytics plugins live in a separate repo                                 | team          | in this repo for now, following `plugins/mock`                                                                                     |

## 7. Version history

| Version | Date       | Changes                                                                                                           |
| :------ | :--------- | :---------------------------------------------------------------------------------------------------------------- |
| 0.5.0   | 2026-11-01 | `deepforest` detector (released model or a fine-tuned checkpoint via `model_path`)                                |
| 0.4.0   | 2026-10-21 | M9/M10 connectors (`plot_summaries`, `compare_plot_summaries`, `tree_points_of`); metrics and artifacts unchanged |
| 0.3.0   | 2026-10-21 | `deeplabv3plus` detector with `model_path`, `backbone`, `probability_threshold`                                   |
| 0.2.0   | 2026-10-18 | `opencv_watershed` becomes the default detector; `manifest.yaml` bumped to 0.2.0                                  |
| 0.1.0   | 2026-09-21 | Initial draft: plugin skeleton, input/output contract, `stub` detector                                            |
