#!/usr/bin/env python3
"""Entrypoint plugin Modul 8: RGB Land-Cover Classification.

Mengikuti kontrak CLI standar Modul 2:
    main.py --healthcheck
    main.py --input payload.json --output result.json

Alur run_analysis (Bagian 4 spesifikasi):
    payload -> LandCoverInput -> resolve_gsd -> classify_landcover
            -> apply_confidence_threshold -> compute_class_statistics
            -> generate_boundary_vector (bila ada georeferensi)
            -> build_plugin_output -> result.json
Progres dicetak ke stdout sebagai baris `PROGRESS: {...}`.
"""

from __future__ import annotations

import argparse
import importlib
import json
import math
import os
import platform
import sys
import time
from datetime import datetime, timezone
from typing import Any

PLUGIN_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, PLUGIN_DIR)

REQUIRED_MODULES = ["numpy", "pydantic", "PIL", "sklearn", "skimage", "joblib"]
OPTIONAL_MODULES = ["rasterio"]  # tanpa rasterio: tidak ada vektor, AOI, dan GSD dari raster


def load_manifest_metadata() -> tuple[str, str]:
    """Baca id dan versi plugin dari manifest.json agar healthcheck selalu sinkron."""
    plugin_id, version = "rgb-landcover-classification", "0.1.0"
    try:
        with open(os.path.join(PLUGIN_DIR, "manifest.json"), "r", encoding="utf-8") as fh:
            meta = json.load(fh).get("metadata", {})
        plugin_id, version = meta.get("id", plugin_id), meta.get("version", version)
    except Exception:
        pass
    return plugin_id, version


def check_dependencies() -> tuple[list[str], list[str], dict[str, str]]:
    """Kembalikan (modul wajib yang hilang, modul opsional yang hilang, status per modul)."""
    missing_required, missing_optional, statuses = [], [], {}
    for name in REQUIRED_MODULES + OPTIONAL_MODULES:
        try:
            importlib.import_module(name)
            statuses[name] = "available"
        except ImportError as err:
            statuses[name] = f"missing: {err}"
            (missing_required if name in REQUIRED_MODULES else missing_optional).append(name)
    return missing_required, missing_optional, statuses


def handle_healthcheck() -> None:
    """Exit 0 + JSON di stdout bila siap; exit 1 + JSON di stderr bila tidak."""
    plugin_id, version = load_manifest_metadata()
    missing, missing_opt, statuses = check_dependencies()
    if missing:
        info = {
            "status": "unhealthy",
            "plugin_id": plugin_id,
            "version": version,
            "details": None,
            "error": (
                f"ModuleNotFoundError: missing {', '.join(missing)}. "
                "Please install required dependencies from requirements.txt."
            ),
        }
        print(json.dumps(info, indent=2), file=sys.stderr)
        sys.exit(1)
    info = {
        "status": "healthy",
        "plugin_id": plugin_id,
        "version": version,
        "details": {
            "python_version": platform.python_version(),
            "platform": sys.platform,
            "dependencies_ready": True,
            "dependencies": statuses,
            "optional_missing": missing_opt,
        },
        "error": None,
    }
    print(json.dumps(info))
    sys.exit(0)


def _write_json(path: str, data: dict[str, Any]) -> None:
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(data, fh, indent=2, allow_nan=False)


def _failure(execution_id: str, start: float, message: str) -> dict[str, Any]:
    return {
        "execution_id": execution_id,
        "status": "failure",
        "execution_time_ms": int((time.perf_counter() - start) * 1000),
        "metrics": {},
        "artifacts": {},
        "error": message,
    }


def _resolve_output_dir(raw_output_dir: str, output_path: str) -> str:
    fallback = os.path.dirname(os.path.abspath(output_path))
    if not raw_output_dir or raw_output_dir.startswith("/absolute/"):  # placeholder contoh template
        return fallback
    try:
        os.makedirs(raw_output_dir, exist_ok=True)
        return raw_output_dir
    except OSError:
        return fallback


def run_analysis(input_path: str, output_path: str) -> None:
    start = time.perf_counter()
    execution_id = "unknown"

    if not os.path.exists(input_path):
        _write_json(output_path, _failure(execution_id, start, f"Input payload file not found: {input_path}"))
        sys.exit(1)

    try:
        from core.classify import apply_confidence_threshold, classify_landcover, default_model_config
        from core.geo import aoi_pixel_mask, read_wgs84_georef
        from core.models import PluginExecutionError
        from core.output import build_plugin_output
        from core.payload import build_input_from_payload
        from core.progress import report_progress
        from core.scale import resolve_gsd
        from core.statistics import compute_class_statistics
        from core.vector import generate_boundary_vector
    except Exception as exc:  # dependensi hilang, dll.
        _write_json(output_path, _failure(execution_id, start, f"{type(exc).__name__}: {exc}"))
        sys.exit(1)

    try:
        with open(input_path, "r", encoding="utf-8") as fh:
            payload = json.load(fh)
        execution_id = str(payload.get("execution_id", "unknown"))
        output_dir = _resolve_output_dir(payload.get("output_dir", ""), output_path)
        os.makedirs(output_dir, exist_ok=True)
        warnings: list[str] = []

        inp = build_input_from_payload(payload, output_dir)

        report_progress(execution_id, 5, "resolving_scale")
        scale = resolve_gsd(inp.parameters, inp.metadata, inp.image_path)

        report_progress(execution_id, 10, "loading_image")
        georef = read_wgs84_georef(inp.image_path)

        report_progress(execution_id, 15, "classifying")
        raw = classify_landcover(inp, default_model_config(inp, PLUGIN_DIR))

        # AOI hanya bisa diterapkan bila piksel dapat dipetakan ke koordinat (georeferensi)
        analysis_mask = None
        if inp.aoi_geometry is not None:
            if georef is not None:
                analysis_mask = aoi_pixel_mask(inp.aoi_geometry, georef.transform, raw.class_mask.shape)
            else:
                warnings.append("AOI diabaikan karena citra tidak memiliki georeferensi")

        report_progress(execution_id, 75, "filtering_confidence")
        filtered = apply_confidence_threshold(raw.class_mask, raw.confidence_map, inp.parameters["min_confidence"])
        class_mask = filtered.class_mask
        if analysis_mask is not None:
            class_mask = class_mask.copy()
            class_mask[~analysis_mask] = 0

        report_progress(execution_id, 80, "computing_statistics")
        stats = compute_class_statistics(class_mask, raw.confidence_map, inp.classes, scale, analysis_mask)

        boundary = None
        if georef is not None:
            report_progress(execution_id, 88, "vectorizing_boundary")
            try:
                boundary = generate_boundary_vector(class_mask, inp.classes, georef.transform)
            except PluginExecutionError as exc:  # langkah non-esensial: hasil tetap dikirim
                warnings.append(f"Vektorisasi dilewati: {exc}")

        report_progress(execution_id, 95, "writing_output")
        _, version = load_manifest_metadata()
        run_metadata = {
            "plugin_version": version,
            "model_name": raw.model_name,
            "model_version": raw.model_name.rsplit("_", 1)[-1],
            "executed_at": datetime.now(timezone.utc).isoformat(),
            "device": "cpu",
            "execution_time_ms": int((time.perf_counter() - start) * 1000),
        }
        result = build_plugin_output(
            class_mask, inp.classes, stats, scale, boundary, run_metadata,
            output_dir, execution_id,
            raster_bounds=georef.bounds if georef else None,
            warnings=warnings, analysis_mask=analysis_mask,
        )  # fmt: skip
        result["execution_time_ms"] = int((time.perf_counter() - start) * 1000)
        for value in result["metrics"].values():  # JSON tidak boleh memuat NaN/Infinity
            if isinstance(value, float) and not math.isfinite(value):
                raise PluginExecutionError("Metrik bernilai tidak terhingga/NaN")
        _write_json(output_path, result)
        print(f"Analysis completed in {result['execution_time_ms']}ms (status: {result['status']})")
        code = 0
    except Exception as exc:
        _write_json(output_path, _failure(execution_id, start, f"{type(exc).__name__}: {exc}"))
        print(f"Analysis failed: {exc}", file=sys.stderr)
        code = 1
    sys.exit(code)


def main() -> None:
    parser = argparse.ArgumentParser(description="RGB Land-Cover Classification plugin")
    parser.add_argument("--healthcheck", action="store_true", help="Cek lingkungan lalu keluar")
    parser.add_argument("--input", type=str, help="Path execution payload (payload.json)")
    parser.add_argument("--output", type=str, help="Path tujuan execution result (result.json)")
    args = parser.parse_args()

    if args.healthcheck:
        handle_healthcheck()
    elif args.input and args.output:
        run_analysis(args.input, args.output)
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
