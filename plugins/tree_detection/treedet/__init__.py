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

from .area import count_trees_in_area, polygon_area_m2
from .counting import count_trees
from .density import calculate_tree_density
from .detection import deduplicate, detect_trees
from .filtering import filter_by_confidence
from .location import locate_detections, map_tree_locations
from .output import build_plugin_output, to_execution_result
from .review import apply_review

__all__ = [
    "apply_review",
    "build_plugin_output",
    "calculate_tree_density",
    "count_trees",
    "count_trees_in_area",
    "deduplicate",
    "detect_trees",
    "filter_by_confidence",
    "locate_detections",
    "map_tree_locations",
    "polygon_area_m2",
    "to_execution_result",
]
