"""Feature 4 - Tree Location Mapping.

Converts pixel positions to WGS84 using the image center GPS, ground sampling
distance (GSD) and rotation. This is a flat-earth approximation that is accurate
for a single nadir drone photo (a few hundred metres across).

TODO(module-7): for orthomosaics / GeoTIFFs, read the affine transform with
rasterio and project with pyproj instead of ``pixel_to_geo``.
"""

import json
import math
from collections.abc import Mapping, Sequence
from typing import Any

from .models import GeoCoordinate, ImageGeoreference, LocationOutput, TreeDetection

METERS_PER_DEGREE_LAT = 111_320.0
OUTPUT_CRS = "EPSG:4326"


def pixel_to_geo(x: float, y: float, georef: ImageGeoreference) -> GeoCoordinate:
    """Map a pixel (x right, y down) to latitude/longitude."""
    # Offset from image center in metres, with +y pointing to the top of the image.
    right_m = (x - georef.width_px / 2) * georef.gsd_m
    up_m = (georef.height_px / 2 - y) * georef.gsd_m

    theta = math.radians(georef.rotation_deg)
    east_m = right_m * math.cos(theta) + up_m * math.sin(theta)
    north_m = -right_m * math.sin(theta) + up_m * math.cos(theta)

    lat0 = georef.center.latitude
    meters_per_degree_lon = METERS_PER_DEGREE_LAT * math.cos(math.radians(lat0))
    return GeoCoordinate(
        latitude=lat0 + north_m / METERS_PER_DEGREE_LAT,
        longitude=georef.center.longitude + east_m / meters_per_degree_lon,
    )


def locate_detection(detection: TreeDetection, georef: ImageGeoreference) -> TreeDetection:
    """Return a copy of ``detection`` with ``center`` and ``area_m2`` filled in."""
    x, y = detection.bbox.center
    return detection.model_copy(
        update={
            "center": pixel_to_geo(x, y, georef),
            "area_m2": detection.bbox.area_px * georef.gsd_m**2,
        }
    )


def locate_detections(
    detections: Sequence[TreeDetection], georef: ImageGeoreference
) -> tuple[TreeDetection, ...]:
    return tuple(locate_detection(d, georef) for d in detections)


def to_feature(detection: TreeDetection) -> dict[str, Any]:
    """One detection as a GeoJSON Point Feature (coordinates are [lon, lat])."""
    assert detection.center is not None, "detection must be located first"
    return {
        "type": "Feature",
        "id": detection.detection_id,
        "geometry": {
            "type": "Point",
            "coordinates": [detection.center.longitude, detection.center.latitude],
        },
        "properties": {
            "detection_id": detection.detection_id,
            "confidence": detection.confidence,
            "area_m2": detection.area_m2,
            "bbox_px": [
                detection.bbox.x_min,
                detection.bbox.y_min,
                detection.bbox.x_max,
                detection.bbox.y_max,
            ],
        },
    }


def parse_georeference(image_metadata: Mapping[str, Any]) -> ImageGeoreference:
    """Build an ``ImageGeoreference`` from Module 1 metadata.

    Expected keys: ``gps`` ({lat, lon}), ``resolution`` ([width, height]),
    ``gsd_m``, optional ``rotation_deg``.
    """
    gps = image_metadata["gps"]
    width, height = image_metadata["resolution"]
    return ImageGeoreference(
        center=GeoCoordinate(latitude=gps["lat"], longitude=gps["lon"]),
        gsd_m=image_metadata["gsd_m"],
        width_px=int(width),
        height_px=int(height),
        rotation_deg=float(image_metadata.get("rotation_deg", 0.0)),
    )


def to_location_output(located: Sequence[TreeDetection]) -> LocationOutput:
    """Build the GeoJSON FeatureCollection from detections that already have a ``center``."""
    collection = {
        "type": "FeatureCollection",
        "features": [to_feature(d) for d in located],
    }
    return LocationOutput(
        tree_points_geojson=json.dumps(collection),
        crs=OUTPUT_CRS,
        detection_ids=tuple(d.detection_id for d in located),
    )


def map_tree_locations(
    detections: Sequence[TreeDetection], image_metadata: Mapping[str, Any]
) -> LocationOutput:
    """Turn pixel detections into a GeoJSON FeatureCollection string for Module 3."""
    return to_location_output(
        locate_detections(detections, parse_georeference(image_metadata))
    )
