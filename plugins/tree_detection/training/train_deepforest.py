"""Beri label dan fine-tuning detector 'deepforest' pada foto drone (mis. UAV-VisLoc).

UAV-VisLoc tidak punya label pohon, jadi pelatihan terdiri dari dua langkah dengan
koreksi manusia di antaranya:

1. ``prelabel`` memotong tiap foto menjadi tile 400 px pada resolusi latih model, lalu
   menulis kotak hasil model rilis di samping tiap tile sebagai Pascal VOC XML. Buka
   foldernya di LabelImg (atau impor ke CVAT / Label Studio sebagai Pascal VOC), lalu
   perbaiki kotaknya: hapus yang salah, tambahkan tajuk yang terlewat, sesuaikan sisanya.
2. ``train`` membaca XML yang sudah dikoreksi, memasukkan semua tile dari satu foto ke
   split yang sama (supaya validasi tidak melihat tetangga tile latih), melakukan
   fine-tuning dari bobot rilis, lalu mencetak precision/recall validasi sebelum dan
   sesudahnya.

Checkpoint yang dihasilkan diisikan ke parameter ``model_path`` plugin dengan
``detector = deepforest``.

Cara pakai::

    python training/train_deepforest.py prelabel --images <foto> --labels <tile> --gsd-cm 5
    python training/train_deepforest.py train --labels <tile> --out <deepforest-visloc.pt>

Butuh ``pip install -r training/requirements.txt``.
"""

import argparse
import os
import sys
import warnings
import xml.etree.ElementTree as ET
import zlib
from collections.abc import Iterable, Iterator, Sequence
from typing import Any, NamedTuple

PLUGIN_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, PLUGIN_DIR)

from treedet.detectors import (  # noqa: E402 (needs the path above)
    DEEPFOREST_PATCH_PX,
    DEEPFOREST_PRETRAINED,
    deepforest_scale,
)

LABEL = "Tree"
PHOTO_SUFFIXES = (".jpg", ".jpeg", ".png", ".tif", ".tiff")
TILE_SEPARATOR = "__"  # <photo>__x<col>_y<row>.jpg, so a tile can be traced to its photo


class Box(NamedTuple):
    x_min: int
    y_min: int
    x_max: int
    y_max: int


class Tile(NamedTuple):
    x: int
    y: int


# ---------------------------------------------------------------------------
# Pure helpers (no torch, no files)
# ---------------------------------------------------------------------------


def grid(width: int, height: int, size: int) -> tuple[Tile, ...]:
    """Top-left corners of the full ``size`` tiles that fit in the image."""
    return tuple(
        Tile(x, y)
        for y in range(0, height - size + 1, size)
        for x in range(0, width - size + 1, size)
    )


def spread(items: Sequence[Any], count: int) -> tuple[Any, ...]:
    """``count`` items spread over ``items`` (all of them when count <= 0)."""
    if count <= 0 or count >= len(items):
        return tuple(items)
    step = len(items) / count
    return tuple(items[int(index * step)] for index in range(count))


def photo_of(tile_name: str) -> str:
    """Photo a tile was cut from, from the tile file name."""
    return os.path.splitext(tile_name)[0].split(TILE_SEPARATOR)[0]


def is_validation(photo: str, fraction: float) -> bool:
    """Stable split by photo: the same photo always lands on the same side."""
    return (zlib.crc32(photo.encode()) % 1000) < fraction * 1000


def voc_xml(file_name: str, size: int, boxes: Iterable[Box]) -> str:
    """Pascal VOC annotation, the format LabelImg, CVAT and Label Studio all read."""
    root = ET.Element("annotation")
    ET.SubElement(root, "filename").text = file_name
    dimensions = ET.SubElement(root, "size")
    for tag, value in (("width", size), ("height", size), ("depth", 3)):
        ET.SubElement(dimensions, tag).text = str(value)
    for box in boxes:
        obj = ET.SubElement(root, "object")
        ET.SubElement(obj, "name").text = LABEL
        ET.SubElement(obj, "difficult").text = "0"
        corners = ET.SubElement(obj, "bndbox")
        for tag, value in zip(("xmin", "ymin", "xmax", "ymax"), box):
            ET.SubElement(corners, tag).text = str(value)
    ET.indent(root)
    return ET.tostring(root, encoding="unicode")


def parse_voc(xml_text: str) -> tuple[str, tuple[Box, ...]]:
    """File name and boxes of one Pascal VOC annotation (every class counts as a tree)."""
    root = ET.fromstring(xml_text)
    boxes = tuple(
        Box(
            *(
                int(float(obj.findtext(f"bndbox/{tag}", "0")))
                for tag in ("xmin", "ymin", "xmax", "ymax")
            )
        )
        for obj in root.iter("object")
    )
    return root.findtext("filename", ""), tuple(
        b for b in boxes if b.x_max > b.x_min and b.y_max > b.y_min
    )


def csv_rows(file_name: str, boxes: Sequence[Box]) -> Iterator[str]:
    """DeepForest CSV rows; a tile without trees is one all-zero row (an empty frame)."""
    for box in boxes or (Box(0, 0, 0, 0),):
        yield f"{file_name},{box.x_min},{box.y_min},{box.x_max},{box.y_max},{LABEL}"


# ---------------------------------------------------------------------------
# Commands (files and the model)
# ---------------------------------------------------------------------------


def _photos(folder: str) -> list[str]:
    return sorted(
        os.path.join(directory, name)
        for directory, _, names in os.walk(folder)
        for name in names
        if name.lower().endswith(PHOTO_SUFFIXES)
    )


def _released_model() -> Any:
    from deepforest import main as deepforest_main

    model = deepforest_main.deepforest()
    model.load_model(DEEPFOREST_PRETRAINED)
    return model


def prelabel(
    images: str, labels: str, gsd_cm: float, tiles_per_photo: int, min_score: float
) -> None:
    import cv2

    os.makedirs(labels, exist_ok=True)
    model = _released_model()
    scale = deepforest_scale(gsd_cm)
    size = DEEPFOREST_PATCH_PX
    photos = _photos(images)
    print(
        f"{len(photos)} foto, diperkecil x{scale:.2f}, {tiles_per_photo} tile per foto",
        flush=True,
    )

    for path in photos:
        bgr = cv2.imread(path, cv2.IMREAD_COLOR)
        if bgr is None:
            print(f"  dilewati (tidak terbaca): {path}", flush=True)
            continue
        bgr = cv2.resize(bgr, None, fx=scale, fy=scale, interpolation=cv2.INTER_AREA)
        stem = os.path.splitext(os.path.basename(path))[0]
        for tile in spread(grid(bgr.shape[1], bgr.shape[0], size), tiles_per_photo):
            crop = bgr[tile.y : tile.y + size, tile.x : tile.x + size]
            frame = model.predict_image(
                image=cv2.cvtColor(crop, cv2.COLOR_BGR2RGB).astype("float32")
            )
            boxes = (
                ()
                if frame is None
                else tuple(
                    Box(int(r.xmin), int(r.ymin), int(r.xmax), int(r.ymax))
                    for r in frame.itertuples()
                    if r.score >= min_score
                )
            )
            name = f"{stem}{TILE_SEPARATOR}x{tile.x}_y{tile.y}"
            cv2.imwrite(os.path.join(labels, name + ".jpg"), crop)
            with open(os.path.join(labels, name + ".xml"), "w", encoding="utf-8") as handle:
                handle.write(voc_xml(name + ".jpg", size, boxes))
        print(f"  {stem}", flush=True)
    print(f"Sekarang koreksi kotak di {labels} (Pascal VOC), lalu jalankan 'train'.", flush=True)


def write_splits(labels: str, val_fraction: float) -> tuple[str, str]:
    """train.csv and val.csv inside ``labels``, split by photo."""
    header = "image_path,xmin,ymin,xmax,ymax,label"
    train, val = [header], [header]
    for name in sorted(n for n in os.listdir(labels) if n.endswith(".xml")):
        with open(os.path.join(labels, name), encoding="utf-8") as handle:
            file_name, boxes = parse_voc(handle.read())
        file_name = file_name or os.path.splitext(name)[0] + ".jpg"
        target = val if is_validation(photo_of(file_name), val_fraction) else train
        target.extend(csv_rows(file_name, boxes))
    if len(train) == 1 or len(val) == 1:
        raise SystemExit(f"Butuh tile berlabel dari beberapa foto di {labels} untuk kedua split")

    paths = os.path.join(labels, "train.csv"), os.path.join(labels, "val.csv")
    for path, rows in zip(paths, (train, val)):
        with open(path, "w", encoding="utf-8") as handle:
            handle.write("\n".join(rows) + "\n")
    return paths


def _scores(results: Sequence[dict]) -> str:
    wanted = ("box_precision", "box_recall", "map_50", "val_loss")
    metrics = results[0] if results else {}
    return ", ".join(f"{key}={float(metrics[key]):.3f}" for key in wanted if key in metrics)


def train(
    labels: str, out: str, epochs: int, lr: float, batch_size: int, val_fraction: float
) -> None:
    from deepforest import main as deepforest_main

    train_csv, val_csv = write_splits(labels, val_fraction)
    model = deepforest_main.deepforest(
        config_args={
            "batch_size": batch_size,
            "log_root": os.path.join(os.path.dirname(os.path.abspath(out)), "lightning_logs"),
            "train": {"csv_file": train_csv, "root_dir": labels, "epochs": epochs, "lr": lr},
            "validation": {"csv_file": val_csv, "root_dir": labels, "val_accuracy_interval": 1},
        }
    )
    model.load_model(DEEPFOREST_PRETRAINED)
    model.create_trainer()

    print(f"model rilis:       {_scores(model.trainer.validate(model))}", flush=True)
    model.trainer.fit(model)
    print(f"hasil fine-tuning: {_scores(model.trainer.validate(model))}", flush=True)

    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    model.save_model(out)
    print(f"Tersimpan di {out}; pakai sebagai model_path dengan detector 'deepforest'.", flush=True)


def main(argv: Sequence[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)

    pre = commands.add_parser("prelabel", help="tile + kotak model rilis untuk dikoreksi")
    pre.add_argument(
        "--images", required=True, help="folder foto drone (dicari rekursif)"
    )
    pre.add_argument("--labels", required=True, help="folder tujuan tile dan VOC XML")
    pre.add_argument("--gsd-cm", type=float, required=True, help="resolusi foto dalam cm/px")
    pre.add_argument("--tiles-per-photo", type=int, default=4)
    pre.add_argument("--min-score", type=float, default=0.3)

    fit = commands.add_parser("train", help="fine-tuning dengan label yang sudah dikoreksi")
    fit.add_argument(
        "--labels", required=True, help="folder berisi tile dan VOC XML yang sudah dikoreksi"
    )
    fit.add_argument(
        "--out", required=True, help="checkpoint yang ditulis, mis. deepforest-visloc.pt"
    )
    fit.add_argument("--epochs", type=int, default=10)
    fit.add_argument("--lr", type=float, default=0.001)
    fit.add_argument("--batch-size", type=int, default=4)
    fit.add_argument("--val-fraction", type=float, default=0.2)

    args = parser.parse_args(argv)
    warnings.filterwarnings("ignore", category=UserWarning)
    if args.command == "prelabel":
        prelabel(args.images, args.labels, args.gsd_cm, args.tiles_per_photo, args.min_score)
    else:
        train(args.labels, args.out, args.epochs, args.lr, args.batch_size, args.val_fraction)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
