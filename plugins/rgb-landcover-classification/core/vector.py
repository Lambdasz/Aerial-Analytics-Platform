from __future__ import annotations
import json

def generate_boundary_vector(class_mask, classes, transform_to_crs):
    if transform_to_crs is None:
        return None
    try:
        import numpy as np
        from rasterio.features import shapes
    except ImportError as exc:
        raise RuntimeError("boundary vectorization requires numpy and rasterio") from exc
    features = []
    for item in classes:
        binary = (class_mask == item.id).astype(np.uint8)
        for geometry, _ in shapes(binary, mask=binary.astype(bool), transform=transform_to_crs):
            features.append({"type": "Feature", "geometry": geometry, "properties": {"class_id": item.id, "class_name": item.name, "color": item.color}})
    return json.dumps({"type": "FeatureCollection", "features": features}) if features else None
