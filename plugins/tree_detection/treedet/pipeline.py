"""Glue between the platform payload and the Module 7 features.

``parse_payload`` and ``analyze`` are pure; ``main.py`` does the I/O around them:

    payload.json --parse_payload--> RunRequest --detect_trees--> DetectionOutput
                 --analyze--> PluginOutput --to_execution_result--> result.json
"""

import os
from collections.abc import Mapping
from datetime import datetime
from typing import Any

from pydantic import BaseModel, ConfigDict, Field

from .area import count_trees_in_area, point_in_polygons, parse_polygons, polygon_area_m2
from .counting import count_trees
from .density import calculate_tree_density
from .errors import PluginExecutionError
from .filtering import filter_by_confidence
from .location import locate_detections, to_location_output
from .models import (
    DensityUnit,
    DetectionInput,
    DetectionOutput,
    GeoCoordinate,
    ImageGeoreference,
    PluginOutput,
)
from .output import build_plugin_output


class RunParameters(BaseModel):
    """User parameters from parameters.json (unknown keys are ignored)."""

    model_config = ConfigDict(frozen=True, extra="ignore")

    confidence_threshold: float = Field(default=0.5, ge=0.0, le=1.0)
    density_unit: DensityUnit = "per_ha"
    detector: str = "stub"
    iou_threshold: float = Field(default=0.5, ge=0.0, le=1.0)
    tile_size: int = Field(default=1024, gt=0)
    gsd_cm: float = Field(default=3.0, gt=0.0)


class RunRequest(BaseModel):
    model_config = ConfigDict(frozen=True)

    execution_id: str
    detection_input: DetectionInput
    parameters: RunParameters
    georef: ImageGeoreference
    aoi: dict[str, Any] | None = None


def parse_payload(payload: Mapping[str, Any]) -> RunRequest:
    """Validate an execution payload and pull out what Module 7 needs."""
    target = payload.get("target") or {}
    metadata = target.get("metadata") or {}
    parameters = RunParameters.model_validate(payload.get("parameters") or {})

    gps = metadata.get("gps")
    resolution = metadata.get("resolution")
    if not gps or not resolution:
        raise PluginExecutionError("Image metadata must contain 'gps' and 'resolution'")

    image_path = str(target.get("image_path", ""))
    image_id = str(
        metadata.get("image_id") or os.path.splitext(os.path.basename(image_path))[0]
    )
    georef = ImageGeoreference(
        center=GeoCoordinate(latitude=gps["lat"], longitude=gps["lon"]),
        gsd_m=float(metadata.get("gsd_m") or parameters.gsd_cm / 100),
        width_px=int(resolution[0]),
        height_px=int(resolution[1]),
        rotation_deg=float(metadata.get("rotation_deg", 0.0)),
    )
    aoi = payload.get("aoi")
    if aoi:
        parse_polygons(aoi["geometry"])  # fail early on an invalid AOI

    return RunRequest(
        execution_id=str(payload.get("execution_id", "exec_unknown")),
        detection_input=DetectionInput(
            image_path=image_path,
            image_id=image_id,
            project_id=str(payload.get("session_id") or "unknown"),
            metadata=dict(metadata),
        ),
        parameters=parameters,
        georef=georef,
        aoi=aoi,
    )


def model_config_of(parameters: RunParameters) -> dict[str, Any]:
    """The ``model_config`` argument for ``detect_trees``."""
    return {
        "detector": parameters.detector,
        "iou_threshold": parameters.iou_threshold,
        "tile_size": parameters.tile_size,
    }


def analyze(
    request: RunRequest,
    detection: DetectionOutput,
    counted_at: datetime,
    plugin_version: str,
) -> PluginOutput:
    """Run Features 2-6 and 8 on detector output. Pure."""
    params = request.parameters
    filtered = filter_by_confidence(detection.detections, params.confidence_threshold)
    located = locate_detections(filtered.kept, request.georef)

    if request.aoi:
        geometry = request.aoi["geometry"]
        boundary = {"type": "Feature", "properties": {"plot_id": "aoi"}, "geometry": geometry}
        polygons = parse_polygons(geometry)
        counted = tuple(
            d
            for d in located
            if d.center and point_in_polygons((d.center.longitude, d.center.latitude), polygons)
        )
        area_count = count_trees_in_area([d.center for d in located if d.center], boundary)
        area_m2 = polygon_area_m2(geometry)
    else:
        counted = located
        area_count = None
        g = request.georef
        area_m2 = g.width_px * g.height_px * g.gsd_m**2

    warnings = (
        ["'stub' detector used: results are placeholders, not real detections"]
        if detection.model_name == "stub"
        else []
    )
    run_metadata = {
        "plugin_version": plugin_version,
        "model_name": detection.model_name,
        "model_version": None,
        "executed_at": counted_at.isoformat(),
        "device": "cpu",
        "image_id": request.detection_input.image_id,
        "project_id": request.detection_input.project_id,
        "processing_time_s": detection.processing_time_s,
        "warnings": warnings,
    }

    return build_plugin_output(
        detections=counted,
        count=count_trees(counted, request.detection_input.image_id, counted_at),
        density=calculate_tree_density(len(counted), area_m2, params.density_unit),
        locations=to_location_output(counted),
        run_metadata=run_metadata,
        filtered=filtered,
        area_count=area_count,
    )
