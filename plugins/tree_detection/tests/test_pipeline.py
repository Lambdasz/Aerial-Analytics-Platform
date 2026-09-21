"""End-to-end tests: payload -> detect -> analyze -> execution_result."""

import json
import os
import tempfile
import unittest
from datetime import datetime, timezone

from treedet.detection import detect_trees
from treedet.errors import PluginExecutionError, UnsupportedFormatError
from treedet.models import BBox, DetectionInput
from treedet.output import to_execution_result
from treedet.pipeline import analyze, model_config_of, parse_payload

PLUGIN_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
NOW = datetime(2026, 9, 21, 9, 0, tzinfo=timezone.utc)


def fake_detector(image_path, model_config):
    """Three crowns: two confident ones near the image center, one weak one."""
    return (
        (BBox(x_min=1990, y_min=1490, x_max=2010, y_max=1510), 0.9),
        (BBox(x_min=2100, y_min=1500, x_max=2120, y_max=1520), 0.8),
        (BBox(x_min=100, y_min=100, x_max=120, y_max=120), 0.2),
    )


class PipelineTest(unittest.TestCase):
    def setUp(self) -> None:
        with open(os.path.join(PLUGIN_DIR, "execution_payload.json"), encoding="utf-8") as f:
            self.payload = json.load(f)
        self.tmp = tempfile.TemporaryDirectory()
        self.image = os.path.join(self.tmp.name, "DJI_0042.JPG")
        with open(self.image, "wb") as f:
            f.write(b"not a real jpeg; the fake detector never reads it")
        self.payload["target"]["image_path"] = self.image

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def run_pipeline(self, detector=fake_detector):
        request = parse_payload(self.payload)
        detection = detect_trees(
            request.detection_input,
            {**model_config_of(request.parameters), "detector": "fake"},
            detectors={"fake": detector},
        )
        return analyze(request, detection, NOW, "0.1.0")

    def test_fixture_payload_produces_valid_result(self) -> None:
        output = self.run_pipeline()
        self.assertEqual(output.status, "success")
        self.assertEqual(output.statistics["total_trees"], 2)
        self.assertEqual(output.statistics["removed_low_confidence"], 1)
        self.assertEqual(len(json.loads(output.geojson)["features"]), 2)

        result = to_execution_result(output, "exec_1", 10, {"tree_points": "/tmp/a.geojson"})
        self.assertEqual(result["metrics"]["tree_count"], 2)
        self.assertEqual(result["metrics"]["tree_density_unit"], "per_ha")
        self.assertIsNone(result["error"])

    def test_stub_detector_is_reported_as_warning(self) -> None:
        request = parse_payload(self.payload)
        detection = detect_trees(request.detection_input, model_config_of(request.parameters))
        output = analyze(request, detection, NOW, "0.1.0")
        self.assertEqual(output.status, "partial")
        result = to_execution_result(output, "exec_1", 10, {})
        self.assertEqual(result["status"], "warning")
        self.assertEqual(result["metrics"]["tree_count"], 0)

    def test_missing_image_raises_plugin_error(self) -> None:
        missing = DetectionInput(image_path=self.image + ".png", image_id="x", project_id="p")
        with self.assertRaises(PluginExecutionError):
            detect_trees(missing, {"detector": "stub"})

    def test_unsupported_format_raises(self) -> None:
        bad = DetectionInput(image_path="photo.bmp", image_id="x", project_id="p")
        with self.assertRaises(UnsupportedFormatError):
            detect_trees(bad, {"detector": "stub"})

    def test_missing_gps_raises_plugin_error(self) -> None:
        del self.payload["target"]["metadata"]["gps"]
        with self.assertRaises(PluginExecutionError):
            parse_payload(self.payload)


if __name__ == "__main__":
    unittest.main()
