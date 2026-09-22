"""Feature 1 - Tree Detection."""

import os
import time
from collections.abc import Mapping, Sequence
from functools import reduce
from typing import Any

from .detectors import DETECTORS, Detector, RawBox
from .errors import PluginExecutionError, UnsupportedFormatError
from .models import BBox, DetectionInput, DetectionOutput, TreeDetection

SUPPORTED_EXTENSIONS = frozenset({".jpg", ".jpeg", ".png", ".tif", ".tiff"})


def make_detection_id(image_id: str, model_name: str, sequence: int) -> str:
    """Build the detection id as ``{image_id}-{model_name}-{sequence}``."""
    return f"{image_id}-{model_name}-{sequence:04d}"


def iou(a: BBox, b: BBox) -> float:
    """Intersection over union of two boxes (0.0 when they do not overlap)."""
    inter_w = max(0.0, min(a.x_max, b.x_max) - max(a.x_min, b.x_min))
    inter_h = max(0.0, min(a.y_max, b.y_max) - max(a.y_min, b.y_min))
    inter = inter_w * inter_h
    union = a.area_px + b.area_px - inter
    return inter / union if union > 0 else 0.0


def deduplicate(boxes: Sequence[RawBox], iou_threshold: float) -> tuple[RawBox, ...]:
    """Drop boxes that overlap a more confident box by more than ``iou_threshold``.

    Needed because neighbouring tiles see the same tree twice.
    """

    def keep_if_new(kept: tuple[RawBox, ...], box: RawBox) -> tuple[RawBox, ...]:
        if any(iou(box[0], other[0]) > iou_threshold for other in kept):
            return kept
        return (*kept, box)

    ordered = sorted(boxes, key=lambda box: box[1], reverse=True)
    return reduce(keep_if_new, ordered, ())


def to_detections(
    boxes: Sequence[RawBox], image_id: str, model_name: str
) -> tuple[TreeDetection, ...]:
    """Give raw boxes stable ids. Pure."""
    return tuple(
        TreeDetection(
            detection_id=make_detection_id(image_id, model_name, index + 1),
            bbox=bbox,
            confidence=confidence,
        )
        for index, (bbox, confidence) in enumerate(boxes)
    )


def detect_trees(
    input_data: DetectionInput,
    model_config: Mapping[str, Any],
    detectors: Mapping[str, Detector] = DETECTORS,
) -> DetectionOutput:
    """Detect tree crowns in one RGB image.

    ``model_config`` keys: ``detector`` (name in ``detectors``), ``iou_threshold``,
    plus anything the detector itself needs (model path, ``tile_size``, device).

    Raises ``PluginExecutionError`` when the image is missing or the detector is
    unknown, and ``UnsupportedFormatError`` for an unsupported file type.
    This is the only public function with I/O, because the detector reads the image.
    """
    extension = os.path.splitext(input_data.image_path)[1].lower()
    if extension not in SUPPORTED_EXTENSIONS:
        raise UnsupportedFormatError(f"Unsupported image format: '{extension or '(none)'}'")
    if not os.path.isfile(input_data.image_path):
        raise PluginExecutionError(f"Image not found: {input_data.image_path}")

    detector_name = str(model_config.get("detector", "stub"))
    detector = detectors.get(detector_name)
    if detector is None:
        raise PluginExecutionError(f"Unknown detector: '{detector_name}'")

    started = time.perf_counter()
    raw_boxes = detector(input_data.image_path, model_config)
    unique_boxes = deduplicate(raw_boxes, float(model_config.get("iou_threshold", 0.5)))
    elapsed = time.perf_counter() - started

    return DetectionOutput(
        detections=to_detections(unique_boxes, input_data.image_id, detector_name),
        image_id=input_data.image_id,
        model_name=detector_name,
        processing_time_s=elapsed,
    )
