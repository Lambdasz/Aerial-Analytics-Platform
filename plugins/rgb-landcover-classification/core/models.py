"""Shared data models for the DeepLabV3+ Module 8 plugin."""
from __future__ import annotations

from dataclasses import dataclass
from typing import NamedTuple
import re

import numpy as np

SUPPORTED_MIME_TYPES = ("image/jpeg", "image/png", "image/tiff")
STANDARD_CLASS_NAMES = ("vegetation", "bare_soil", "water", "built_up", "other")

class PluginExecutionError(Exception):
    pass

class UnsupportedFormatError(PluginExecutionError):
    pass

@dataclass(frozen=True)
class ClassDefinition:
    id: int
    name: str
    color: str

@dataclass(frozen=True)
class ImageMetadata:
    resolution: list[float]
    gps: dict | None = None
    altitude: float | None = None
    timestamp: str | None = None

@dataclass(frozen=True)
class ScaleInfo:
    gsd_m_per_px: float | None = None
    source: str = "none"
    confidence: str = "none"

@dataclass(frozen=True)
class ClassPixelStatistic:
    id: int
    name: str
    color: str
    pixels: int
    percent: float
    mean_confidence: float
    area_m2: float | None = None

class ClassificationRaw(NamedTuple):
    class_mask: np.ndarray
    confidence_map: np.ndarray
    model_name: str
    processing_time_s: float

class FilteredMask(NamedTuple):
    class_mask: np.ndarray
    unclassified_added: int

DEFAULT_CLASSES = [
    ClassDefinition(1, "vegetation", "#2E7D32"),
    ClassDefinition(2, "bare_soil", "#8D6E63"),
    ClassDefinition(3, "water", "#1976D2"),
    ClassDefinition(4, "built_up", "#757575"),
    ClassDefinition(5, "other", "#BDBDBD"),
]

@dataclass(frozen=True)
class LandCoverInput:
    execution_id: str
    output_dir: str
    image_path: str
    mime_type: str
    metadata: ImageMetadata | None
    aoi_geometry: dict | None
    classes: list[ClassDefinition]
    parameters: dict
    session_id: str | None = None

def parse_class_definitions(raw: list[dict] | None) -> list[ClassDefinition]:
    if not raw:
        return list(DEFAULT_CLASSES)
    if len(raw) != 5 or {item.get("id") for item in raw} != set(range(1, 6)):
        raise PluginExecutionError("classes must contain exactly IDs 1 through 5")
    result = [ClassDefinition(int(item["id"]), str(item["name"]), str(item["color"]).upper()) for item in raw]
    if len({c.name for c in result}) != 5 or any(not re.fullmatch(r"#[0-9A-F]{6}", c.color) for c in result):
        raise PluginExecutionError("classes require unique names and #RRGGBB colors")
    return result

def mask_to_rgb(class_mask: np.ndarray, classes: list[ClassDefinition]) -> np.ndarray:
    lut = np.zeros((256, 4), dtype=np.uint8)
    for item in classes:
        lut[item.id] = tuple(bytes.fromhex(item.color[1:])) + (255,)
    return lut[class_mask]
