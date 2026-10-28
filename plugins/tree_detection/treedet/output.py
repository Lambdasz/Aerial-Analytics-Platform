"""Feature 8 - Plugin-Compatible Output."""

from collections.abc import Mapping, Sequence
from typing import Any

from .models import (
    AreaCountOutput,
    CountOutput,
    DensityOutput,
    FilteredOutput,
    LocationOutput,
    PluginOutput,
    TreeDetection,
)

# PluginOutput.status -> execution_result.schema.json status
EXECUTION_STATUS = {"success": "success", "partial": "warning", "failed": "failure"}


def build_plugin_output(
    detections: Sequence[TreeDetection],
    count: CountOutput,
    density: DensityOutput,
    locations: LocationOutput,
    run_metadata: Mapping[str, Any],
    filtered: FilteredOutput | None = None,
    area_count: AreaCountOutput | None = None,
) -> PluginOutput:
    """Combine every feature's result into one structure for Modules 9 and 11.

    Status is ``"partial"`` when ``run_metadata["warnings"]`` is non-empty.
    """
    statistics = {
        "total_trees": count.total_trees,
        "avg_confidence": count.avg_confidence,
        "counted_at": count.counted_at.isoformat(),
        "density_value": density.density_value,
        "density_unit": density.unit,
        "area_m2": density.area_m2,
        "removed_low_confidence": filtered.removed_count if filtered else 0,
        "threshold_used": filtered.threshold_used if filtered else None,
        "plot_id": area_count.plot_id if area_count else None,
        "trees_outside_area": area_count.outside_count if area_count else 0,
    }
    return PluginOutput(
        status="partial" if run_metadata.get("warnings") else "success",
        detections=tuple(detections),
        statistics=statistics,
        geojson=locations.tree_points_geojson,
        metadata=dict(run_metadata),
    )


def to_execution_result(
    output: PluginOutput,
    execution_id: str,
    execution_time_ms: int,
    artifact_paths: Mapping[str, str],
) -> dict[str, Any]:
    """Translate ``PluginOutput`` into the platform's ``execution_result.json`` shape.

    ``artifact_paths`` maps artifact keys from outputs.json to absolute file paths.
    """
    stats = output.statistics
    warnings = output.metadata.get("warnings") or []
    formats = {"tree_points": "application/geo+json", "plugin_output": "application/json"}
    return {
        "execution_id": execution_id,
        "status": EXECUTION_STATUS[output.status],
        "execution_time_ms": execution_time_ms,
        "metrics": {
            "tree_count": stats["total_trees"],
            "avg_confidence": stats["avg_confidence"],
            "removed_low_confidence": stats["removed_low_confidence"],
            "trees_outside_aoi": stats["trees_outside_area"],
            "analysis_area_m2": stats["area_m2"],
            "tree_density": stats["density_value"],
            "tree_density_unit": stats["density_unit"],
        },
        "artifacts": {
            key: {"file_path": path, "format": formats[key]}
            for key, path in artifact_paths.items()
        },
        "error": "; ".join(warnings) if warnings else None,
    }
