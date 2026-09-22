"""Fitur 4: Class Statistics Aggregation."""

from __future__ import annotations

import numpy as np

from .models import ClassDefinition, ClassPixelStatistic, ScaleInfo


def total_analyzed_pixels(class_mask: np.ndarray, analysis_mask: np.ndarray | None = None) -> int:
    """Total piksel yang dianalisis: seluruh citra, atau hanya piksel di dalam AOI."""
    if analysis_mask is None:
        return int(class_mask.size)
    return int(np.count_nonzero(analysis_mask))


def compute_class_statistics(
    class_mask: np.ndarray,
    confidence_map: np.ndarray,
    classes: list[ClassDefinition],
    scale: ScaleInfo,
    analysis_mask: np.ndarray | None = None,
) -> list[ClassPixelStatistic]:
    """Statistik per kelas, diurutkan menurun berdasarkan percent.

    `percent` dihitung terhadap seluruh piksel yang dianalisis, termasuk piksel
    tidak terklasifikasi. Kelas tanpa piksel (pixels = 0) tidak dimasukkan.
    `analysis_mask` (opsional) menandai piksel di dalam AOI.
    """
    total = total_analyzed_pixels(class_mask, analysis_mask)
    if total == 0:
        return []
    flat = class_mask.ravel()
    pixel_counts = np.bincount(flat, minlength=256)
    conf_sums = np.bincount(flat, weights=confidence_map.ravel().astype(np.float64), minlength=256)
    gsd = scale.gsd_m_per_px

    stats: list[ClassPixelStatistic] = []
    for c in classes:
        pixels = int(pixel_counts[c.id])
        if pixels == 0:
            continue
        stats.append(
            ClassPixelStatistic(
                id=c.id,
                name=c.name,
                color=c.color,
                pixels=pixels,
                percent=min(100.0, pixels / total * 100.0),
                mean_confidence=float(min(1.0, max(0.0, conf_sums[c.id] / pixels))),
                area_m2=(pixels * gsd * gsd) if gsd else None,
            )
        )
    stats.sort(key=lambda s: s.percent, reverse=True)
    return stats
