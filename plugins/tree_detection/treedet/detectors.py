"""Detection backends.

A detector is any function ``(image_path, model_config) -> Sequence[RawBox]``.
It returns crowns in pixel coordinates with a confidence; ids, filtering and
georeferencing are done by the rest of the pipeline, so a new model only has
to implement this one function and be added to ``DETECTORS``.
"""

from collections.abc import Callable, Mapping, Sequence
from typing import Any

from .models import BBox

RawBox = tuple[BBox, float]
Detector = Callable[[str, Mapping[str, Any]], Sequence[RawBox]]


def stub_detector(image_path: str, model_config: Mapping[str, Any]) -> Sequence[RawBox]:
    """Placeholder that detects nothing. Lets the pipeline run end to end
    before a real model is chosen."""
    return ()


# TODO(module-7): add the real model here once chosen, e.g.
#   "opencv_watershed": opencv_watershed_detector,
#   "deepforest": deepforest_detector,
# and add its name to the "detector" options in parameters.json.
DETECTORS: Mapping[str, Detector] = {
    "stub": stub_detector,
}
