"""Fitur 6: Plugin-Compatible Output.

Menulis artifact ke `output_dir`, meratakan statistik menjadi metrik skalar, dan
mengembalikan dict berformat execution_result.schema.json (Modul 2):
execution_id, status, execution_time_ms, metrics, artifacts, error.
"""

from __future__ import annotations

import json
import os

import numpy as np
from PIL import Image

from .models import (
    STANDARD_CLASS_NAMES,
    ClassDefinition,
    ClassPixelStatistic,
    ScaleInfo,
    mask_to_rgb,
)
from .statistics import total_analyzed_pixels

_NDIGITS = 4


def _round(value: float | None) -> float | None:
    return None if value is None else round(float(value), _NDIGITS)


def flatten_metrics(
    classes: list[ClassDefinition],
    per_class_statistics: list[ClassPixelStatistic],
    scale: ScaleInfo,
    total_pixels: int,
) -> dict:
    """Ratakan statistik ke metrik skalar sesuai outputs.json.

    Hanya lima kelas standar yang mendapat metrik sendiri (key harus dideklarasikan
    statis di outputs.json); kelas kustom hanya ada di artifact class_statistics.
    Nilai None (null) berarti: kelas tidak dikonfigurasi, atau skala tidak diketahui.
    """
    by_name = {s.name: s for s in per_class_statistics}
    configured = {c.name for c in classes}
    gsd = scale.gsd_m_per_px
    metrics: dict = {}
    for name in STANDARD_CLASS_NAMES:
        if name not in configured:
            metrics[f"{name}_pct"] = None
            metrics[f"{name}_area_m2"] = None
            continue
        stat = by_name.get(name)
        metrics[f"{name}_pct"] = _round(stat.percent) if stat else 0.0
        if stat is not None:
            metrics[f"{name}_area_m2"] = _round(stat.area_m2)
        else:
            metrics[f"{name}_area_m2"] = 0.0 if gsd else None

    classified = sum(s.pixels for s in per_class_statistics)
    unclassified = max(0, total_pixels - classified)
    metrics["unclassified_pct"] = _round(unclassified / total_pixels * 100.0) if total_pixels else 0.0
    metrics["total_pixels_analyzed"] = int(total_pixels)
    metrics["gsd_m_per_px"] = _round(gsd) if gsd else None
    return metrics


def build_plugin_output(
    class_mask: np.ndarray,
    classes: list[ClassDefinition],
    per_class_statistics: list[ClassPixelStatistic],
    scale: ScaleInfo,
    boundary_geojson: str | None,
    run_metadata: dict,
    output_dir: str,
    execution_id: str,
    raster_bounds: list[float] | None = None,
    warnings: list[str] | None = None,
    analysis_mask: np.ndarray | None = None,
) -> dict:
    """Tulis artifact dan susun hasil akhir.

    Argumen opsional di luar spesifikasi dasar: `warnings` (bila tidak kosong,
    status menjadi "warning" dan pesan digabung ke field `error`) dan
    `analysis_mask` (mask AOI untuk menghitung total piksel).
    Status "failure" tidak dibuat di sini, melainkan oleh blok except di main.py.
    """
    output_dir = os.path.abspath(output_dir)
    os.makedirs(output_dir, exist_ok=True)

    # 1. mask raster (RGBA; kelas 0 transparan)
    mask_path = os.path.join(output_dir, "mask.png")
    Image.fromarray(mask_to_rgb(class_mask, classes)).save(mask_path)
    mask_artifact: dict = {"file_path": mask_path, "format": "image/png"}
    if raster_bounds is not None:
        mask_artifact["bounds"] = [float(x) for x in raster_bounds]
    artifacts: dict = {"mask_raster": mask_artifact}

    # 2. boundary vector (opsional; tanpa key bounds)
    if boundary_geojson is not None:
        boundary_path = os.path.join(output_dir, "boundary.geojson")
        with open(boundary_path, "w", encoding="utf-8") as fh:
            fh.write(boundary_geojson)
        artifacts["boundary_vector"] = {"file_path": boundary_path, "format": "application/geo+json"}

    # 3. statistik lengkap (dibaca Modul 9 dan 11), termasuk kelas kustom
    total = total_analyzed_pixels(class_mask, analysis_mask)
    metrics = flatten_metrics(classes, per_class_statistics, scale, total)
    stats_path = os.path.join(output_dir, "class_statistics.json")
    with open(stats_path, "w", encoding="utf-8") as fh:
        json.dump(
            {
                "per_class_statistics": [s.model_dump() for s in per_class_statistics],
                "unclassified": {
                    "pixels": total - sum(s.pixels for s in per_class_statistics),
                    "percent": metrics["unclassified_pct"],
                },
                "total_pixels_analyzed": total,
                "scale": scale.model_dump(),
                "run_metadata": run_metadata,
            },
            fh,
            indent=2,
        )
    artifacts["class_statistics"] = {"file_path": stats_path, "format": "application/json"}

    return {
        "execution_id": execution_id,
        "status": "warning" if warnings else "success",
        "execution_time_ms": int(run_metadata.get("execution_time_ms", 0)),
        "metrics": metrics,
        "artifacts": artifacts,
        "error": "; ".join(warnings) if warnings else None,
    }
