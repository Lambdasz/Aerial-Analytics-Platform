from __future__ import annotations
import os
import time
import numpy as np
from .models import *


def load_checkpoint_state_dict(checkpoint):
    """Extract a model state dictionary from a raw or metadata-wrapped checkpoint."""
    if isinstance(checkpoint, dict) and isinstance(checkpoint.get("state_dict"), dict):
        return checkpoint["state_dict"]
    if isinstance(checkpoint, dict):
        return checkpoint
    raise PluginExecutionError("trained checkpoint must contain a state dictionary")

def load_rgb(path: str, mime_type: str) -> np.ndarray:
    if not os.path.isfile(path):
        raise PluginExecutionError(f"image not found: {path}")
    if mime_type not in SUPPORTED_MIME_TYPES:
        raise UnsupportedFormatError(f"unsupported image MIME type: {mime_type}")
    from PIL import Image
    try:
        Image.MAX_IMAGE_PIXELS = None
        with Image.open(path) as image:
            return np.asarray(image.convert("RGB"), dtype=np.uint8)
    except Exception as exc:
        try:
            import rasterio
            with rasterio.open(path) as source:
                if source.count < 3:
                    raise PluginExecutionError("image must contain at least RGB bands")
                data = source.read([1, 2, 3])
            data = data.astype(np.float32)
            if data.max() > 255:
                data = data / (data.max() or 1) * 255
            return np.transpose(data.astype(np.uint8), (1, 2, 0))
        except Exception as raster_error:
            raise PluginExecutionError(f"failed to read image: {exc}; {raster_error}") from raster_error

def default_model_config(input_data: LandCoverInput, plugin_dir: str) -> dict:
    backbone = input_data.parameters.get("backbone", "resnet101")
    return {"model_id": f"deeplabv3plus_{backbone}", "backbone": backbone,
            "weights_path": os.path.join(plugin_dir, "models", f"deeplabv3plus_{backbone}.pt"),
            "device": input_data.parameters.get("device", "auto")}

def classify_landcover(input_data: LandCoverInput, model_config: dict) -> ClassificationRaw:
    from model import build_model
    import torch
    started = time.perf_counter()
    rgb = load_rgb(input_data.image_path, input_data.mime_type)
    weights = model_config["weights_path"]
    if not os.path.isfile(weights):
        raise PluginExecutionError(f"trained weights not found: {weights}")
    tensor = torch.from_numpy(rgb).permute(2, 0, 1).float().div(255).unsqueeze(0)
    model = build_model(model_config["backbone"], len(input_data.classes))
    checkpoint = torch.load(weights, map_location="cpu", weights_only=True)
    model.load_state_dict(load_checkpoint_state_dict(checkpoint))
    model.eval()
    with torch.no_grad():
        probabilities = torch.softmax(model(tensor), dim=1)[0]
    confidence, labels = probabilities.max(dim=0)
    labels = labels.add(1)
    return ClassificationRaw(labels.numpy().astype(np.uint8), confidence.numpy().astype(np.float32), model_config["model_id"], time.perf_counter() - started)

def apply_confidence_threshold(class_mask: np.ndarray, confidence_map: np.ndarray, threshold: float) -> FilteredMask:
    if not 0 <= threshold <= 1:
        raise PluginExecutionError("threshold must be between 0 and 1")
    low = confidence_map < threshold
    output = class_mask.copy()
    changed = int(np.count_nonzero(low & (output != 0)))
    output[low] = 0
    return FilteredMask(output, changed)
