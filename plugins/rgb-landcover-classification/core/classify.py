"""Fitur 1: Land-Cover Classification dan Fitur 2: Confidence Filtering."""

from __future__ import annotations

import os
import time
from typing import Callable

import numpy as np

from .models import (
    SUPPORTED_MIME_TYPES,
    ClassificationRaw,
    FilteredMask,
    LandCoverInput,
    PluginExecutionError,
    UnsupportedFormatError,
)

FEATURE_VERSION = 1
FEATURE_NAMES = (
    "mean_r", "mean_g", "mean_b", "exg", "vari",
    "hue_cos", "hue_sin", "saturation", "value", "gray_std",
)  # fmt: skip


# ------------------------------------------------------------------ image I/O
def load_rgb(path: str, mime_type: str) -> np.ndarray:
    """Baca citra sebagai array uint8 (H, W, 3)."""
    if not os.path.isfile(path):
        raise PluginExecutionError(f"Citra tidak ditemukan: {path}")
    if mime_type not in SUPPORTED_MIME_TYPES:
        raise UnsupportedFormatError(
            f"Format '{mime_type}' tidak didukung. Format yang diterima: {list(SUPPORTED_MIME_TYPES)}"
        )
    pil_error: Exception | None = None
    try:
        from PIL import Image

        Image.MAX_IMAGE_PIXELS = None  # orthomosaic drone bisa sangat besar
        with Image.open(path) as im:
            return np.asarray(im.convert("RGB"), dtype=np.uint8)
    except Exception as exc:  # mis. GeoTIFF 16-bit atau multi-band
        pil_error = exc
    try:
        import rasterio

        with rasterio.open(path) as src:
            if src.count < 3:
                raise PluginExecutionError("Citra harus memiliki minimal 3 band (RGB)")
            data = src.read([1, 2, 3])
        if data.dtype != np.uint8:
            top = float(data.max()) or 1.0
            data = (data.astype(np.float32) / top * 255.0).astype(np.uint8)
        return np.transpose(data, (1, 2, 0))
    except PluginExecutionError:
        raise
    except Exception as exc:
        raise PluginExecutionError(f"Gagal membaca citra ({pil_error}; {exc})") from exc


# ------------------------------------------------------------ superpixel + RF
def _segment_mean(values: np.ndarray, flat_labels: np.ndarray, counts: np.ndarray) -> np.ndarray:
    sums = np.bincount(flat_labels, weights=values.ravel(), minlength=counts.size)
    return sums / np.maximum(counts, 1)


def superpixel_features(rgb: np.ndarray, segment_size: int) -> tuple[np.ndarray, np.ndarray]:
    """Segmentasi SLIC lalu hitung fitur per superpixel.

    `segment_size` = target luas satu superpixel dalam piksel, sehingga jumlah
    segmen = H*W / segment_size. Mengembalikan (labels[H, W] mulai dari 1,
    features[n+1, len(FEATURE_NAMES)]); baris 0 tidak dipakai.
    Urutan kolom fitur: FEATURE_NAMES.
    """
    from skimage.color import rgb2hsv
    from skimage.segmentation import slic

    h, w, _ = rgb.shape
    n_segments = max(1, (h * w) // max(1, segment_size))
    labels = slic(rgb, n_segments=n_segments, compactness=10.0, start_label=1,
                  channel_axis=-1, enforce_connectivity=True)  # fmt: skip
    labels = labels.astype(np.int32)
    n = int(labels.max())
    flat = labels.ravel()
    counts = np.bincount(flat, minlength=n + 1).astype(np.float64)

    f = rgb.astype(np.float32) / 255.0
    r, g, b = f[..., 0], f[..., 1], f[..., 2]
    total = r + g + b + 1e-6
    exg = 2 * (g / total) - (r / total) - (b / total)
    vari = np.clip((g - r) / (g + r - b + 1e-6), -1.0, 1.0)
    hsv = rgb2hsv(f)
    hue_rad = hsv[..., 0] * 2 * np.pi
    gray = 0.299 * r + 0.587 * g + 0.114 * b

    mean_gray = _segment_mean(gray, flat, counts)
    mean_gray_sq = _segment_mean(gray * gray, flat, counts)
    gray_std = np.sqrt(np.maximum(mean_gray_sq - mean_gray**2, 0.0))

    columns = [
        _segment_mean(r, flat, counts), _segment_mean(g, flat, counts), _segment_mean(b, flat, counts),
        _segment_mean(exg, flat, counts), _segment_mean(vari, flat, counts),
        _segment_mean(np.cos(hue_rad), flat, counts), _segment_mean(np.sin(hue_rad), flat, counts),
        _segment_mean(hsv[..., 1], flat, counts), _segment_mean(hsv[..., 2], flat, counts),
        gray_std,
    ]  # fmt: skip
    return labels, np.stack(columns, axis=1).astype(np.float32)


def _load_rf_bundle(weights_path: str):
    """Muat bobot RF. Hanya muat berkas milik sendiri: joblib memakai pickle."""
    if not os.path.isfile(weights_path):
        raise PluginExecutionError(
            f"Bobot model tidak ditemukan: {weights_path}. "
            "Latih dengan tools/train_rf_baseline.py atau letakkan berkas bobot di folder models/."
        )
    import joblib

    bundle = joblib.load(weights_path)
    if isinstance(bundle, dict):
        estimator = bundle.get("estimator")
        if bundle.get("feature_version", FEATURE_VERSION) != FEATURE_VERSION:
            raise PluginExecutionError("Versi fitur pada bobot RF tidak cocok dengan plugin")
    else:
        estimator = bundle
    if estimator is None or not hasattr(estimator, "predict_proba"):
        raise PluginExecutionError("Berkas bobot RF tidak berisi estimator dengan predict_proba")
    return estimator


def _run_rf_superpixel(rgb: np.ndarray, input_data: LandCoverInput, model_config: dict):
    estimator = _load_rf_bundle(model_config["weights_path"])
    labels, feats = superpixel_features(rgb, int(input_data.parameters["segment_size"]))
    proba = estimator.predict_proba(feats[1:])
    best = proba.argmax(axis=1)
    class_ids = np.asarray(estimator.classes_).astype(np.int64)[best]
    valid = {c.id for c in input_data.classes}
    class_ids = np.where(np.isin(class_ids, list(valid)), class_ids, 0)

    seg_class = np.zeros(feats.shape[0], dtype=np.uint8)
    seg_conf = np.zeros(feats.shape[0], dtype=np.float32)
    seg_class[1:] = class_ids.astype(np.uint8)
    seg_conf[1:] = proba.max(axis=1).astype(np.float32)
    return seg_class[labels], seg_conf[labels]


def _run_deeplab(rgb: np.ndarray, input_data: LandCoverInput, model_config: dict):
    raise PluginExecutionError(
        "Model deeplab_resnet101_v1 belum tersedia: bobot hasil fine-tuning dan kode inferensi "
        "belum ditambahkan. Daftarkan fungsinya di MODEL_REGISTRY (core/classify.py) atau "
        "gunakan model_id rf_superpixel_v1."
    )


MODEL_REGISTRY: dict[str, Callable] = {
    "rf_superpixel_v1": _run_rf_superpixel,
    "deeplab_resnet101_v1": _run_deeplab,
}
DEFAULT_WEIGHT_FILES = {
    "rf_superpixel_v1": "rf_superpixel_v1.joblib",
    "deeplab_resnet101_v1": "deeplab_resnet101_v1.pt",
}


def default_model_config(input_data: LandCoverInput, plugin_dir: str) -> dict:
    """Susun model_config dari parameter: model_id, weights_path, device."""
    model_id = input_data.parameters["model_id"]
    return {
        "model_id": model_id,
        "weights_path": os.path.join(plugin_dir, "models", DEFAULT_WEIGHT_FILES[model_id]),
        "device": input_data.parameters["device"],
    }


# ------------------------------------------------------------------ Fitur 1
def classify_landcover(input_data: LandCoverInput, model_config: dict) -> ClassificationRaw:
    """Klasifikasikan tiap piksel/superpixel ke kelas land-cover.

    Kesalahan: PluginExecutionError (citra tidak ditemukan, bobot hilang) dan
    UnsupportedFormatError (format di luar supported_mime_types).
    """
    start = time.perf_counter()
    rgb = load_rgb(input_data.image_path, input_data.mime_type)
    runner = MODEL_REGISTRY.get(model_config["model_id"])
    if runner is None:
        raise PluginExecutionError(f"model_id tidak dikenal: {model_config['model_id']}")
    class_mask, confidence_map = runner(rgb, input_data, model_config)
    return ClassificationRaw(
        class_mask=class_mask.astype(np.uint8),
        confidence_map=confidence_map.astype(np.float32),
        model_name=model_config["model_id"],
        processing_time_s=time.perf_counter() - start,
    )


# ------------------------------------------------------------------ Fitur 2
def apply_confidence_threshold(
    class_mask: np.ndarray, confidence_map: np.ndarray, threshold: float
) -> FilteredMask:
    """Piksel dengan confidence < threshold diset menjadi 0 (tidak terklasifikasi)."""
    if not 0.0 <= threshold <= 1.0:
        raise PluginExecutionError(f"threshold harus 0.0-1.0, diterima: {threshold}")
    low = confidence_map < threshold
    changed = int(np.count_nonzero(low & (class_mask != 0)))
    out = class_mask.copy()
    out[low] = 0
    return FilteredMask(class_mask=out, unclassified_added=changed)


__all__ = [
    "FEATURE_NAMES", "FEATURE_VERSION", "MODEL_REGISTRY", "apply_confidence_threshold",
    "classify_landcover", "default_model_config", "load_rgb", "superpixel_features",
]  # fmt: skip
