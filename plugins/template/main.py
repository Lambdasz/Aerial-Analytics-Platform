#!/usr/bin/env python3
"""
Starter Template Entrypoint for Aerial Analytics Plugins.
Implements standard command-line flags (--healthcheck, --input, --output)
and processes execution payload to generate execution results and artifacts.
"""

import argparse
import json
import os
import sys
import time


def handle_healthcheck() -> None:
    """
    Fast environment healthcheck executed by the platform supervisor.
    Verify that necessary third-party packages can be imported.
    """
    health_info = {
        "status": "healthy",
        "plugin_id": "template-analysis-plugin",
        "version": "1.0.0",
        "python_version": sys.version.split()[0],
    }
    print(json.dumps(health_info))
    sys.exit(0)


def run_analysis(input_path: str, output_path: str) -> None:
    """
    Main analytical execution pipeline.
    Reads input payload, processes imagery, generates artifacts, and writes results.
    """
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

        target = payload.get("target", {})
        image_path = target.get("image_path", "")
        parameters = payload.get("parameters", {})

        # -------------------------------------------------------------
        # [TODO: Implement analytical computer vision / ML logic here]
        # Example:
        # - Read image from image_path
        # - Clip by payload.get("aoi") if present
        # - Apply algorithm using parameters (e.g. parameters.get("example_number"))
        # - Export raster masks and vector features into output_dir
        # -------------------------------------------------------------

        duration_ms = int((time.perf_counter() - start_time) * 1000)

        # Build execution result matching outputs.json contract
        result = {
            "execution_id": exec_id,
            "status": "success",
            "execution_time_ms": duration_ms,
            "metrics": {
                "primary_metric": 68.45,
                "count_metric": 128,
            },
            "artifacts": {
                # Add references to files generated in output_dir
            },
            "error": None,
        }

        with open(output_path, "w", encoding="utf-8") as f:
            json.dump(result, f, indent=2)

        print(f"Analysis completed in {duration_ms}ms")
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
    parser = argparse.ArgumentParser(description="Aerial Analytics Plugin Starter")
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
