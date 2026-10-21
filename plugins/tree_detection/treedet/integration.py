"""Connectors for Modules 9, 10 and 11.

Module 9 aggregates tree points per plot, Module 10 compares those summaries
between two observation dates, and Module 11 charts the same numbers. All three
read files the plugin already writes, so nothing here touches the disk and
nothing here needs an installed detector.
"""

import json
from collections.abc import Mapping, Sequence
from typing import Any

from .area import count_trees_in_area, polygon_area_m2
from .density import calculate_tree_density
from .errors import PluginExecutionError
from .models import DensityUnit, GeoCoordinate

PlotSummary = dict[str, Any]


def tree_points_of(tree_points: Mapping[str, Any] | str) -> tuple[GeoCoordinate, ...]:
    """Read the ``tree_points`` artifact (GeoJSON string or mapping) into coordinates."""
    collection = json.loads(tree_points) if isinstance(tree_points, str) else tree_points
    features = collection.get("features") or []
    points: list[GeoCoordinate] = []
    for feature in features:
        geometry = feature.get("geometry") or {}
        coordinates = geometry.get("coordinates")
        if geometry.get("type") != "Point" or not isinstance(coordinates, list):
            raise PluginExecutionError(f"Expected a Point geometry, got '{geometry.get('type')}'")
        if len(coordinates) < 2:
            raise PluginExecutionError("Point geometry needs a longitude and a latitude")
        longitude, latitude = coordinates[0], coordinates[1]
        points.append(GeoCoordinate(latitude=latitude, longitude=longitude))
    return tuple(points)


def plot_summaries(
    tree_points: Sequence[GeoCoordinate],
    plot_boundaries: Sequence[Mapping[str, Any]],
    unit: DensityUnit = "per_ha",
) -> tuple[PlotSummary, ...]:
    """Count, area and density for every plot, one summary per plot.

    ``plot_boundaries`` holds Module 9 plot boundaries as GeoJSON (WGS84).
    Invalid geometry raises ``InvalidGeometryError``; a plot with zero area
    raises ``ZeroAreaError`` instead of reporting a density of zero.
    """
    summaries: list[PlotSummary] = []
    for boundary in plot_boundaries:
        count = count_trees_in_area(tree_points, boundary)
        area_m2 = polygon_area_m2(boundary)
        density = calculate_tree_density(count.tree_count, area_m2, unit)
        summaries.append(
            {
                "plot_id": count.plot_id,
                "tree_count": count.tree_count,
                "plot_area_m2": area_m2,
                "tree_density": density.density_value,
                "tree_density_unit": density.unit,
            }
        )
    return tuple(summaries)


def compare_plot_summaries(
    before: Sequence[PlotSummary], after: Sequence[PlotSummary]
) -> tuple[PlotSummary, ...]:
    """Per-plot tree count change for Module 10, joined on ``plot_id``.

    A plot that exists in only one of the two runs keeps ``None`` on the missing
    side instead of being dropped, so an added or removed plot stays visible.
    """
    old = {summary["plot_id"]: summary for summary in before}
    new = {summary["plot_id"]: summary for summary in after}
    changes: list[PlotSummary] = []
    for plot_id in dict.fromkeys([*old, *new]):
        old_count = old[plot_id]["tree_count"] if plot_id in old else None
        new_count = new[plot_id]["tree_count"] if plot_id in new else None
        change = new_count - old_count if old_count is not None and new_count is not None else None
        changes.append(
            {
                "plot_id": plot_id,
                "trees_before": old_count,
                "trees_after": new_count,
                "tree_count_change": change,
            }
        )
    return tuple(changes)
