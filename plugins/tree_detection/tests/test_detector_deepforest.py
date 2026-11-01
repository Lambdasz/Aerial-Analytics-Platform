"""Tests for the DeepForest detector, without deepforest, torch or weights.

What this module owns is the resolution matching and the mapping of model boxes back
to the original photo, plus failing with a clear message; both are tested here. The
model itself was checked by hand on UAV-VisLoc photos (see training/README.md).

Run from the repository root:

    python -m unittest discover -s tests -v
"""

import importlib.util
import os
import tempfile
import unittest

try:
    import cv2
    import numpy as np
except ImportError:  # optional at import time, see treedet/detectors.py
    cv2 = None

from treedet.detectors import (
    DEEPFOREST_GSD_CM,
    DEEPFOREST_SCALE_LIMITS,
    _rescaled_boxes,
    deepforest_detector,
    deepforest_scale,
)
from treedet.errors import PluginExecutionError

HAS_DEEPFOREST = importlib.util.find_spec("deepforest") is not None


class DeepForestScaleTest(unittest.TestCase):
    def test_training_resolution_is_left_alone(self) -> None:
        self.assertEqual(deepforest_scale(DEEPFOREST_GSD_CM), 1.0)

    def test_finer_photos_are_shrunk(self) -> None:
        self.assertAlmostEqual(deepforest_scale(5.0), 0.5)

    def test_scale_is_clamped(self) -> None:
        low, high = DEEPFOREST_SCALE_LIMITS
        self.assertEqual(deepforest_scale(0.01), low)
        self.assertEqual(deepforest_scale(1000.0), high)


class DeepForestBoxesTest(unittest.TestCase):
    def test_boxes_return_to_original_pixels(self) -> None:
        ((bbox, confidence),) = _rescaled_boxes([(10.0, 20.0, 30.0, 40.0, 0.87654)], 0.5)
        self.assertEqual((bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max), (20, 40, 60, 80))
        self.assertEqual(confidence, 0.877)

    def test_degenerate_boxes_are_dropped(self) -> None:
        self.assertEqual(_rescaled_boxes([(10.0, 10.0, 10.0, 30.0, 0.9)], 1.0), ())

    def test_no_predictions_is_no_boxes(self) -> None:
        self.assertEqual(_rescaled_boxes((), 0.5), ())


@unittest.skipIf(cv2 is None, "numpy and opencv-python-headless are not installed")
class DeepForestFailureTest(unittest.TestCase):
    """The plugin must fail with a message that says what to do."""

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.image = os.path.join(self.tmp.name, "photo.png")
        cv2.imwrite(self.image, np.full((32, 32, 3), 30, np.uint8))

    def test_missing_checkpoint_is_reported(self) -> None:
        missing = os.path.join(self.tmp.name, "nope.pt")
        with self.assertRaisesRegex(PluginExecutionError, "checkpoint not found"):
            deepforest_detector(self.image, {"model_path": missing})

    def test_unreadable_image_is_reported(self) -> None:
        with self.assertRaisesRegex(PluginExecutionError, "Cannot read image"):
            deepforest_detector(os.path.join(self.tmp.name, "nope.png"), {})

    @unittest.skipIf(HAS_DEEPFOREST, "deepforest is installed")
    def test_missing_package_names_the_install_command(self) -> None:
        with self.assertRaisesRegex(PluginExecutionError, "pip install deepforest"):
            deepforest_detector(self.image, {})
