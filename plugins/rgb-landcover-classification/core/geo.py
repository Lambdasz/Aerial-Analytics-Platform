from __future__ import annotations
from .models import PluginExecutionError

def read_wgs84_georef(image_path: str):
    try:
        import rasterio
        from rasterio.warp import transform_bounds
    except ImportError:
        return None
    try:
        with rasterio.open(image_path) as source:
            if source.crs is None or source.transform.is_identity:
                return None
            bounds = source.bounds
            if source.crs.to_epsg() != 4326:
                bounds = transform_bounds(source.crs, "EPSG:4326", *bounds)
            return {"transform": source.transform, "bounds": list(bounds), "crs": "EPSG:4326"}
    except Exception:
        return None

def aoi_pixel_mask(aoi_geometry: dict, transform, shape: tuple[int, int]):
    try:
        from rasterio.features import geometry_mask
        if aoi_geometry.get("type") not in {"Polygon", "MultiPolygon"}:
            raise PluginExecutionError("AOI must be Polygon or MultiPolygon")
        mask = geometry_mask([aoi_geometry], out_shape=shape, transform=transform, invert=True)
        if not mask.any():
            raise PluginExecutionError("AOI does not intersect image")
        return mask
    except ImportError as exc:
        raise PluginExecutionError("AOI requires rasterio") from exc

def assert_lon_lat(coords):
    def walk(value):
        if isinstance(value, list) and len(value) >= 2 and all(isinstance(x, (int, float)) for x in value[:2]):
            if not (-180 <= value[0] <= 180 and -90 <= value[1] <= 90):
                raise PluginExecutionError("coordinates must be [longitude, latitude]")
        elif isinstance(value, list):
            for child in value: walk(child)
    walk(coords)
