from __future__ import annotations
import json
from pathlib import Path
from PIL import Image
from .models import mask_to_rgb, STANDARD_CLASS_NAMES, ClassDefinition, ClassPixelStatistic, ScaleInfo

def flatten_metrics(classes, statistics, scale, total_pixels):
    by_name = {item.name: item for item in statistics}
    configured = {item.name: item for item in classes}
    metrics = {}
    for name in STANDARD_CLASS_NAMES:
        item = by_name.get(name)
        metrics[f"{name}_pct"] = round(item.percent, 4) if item else (None if name not in configured else 0.0)
        metrics[f"{name}_area_m2"] = round(item.area_m2, 4) if item and item.area_m2 is not None else (None if not scale.gsd_m_per_px else 0.0)
    classified = sum(item.pixels for item in statistics)
    metrics["unclassified_pct"] = round(max(0, total_pixels - classified) / total_pixels * 100, 4) if total_pixels else 0.0
    metrics["total_pixels_analyzed"] = int(total_pixels)
    metrics["gsd_m_per_px"] = scale.gsd_m_per_px
    return metrics

def build_plugin_output(class_mask, classes, per_class_statistics, scale, boundary_geojson, run_metadata, output_dir, execution_id, raster_bounds=None, warnings=None, analysis_mask=None):
    out = Path(output_dir); out.mkdir(parents=True, exist_ok=True)
    mask_path = out / "mask.png"; Image.fromarray(mask_to_rgb(class_mask, classes)).save(mask_path)
    artifacts = {"mask_raster": {"file_path": str(mask_path.resolve()), "format": "image/png"}}
    if raster_bounds: artifacts["mask_raster"]["bounds"] = raster_bounds
    if boundary_geojson:
        boundary_path = out / "boundary.geojson"; boundary_path.write_text(boundary_geojson, encoding="utf-8")
        artifacts["boundary_vector"] = {"file_path": str(boundary_path.resolve()), "format": "application/geo+json"}
    total = int(analysis_mask.sum()) if analysis_mask is not None else int(class_mask.size)
    metrics = flatten_metrics(classes, per_class_statistics, scale, total)
    stats_path = out / "class_statistics.json"
    stats_path.write_text(json.dumps({"per_class_statistics": [item.__dict__ for item in per_class_statistics], "metrics": metrics, "scale": scale.__dict__, "run_metadata": run_metadata}, indent=2), encoding="utf-8")
    artifacts["class_statistics"] = {"file_path": str(stats_path.resolve()), "format": "application/json"}
    return {"execution_id": execution_id, "status": "warning" if warnings else "success", "execution_time_ms": int(run_metadata.get("execution_time_ms", 0)), "metrics": metrics, "artifacts": artifacts, "error": "; ".join(warnings) if warnings else None}
