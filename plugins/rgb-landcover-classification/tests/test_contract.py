import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np

ROOT = Path(__file__).parents[1]
MAIN = ROOT / "main.py"
sys.path.insert(0, str(ROOT))
from main import (  # noqa: E402
    build_analysis_mask,
    build_boundary_geojson,
    georeference_boundary,
    calculate_metrics,
    parse_classes,
    report_progress,
)
from core.classify import (  # noqa: E402
    apply_confidence_threshold,
    default_model_config,
    load_checkpoint_state_dict,
    load_rgb,
)
from core.geo import aoi_pixel_mask
from core.models import FilteredMask
from core.payload import build_input_from_payload
from core.scale import resolve_gsd
from core.statistics import compute_class_statistics
from core.vector import generate_boundary_vector


class ContractTests(unittest.TestCase):
    def test_manifest_declares_deeplabv3plus_backbones(self):
        manifest = json.loads((ROOT / "manifest.json").read_text(encoding="utf-8"))
        model = json.loads((ROOT / "model.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["runtime"]["type"], "python")
        self.assertEqual(model["family"], "DeepLabV3+")
        self.assertEqual(
            model["supported_backbones"], ["resnet101", "xception"]
        )

    def test_healthcheck_returns_contract_json(self):
        proc = subprocess.run(
            [sys.executable, str(MAIN), "--healthcheck"],
            capture_output=True,
            text=True,
            check=False,
        )
        info = json.loads(proc.stdout)
        self.assertEqual(info["plugin_id"], "rgb-landcover-classification")
        self.assertIn(info["status"], {"healthy", "unhealthy"})
        self.assertIn("model_family", info["details"])

    def test_invalid_backbone_is_rejected_before_inference(self):
        proc = subprocess.run(
            [sys.executable, str(MAIN), "--validate-parameters", '{"backbone":"invalid"}'],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("backbone", proc.stderr)

    def test_custom_classes_must_match_model_class_count(self):
        classes = [{"id": index, "name": f"class_{index}", "color": "#112233"} for index in range(1, 6)]
        self.assertEqual(parse_classes(json.dumps(classes))[0]["id"], 1)
        with self.assertRaises(ValueError):
            parse_classes(json.dumps(classes[:4]))

        classes[0]["id"] = 10
        with self.assertRaises(ValueError):
            parse_classes(json.dumps(classes))

    def test_aoi_pixel_bbox_limits_analysis(self):
        mask = build_analysis_mask(
            {"type": "geojson_polygon", "geometry": {"type": "Polygon", "coordinates": [[[1, 1], [3, 1], [3, 3], [1, 3], [1, 1]]]}},
            4,
            4,
            {},
        )
        self.assertEqual(sum(sum(row) for row in mask), 4)

    def test_area_metrics_use_gsd_and_aoi(self):
        metrics = calculate_metrics([[1, 1], [2, 0]], 0.5, [[True, True], [True, False]])
        self.assertEqual(metrics["total_pixels_analyzed"], 3)
        self.assertEqual(metrics["vegetation_area_m2"], 0.5)
        self.assertEqual(metrics["bare_soil_area_m2"], 0.25)

    def test_custom_class_statistics_preserve_area_and_standard_keys(self):
        classes = [{"id": index, "name": f"custom_{index}", "color": "#112233"} for index in range(1, 6)]
        metrics = calculate_metrics([[1, 2]], 2.0, [[True, True]], classes)
        self.assertIsNone(metrics["vegetation_pct"])
        self.assertNotIn("custom_1_area_m2", metrics)

    def test_progress_protocol_is_json_and_monotonic(self):
        with tempfile.TemporaryFile(mode="w+") as stream:
            report_progress("job-1", 10, "loading", stream)
            report_progress("job-1", 90, "writing", stream)
            stream.seek(0)
            lines = stream.read().splitlines()
        events = [json.loads(line.removeprefix("PROGRESS:")) for line in lines]
        self.assertEqual([event["percent"] for event in events], [10, 90])
        self.assertEqual(events[0]["job_id"], "job-1")

    def test_boundary_artifact_uses_pixel_coordinates(self):
        boundary = build_boundary_geojson([[1, 0], [1, 2]], [{"id": 1, "name": "vegetation", "color": "#112233"}], [[True, True], [True, True]])
        self.assertEqual(boundary["coordinate_system"], "pixel")
        self.assertEqual(boundary["features"][0]["properties"]["class_name"], "vegetation")

    def test_boundary_is_transformed_to_wgs84_from_image_bounds(self):
        boundary = build_boundary_geojson(
            [[1, 0], [1, 2]],
            [{"id": 1, "name": "vegetation", "color": "#112233"}],
            [[True, True], [True, True]],
        )
        transformed = georeference_boundary(
            boundary, {"bounds": [100.0, -2.0, 102.0, 0.0], "crs": "EPSG:4326"}, 2, 2
        )
        self.assertEqual(transformed["coordinate_system"], "WGS84")
        self.assertEqual(transformed["features"][0]["geometry"]["coordinates"][0][0], [100.0, 0.0])

    def test_georeferenced_boundary_rejects_non_wgs84_crs(self):
        boundary = {"type": "FeatureCollection", "features": []}
        with self.assertRaises(ValueError):
            georeference_boundary(boundary, {"bounds": [0, 0, 1, 1], "crs": "EPSG:32750"}, 2, 2)

    def test_public_core_api_is_available_for_plugin_host(self):
        self.assertTrue(callable(load_rgb))
        self.assertTrue(callable(default_model_config))
        self.assertTrue(callable(aoi_pixel_mask))
        self.assertTrue(callable(resolve_gsd))
        self.assertTrue(callable(compute_class_statistics))
        self.assertTrue(callable(generate_boundary_vector))

    def test_public_confidence_api_preserves_mask_contract(self):
        result = apply_confidence_threshold(
            np.array([[1, 2], [0, 3]], dtype=np.uint8),
            np.array([[0.9, 0.1], [0.2, 0.8]], dtype=np.float32),
            0.5,
        )
        self.assertIsInstance(result, FilteredMask)
        self.assertEqual(result.class_mask.tolist(), [[1, 0], [0, 3]])

    def test_public_payload_adapter_reads_standard_payload(self):
        result = build_input_from_payload(
            {
                "execution_id": "job-1",
                "target": {"image_path": "image.jpg", "mime_type": "image/jpeg", "metadata": {"resolution": [2, 2]}},
                "parameters": {},
            },
            "runs/job-1",
        )
        self.assertEqual(result.execution_id, "job-1")
        self.assertEqual(result.image_path, "image.jpg")

    def test_checkpoint_loader_accepts_training_metadata_wrapper(self):
        state = {"layer.weight": object()}
        self.assertIs(load_checkpoint_state_dict({"state_dict": state, "backbone": "resnet101"}), state)
        self.assertIs(load_checkpoint_state_dict(state), state)


if __name__ == "__main__":
    unittest.main()
