"""Tests for the DeepLabV3+ detector, without torch and without weights.

The network is replaced by a predictor that draws the probability map a trained
model would return, so what gets tested is what this module actually owns: turning
a probability map into boxes, running that per tile, and failing loudly when the
checkpoint or the optional packages are missing. Accuracy on real photos stays
unmeasured until there is a checkpoint and labelled data.

Run from the repository root:

    python -m unittest discover -s tests -v
"""

import os
import tempfile
import unittest

try:
    import cv2
    import numpy as np
except ImportError:  # optional at import time, see treedet/detectors.py
    cv2 = None

from treedet.detection import detect_trees
from treedet.detectors import (
    _crown_limits,
    _probability_boxes,
    _tiled_predictions,
    deeplabv3plus_detector,
)
from treedet.errors import PluginExecutionError
from treedet.models import DetectionInput

GSD_CM = 3.0
CROWN_RADIUS_PX = 18
CANOPY_PROBABILITY = 0.9
SOIL_PROBABILITY = 0.05


def probability_map(size: tuple[int, int], crowns, probability=CANOPY_PROBABILITY):
    """Per-pixel crown probability with a disk on every crown centre."""
    canvas = np.full((size[1], size[0]), SOIL_PROBABILITY, np.float32)
    for centre_x, centre_y in crowns:
        cv2.circle(canvas, (centre_x, centre_y), CROWN_RADIUS_PX, probability, -1)
    return canvas


def disk_predictor(crowns, probability=CANOPY_PROBABILITY):
    """A stand-in for the network: returns the disks in tile coordinates."""

    def predict(tile, offset):
        local = [(x - offset[0], y - offset[1]) for x, y in crowns]
        return probability_map((tile.shape[1], tile.shape[0]), local, probability)

    return predict


@unittest.skipIf(cv2 is None, "numpy and opencv-python-headless are not installed")
class DeepLabMaskTest(unittest.TestCase):
    """The part that turns a probability map into crowns."""

    def boxes(self, crowns, **config):
        limits = _crown_limits(GSD_CM)
        probability = probability_map((256, 256), crowns)
        threshold = config.get("probability_threshold", 0.5)
        return tuple(_probability_boxes(probability, threshold, limits, np, cv2))

    def test_mask_becomes_one_box_per_crown(self) -> None:
        boxes = self.boxes([(80, 80), (180, 180)])
        self.assertEqual(len(boxes), 2)
        for _, confidence in boxes:
            self.assertAlmostEqual(confidence, CANOPY_PROBABILITY, places=3)

    def test_threshold_drops_faint_crowns(self) -> None:
        limits = _crown_limits(GSD_CM)
        probability = probability_map((256, 256), [(100, 100)], probability=0.3)
        self.assertEqual(len(tuple(_probability_boxes(probability, 0.5, limits, np, cv2))), 0)

    def test_specks_below_the_minimum_crown_size_are_dropped(self) -> None:
        limits = _crown_limits(GSD_CM)
        probability = np.full((256, 256), SOIL_PROBABILITY, np.float32)
        cv2.circle(probability, (100, 100), 2, CANOPY_PROBABILITY, -1)
        self.assertEqual(len(tuple(_probability_boxes(probability, 0.5, limits, np, cv2))), 0)

    def test_blank_probability_map_returns_nothing(self) -> None:
        self.assertEqual(self.boxes([]), ())


@unittest.skipIf(cv2 is None, "numpy and opencv-python-headless are not installed")
class DeepLabTilingTest(unittest.TestCase):
    """Tiling and de-duplication, driven by the stand-in predictor."""

    CONFIG = {"gsd_cm": GSD_CM, "tile_size": 256, "probability_threshold": 0.5}

    def test_a_crown_on_a_tile_border_is_counted_once(self) -> None:
        # 256 px tiles step by 192 px, so x=240 sits inside two tiles.
        crowns = [(240, 128)]
        image = np.full((256, 1024, 3), 30, np.uint8)
        raw = _tiled_predictions(image, disk_predictor(crowns), self.CONFIG, np, cv2)
        self.assertEqual(len(raw), 2)  # the same crown seen by both tiles

        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        path = os.path.join(tmp.name, "border.png")
        cv2.imwrite(path, image)
        output = detect_trees(
            DetectionInput(image_path=path, image_id="border", project_id="p"),
            {**self.CONFIG, "iou_threshold": 0.5, "detector": "deeplabv3plus"},
            detectors={"deeplabv3plus": self.fake_detector(crowns)},
        )
        self.assertEqual(len(output.detections), 1)
        self.assertAlmostEqual(output.detections[0].bbox.center[0], 240, delta=6)

    def test_two_separated_crowns_stay_two(self) -> None:
        crowns = [(120, 128), (760, 128)]
        image = np.full((256, 1024, 3), 30, np.uint8)
        raw = _tiled_predictions(image, disk_predictor(crowns), self.CONFIG, np, cv2)
        self.assertEqual(len(raw), 2)

    def fake_detector(self, crowns):
        """The real detector with only the network swapped out."""

        def detect(image_path, model_config):
            bgr = cv2.imread(image_path, cv2.IMREAD_COLOR)
            return _tiled_predictions(bgr, disk_predictor(crowns), model_config, np, cv2)

        return detect


class DeepLabFailureTest(unittest.TestCase):
    """The plugin must fail with a message that says what to do."""

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.image = os.path.join(self.tmp.name, "photo.png")
        # A readable image on purpose: these cases must fail on configuration, not
        # because OpenCV could not open the file.
        cv2.imwrite(self.image, np.full((32, 32, 3), 30, np.uint8))

    def test_missing_checkpoint_parameter_is_named(self) -> None:
        with self.assertRaises(PluginExecutionError) as caught:
            deeplabv3plus_detector(self.image, {"model_path": ""})
        self.assertIn("model_path", str(caught.exception))

    def test_checkpoint_on_disk_is_required(self) -> None:
        missing = os.path.join(self.tmp.name, "crowns.pth")
        with self.assertRaises(PluginExecutionError) as caught:
            deeplabv3plus_detector(self.image, {"model_path": missing})
        self.assertIn("not found", str(caught.exception))

    def test_unknown_backbone_is_rejected(self) -> None:
        with self.assertRaises(PluginExecutionError) as caught:
            deeplabv3plus_detector(self.image, {"model_path": self.image, "backbone": "mobilenet"})
        self.assertIn("backbone", str(caught.exception))

    def test_missing_optional_packages_are_reported_with_the_install_command(self) -> None:
        try:
            import torch  # noqa: F401
        except ImportError:
            pass
        else:
            self.skipTest("torch is installed; the missing-package path cannot be reached")

        with self.assertRaises(PluginExecutionError) as caught:
            deeplabv3plus_detector(self.image, {"model_path": self.image})
        self.assertIn("segmentation-models-pytorch", str(caught.exception))


class DeepLabHealthcheckTest(unittest.TestCase):
    """torch is optional, so it must not decide whether the plugin is healthy."""

    def test_optional_dependencies_are_reported_but_do_not_fail_the_healthcheck(self) -> None:
        import main

        ok, statuses = main.check_dependencies()
        optional = main.check_optional_dependencies()
        self.assertTrue(ok, statuses)
        self.assertIn("torch", optional)
        self.assertIn("segmentation_models_pytorch", optional)
        self.assertNotIn("torch", statuses)


if __name__ == "__main__":
    unittest.main()
