"""Adapter payload Modul 2 -> LandCoverInput (Bagian 2 dan 4 spesifikasi)."""

from __future__ import annotations

import json
from typing import Any

from pydantic import ValidationError

from .models import (
    DEFAULT_CLASSES,
    ClassDefinition,
    ImageMetadata,
    LandCoverInput,
    PluginExecutionError,
    PluginParameters,
)


def parse_classes(classes_json: str) -> list[ClassDefinition]:
    """Kosong -> DEFAULT_CLASSES. Selain itu harus list JSON [{id, name, color}]."""
    if not classes_json or not classes_json.strip():
        return list(DEFAULT_CLASSES)
    try:
        raw = json.loads(classes_json)
        classes = [ClassDefinition(**item) for item in raw]
    except (json.JSONDecodeError, TypeError, ValidationError) as exc:
        raise PluginExecutionError(f"parameter classes_json tidak valid: {exc}") from exc
    if not classes:
        raise PluginExecutionError("classes_json tidak boleh berupa list kosong")
    if len({c.id for c in classes}) != len(classes):
        raise PluginExecutionError("id kelas pada classes_json harus unik")
    if len({c.name for c in classes}) != len(classes):
        raise PluginExecutionError("nama kelas pada classes_json harus unik")
    return classes


def _image_size(path: str) -> list[float] | None:
    try:
        from PIL import Image

        Image.MAX_IMAGE_PIXELS = None
        with Image.open(path) as im:
            return [float(im.width), float(im.height)]
    except Exception:
        return None


def build_input_from_payload(payload: dict[str, Any], output_dir: str) -> LandCoverInput:
    """Bentuk LandCoverInput dari isi payload.json (skema execution_payload).

    Field yang dibaca: execution_id, session_id, target.image_path,
    target.mime_type, target.metadata, aoi.geometry, parameters.
    `output_dir` dilewatkan terpisah karena sudah di-resolve oleh main.py.
    """
    try:
        target = payload["target"]
        image_path = target["image_path"]
        mime_type = target["mime_type"]
        execution_id = payload["execution_id"]
    except KeyError as exc:
        raise PluginExecutionError(f"payload tidak memiliki field wajib: {exc}") from exc

    try:
        params = PluginParameters(**(payload.get("parameters") or {}))
    except ValidationError as exc:
        raise PluginExecutionError(f"parameter tidak valid: {exc}") from exc

    meta_raw = dict(target.get("metadata") or {})
    if "resolution" not in meta_raw:  # inputs.json mewajibkan resolution; cadangan: baca dari file
        size = _image_size(image_path)
        if size is not None:
            meta_raw["resolution"] = size
    try:
        metadata = ImageMetadata(**meta_raw) if "resolution" in meta_raw else None
    except ValidationError as exc:
        raise PluginExecutionError(f"target.metadata tidak valid: {exc}") from exc

    aoi = payload.get("aoi") or {}
    return LandCoverInput(
        execution_id=execution_id,
        session_id=payload.get("session_id"),
        output_dir=output_dir,
        image_path=image_path,
        mime_type=mime_type,
        metadata=metadata,
        aoi_geometry=aoi.get("geometry"),
        classes=parse_classes(params.classes_json),
        parameters=params.model_dump(),
    )
