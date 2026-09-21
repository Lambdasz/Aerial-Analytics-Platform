"""Model data bersama (Bagian 2 spesifikasi Modul 8)."""

from __future__ import annotations

import re
from typing import Literal, NamedTuple

import numpy as np
from pydantic import BaseModel, ConfigDict, Field, field_validator

SUPPORTED_MIME_TYPES = ("image/jpeg", "image/png", "image/tiff")
STANDARD_CLASS_NAMES = ("vegetation", "bare_soil", "water", "built_up", "other")
_HEX_COLOR = re.compile(r"^#[0-9A-Fa-f]{6}$")


# ---------------------------------------------------------------- exceptions
class PluginExecutionError(Exception):
    """Kesalahan yang menghentikan eksekusi; dilaporkan sebagai status 'failure'."""


class UnsupportedFormatError(PluginExecutionError):
    """Format citra di luar supported_mime_types pada inputs.json."""


# ---------------------------------------------------------------- data models
class ClassDefinition(BaseModel):
    """Definisi satu kelas land-cover yang dikonfigurasi pengguna."""

    id: int = Field(ge=1, le=255)  # 0 dicadangkan untuk "tidak terklasifikasi"
    name: str = Field(min_length=1)  # kelas standar: lihat STANDARD_CLASS_NAMES
    color: str  # hex, contoh "#2E7D32"

    @field_validator("color")
    @classmethod
    def _valid_color(cls, v: str) -> str:
        if not _HEX_COLOR.match(v):
            raise ValueError(f"warna harus berformat #RRGGBB, diterima: {v!r}")
        return v.upper()


DEFAULT_CLASSES = [  # dipakai bila parameters.classes_json kosong
    ClassDefinition(id=1, name="vegetation", color="#2E7D32"),
    ClassDefinition(id=2, name="bare_soil", color="#8D6E63"),
    ClassDefinition(id=3, name="water", color="#1976D2"),
    ClassDefinition(id=4, name="built_up", color="#757575"),
    ClassDefinition(id=5, name="other", color="#BDBDBD"),
]


class GpsPoint(BaseModel):
    lat: float = Field(ge=-90.0, le=90.0)
    lon: float = Field(ge=-180.0, le=180.0)


class ImageMetadata(BaseModel):
    """Sesuai target.metadata pada execution_payload (Modul 2)."""

    model_config = ConfigDict(extra="ignore")

    resolution: list[float]  # [width, height] dalam piksel - WAJIB
    gps: GpsPoint | None = None
    altitude: float | None = None  # meter; belum jelas relatif/ASL (Temuan 5)
    timestamp: str | None = None  # ISO 8601 UTC

    @field_validator("resolution")
    @classmethod
    def _two_positive(cls, v: list[float]) -> list[float]:
        if len(v) != 2 or any(x <= 0 for x in v):
            raise ValueError("resolution harus [width, height] dengan nilai > 0")
        return v


class PluginParameters(BaseModel):
    """Parameter eksekusi; nama dan batas nilai sama dengan parameters.json."""

    model_config = ConfigDict(extra="ignore")

    model_id: Literal["rf_superpixel_v1", "deeplab_resnet101_v1"] = "rf_superpixel_v1"
    min_confidence: float = Field(default=0.5, ge=0.0, le=1.0)
    segment_size: int = Field(default=500, ge=1)
    gsd_m_per_px: float = Field(default=0.0, ge=0.0)  # 0 = tidak diisi pengguna
    device: Literal["auto", "cpu", "cuda"] = "auto"
    classes_json: str = ""  # kosong = DEFAULT_CLASSES


class LandCoverInput(BaseModel):
    """Masukan internal plugin, dibentuk dari payload (lihat core/payload.py)."""

    execution_id: str
    session_id: str | None = None
    output_dir: str
    image_path: str
    mime_type: str
    metadata: ImageMetadata | None = None
    aoi_geometry: dict | None = None  # Polygon/MultiPolygon WGS84, urutan [lon, lat]
    classes: list[ClassDefinition]
    parameters: dict  # hasil PluginParameters.model_dump()


class ScaleInfo(BaseModel):
    """Informasi ground sampling distance untuk konversi piksel ke luas."""

    gsd_m_per_px: float | None = None
    source: Literal["explicit_parameter", "exif_xmp", "raster_crs", "none"] = "none"
    confidence: Literal["measured", "estimated", "none"] = "none"


class ClassPixelStatistic(BaseModel):
    """Statistik satu kelas hasil klasifikasi."""

    id: int
    name: str
    color: str
    pixels: int
    percent: float = Field(ge=0.0, le=100.0)  # terhadap total piksel yang dianalisis
    mean_confidence: float = Field(ge=0.0, le=1.0)
    area_m2: float | None = None  # None bila ScaleInfo.source == "none"


class ClassificationRaw(NamedTuple):  # keluaran Fitur 1
    class_mask: np.ndarray  # uint8, 2D
    confidence_map: np.ndarray  # float32, 2D
    model_name: str
    processing_time_s: float


class FilteredMask(NamedTuple):  # keluaran Fitur 2
    class_mask: np.ndarray
    unclassified_added: int


# ---------------------------------------------------------------- helpers
def mask_to_rgb(class_mask: np.ndarray, classes: list[ClassDefinition]) -> np.ndarray:
    """Ubah mask kelas menjadi gambar RGBA (H, W, 4) uint8.

    Piksel tidak terklasifikasi (0) dibuat transparan agar layer bisa ditumpuk
    di peta (raster_overlay) tanpa menutupi citra dasar.
    """
    lut = np.zeros((256, 4), dtype=np.uint8)
    for c in classes:
        r, g, b = (int(c.color[i : i + 2], 16) for i in (1, 3, 5))
        lut[c.id] = (r, g, b, 255)
    return lut[class_mask]
