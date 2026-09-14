#!/usr/bin/env python3
"""
RGB Vegetation Detection (Mock Plugin Entrypoint)
Demonstrates standard plugin lifecycle: CLI flags, input payload consumption,
simulated analysis, artifact generation, and structured result emission.
"""

import argparse
import json
import os
import struct
import sys
import time
import zlib


def create_minimal_png(filepath: str, width: int = 128, height: int = 128) -> None:
    """Generate a valid standalone PNG raster mask using only the Python standard library."""
    # RGBA: Forest green (#228B22) with 200 alpha for mock mask
    pixel = b"\x22\x8b\x22\xc8"
    raw_scanlines = b"".join(b"\x00" + (pixel * width) for _ in range(height))
    compressed = zlib.compress(raw_scanlines)

    def make_chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    png_bytes = (
        b"\x89PNG\r\n\x1a\n"
        + make_chunk(b"IHDR", ihdr)
        + make_chunk(b"IDAT", compressed)
        + make_chunk(b"IEND", b"")
    )

    os.makedirs(os.path.dirname(os.path.abspath(filepath)), exist_ok=True)
    with open(filepath, "wb") as f:
        f.write(png_bytes)


def create_mock_geojson(filepath: str, bbox: list) -> None:
    """Generate a sample GeoJSON boundary artifact."""
    min_lon, min_lat, max_lon, max_lat = bbox
    geojson_payload = {
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
                "properties": {
                    "class": "vegetation",
                    "index_type": "ExG",
                    "confidence": 0.92,
                },
            }
        ],
    }
    os.makedirs(os.path.dirname(os.path.abspath(filepath)), exist_ok=True)
    with open(filepath, "w", encoding="utf-8") as f:
        json.dump(geojson_payload, f, indent=2)


def handle_healthcheck() -> None:
    """Fast liveness check executed by platform supervisor."""
    health_info = {
        "status": "healthy",
        "plugin_id": "rgb-vegetation-exg",
        "version": "1.0.0",
        "python_version": sys.version.split()[0],
    }
    print(json.dumps(health_info))
    sys.exit(0)


def run_analysis(input_path: str, output_path: str) -> None:
    """Execute analytical workflow."""
    start_time = time.perf_counter()

    if not os.path.exists(input_path):
        error_result = {
            "execution_id": "unknown",
            "status": "failure",
            "execution_time_ms": 0,
            "metrics": {},
            "artifacts": {},
            "error": f"Input payload file not found: {input_path}",
        }
        with open(output_path, "w", encoding="utf-8") as f:
            json.dump(error_result, f, indent=2)
        sys.exit(1)

    try:
        with open(input_path, "r", encoding="utf-8") as f:
            payload = json.load(f)

        exec_id = payload.get("execution_id", "exec_unknown")
        raw_output_dir = payload.get("output_dir", "")

        # Determine writable output directory, falling back to output_path directory if dummy/unwritable
        fallback_dir = os.path.dirname(os.path.abspath(output_path))
        if not raw_output_dir or raw_output_dir.startswith("/absolute/"):
            output_dir = fallback_dir
        else:
            try:
                os.makedirs(raw_output_dir, exist_ok=True)
                output_dir = raw_output_dir
            except OSError:
                output_dir = fallback_dir

        os.makedirs(output_dir, exist_ok=True)

        params = payload.get("parameters", {})
        threshold = float(params.get("threshold", 0.35))
        export_mask = bool(params.get("export_mask", True))

        # Simulated artifact generation
        mask_file = os.path.join(output_dir, f"{exec_id}_mask.png")
        geojson_file = os.path.join(output_dir, f"{exec_id}_boundary.geojson")
        bounds = [116.8523, -1.2460, 116.8550, -1.2435]

        artifacts = {}

        if export_mask:
            create_minimal_png(mask_file)
            artifacts["mask_image"] = {
                "file_path": mask_file,
                "format": "image/png",
                "bounds": bounds,
            }

        create_mock_geojson(geojson_file, bounds)
        artifacts["geojson_boundary"] = {
            "file_path": geojson_file,
            "format": "application/geo+json",
        }

        # Simulate computation metrics
        duration_ms = int((time.perf_counter() - start_time) * 1000)
        simulated_coverage = round(68.4 + (0.35 - threshold) * 10, 2)

        result = {
            "execution_id": exec_id,
            "status": "success",
            "execution_time_ms": duration_ms,
            "metrics": {
                "vegetation_coverage_pct": simulated_coverage,
                "total_pixels_analyzed": 12000000,
            },
            "artifacts": artifacts,
            "error": None,
        }

        with open(output_path, "w", encoding="utf-8") as f:
            json.dump(result, f, indent=2)

        print(f"Analysis completed successfully in {duration_ms}ms")
        sys.exit(0)

    except Exception as exc:
        duration_ms = int((time.perf_counter() - start_time) * 1000)
        error_result = {
            "execution_id": payload.get("execution_id", "unknown") if "payload" in locals() else "unknown",
            "status": "failure",
            "execution_time_ms": duration_ms,
            "metrics": {},
            "artifacts": {},
            "error": str(exc),
        }
        with open(output_path, "w", encoding="utf-8") as f:
            json.dump(error_result, f, indent=2)
        sys.exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(description="RGB Vegetation Detection Plugin")
    parser.add_argument(
        "--healthcheck",
        action="store_true",
        help="Run fast environment healthcheck and exit",
    )
    parser.add_argument(
        "--input",
        type=str,
        help="Path to execution_payload.json input file",
    )
    parser.add_argument(
        "--output",
        type=str,
        help="Path to execution_result.json destination file",
    )

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
