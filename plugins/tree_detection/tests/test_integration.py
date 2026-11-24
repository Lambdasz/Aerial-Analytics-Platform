"""Unit tests for the Module 9, 10 and 11 connectors."""

import json
import unittest

from treedet.errors import PluginExecutionError
from treedet.integration import compare_plot_summaries, plot_summaries, tree_points_of


def square(left: float, plot_id: str) -> dict:
    return {
        "type": "Feature",
        "properties": {"plot_id": plot_id},
        "geometry": {
            "type": "Polygon",
            "coordinates": [
                [
                    [left, 0.0],
                    [left + 0.001, 0.0],
                    [left + 0.001, 0.001],
                    [left, 0.001],
                    [left, 0.0],
                ]
            ],
        },
    }


def collection(*positions: tuple[float, float]) -> str:
    return json.dumps(
        {
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {"type": "Point", "coordinates": [lon, lat]},
                    "properties": {"detection_id": f"t{index}"},
                }
                for index, (lon, lat) in enumerate(positions)
            ],
        }
    )


class TreePointsTest(unittest.TestCase):
    def test_reads_the_artifact_as_written_by_the_plugin(self):
        points = tree_points_of(collection((0.0005, 0.0005), (0.002, 0.002)))
        self.assertEqual(len(points), 2)
        self.assertAlmostEqual(points[0].latitude, 0.0005)
        self.assertAlmostEqual(points[0].longitude, 0.0005)

    def test_accepts_an_already_parsed_mapping(self):
        points = tree_points_of(json.loads(collection((0.0005, 0.0005))))
        self.assertEqual(len(points), 1)

    def test_rejects_geometry_that_is_not_a_point(self):
        bad = json.dumps(
            {
                "type": "FeatureCollection",
                "features": [{"type": "Feature", "geometry": {"type": "LineString"}}],
            }
        )
        with self.assertRaises(PluginExecutionError):
            tree_points_of(bad)


class PlotSummaryTest(unittest.TestCase):
    def setUp(self):
        self.plots = [square(0.0, "BLOK_A1"), square(0.01, "BLOK_A2")]
        self.points = tree_points_of(collection((0.0005, 0.0005), (0.0008, 0.0005), (0.02, 0.02)))

    def test_counts_each_plot_separately(self):
        summaries = plot_summaries(self.points, self.plots)
        self.assertEqual([s["plot_id"] for s in summaries], ["BLOK_A1", "BLOK_A2"])
        self.assertEqual([s["tree_count"] for s in summaries], [2, 0])

    def test_area_matches_a_hundred_meter_square(self):
        summary = plot_summaries(self.points, self.plots)[0]
        self.assertAlmostEqual(summary["plot_area_m2"], 12_364, delta=50)

    def test_density_is_per_hectare_by_default(self):
        summary = plot_summaries(self.points, self.plots)[0]
        expected = 2 / summary["plot_area_m2"] * 10_000
        self.assertEqual(summary["tree_density_unit"], "per_ha")
        self.assertAlmostEqual(summary["tree_density"], expected)

    def test_density_can_be_per_square_meter(self):
        summary = plot_summaries(self.points, self.plots, unit="per_m2")[0]
        self.assertEqual(summary["tree_density_unit"], "per_m2")
        self.assertAlmostEqual(summary["tree_density"], 2 / summary["plot_area_m2"])


class PlotComparisonTest(unittest.TestCase):
    def test_reports_change_per_plot(self):
        before = ({"plot_id": "A", "tree_count": 5}, {"plot_id": "B", "tree_count": 2})
        after = ({"plot_id": "A", "tree_count": 3}, {"plot_id": "B", "tree_count": 2})
        changes = compare_plot_summaries(before, after)
        self.assertEqual([c["plot_id"] for c in changes], ["A", "B"])
        self.assertEqual([c["tree_count_change"] for c in changes], [-2, 0])

    def test_plot_missing_from_one_run_is_not_dropped(self):
        before = ({"plot_id": "A", "tree_count": 5},)
        after = ({"plot_id": "A", "tree_count": 5}, {"plot_id": "NEW", "tree_count": 1})
        changes = {c["plot_id"]: c for c in compare_plot_summaries(before, after)}
        self.assertIsNone(changes["NEW"]["trees_before"])
        self.assertEqual(changes["NEW"]["trees_after"], 1)
        self.assertIsNone(changes["NEW"]["tree_count_change"])


if __name__ == "__main__":
    unittest.main()
