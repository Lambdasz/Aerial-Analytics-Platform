#!/usr/bin/env python3
"""
Entrypoint for the Tree Detection & Counting plugin (Module 7).

Implements the platform CLI contract from plugins/template:
  python main.py --healthcheck
  python main.py --input <execution_payload.json> --output <execution_result.json>

This file is the impure shell (files, clock, exit codes). The analysis itself
lives in the pure functions of the ``treedet`` package.
"""

import argparse
import importlib
import json
import os
import sys
import time
from datetime import datetime, timezone
from typing import Any, Dict, List, Tuple

PLUGIN_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, PLUGIN_DIR)  # make ``treedet`` importable from any working directory


def load_manifest_metadata() -> Tuple[str, str]:
    """Read plugin ID and version from the adjacent manifest.json."""
    plugin_id, version = "tree-detection-counting", "0.0.0"
    try:
        with open(os.path.join(PLUGIN_DIR, "manifest.json"), "r", encoding="utf-8") as f:
            meta = json.load(f).get("metadata", {})
            plugin_id = meta.get("id", plugin_id)
            version = meta.get("version", version)
    except (OSError, ValueError):
        pass
    return plugin_id, version


def check_dependencies() -> Tuple[bool, Dict[str, str]]:
    """Verify that required third-party modules can be imported."""
    # Add the detection model's packages here once chosen (e.g. "numpy", "cv2", "torch").
    required_modules: List[str] = ["pydantic"]

    statuses: Dict[str, str] = {}
    for module_name in required_modules:
        try:
            importlib.import_module(module_name)
            statuses[module_name] = "available"
        except ImportError as err:
            statuses[module_name] = f"missing: {err}"
    all_ok = all(status == "available" for status in statuses.values())
    return all_ok, statuses


def handle_healthcheck() -> None:
    plugin_id, version = load_manifest_metadata()
    deps_ok, deps_status = check_dependencies()
    health_info: Dict[str, Any] = {
        "status": "healthy" if deps_ok else "unhealthy",
        "plugin_id": plugin_id,
        "version": version,
        "python_version": sys.version.split()[0],
        "dependencies": deps_status,
    }
    if not deps_ok:
        health_info["error"] = (
            "One or more required dependencies failed to import. "
            "Run pip install -r requirements.txt"
        )
        print(json.dumps(health_info, indent=2), file=sys.stderr)
        sys.exit(1)
    print(json.dumps(health_info))
    sys.exit(0)


def resolve_output_dir(raw_output_dir: str, output_path: str) -> str:
    """Use the payload's output_dir, or the result file's folder for placeholder paths."""
    fallback_dir = os.path.dirname(os.path.abspath(output_path))
    if not raw_output_dir or raw_output_dir.startswith("/absolute/"):
        return fallback_dir
    try:
        os.makedirs(raw_output_dir, exist_ok=True)
        return raw_output_dir
    except OSError:
        return fallback_dir


def write_json(path: str, data: Any) -> None:
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)


def failure_result(execution_id: str, duration_ms: int, error: str) -> Dict[str, Any]:
    return {
        "execution_id": execution_id,
        "status": "failure",
        "execution_time_ms": duration_ms,
        "metrics": {},
        "artifacts": {},
        "error": error,
    }


def run_analysis(input_path: str, output_path: str) -> None:
    start_time = time.perf_counter()
    execution_id = "unknown"

    try:
        # Imported here so a missing dependency becomes a "failure" result, not a crash.
        from treedet.detection import detect_trees
        from treedet.output import to_execution_result
        from treedet.pipeline import analyze, model_config_of, parse_payload

        with open(input_path, "r", encoding="utf-8") as f:
            payload: Dict[str, Any] = json.load(f)
        execution_id = str(payload.get("execution_id", "unknown"))

        request = parse_payload(payload)
        detection = detect_trees(request.detection_input, model_config_of(request.parameters))
        _, version = load_manifest_metadata()
        output = analyze(request, detection, datetime.now(timezone.utc), version)

        output_dir = os.path.abspath(
            resolve_output_dir(str(payload.get("output_dir", "")), output_path)
        )
        artifact_paths = {
            "tree_points": os.path.join(output_dir, f"{execution_id}_tree_points.geojson"),
            "plugin_output": os.path.join(output_dir, f"{execution_id}_plugin_output.json"),
        }
        with open(artifact_paths["tree_points"], "w", encoding="utf-8") as f:
            f.write(output.geojson)
        write_json(artifact_paths["plugin_output"], output.model_dump(mode="json"))

        duration_ms = int((time.perf_counter() - start_time) * 1000)
        write_json(output_path, to_execution_result(output, execution_id, duration_ms, artifact_paths))
        print(f"Analysis finished ({output.status}) in {duration_ms}ms")
        sys.exit(0)

    except Exception as exc:  # a failing plugin must never crash unhandled
        duration_ms = int((time.perf_counter() - start_time) * 1000)
        write_json(
            output_path,
            failure_result(execution_id, duration_ms, f"{type(exc).__name__}: {exc}"),
        )
        print(f"Analysis failed: {exc}", file=sys.stderr)
        sys.exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(description="Tree Detection & Counting plugin (Module 7)")
    parser.add_argument("--healthcheck", action="store_true", help="Check environment and exit")
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
