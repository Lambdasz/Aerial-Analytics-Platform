"""Shared data contracts for Module 7 (Tree Detection & Counting).

Every model is frozen: functions never mutate a model, they return a new one.
All models serialize to JSON with ``model.model_dump(mode="json")``.
"""

from datetime import datetime
from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field, model_validator

DensityUnit = Literal["per_m2", "per_ha"]
PluginStatus = Literal["success", "partial", "failed"]


class _Frozen(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")


class BBox(_Frozen):
    """Bounding box of one tree crown in image pixel coordinates."""

    x_min: float
    y_min: float
    x_max: float
    y_max: float

    @model_validator(mode="after")
    def _check_order(self) -> "BBox":
        if self.x_max < self.x_min or self.y_max < self.y_min:
            raise ValueError("bbox max must be >= min")
        return self

    @property
    def center(self) -> tuple[float, float]:
        return ((self.x_min + self.x_max) / 2, (self.y_min + self.y_max) / 2)

    @property
    def area_px(self) -> float:
        return (self.x_max - self.x_min) * (self.y_max - self.y_min)


class GeoCoordinate(_Frozen):
    """Real-world position (WGS84) derived from image pixels."""

    latitude: float = Field(ge=-90, le=90)
    longitude: float = Field(ge=-180, le=180)


class TreeDetection(_Frozen):
    """One detected tree.

    ``center`` and ``area_m2`` stay ``None`` until the detection has been
    georeferenced by ``location.locate_detections``.
    """

    detection_id: str
    bbox: BBox
    confidence: float = Field(ge=0.0, le=1.0)
    center: GeoCoordinate | None = None
    area_m2: float | None = Field(default=None, ge=0.0)


class DetectionInput(_Frozen):
    """Input the plugin receives from the Plugin Manager (Module 2)."""

    image_path: str
    image_id: str
    project_id: str
    crs: str = "EPSG:4326"
    metadata: dict[str, Any] | None = None


class ImageGeoreference(_Frozen):
    """What is needed to turn pixel positions into WGS84 coordinates.

    Assumes a nadir (straight-down) photo whose center is at ``center``.
    ``rotation_deg`` is the clockwise angle from north to the image's top edge.
    """

    center: GeoCoordinate
    gsd_m: float = Field(gt=0.0)
    width_px: int = Field(gt=0)
    height_px: int = Field(gt=0)
    rotation_deg: float = 0.0


class DetectionOutput(_Frozen):
    detections: tuple[TreeDetection, ...]
    image_id: str
    model_name: str
    processing_time_s: float = Field(ge=0.0)


class CountOutput(_Frozen):
    total_trees: int = Field(ge=0)
    image_id: str
    avg_confidence: float = Field(ge=0.0, le=1.0)
    counted_at: datetime


class FilteredOutput(_Frozen):
    kept: tuple[TreeDetection, ...]
    removed_count: int = Field(ge=0)
    threshold_used: float = Field(ge=0.0, le=1.0)


class LocationOutput(_Frozen):
    tree_points_geojson: str
    crs: str
    detection_ids: tuple[str, ...]


class AreaCountOutput(_Frozen):
    plot_id: str
    tree_count: int = Field(ge=0)
    outside_count: int = Field(ge=0)


class DensityOutput(_Frozen):
    density_value: float = Field(ge=0.0)
    unit: DensityUnit
    area_m2: float = Field(gt=0.0)


class ReviewCorrections(_Frozen):
    remove_detection_ids: tuple[str, ...] = ()
    manual_additions: tuple[TreeDetection, ...] = ()
    reviewer_id: str
    note: str | None = None


class ReviewOutput(_Frozen):
    final_detections: tuple[TreeDetection, ...]
    removed_ids: tuple[str, ...]
    added_count: int = Field(ge=0)
    reviewer_id: str


class PluginOutput(_Frozen):
    status: PluginStatus
    detections: tuple[TreeDetection, ...]
    statistics: dict[str, Any]
    geojson: str
    metadata: dict[str, Any]
