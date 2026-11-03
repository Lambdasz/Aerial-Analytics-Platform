#!/usr/bin/env python3
"""Latih model baseline rf_superpixel_v1 dari citra + mask label.

Mask label adalah PNG satu kanal (uint8) berukuran sama dengan citra:
nilai piksel = id kelas (1..255), 0 = tidak berlabel. Mask ini dapat dibuat
dari polygon training yang digambar pengguna (Modul 3).

Contoh:
    python tools/train_rf_baseline.py --pair foto1.jpg label1.png --pair foto2.jpg label2.png \
        --out models/rf_superpixel_v1.joblib --segment-size 500

Catatan keamanan: bobot disimpan dengan joblib (pickle). Hanya muat berkas bobot
yang Anda buat sendiri atau berasal dari sumber tepercaya.
"""

from __future__ import annotations

import argparse
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from core.classify import FEATURE_VERSION, load_rgb, superpixel_features  # noqa: E402


def segment_labels(labels: np.ndarray, label_mask: np.ndarray, min_labeled_fraction: float = 0.5):
    """Label mayoritas tiap superpixel; None bila kurang dari min_labeled_fraction piksel berlabel."""
    n = int(labels.max())
    key = labels.ravel().astype(np.int64) * 256 + label_mask.ravel().astype(np.int64)
    counts = np.bincount(key, minlength=(n + 1) * 256).reshape(n + 1, 256)
    labeled = counts[:, 1:].sum(axis=1)
    total = counts.sum(axis=1)
    majority = counts[:, 1:].argmax(axis=1) + 1
    keep = (total > 0) & (labeled / np.maximum(total, 1) >= min_labeled_fraction)
    keep[0] = False
    return keep, majority


def _mime_from_extension(path: str) -> str:
    ext = os.path.splitext(path)[1].lower()
    return {".png": "image/png", ".tif": "image/tiff", ".tiff": "image/tiff"}.get(ext, "image/jpeg")


def build_training_set(pairs, segment_size: int):
    from PIL import Image

    xs, ys = [], []
    for image_path, label_path in pairs:
        rgb = load_rgb(image_path, _mime_from_extension(image_path))
        label_mask = np.asarray(Image.open(label_path).convert("L"), dtype=np.uint8)
        if label_mask.shape != rgb.shape[:2]:
            raise SystemExit(f"Ukuran mask label tidak sama dengan citra: {label_path}")
        labels, feats = superpixel_features(rgb, segment_size)
        keep, majority = segment_labels(labels, label_mask)
        xs.append(feats[keep])
        ys.append(majority[keep])
    return np.concatenate(xs), np.concatenate(ys)


def train(pairs, out_path: str, segment_size: int = 500, n_estimators: int = 200) -> None:
    import joblib
    from sklearn.ensemble import RandomForestClassifier

    x, y = build_training_set(pairs, segment_size)
    if len(np.unique(y)) < 2:
        raise SystemExit("Butuh minimal dua kelas berlabel untuk melatih model")
    clf = RandomForestClassifier(n_estimators=n_estimators, class_weight="balanced", random_state=0, n_jobs=-1)
    clf.fit(x, y)
    os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
    joblib.dump({"estimator": clf, "version": "rf_superpixel_v1", "feature_version": FEATURE_VERSION}, out_path)
    print(f"Tersimpan: {out_path} ({len(y)} superpixel, kelas: {sorted(int(c) for c in np.unique(y))})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--pair", nargs=2, action="append", metavar=("CITRA", "LABEL"), required=True)
    parser.add_argument("--out", default="models/rf_superpixel_v1.joblib")
    parser.add_argument("--segment-size", type=int, default=500)
    parser.add_argument("--n-estimators", type=int, default=200)
    args = parser.parse_args()
    train(args.pair, args.out, args.segment_size, args.n_estimators)


if __name__ == "__main__":
    main()
