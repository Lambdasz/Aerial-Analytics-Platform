"""Feature 5 - Area-Based Tree Counting, plus polygon area for Feature 6."""

import math
from collections.abc import Mapping, Sequence
from typing import Any

from .errors import InvalidGeometryError
from .models import AreaCountOutput, GeoCoordinate

EARTH_RADIUS_M = 6_371_008.8

Position = tuple[float, float]  # (lon, lat)
Ring = tuple[Position, ...]
Polygon = tuple[Ring, ...]  # first ring = exterior, the rest = holes


def _parse_ring(raw: Any) -> Ring:
    if not isinstance(raw, list) or len(raw) < 4:
        raise InvalidGeometryError("A polygon ring needs at least 4 positions")
    try:
        ring = tuple((float(p[0]), float(p[1])) for p in raw)
    except (TypeError, ValueError, IndexError) as exc:
        raise InvalidGeometryError(f"Invalid position in ring: {exc}") from exc
    if ring[0] != ring[-1]:
        raise InvalidGeometryError("A polygon ring must be closed (first == last position)")
    return ring


def _parse_polygon(raw: Any) -> Polygon:
    if not isinstance(raw, list) or not raw:
        raise InvalidGeometryError("Polygon coordinates must be a non-empty list of rings")
    return tuple(_parse_ring(ring) for ring in raw)


def parse_polygons(boundary: Mapping[str, Any]) -> tuple[Polygon, ...]:
    """Accept a GeoJSON Feature, Polygon or MultiPolygon. Raise ``InvalidGeometryError`` otherwise."""
    geometry = boundary.get("geometry") if boundary.get("type") == "Feature" else boundary
    if not isinstance(geometry, Mapping):
        raise InvalidGeometryError("Feature has no geometry")
    kind = geometry.get("type")
    coords = geometry.get("coordinates")
    if kind == "Polygon":
        return (_parse_polygon(coords),)
    if kind == "MultiPolygon" and isinstance(coords, list) and coords:
        return tuple(_parse_polygon(p) for p in coords)
    raise InvalidGeometryError(f"Expected Polygon or MultiPolygon, got '{kind}'")


def plot_id_of(boundary: Mapping[str, Any], default: str = "aoi") -> str:
    """Read the plot id from a Feature (``properties.plot_id``, ``id`` or ``properties.name``)."""
    properties = boundary.get("properties") or {}
    for value in (properties.get("plot_id"), boundary.get("id"), properties.get("name")):
        if value is not None:
            return str(value)
    return default


def _point_in_ring(point: Position, ring: Ring) -> bool:
    """Ray casting: count how many edges a ray going east from ``point`` crosses."""
    x, y = point
    edges = zip(ring, ring[1:])
    crossings = sum(
        1
        for (x1, y1), (x2, y2) in edges
        if (y1 > y) != (y2 > y) and x < x1 + (y - y1) * (x2 - x1) / (y2 - y1)
    )
    return crossings % 2 == 1


def point_in_polygons(point: Position, polygons: Sequence[Polygon]) -> bool:
    return any(
        _point_in_ring(point, polygon[0])
        and not any(_point_in_ring(point, hole) for hole in polygon[1:])
        for polygon in polygons
    )


def count_trees_in_area(
    tree_points: Sequence[GeoCoordinate], plot_boundary: Mapping[str, Any]
) -> AreaCountOutput:
    """Count trees inside a plot boundary (GeoJSON, WGS84) from Module 9 or an AOI from Module 3.

    A plot without trees gives ``tree_count=0``. Invalid GeoJSON raises
    ``InvalidGeometryError``.
    """
    polygons = parse_polygons(plot_boundary)
    inside = sum(
        1 for p in tree_points if point_in_polygons((p.longitude, p.latitude), polygons)
    )
    return AreaCountOutput(
        plot_id=plot_id_of(plot_boundary),
        tree_count=inside,
        outside_count=len(tree_points) - inside,
    )


def _ring_area_m2(ring: Ring, lat0: float) -> float:
    """Shoelace formula on a local equirectangular projection around ``lat0``."""
    k = math.pi / 180 * EARTH_RADIUS_M
    xy = [(lon * k * math.cos(math.radians(lat0)), lat * k) for lon, lat in ring]
    return abs(sum(x1 * y2 - x2 * y1 for (x1, y1), (x2, y2) in zip(xy, xy[1:]))) / 2


def polygon_area_m2(boundary: Mapping[str, Any]) -> float:
    """Approximate area in m² of a WGS84 polygon (exterior minus holes)."""
    polygons = parse_polygons(boundary)
    lats = [lat for polygon in polygons for _, lat in polygon[0]]
    lat0 = sum(lats) / len(lats)
    return sum(
        _ring_area_m2(polygon[0], lat0) - sum(_ring_area_m2(h, lat0) for h in polygon[1:])
        for polygon in polygons
    )
