#!/usr/bin/env python3
"""
RGB Vegetation Detection (Module 4) - plugin skeleton.

Follows the Module 2 plugin contract (see plugins/template and plugins/mock):

    python main.py --healthcheck
    python main.py --input <execution_payload.json> --output <execution_result.json>

This is a skeleton: the detection step is simulated (fixed mask and coverage
value). Replace `simulate_detection()` with the real ExG/VARI/GLI pipeline later.
Only the Python standard library is used so the healthcheck passes anywhere.
"""

import argparse
import json
import os
import struct
import sys
import time
import zlib

PLUGIN_ID = "rgb-vegetation-detection"
VERSION = "1.0.0"
VALID_INDEXES = ("ExG", "VARI", "GLI")


def emit_progress(job_id: str, percent: int, stage: str) -> None:
    """Print one progress line that Module 2 parses (`PROGRESS: {json}`)."""
    line = {"job_id": job_id, "percent": percent, "stage": stage}
    print("PROGRESS: " + json.dumps(line), flush=True)


def write_json(path: str, data: dict) -> None:
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)


def create_minimal_png(filepath: str, width: int = 128, height: int = 128) -> None:
    """Write a valid RGBA PNG (green, semi-transparent) using only the stdlib."""
    pixel = b"\x22\x8b\x22\xc8"
    raw = b"".join(b"\x00" + pixel * width for _ in range(height))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw))
        + chunk(b"IEND", b"")
    )
    os.makedirs(os.path.dirname(os.path.abspath(filepath)), exist_ok=True)
    with open(filepath, "wb") as f:
        f.write(png)


def create_mock_geojson(filepath: str, bbox: list, index_type: str) -> None:
    """Write a sample GeoJSON boundary artifact."""
    min_lon, min_lat, max_lon, max_lat = bbox
    write_json(
        filepath,
        {
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {
                        "type": "Polygon",
                        "coordinates": [
                            [
                                [min_lon, min_lat],
                                [max_lon, min_lat],
                                [max_lon, max_lat],
                                [min_lon, max_lat],
                                [min_lon, min_lat],
                            ]
                        ],
                    },
                    "properties": {"class": "vegetation", "index_type": index_type},
                }
            ],
        },
    )


def handle_healthcheck() -> None:
    """Fast liveness check run by the Module 2 supervisor."""
    print(
        json.dumps(
            {
                "status": "healthy",
                "plugin_id": PLUGIN_ID,
                "version": VERSION,
                "python_version": sys.version.split()[0],
            }
        )
    )
    sys.exit(0)


def resolve_output_dir(payload: dict, output_path: str) -> str:
    """Use payload.output_dir; fall back to the result file's folder if unusable."""
    fallback = os.path.dirname(os.path.abspath(output_path))
    raw = payload.get("output_dir", "")
    if not raw or raw.startswith("/absolute/"):
        return fallback
    try:
        os.makedirs(raw, exist_ok=True)
        return raw
    except OSError:
        return fallback


def simulate_detection(threshold: float) -> dict:
    """Placeholder for the real detection. Returns fixed, plausible metrics."""
    return {
        "vegetation_coverage_pct": round(68.4 + (0.35 - threshold) * 10, 2),
        "total_pixels_analyzed": 12000000,
    }


def failure_result(exec_id: str, started: float, error: str) -> dict:
    return {
        "execution_id": exec_id,
        "status": "failure",
        "execution_time_ms": int((time.perf_counter() - started) * 1000),
        "metrics": {},
        "artifacts": {},
        "error": error,
    }


def run_analysis(input_path: str, output_path: str) -> None:
    started = time.perf_counter()
    exec_id = "unknown"

    try:
        if not os.path.exists(input_path):
            raise FileNotFoundError(f"Input payload file not found: {input_path}")

        with open(input_path, "r", encoding="utf-8") as f:
            payload = json.load(f)

        exec_id = payload.get("execution_id", "unknown")
        params = payload.get("parameters", {})
        index_type = params.get("index_type", "ExG")
        threshold = float(params.get("threshold", 0.35))
        export_mask = bool(params.get("export_mask", True))

        if index_type not in VALID_INDEXES:
            raise ValueError(f"Unsupported index_type: {index_type}")
        if not 0.0 <= threshold <= 1.0:
            raise ValueError("threshold must be between 0.0 and 1.0")

        emit_progress(exec_id, 10, "loading_image")
        output_dir = resolve_output_dir(payload, output_path)
        os.makedirs(output_dir, exist_ok=True)

        emit_progress(exec_id, 45, "evaluating_vegetation_index")
        metrics = simulate_detection(threshold)

        emit_progress(exec_id, 90, "saving_mask")
        bounds = [116.8523, -1.2460, 116.8550, -1.2435]
        artifacts = {}

        if export_mask:
            mask_file = os.path.join(output_dir, f"{exec_id}_mask.png")
            create_minimal_png(mask_file)
            artifacts["mask_image"] = {
                "file_path": mask_file,
                "format": "image/png",
                "bounds": bounds,
            }

        geojson_file = os.path.join(output_dir, f"{exec_id}_boundary.geojson")
        create_mock_geojson(geojson_file, bounds, index_type)
        artifacts["geojson_boundary"] = {
            "file_path": geojson_file,
            "format": "application/geo+json",
        }

        write_json(
            output_path,
            {
                "execution_id": exec_id,
                "status": "success",
                "execution_time_ms": int((time.perf_counter() - started) * 1000),
                "metrics": metrics,
                "artifacts": artifacts,
                "error": None,
            },
        )
        emit_progress(exec_id, 100, "done")
        sys.exit(0)

    except Exception as exc:  # a failing plugin must never crash unhandled
        write_json(output_path, failure_result(exec_id, started, str(exc)))
        sys.exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(description="RGB Vegetation Detection Plugin")
    parser.add_argument("--healthcheck", action="store_true", help="Run healthcheck and exit")
    parser.add_argument("--input", type=str, help="Path to execution_payload.json")
    parser.add_argument("--output", type=str, help="Path to write execution_result.json")
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
