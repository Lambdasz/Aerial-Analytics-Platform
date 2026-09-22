"""Fitur 5: Boundary Vectorization."""

from __future__ import annotations

import json

import numpy as np

from .geo import assert_lon_lat
from .models import ClassDefinition, PluginExecutionError


def generate_boundary_vector(
    class_mask: np.ndarray, classes: list[ClassDefinition], transform_to_crs
) -> str | None:
    """Ubah mask kelas menjadi FeatureCollection GeoJSON (RFC 7946, [lon, lat], WGS84).

    Mengembalikan None bila `transform_to_crs` tidak tersedia (citra tanpa
    georeferensi) atau tidak ada poligon: lebih baik tidak mengirim vektor
    daripada mengirim koordinat yang salah.
    """
    if transform_to_crs is None:
        return None
    try:
        from rasterio.features import shapes
    except ImportError as exc:
        raise PluginExecutionError("Vektorisasi membutuhkan library rasterio") from exc

    features = []
    for c in classes:
        binary = (class_mask == c.id).astype(np.uint8)
        if not binary.any():
            continue
        for geom, _ in shapes(binary, mask=binary.astype(bool), transform=transform_to_crs, connectivity=4):
            assert_lon_lat(geom["coordinates"])
            features.append(
                {
                    "type": "Feature",
                    "geometry": geom,
                    "properties": {"class_id": c.id, "class_name": c.name, "color": c.color},
                }
            )
    if not features:
        return None
    return json.dumps({"type": "FeatureCollection", "features": features})
