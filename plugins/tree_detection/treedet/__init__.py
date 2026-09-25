"""Module 7: Tree Detection & Counting.

Public functions (one per feature in the Module 7 specification):

- ``detect_trees``            Feature 1 - Tree Detection
- ``count_trees``             Feature 2 - Tree Counting
- ``filter_by_confidence``    Feature 3 - Confidence Filtering
- ``map_tree_locations``      Feature 4 - Tree Location Mapping
- ``count_trees_in_area``     Feature 5 - Area-Based Tree Counting
- ``calculate_tree_density``  Feature 6 - Tree Density Analysis
- ``apply_review``            Feature 7 - Detection Review
- ``build_plugin_output``     Feature 8 - Plugin-Compatible Output

Everything except ``detect_trees`` is a pure function: same input, same
output, no I/O. ``detect_trees`` reads the image through the chosen detector.
"""

from .models import (
    TreeDetection,
    BBox,
    ImageGeoreference,
    PluginOutput,
    ReviewCorrections,
)
from .errors import PluginExecutionError, InvalidGeometryError, ZeroAreaError

# Public Feature Functions (Sesuai Kontrak)
from .detection import detect_trees, deduplicate
from .counting import count_trees
from .filtering import filter_by_confidence
from .location import locate_detections, map_tree_locations
from .area import count_trees_in_area, polygon_area_m2
from .density import calculate_tree_density
from .review import apply_review
from .output import build_plugin_output

# Pipeline Orchestrator
from .pipeline import parse_payload, analyze

__all__ = [
    # Models
    "TreeDetection", "BBox", "ImageGeoreference", "PluginOutput", "ReviewCorrections",
    # Errors
    "PluginExecutionError", "InvalidGeometryError", "ZeroAreaError",
    # Features
    "detect_trees", "deduplicate", "count_trees", "filter_by_confidence",
    "locate_detections", "map_tree_locations", "count_trees_in_area", 
    "polygon_area_m2", "calculate_tree_density", "apply_review", "build_plugin_output",
    # Pipeline
    "parse_payload", "analyze",
]
