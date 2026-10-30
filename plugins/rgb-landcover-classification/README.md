# Module 8 test — DeepLabV3+ land-cover plugin

This is a separate Module 8 implementation workspace. It classifies standard RGB aerial imagery with a DeepLabV3+ semantic-segmentation model and supports `resnet101` or `xception` encoders.

## Model requirement

The repository does not contain trained weights or a labelled aerial dataset. Put trained weights at the path given by `weights_path` (default: `models/deeplabv3plus_resnet101.pt`). The plugin never pretends that an untrained model is production-ready: healthcheck reports missing weights and inference exits with a structured failure.

## Contract

```bash
python main.py --healthcheck
python main.py --input payload.json --output result.json
python main.py --validate-parameters '{"backbone":"resnet101"}'
```

Outputs are `result.json`, `mask.png`, and `class_statistics.json` in `output_dir`. The payload and result follow the platform's Module 2 execution contract.

The plugin emits `PROGRESS:{json}` lines for the Module 2 supervisor. Standard
class metrics remain fixed so the declared `outputs.json` contract is stable;
custom classes are returned in `class_statistics.json`.

When `target.metadata.georeference` contains WGS84 bounds, the `boundary_vector`
artifact is emitted as `[longitude, latitude]` GeoJSON for Module 3. The
metadata shape is `{ "crs": "EPSG:4326", "bounds": [min_lon, min_lat, max_lon, max_lat] }`.
Without that georeference, the artifact remains explicitly marked as pixel
coordinates and must not be placed directly on a geographic map.

## Backbones

- `resnet101`: torchvision ResNet-101 encoder.
- `xception`: timm Xception encoder (`timm:xception41` by default).

Both require trained DeepLabV3+ weights matching the selected backbone and five output classes.
