from __future__ import annotations
import json
from .models import LandCoverInput, ImageMetadata, PluginExecutionError, parse_class_definitions

def parse_classes(classes_json: str):
    try:
        return parse_class_definitions(json.loads(classes_json) if classes_json and classes_json.strip() else None)
    except (json.JSONDecodeError, TypeError, KeyError) as exc:
        raise PluginExecutionError(f"invalid classes_json: {exc}") from exc

def build_input_from_payload(payload: dict, output_dir: str) -> LandCoverInput:
    try:
        target = payload["target"]
        metadata_raw = target.get("metadata")
        metadata = ImageMetadata(**metadata_raw) if metadata_raw and "resolution" in metadata_raw else None
        params = dict(payload.get("parameters") or {})
        classes = parse_classes(params.get("classes_json", ""))
        return LandCoverInput(str(payload["execution_id"]), output_dir, target["image_path"], target["mime_type"], metadata, (payload.get("aoi") or {}).get("geometry"), classes, params, payload.get("session_id"))
    except (KeyError, TypeError, ValueError) as exc:
        raise PluginExecutionError(f"invalid execution payload: {exc}") from exc
