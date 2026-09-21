"""Unit tests for the pure Module 7 features. Run from plugins/tree_detection:

    python -m unittest discover -s tests -v
"""

import json
import unittest
from datetime import datetime, timezone

from pydantic import ValidationError

from treedet import (
    apply_review,
    calculate_tree_density,
    count_trees,
    count_trees_in_area,
    deduplicate,
    filter_by_confidence,
    map_tree_locations,
    polygon_area_m2,
)
from treedet.errors import InvalidGeometryError, ZeroAreaError
from treedet.models import BBox, GeoCoordinate, ReviewCorrections, TreeDetection

NOW = datetime(2026, 9, 21, 9, 0, tzinfo=timezone.utc)
SQUARE = {
    "type": "Polygon",
    "coordinates": [[[0.0, 0.0], [0.001, 0.0], [0.001, 0.001], [0.0, 0.001], [0.0, 0.0]]],
}


def tree(detection_id: str, confidence: float, x: float = 0.0, y: float = 0.0) -> TreeDetection:
    return TreeDetection(
        detection_id=detection_id,
        bbox=BBox(x_min=x, y_min=y, x_max=x + 10, y_max=y + 10),
        confidence=confidence,
    )


class TreeCountingTest(unittest.TestCase):
    def test_counts_and_averages(self) -> None:
        result = count_trees([tree("a", 0.6), tree("b", 0.8)], "img", NOW)
        self.assertEqual(result.total_trees, 2)
        self.assertAlmostEqual(result.avg_confidence, 0.7)

    def test_no_detections_is_zero_not_error(self) -> None:
        result = count_trees([], "img", NOW)
        self.assertEqual(result.total_trees, 0)
        self.assertEqual(result.avg_confidence, 0.0)


class ConfidenceFilteringTest(unittest.TestCase):
    def test_keeps_detections_at_or_above_threshold(self) -> None:
        result = filter_by_confidence([tree("a", 0.4), tree("b", 0.5), tree("c", 0.9)], 0.5)
        self.assertEqual([d.detection_id for d in result.kept], ["b", "c"])
        self.assertEqual(result.removed_count, 1)
        self.assertEqual(result.threshold_used, 0.5)

    def test_rejects_threshold_out_of_range(self) -> None:
        with self.assertRaises(ValidationError):
            filter_by_confidence([tree("a", 0.4)], 1.5)

    def test_input_is_not_mutated(self) -> None:
        detections = (tree("a", 0.4), tree("b", 0.9))
        filter_by_confidence(detections, 0.5)
        self.assertEqual(len(detections), 2)


class DeduplicationTest(unittest.TestCase):
    def test_overlapping_boxes_keep_the_most_confident(self) -> None:
        box = BBox(x_min=0, y_min=0, x_max=10, y_max=10)
        shifted = BBox(x_min=1, y_min=1, x_max=11, y_max=11)
        far = BBox(x_min=100, y_min=100, x_max=110, y_max=110)
        result = deduplicate([(box, 0.6), (shifted, 0.9), (far, 0.5)], 0.5)
        self.assertEqual([conf for _, conf in result], [0.9, 0.5])


class TreeLocationMappingTest(unittest.TestCase):
    METADATA = {"gps": {"lat": -1.0, "lon": 116.0}, "resolution": [100, 100], "gsd_m": 1.0}

    def test_image_center_maps_to_gps(self) -> None:
        center = TreeDetection(
            detection_id="c", bbox=BBox(x_min=45, y_min=45, x_max=55, y_max=55), confidence=0.9
        )
        output = map_tree_locations([center], self.METADATA)
        feature = json.loads(output.tree_points_geojson)["features"][0]
        lon, lat = feature["geometry"]["coordinates"]
        self.assertAlmostEqual(lon, 116.0)
        self.assertAlmostEqual(lat, -1.0)
        self.assertEqual(feature["properties"]["area_m2"], 100.0)
        self.assertEqual(output.crs, "EPSG:4326")
        self.assertEqual(output.detection_ids, ("c",))

    def test_top_of_image_is_north(self) -> None:
        top = TreeDetection(
            detection_id="t", bbox=BBox(x_min=45, y_min=0, x_max=55, y_max=10), confidence=0.9
        )
        feature = json.loads(map_tree_locations([top], self.METADATA).tree_points_geojson)
        _, lat = feature["features"][0]["geometry"]["coordinates"]
        self.assertGreater(lat, -1.0)


class AreaBasedCountingTest(unittest.TestCase):
    def test_counts_inside_and_outside(self) -> None:
        points = [
            GeoCoordinate(latitude=0.0005, longitude=0.0005),
            GeoCoordinate(latitude=0.002, longitude=0.002),
        ]
        plot = {"type": "Feature", "properties": {"plot_id": "BLOK_A1"}, "geometry": SQUARE}
        result = count_trees_in_area(points, plot)
        self.assertEqual((result.plot_id, result.tree_count, result.outside_count), ("BLOK_A1", 1, 1))

    def test_empty_plot_is_zero(self) -> None:
        self.assertEqual(count_trees_in_area([], SQUARE).tree_count, 0)

    def test_invalid_geometry_raises(self) -> None:
        with self.assertRaises(InvalidGeometryError):
            count_trees_in_area([], {"type": "Point", "coordinates": [0, 0]})
        with self.assertRaises(InvalidGeometryError):
            count_trees_in_area([], {"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1]]]})

    def test_polygon_area_near_equator(self) -> None:
        # 0.001° x 0.001° at the equator is about 111.2 m x 111.2 m.
        self.assertAlmostEqual(polygon_area_m2(SQUARE), 12_364, delta=50)


class TreeDensityTest(unittest.TestCase):
    def test_per_hectare_and_per_m2(self) -> None:
        self.assertEqual(calculate_tree_density(50, 20_000).density_value, 25.0)
        self.assertEqual(calculate_tree_density(50, 20_000, "per_m2").density_value, 0.0025)

    def test_zero_area_raises(self) -> None:
        with self.assertRaises(ZeroAreaError):
            calculate_tree_density(5, 0.0)


class DetectionReviewTest(unittest.TestCase):
    def test_removes_and_adds_with_manual_prefix(self) -> None:
        corrections = ReviewCorrections(
            remove_detection_ids=("a", "does-not-exist"),
            manual_additions=(tree("x", 1.0),),
            reviewer_id="nathanael",
        )
        result = apply_review([tree("a", 0.6), tree("b", 0.8)], corrections)
        self.assertEqual([d.detection_id for d in result.final_detections], ["b", "manual-x"])
        self.assertEqual(result.removed_ids, ("a",))
        self.assertEqual(result.added_count, 1)


if __name__ == "__main__":
    unittest.main()
