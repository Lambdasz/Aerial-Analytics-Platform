#!/usr/bin/env python3
"""
Starter Template Entrypoint for Aerial Analytics Plugins.
Implements standard command-line flags (--healthcheck, --input, --output)
and processes execution payloads to generate artifacts and structured results.
"""

import argparse
import importlib
import json
import os
import sys
import time
from typing import Any, Dict, List, Tuple


def load_manifest_metadata() -> Tuple[str, str]:
    """
    Dynamically read plugin ID and version from the adjacent manifest.json.
    Ensures healthcheck output always stays in sync with your manifest edits.
    """
    plugin_dir = os.path.dirname(os.path.abspath(__file__))
    manifest_path = os.path.join(plugin_dir, "manifest.json")

    plugin_id = "template-analysis-plugin"
    version = "1.0.0"

    if os.path.exists(manifest_path):
        try:
            with open(manifest_path, "r", encoding="utf-8") as f:
                manifest_data = json.load(f)
                meta = manifest_data.get("metadata", {})
                plugin_id = meta.get("id", plugin_id)
                version = meta.get("version", version)
        except Exception:
            pass  # Fall back to defaults if manifest cannot be read

    return plugin_id, version


def check_dependencies() -> Tuple[bool, Dict[str, str]]:
    """
    Verify that all external libraries required by your plugin can be imported.

    CUSTOMIZATION INSTRUCTION:
    Add your third-party package import names to `required_modules` below
    (e.g., ["numpy", "cv2", "rasterio", "torch"]).
    """
    # Template: add your required third-party imports here
    required_modules: List[str] = [
        # "numpy",
        # "cv2",
    ]

    statuses: Dict[str, str] = {}
    all_ok = True

    for module_name in required_modules:
        try:
            importlib.import_module(module_name)
            statuses[module_name] = "available"
        except ImportError as err:
            statuses[module_name] = f"missing: {err}"
            all_ok = False

    return all_ok, statuses


def handle_healthcheck() -> None:
    """
    Fast environment healthcheck executed by the platform supervisor before
    scheduling analytical tasks.

    MUST exit with:
      - status code 0: plugin environment is ready to execute analysis
      - status code 1: environment is missing packages, weights, or device drivers
    """
    plugin_id, version = load_manifest_metadata()
    deps_ok, deps_status = check_dependencies()

    if not deps_ok:
        health_info = {
            "status": "unhealthy",
            "plugin_id": plugin_id,
            "version": version,
            "python_version": sys.version.split()[0],
            "dependencies": deps_status,
            "error": "One or more required dependencies failed to import. Run pip install -r requirements.txt",
        }
        print(json.dumps(health_info, indent=2), file=sys.stderr)
        sys.exit(1)

    health_info = {
        "status": "healthy",
        "plugin_id": plugin_id,
        "version": version,
        "python_version": sys.version.split()[0],
        "dependencies": deps_status if deps_status else "standard_library_only",
    }
    print(json.dumps(health_info))
    sys.exit(0)


def run_analysis(input_path: str, output_path: str) -> None:
    """
    Main analytical execution pipeline.
    Reads input payload, processes imagery, generates artifacts, and writes results.
    """
    start_time = time.perf_counter()

    # 1. Verify input file existence
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
        # 2. Parse execution payload
        with open(input_path, "r", encoding="utf-8") as f:
            payload: Dict[str, Any] = json.load(f)

        exec_id = payload.get("execution_id", "exec_unknown")
        raw_output_dir = payload.get("output_dir", "")

        # 3. Resolve sandboxed output directory
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

        # 4. Extract target image, metadata, and user parameters
        target = payload.get("target", {})
        image_path = target.get("image_path", "")
        aoi_geometry = payload.get("aoi", {}).get("geometry")
        parameters = payload.get("parameters", {})

        # Example parameter extraction:
        example_select = parameters.get("example_select", "OPTION_A")
        example_number = float(parameters.get("example_number", 0.5))
        example_boolean = bool(parameters.get("example_boolean", True))

        # ---------------------------------------------------------------------
        # [CUSTOMIZE: Implement your analytical CV / ML logic here]
        #
        # Steps to implement:
        # a. Read image from `image_path` using OpenCV / Rasterio / PIL
        # b. If `aoi_geometry` is provided, mask/clip to polygon bounds
        # c. Apply analytical algorithm with `parameters`
        # d. Save output raster mask(s) (PNG/GeoTIFF) into `output_dir`
        # e. Save vector detection feature(s) (GeoJSON) into `output_dir`
        # f. Calculate scalar metrics (coverage percentage, counts, area)
        # ---------------------------------------------------------------------

        duration_ms = int((time.perf_counter() - start_time) * 1000)

        # 5. Build structured result conforming to schemas/execution_result.schema.json
        # NOTE: Keys under "metrics" must match the keys declared in outputs.json!
        # NOTE: Keys under "artifacts" must match the keys declared in outputs.json!
        result: Dict[str, Any] = {
            "execution_id": exec_id,
            "status": "success",
            "execution_time_ms": duration_ms,
            "metrics": {
                "primary_metric": 68.45,
                "count_metric": 128,
            },
            "artifacts": {
                # Example:
                # "mask_raster": {
                #     "file_path": os.path.join(output_dir, f"{exec_id}_mask.png"),
                #     "format": "image/png",
                #     "bounds": [116.8523, -1.2460, 116.8550, -1.2435]
                # }
            },
            "error": None,
        }

        # 6. Write final execution result to destination path
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
            "error": f"{type(exc).__name__}: {str(exc)}",
        }
        with open(output_path, "w", encoding="utf-8") as f:
            json.dump(error_result, f, indent=2)
        print(f"Analysis failed: {exc}", file=sys.stderr)
        sys.exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Aerial Analytics Analytical Plugin Starter Entrypoint"
    )
    parser.add_argument(
        "--healthcheck",
        action="store_true",
        help="Run fast environment healthcheck (imports, device check) and exit",
    )
    parser.add_argument(
        "--input",
        type=str,
        help="Filesystem path to execution_payload.json",
    )
    parser.add_argument(
        "--output",
        type=str,
        help="Filesystem path to write execution_result.json",
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
