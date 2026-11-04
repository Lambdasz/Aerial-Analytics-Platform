from __future__ import annotations
import math
from .models import ScaleInfo

def gsd_from_optics(altitude_m: float, sensor_width_mm: float, focal_length_mm: float, image_width_px: float) -> float:
    return (altitude_m * sensor_width_mm) / (focal_length_mm * image_width_px)

def resolve_gsd(parameters: dict, metadata, image_path: str) -> ScaleInfo:
    explicit = float(parameters.get("gsd_m_per_px") or 0)
    if explicit > 0:
        return ScaleInfo(explicit, "explicit_parameter", "measured")
    georef = (getattr(metadata, "georeference", None) or {}) if metadata else {}
    if georef.get("bounds") and getattr(metadata, "resolution", None):
        width, height = metadata.resolution
        lon_span = abs(georef["bounds"][2] - georef["bounds"][0])
        lat_span = abs(georef["bounds"][3] - georef["bounds"][1])
        lat = (georef["bounds"][1] + georef["bounds"][3]) / 2
        gsd = ((lon_span * 111320 * math.cos(math.radians(lat)) / width) + (lat_span * 110540 / height)) / 2
        if gsd > 0:
            return ScaleInfo(gsd, "raster_crs", "estimated")
    return ScaleInfo()
