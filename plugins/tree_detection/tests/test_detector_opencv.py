"""Tests for the built-in OpenCV detector (Feature 1) on synthetic imagery.

The images are drawn here instead of being taken from a dataset, so the test stays
offline and gives the same answer on every machine: a green disk on a dark
background stands for one crown, and the detector has to find as many disks as were
drawn. That proves the wiring of Feature 1, not its accuracy on real aerial photos.

Run from the repository root:

    python -m unittest discover -s tests -v
"""

import json
import os
import tempfile
import unittest
from datetime import datetime, timezone

try:
    import cv2
    import numpy as np
except ImportError:  # numpy/opencv are optional at import time, see treedet/detectors.py
    cv2 = None

from treedet.detection import detect_trees
from treedet.detectors import DETECTORS
from treedet.errors import PluginExecutionError
from treedet.models import DetectionInput
from treedet.output import to_execution_result
from treedet.pipeline import analyze, model_config_of, parse_payload

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
NOW = datetime(2026, 9, 21, 9, 0, tzinfo=timezone.utc)

BACKGROUND = (30, 30, 30)  # BGR, dark soil
CROWN = (40, 180, 60)  # BGR, green
CROWN_RADIUS_PX = 18
GSD_CM = 3.0
MODEL_CONFIG = {
    "detector": "opencv_watershed",
    "gsd_cm": GSD_CM,
    "iou_threshold": 0.5,
    "tile_size": 1024,
}


def draw_image(path: str, size: tuple[int, int], crowns: list[tuple[int, int]]) -> None:
    """Write a synthetic photo: green disks (`crowns` are their centres) on dark soil."""
    canvas = np.full((size[1], size[0], 3), BACKGROUND, np.uint8)
    for centre_x, centre_y in crowns:
        cv2.circle(canvas, (centre_x, centre_y), CROWN_RADIUS_PX, CROWN, -1)
    cv2.imwrite(path, canvas)


@unittest.skipIf(cv2 is None, "numpy and opencv-python-headless are not installed")
class OpenCVDetectorTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.image = os.path.join(self.tmp.name, "synthetic.png")

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def detect(self, crowns: list[tuple[int, int]], size=(512, 512), **config):
        draw_image(self.image, size, crowns)
        return detect_trees(
            DetectionInput(image_path=self.image, image_id="synthetic", project_id="p"),
            {**MODEL_CONFIG, **config},
        )

    def centres(self, detection_output) -> list[tuple[int, int]]:
        """Crown centres rounded to whole pixels.

        A bounding box is half-open (``x_max`` is exclusive), so a crown drawn at
        (80, 80) is reported at 80.5, which is the same tree.
        """
        return [tuple(round(value) for value in d.bbox.center) for d in detection_output.detections]

    def test_finds_every_separated_crown(self) -> None:
        crowns = [(80, 80), (200, 200), (350, 350), (450, 120)]
        output = self.detect(crowns)

        self.assertEqual(output.model_name, "opencv_watershed")
        self.assertEqual(len(output.detections), 4)
        for centre_x, centre_y in crowns:
            self.assertIn((centre_x, centre_y), self.centres(output))

    def test_confidence_survives_the_default_threshold(self) -> None:
        output = self.detect([(100, 100)])
        self.assertGreaterEqual(output.detections[0].confidence, 0.5)
        self.assertLessEqual(output.detections[0].confidence, 1.0)

    def test_splits_two_touching_crowns(self) -> None:
        # 36 px apart with a radius of 18 px: the disks touch, so the split is visible.
        output = self.detect([(100, 128), (136, 128)], size=(256, 256))
        self.assertEqual(len(output.detections), 2)
        x_centres = sorted(centre[0] for centre in self.centres(output))
        self.assertAlmostEqual(x_centres[0], 100, delta=3)
        self.assertAlmostEqual(x_centres[1], 136, delta=3)

    def test_crown_at_a_tile_border_is_neither_lost_nor_counted_twice(self) -> None:
        # 256 px tiles over a 1024 px wide image: every crown sits on a tile border.
        output = self.detect([(250, 128), (512, 128), (1000, 128)], size=(1024, 256), tile_size=256)
        self.assertEqual(len(output.detections), 3)
        # The crown in the middle of a tile keeps its exact position. A crown that
        # straddles a border can come back a few pixels off centre, because the
        # de-duplication keeps the most confident copy and a clipped crown can win.
        self.assertIn((512, 128), self.centres(output))

    def test_blank_image_detects_nothing_without_failing(self) -> None:
        self.assertEqual(len(self.detect([]).detections), 0)

    def test_same_image_gives_the_same_detections(self) -> None:
        crowns = [(100, 100), (180, 180)]
        draw_image(self.image, (256, 256), crowns)
        first = self.detect(crowns, size=(256, 256))
        second = self.detect(crowns, size=(256, 256))
        self.assertEqual(first.detections, second.detections)

    def test_unreadable_image_raises_plugin_error(self) -> None:
        with open(self.image, "wb") as handle:
            handle.write(b"not an image at all")
        with self.assertRaises(PluginExecutionError):
            detect_trees(
                DetectionInput(image_path=self.image, image_id="x", project_id="p"),
                MODEL_CONFIG,
            )

    def test_end_to_end_run_with_the_real_detector(self) -> None:
        draw_image(self.image, (512, 512), [(200, 200), (300, 300)])
        payload = {
            "execution_id": "exec_synthetic_1",
            "session_id": "session_synthetic",
            "output_dir": self.tmp.name,
            "target": {
                "image_path": self.image,
                "metadata": {
                    "image_id": "synthetic",
                    "resolution": [512, 512],
                    "gps": {"lat": -1.2445, "lon": 116.8535},
                    "gsd_m": GSD_CM / 100,
                    "rotation_deg": 0.0,
                },
            },
            "parameters": dict(MODEL_CONFIG, confidence_threshold=0.5),
        }
        request = parse_payload(payload)
        detection = detect_trees(request.detection_input, model_config_of(request.parameters))
        output = analyze(request, detection, NOW, "0.1.0")

        self.assertEqual(output.status, "success")
        self.assertEqual(output.statistics["total_trees"], 2)
        self.assertEqual(output.statistics["density_unit"], "per_ha")
        self.assertEqual(len(json.loads(output.geojson)["features"]), 2)

        result = to_execution_result(output, "exec_synthetic_1", 10, {"tree_points": "a.geojson"})
        self.assertEqual(result["status"], "success")
        self.assertEqual(result["metrics"]["tree_count"], 2)
        self.assertIsNone(result["error"])


class DetectorContractTest(unittest.TestCase):
    """The registry, parameters.json and healthcheck have to stay in sync."""

    def test_every_registered_detector_is_offered_in_the_form(self) -> None:
        with open(os.path.join(REPO_ROOT, "parameters.json"), encoding="utf-8") as handle:
            parameters = json.load(handle)["parameters"]
        offered = next(p for p in parameters if p["name"] == "detector")
        self.assertEqual(sorted(offered["options"]), sorted(DETECTORS))
        self.assertIn(offered["default"], DETECTORS)

    def test_healthcheck_lists_the_detector_dependencies(self) -> None:
        import main

        ok, statuses = main.check_dependencies()
        self.assertIn("numpy", statuses)
        self.assertIn("cv2", statuses)
        self.assertTrue(ok, statuses)


if __name__ == "__main__":
    unittest.main()
