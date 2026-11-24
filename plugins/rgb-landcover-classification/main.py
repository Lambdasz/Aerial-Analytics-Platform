#!/usr/bin/env python3
"""Module 8 RGB land-cover classification plugin using DeepLabV3+."""
from __future__ import annotations

import argparse
import json
import re
import sys
import time
from pathlib import Path

PLUGIN_DIR = Path(__file__).resolve().parent
DEFAULT_CLASSES = [
    {"id": 1, "name": "vegetation", "color": "#2E7D32"},
    {"id": 2, "name": "bare_soil", "color": "#8D6E63"},
    {"id": 3, "name": "water", "color": "#1976D2"},
    {"id": 4, "name": "built_up", "color": "#757575"},
    {"id": 5, "name": "other", "color": "#BDBDBD"},
]
BACKBONES = {"resnet101", "xception"}
SUPPORTED_MIME_TYPES = {"image/jpeg", "image/png", "image/tiff"}


def load_manifest() -> dict:
    return json.loads((PLUGIN_DIR / "manifest.json").read_text(encoding="utf-8"))


def validate_parameters(raw: dict) -> dict:
    if not isinstance(raw, dict):
        raise ValueError("parameters must be an object")
    backbone = raw.get("backbone", "resnet101")
    if backbone not in BACKBONES:
        raise ValueError("backbone must be 'resnet101' or 'xception'")
    confidence = float(raw.get("min_confidence", 0.5))
    if not 0 <= confidence <= 1:
        raise ValueError("min_confidence must be between 0 and 1")
    gsd = float(raw.get("gsd_m_per_px", 0))
    if gsd < 0:
        raise ValueError("gsd_m_per_px must be non-negative")
    return {**raw, "backbone": backbone, "min_confidence": confidence, "gsd_m_per_px": gsd}


def parse_classes(raw: str) -> list[dict]:
    if not raw or not raw.strip():
        return list(DEFAULT_CLASSES)
    try:
        classes = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise ValueError(f"classes_json is invalid: {exc}") from exc
    if not isinstance(classes, list) or len(classes) != len(DEFAULT_CLASSES):
        raise ValueError("classes_json must contain exactly five model classes")
    if any(not isinstance(item, dict) for item in classes):
        raise ValueError("classes_json entries must be objects")
    ids = {item.get("id") for item in classes}
    names = {item.get("name") for item in classes}
    if ids != set(range(1, 6)) or len(names) != 5:
        raise ValueError("classes_json IDs must be exactly 1, 2, 3, 4, 5 with unique names")
    if any(
        not isinstance(item.get("name"), str)
        or not item["name"].strip()
        or not isinstance(item.get("color"), str)
        or not re.fullmatch(r"#[0-9A-Fa-f]{6}", item["color"])
        for item in classes
    ):
        raise ValueError("classes_json entries require a name and #RRGGBB color")
    return classes


def report_progress(job_id: str, percent: int, stage: str, stream=sys.stdout) -> None:
    event = {"job_id": job_id, "percent": max(0, min(100, int(percent))), "stage": stage}
    print(f"PROGRESS:{json.dumps(event, separators=(',', ':'))}", file=stream, flush=True)


def _point_in_polygon(x: float, y: float, polygon: list[list[float]]) -> bool:
    inside = False
    for index, (x1, y1) in enumerate(polygon):
        x2, y2 = polygon[(index + 1) % len(polygon)]
        crosses = (y1 > y) != (y2 > y)
        if crosses and x < (x2 - x1) * (y - y1) / (y2 - y1) + x1:
            inside = not inside
    return inside


def build_analysis_mask(aoi: dict | None, width: int, height: int, metadata: dict) -> list[list[bool]]:
    if not aoi:
        return [[True] * width for _ in range(height)]
    geometry = aoi.get("geometry", aoi)
    if geometry.get("type") != "Polygon":
        raise ValueError("AOI geometry must be a GeoJSON Polygon")
    coordinates = geometry.get("coordinates") or []
    if not coordinates or len(coordinates[0]) < 4:
        raise ValueError("AOI polygon must contain a closed ring")
    ring = coordinates[0]
    return [[_point_in_polygon(x + 0.5, y + 0.5, ring) for x in range(width)] for y in range(height)]


def calculate_metrics(
    labels: list[list[int]],
    gsd: float,
    analysis_mask: list[list[bool]],
    classes: list[dict] | None = None,
) -> dict:
    classes = classes or DEFAULT_CLASSES
    selected = [label for row, mask_row in zip(labels, analysis_mask) for label, selected in zip(row, mask_row) if selected]
    total = len(selected)
    metrics = {
        f"{name}_pct": (
            round(selected.count(next(item["id"] for item in classes if item["name"] == name)) / total * 100, 4)
            if total and any(item["name"] == name for item in classes)
            else None
        )
        for name in (item["name"] for item in DEFAULT_CLASSES)
    }
    metrics["unclassified_pct"] = round(selected.count(0) / total * 100, 4) if total else 0.0
    metrics["total_pixels_analyzed"] = total
    metrics["gsd_m_per_px"] = gsd or None
    for item in DEFAULT_CLASSES:
        key = f"{item['name']}_area_m2"
        configured = next((configured for configured in classes if configured["name"] == item["name"]), None)
        metrics[key] = (
            round(selected.count(configured["id"]) * gsd * gsd, 4)
            if configured and gsd
            else None
        )
    return metrics


def build_boundary_geojson(labels: list[list[int]], classes: list[dict], analysis_mask: list[list[bool]]) -> dict:
    """Create pixel-coordinate class extents for map/layer adapters.

    The result is explicitly pixel CRS unless the caller supplies a georeference
    transform in a later integration layer.
    """
    features = []
    for item in classes:
        points = [
            (column, row)
            for row, mask_row in enumerate(analysis_mask)
            for column, selected in enumerate(mask_row)
            if selected and labels[row][column] == item["id"]
        ]
        if not points:
            continue
        xs, ys = zip(*points)
        min_x, max_x, min_y, max_y = min(xs), max(xs) + 1, min(ys), max(ys) + 1
        features.append(
            {
                "type": "Feature",
                "properties": {"class_id": item["id"], "class_name": item["name"], "color": item["color"]},
                "geometry": {
                    "type": "Polygon",
                    "coordinates": [[[min_x, min_y], [max_x, min_y], [max_x, max_y], [min_x, max_y], [min_x, min_y]]],
                },
            }
        )
    return {"type": "FeatureCollection", "features": features, "coordinate_system": "pixel"}


def georeference_boundary(boundary: dict, georeference: dict, width: int, height: int) -> dict:
    """Convert pixel GeoJSON to WGS84 using [min_lon, min_lat, max_lon, max_lat]."""
    bounds = georeference.get("bounds")
    crs = georeference.get("crs")
    if crs not in {"EPSG:4326", "WGS84"}:
        raise ValueError("boundary georeferencing requires WGS84/EPSG:4326 metadata")
    if not isinstance(bounds, list) or len(bounds) != 4 or width <= 0 or height <= 0:
        raise ValueError("georeferencing bounds must be [min_lon, min_lat, max_lon, max_lat]")
    min_lon, min_lat, max_lon, max_lat = (float(value) for value in bounds)

    def transform(point: list[float]) -> list[float]:
        x, y = point
        return [
            round(min_lon + (x / width) * (max_lon - min_lon), 9),
            round(max_lat - (y / height) * (max_lat - min_lat), 9),
        ]

    transformed = json.loads(json.dumps(boundary))
    for feature in transformed["features"]:
        for ring in feature["geometry"]["coordinates"]:
            for index, point in enumerate(ring):
                ring[index] = transform(point)
    transformed["coordinate_system"] = "WGS84"
    transformed["crs"] = "EPSG:4326"
    return transformed


def healthcheck() -> int:
    manifest = load_manifest()
    model_spec = json.loads((PLUGIN_DIR / "model.json").read_text(encoding="utf-8"))
    details = {"model_family": model_spec["family"], "supported_backbones": model_spec["supported_backbones"]}
    missing = []
    for module in ("torch", "torchvision", "PIL"):
        try:
            __import__(module)
        except ImportError:
            missing.append(module)
    details["dependencies_ready"] = not missing
    details["missing_dependencies"] = missing
    info = {
        "status": "healthy" if not missing else "unhealthy",
        "plugin_id": manifest["metadata"]["id"],
        "version": manifest["metadata"]["version"],
        "details": details,
        "error": None if not missing else "Missing dependencies: " + ", ".join(missing),
    }
    print(json.dumps(info))
    return 0 if not missing else 1


def _write_failure(path: Path, execution_id: str, message: str) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"execution_id": execution_id, "status": "failure", "execution_time_ms": 0, "metrics": {}, "artifacts": {}, "error": message}, indent=2), encoding="utf-8")
    print(message, file=sys.stderr)
    return 1


def run(input_path: Path, output_path: Path) -> int:
    started = time.perf_counter()
    try:
        payload = json.loads(input_path.read_text(encoding="utf-8"))
        execution_id = str(payload.get("execution_id", "unknown"))
        report_progress(execution_id, 5, "validating_input")
        target = payload["target"]
        image_path = Path(target["image_path"])
        params = validate_parameters(payload.get("parameters", {}))
        classes = parse_classes(params.get("classes_json", ""))
        if target["mime_type"] not in SUPPORTED_MIME_TYPES:
            raise ValueError("unsupported image MIME type")
        if not image_path.is_file():
            raise FileNotFoundError(f"image not found: {image_path}")
        from PIL import Image
        import torch
        from core.classify import load_checkpoint_state_dict
        from model import build_model
        image = Image.open(image_path).convert("RGB")
        width, height = image.size
        report_progress(execution_id, 15, "loading_model")
        tensor = torch.from_numpy(__import__("numpy").array(image)).permute(2, 0, 1).float().div(255).unsqueeze(0)
        model = build_model(params["backbone"], len(classes))
        weights = Path(params.get("weights_path", f"models/deeplabv3plus_{params['backbone']}.pt"))
        if not weights.is_absolute():
            weights = PLUGIN_DIR / weights
        if not weights.is_file():
            raise FileNotFoundError(f"trained weights not found: {weights}")
        checkpoint = torch.load(weights, map_location="cpu", weights_only=True)
        model.load_state_dict(load_checkpoint_state_dict(checkpoint))
        model.eval()
        report_progress(execution_id, 45, "classifying")
        with torch.no_grad():
            probabilities = torch.softmax(model(tensor), dim=1)[0]
        confidence, labels = probabilities.max(dim=0)
        labels = labels.add(1)
        labels[confidence < params["min_confidence"]] = 0
        values = labels.cpu().tolist()
        metadata = target.get("metadata") or {}
        analysis_mask = build_analysis_mask(payload.get("aoi"), width, height, metadata)
        values = [[value if analysis_mask[row][column] else 0 for column, value in enumerate(line)] for row, line in enumerate(values)]
        colors = {0: (0, 0, 0, 0), **{c["id"]: tuple(bytes.fromhex(c["color"][1:])) + (255,) for c in classes}}
        rgba = [colors.get(v, colors[0]) for row in values for v in row]
        out_dir = Path(payload.get("output_dir") or output_path.parent)
        out_dir.mkdir(parents=True, exist_ok=True)
        mask_path = out_dir / "mask.png"
        mask = Image.new("RGBA", (width, height)); mask.putdata(rgba); mask.save(mask_path)
        report_progress(execution_id, 85, "computing_statistics")
        metrics = calculate_metrics(values, params["gsd_m_per_px"], analysis_mask, classes)
        per_class = []
        selected_count = metrics["total_pixels_analyzed"]
        for item in classes:
            pixels = sum(
                value == item["id"]
                for row, mask_row in zip(values, analysis_mask)
                for value, selected in zip(row, mask_row)
                if selected
            )
            per_class.append(
                {
                    **item,
                    "pixels": pixels,
                    "percent": round(pixels / selected_count * 100, 4) if selected_count else 0.0,
                    "area_m2": round(pixels * params["gsd_m_per_px"] ** 2, 4)
                    if params["gsd_m_per_px"]
                    else None,
                }
            )
        stats = {
            "backbone": params["backbone"],
            "model_family": "DeepLabV3+",
            "classes": per_class,
            "metrics": metrics,
        }
        stats_path = out_dir / "class_statistics.json"
        stats_path.write_text(json.dumps(stats, indent=2), encoding="utf-8")
        boundary = build_boundary_geojson(values, classes, analysis_mask)
        georeference = (metadata.get("georeference") or {})
        if georeference:
            boundary = georeference_boundary(boundary, georeference, width, height)
        boundary_path = out_dir / "boundary.geojson"
        boundary_path.write_text(json.dumps(boundary, indent=2), encoding="utf-8")
        elapsed = int((time.perf_counter() - started) * 1000)
        report_progress(execution_id, 100, "writing_output")
        mask_artifact = {"file_path": str(mask_path.resolve()), "format": "image/png"}
        if georeference.get("crs") in {"EPSG:4326", "WGS84"} and georeference.get("bounds"):
            mask_artifact["bounds"] = georeference["bounds"]
        boundary_artifact = {"file_path": str(boundary_path.resolve()), "format": "application/geo+json"}
        result = {"execution_id": execution_id, "status": "success", "execution_time_ms": elapsed, "metrics": metrics, "artifacts": {"mask_raster": mask_artifact, "boundary_vector": boundary_artifact, "class_statistics": {"file_path": str(stats_path.resolve()), "format": "application/json"}}, "error": None}
        output_path.write_text(json.dumps(result, indent=2), encoding="utf-8")
        return 0
    except Exception as exc:
        return _write_failure(output_path, locals().get("execution_id", "unknown"), f"{type(exc).__name__}: {exc}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--healthcheck", action="store_true")
    parser.add_argument("--validate-parameters")
    parser.add_argument("--input")
    parser.add_argument("--output")
    args = parser.parse_args()
    if args.healthcheck:
        return healthcheck()
    if args.validate_parameters:
        try:
            validate_parameters(json.loads(args.validate_parameters)); print("valid"); return 0
        except (ValueError, json.JSONDecodeError) as exc:
            print(str(exc), file=sys.stderr); return 2
    if args.input and args.output:
        return run(Path(args.input), Path(args.output))
    parser.error("provide --healthcheck or --input and --output")


if __name__ == "__main__":
    raise SystemExit(main())
