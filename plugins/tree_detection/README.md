# Tree Detection & Counting (Module 7)

An Aerial Analytics Platform analytics plugin that detects tree crowns in RGB drone imagery,
filters the results by confidence, maps each tree to WGS84 coordinates, and then counts the trees
and computes their density.

- **Plugin ID:** `tree-detection-counting`
- **Category:** `tree_detection`
- **Runtime:** Python ≥ 3.10
- **Inter-module contract:** [CONTRACT.md](CONTRACT.md) (read this if you work on Module 2, 3, 9, 10, or 11)

> **Status:** three real detectors are registered. `opencv_watershed` (the default) uses classic
> image processing and runs right away without model weights. `deepforest` is a pre-trained crown
> detection model that works without training your own; its fine-tuning lives in
> [training/](training/README.md). `deeplabv3plus` is wired into the registry, but only works once
> a trained checkpoint, torch, and segmentation-models-pytorch are available.
> `stub` is still there, and its results get the `warning` status so they are not mistaken for
> real results.

## Structure

```text
plugins/tree_detection/
├── manifest.yaml            # Contract 1: plugin identity & runtime
├── parameters.yaml          # Contract 2: user-adjustable parameters (Module 2 form)
├── inputs.yaml              # Contract 3: required inputs (image formats, metadata, AOI)
├── outputs.yaml             # Contract 4: produced metrics & artifacts
├── execution_payload.yaml   # Fixture: sample payload from the host
├── execution_result.yaml    # Fixture: sample result written by the plugin
├── main.py                  # CLI entrypoint (I/O layer)
├── requirements.txt
├── CONTRACT.md              # Module 7 contract document for other modules
├── treedet/                 # Core logic (pure functions)
│   ├── models.py            # Shared data models (Pydantic v2, immutable)
│   ├── errors.py            # PluginExecutionError and its subclasses
│   ├── detectors.py         # Registry + opencv_watershed and deeplabv3plus detectors
│   ├── detection.py         # Feature 1: detect_trees (+ cross-tile deduplication)
│   ├── counting.py          # Feature 2: count_trees
│   ├── filtering.py         # Feature 3: filter_by_confidence
│   ├── location.py          # Feature 4: map_tree_locations
│   ├── area.py              # Feature 5: count_trees_in_area (+ polygon area)
│   ├── density.py           # Feature 6: calculate_tree_density
│   ├── review.py            # Feature 7: apply_review
│   ├── output.py            # Feature 8: build_plugin_output → execution_result
│   ├── pipeline.py          # Payload → the features above
│   └── integration.py       # M9/M10 connectors: per-plot summary, two-run comparison
├── training/                # Fetch UAV-VisLoc, label, fine-tune deepforest (not installed by the platform)
└── tests/                   # Unit & end-to-end tests (unittest)
```

## Setup

Run from the repo root:

```powershell
py -3.10 -m venv $HOME\.venvs\aerial-m7
& $HOME\.venvs\aerial-m7\Scripts\Activate.ps1
pip install -r plugins\tree_detection\requirements.txt
pip install check-jsonschema   # for contract validation
```

On Linux/macOS: `python3.10 -m venv ~/.venvs/aerial-m7 && source ~/.venvs/aerial-m7/bin/activate`.

> Keep the virtual environment **outside the repo**. Prettier in the pre-commit hook
> (`npm run format:check`) does not read `.gitignore` files in subfolders, so a venv inside
> `plugins/` gets checked too and makes the commit fail.

## Running

```bash
# Healthcheck (called by Module 2 before showing the plugin as active)
python plugins/tree_detection/main.py --healthcheck

# Analysis
python plugins/tree_detection/main.py --input <execution_payload.yaml> --output <result.yaml>
```

The `execution_payload.yaml` in this folder holds a placeholder image path (`/absolute/path/...`),
so running it as-is yields `status: "failure"` with the message `Image not found`. That is on
purpose and shows the plugin fails cleanly without crashing. Replace `target.image_path` with a
real image to try the success path.

## Detectors

| `detector`         | Requires                                                        | When to use                                               |
| :----------------- | :-------------------------------------------------------------- | :-------------------------------------------------------- |
| `opencv_watershed` | numpy, opencv-python-headless                                   | default; ordinary RGB imagery, same output for same input |
| `deepforest`       | deepforest (brings torch); optional `.pt` checkpoint            | deep-learning crown detection without training your own   |
| `deeplabv3plus`    | torch, segmentation-models-pytorch, a trained `.pth` checkpoint | once weights are available; ResNet101 or Xception encoder |
| `stub`             | -                                                               | testing the pipeline without detection                    |

`--healthcheck` reports torch, deepforest, and segmentation-models-pytorch as
`optional_dependencies`: if they are all missing the plugin is still healthy, because the default
detector does not use them.

`deepforest` downscales the photo to 10 cm/px (the model's training resolution) based on
`gsd_cm`. Set `gsd_cm` correctly: on UAV-VisLoc flight 01 photos (about 5 cm/px), without
rescaling the boxes cover only half a crown. On a CPU, one 4000×2650 px photo takes about
40 seconds.

## Tests

```bash
cd plugins/tree_detection
python -m unittest discover -s tests -v
```

71 tests: 20 for the eight features and the payload flow, 21 for the `opencv_watershed` and
`deeplabv3plus` detectors on synthetic imagery, 9 for the `deepforest` detector (without a model),
12 for the training tools in `training/`, and 9 for the M9/M10 connectors.

The tests cover all eight features, including the edge cases from the specification: no
detections (`0`, not an error), a threshold outside 0–1 (rejected by Pydantic), zero area
(`ZeroAreaError`), invalid GeoJSON (`InvalidGeometryError`), a missing image
(`PluginExecutionError`), and an unsupported format (`UnsupportedFormatError`).

## Contract validation

From the repo root:

```bash
check-jsonschema --schemafile schemas/plugin.schema.yaml            plugins/tree_detection/manifest.yaml
check-jsonschema --schemafile schemas/parameters.schema.yaml        plugins/tree_detection/parameters.yaml
check-jsonschema --schemafile schemas/inputs.schema.yaml            plugins/tree_detection/inputs.yaml
check-jsonschema --schemafile schemas/outputs.schema.yaml           plugins/tree_detection/outputs.yaml
check-jsonschema --schemafile schemas/execution_payload.schema.yaml plugins/tree_detection/execution_payload.yaml
check-jsonschema --schemafile schemas/execution_result.schema.yaml  plugins/tree_detection/execution_result.yaml
```

## Design (functional programming)

- **Pure core, impure shell.** Every function in `treedet/` is pure: the same input always gives
  the same output, with no I/O. The only exception is `detect_trees`, because the detector has to
  read the image. File reads/writes, the clock, and exit codes live only in `main.py`.
- **Immutable data.** All Pydantic models use `frozen=True`, and lists are `tuple`s. Functions
  never mutate their input; changes are made through `model_copy(update=...)`.
- **Time as a parameter.** `count_trees` and `analyze` take `counted_at` from the caller instead
  of reading the clock themselves, so results are testable and reproducible.
- **Detectors as functions.** A detection model is a plain function injected through a registry,
  so tests use fake detectors without a real model.
- **Isolated errors.** Every `PluginExecutionError` subclass is caught by `main.py` and written as
  a `failure` result. The plugin never crashes without writing a result (a Module 2 requirement).

## Adding a detection model

1. Write a `(image_path: str, model_config: Mapping) -> Sequence[tuple[BBox, float]]` function in
   `treedet/detectors.py`. It returns crown boxes in pixels together with a 0–1 confidence.
2. Register it in `DETECTORS`, e.g. `"opencv_watershed": opencv_watershed_detector`.
3. Add its name to the `options` of the `detector` parameter in `parameters.yaml`.
4. Add its packages to `requirements.txt` and to `check_dependencies()` in `main.py`.
5. If it uses a GPU, adjust `execution.gpu` in `manifest.yaml`.

The detector handles tiling (the size is in `model_config["tile_size"]`) and inference.
Cross-tile deduplication, confidence filtering, georeferencing, and counting are already handled
by the pipeline.
