from __future__ import annotations
import numpy as np
from .models import ClassPixelStatistic, ScaleInfo

def total_analyzed_pixels(class_mask: np.ndarray, analysis_mask=None) -> int:
    return int(np.count_nonzero(analysis_mask)) if analysis_mask is not None else int(class_mask.size)

def compute_class_statistics(class_mask, confidence_map, classes, scale: ScaleInfo, analysis_mask=None):
    mask = np.ones(class_mask.shape, dtype=bool) if analysis_mask is None else analysis_mask
    total = int(mask.sum())
    stats = []
    for item in classes:
        pixels_mask = (class_mask == item.id) & mask
        pixels = int(pixels_mask.sum())
        if pixels == 0: continue
        stats.append(ClassPixelStatistic(item.id, item.name, item.color, pixels, pixels / total * 100 if total else 0.0, float(confidence_map[pixels_mask].mean()), pixels * scale.gsd_m_per_px ** 2 if scale.gsd_m_per_px else None))
    return sorted(stats, key=lambda item: item.percent, reverse=True)
