"""Public compatibility API for the DeepLabV3+ Module 8 plugin."""
from .classify import apply_confidence_threshold, classify_landcover, default_model_config, load_rgb
from .geo import aoi_pixel_mask, read_wgs84_georef
from .models import DEFAULT_CLASSES, FilteredMask, LandCoverInput, PluginExecutionError
from .output import build_plugin_output
from .payload import build_input_from_payload
from .progress import report_progress
from .scale import resolve_gsd
from .statistics import compute_class_statistics
from .vector import generate_boundary_vector

__all__ = [
    "DEFAULT_CLASSES", "FilteredMask", "LandCoverInput", "PluginExecutionError",
    "apply_confidence_threshold", "aoi_pixel_mask", "build_input_from_payload",
    "build_plugin_output", "classify_landcover", "compute_class_statistics",
    "default_model_config", "generate_boundary_vector", "load_rgb", "read_wgs84_georef",
    "report_progress", "resolve_gsd",
]
