"""Georeferensi citra dan AOI (dipakai main.py, Fitur 5, dan Fitur 6).

Semua koordinat keluaran memakai WGS84 (EPSG:4326) dengan urutan [lon, lat]
sesuai RFC 7946 dan Kontrak Module 3. Modul ini membutuhkan `rasterio`; bila
tidak terpasang, citra dianggap tidak memiliki georeferensi.
"""

from __future__ import annotations

from typing import Iterator, NamedTuple

import numpy as np

from .models import PluginExecutionError


class GeoRef(NamedTuple):
    transform: object  # affine.Affine: piksel -> (lon, lat)
    bounds: list[float]  # [min_lon, min_lat, max_lon, max_lat]
    approximate: bool  # True bila CRS asal bukan EPSG:4326 (dihitung dari bounds)


def read_wgs84_georef(image_path: str) -> GeoRef | None:
    """Baca georeferensi raster; None bila tidak ada, berotasi, atau rasterio tidak terpasang.

    Untuk raster ber-CRS proyeksi (mis. UTM), transform diaproksimasi dari bounds
    yang direproyeksi ke WGS84 (cukup akurat untuk area drone berukuran kecil).
    """
    try:
        import rasterio
        from rasterio.transform import from_bounds
        from rasterio.warp import transform_bounds
    except ImportError:
        return None
    try:
        with rasterio.open(image_path) as src:
            t = src.transform
            if src.crs is None or t.is_identity or t.b != 0 or t.d != 0 or t.e >= 0:
                return None
            if src.crs.to_epsg() == 4326:
                b = src.bounds
                return GeoRef(t, [b.left, b.bottom, b.right, b.top], False)
            w, s, e, n = transform_bounds(src.crs, "EPSG:4326", *src.bounds, densify_pts=21)
            return GeoRef(from_bounds(w, s, e, n, src.width, src.height), [w, s, e, n], True)
    except Exception:
        return None


def _positions(coords) -> Iterator[tuple[float, float]]:
    if coords and isinstance(coords[0], (int, float)):
        yield coords[0], coords[1]
    else:
        for item in coords:
            yield from _positions(item)


def assert_lon_lat(coords) -> None:
    """Pastikan urutan [lon, lat] masuk akal (Temuan 3: jangan tertukar dengan [lat, lng])."""
    for x, y in _positions(coords):
        if not (-180.0 <= x <= 180.0 and -90.0 <= y <= 90.0):
            raise PluginExecutionError(
                f"Koordinat ({x}, {y}) di luar rentang [lon, lat]; periksa urutan koordinat"
            )


def aoi_pixel_mask(aoi_geometry: dict, transform, shape: tuple[int, int]) -> np.ndarray:
    """Mask boolean (True = di dalam AOI) berukuran `shape` (tinggi, lebar)."""
    if aoi_geometry.get("type") not in ("Polygon", "MultiPolygon"):
        raise PluginExecutionError("AOI harus berupa geometri Polygon atau MultiPolygon")
    coords = aoi_geometry.get("coordinates")
    if not coords:
        raise PluginExecutionError("AOI tidak memiliki koordinat")
    assert_lon_lat(coords)
    from rasterio.features import geometry_mask

    mask = geometry_mask([aoi_geometry], out_shape=shape, transform=transform, invert=True)
    if not mask.any():
        raise PluginExecutionError("AOI tidak beririsan dengan area citra")
    return mask
