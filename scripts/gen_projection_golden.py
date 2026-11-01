#!/usr/bin/env python3
"""Hasilkan golden proyeksi dari pyproj (PROJ) ke src-tauri/tests/fixtures/projection_golden.json.

Hanya INPUT (lintang/bujur) yang didefinisikan di sini. Seluruh nilai referensi (E/N atau X/Y)
dihitung oleh pyproj; jangan mengetik angka referensi manual.

Jalankan dari akar repo:
    python scripts/gen_projection_golden.py
"""

import json
from pathlib import Path

import pyproj
from pyproj import Transformer

# (nama, jenis, lat, lon, zona, selatan)
CASES = [
    ("Paris 31N", "utm", 48.85837, 2.29448, 31, False),
    ("Greenwich 30N", "utm", 51.476852, 0.0, 30, False),
    ("Balikpapan 50S", "utm", -1.26539, 116.83120, 50, True),
    ("Sepinggan 50S", "utm", -1.268270, 116.894450, 50, True),
    ("Kariangau 50S", "utm", -1.173800, 116.812300, 50, True),
    ("Semayang 50S", "utm", -1.282500, 116.817200, 50, True),
    ("Monas 48S", "utm", -6.17539, 106.82715, 48, True),
    ("New York 18N", "utm", 40.71278, -74.00594, 18, False),
    ("Zona 48S dekat batas timur", "utm", -7.0, 107.95, 48, True),
    ("Zona 50S dekat batas barat", "utm", -1.2, 114.05, 50, True),
    ("Null Island 3857", "3857", 0.0, 0.0, None, None),
    ("Monas 3857", "3857", -6.17539, 106.82715, None, None),
    ("Paris 3857", "3857", 48.85837, 2.29448, None, None),
    ("Greenwich 3857", "3857", 51.482578, 0.0, None, None),
]


def main() -> None:
    out_cases = []
    for name, kind, lat, lon, zone, south in CASES:
        if kind == "utm":
            epsg = (32700 if south else 32600) + zone
            t = Transformer.from_crs("EPSG:4326", f"EPSG:{epsg}", always_xy=True)
            x, y = t.transform(lon, lat)
        else:
            epsg = 3857
            t = Transformer.from_crs("EPSG:4326", "EPSG:3857", always_xy=True)
            x, y = t.transform(lon, lat)
        out_cases.append(
            {
                "name": name,
                "kind": kind,
                "epsg": epsg,
                "zone": zone,
                "south": south,
                "lat": lat,
                "lon": lon,
                "x": round(x, 4),
                "y": round(y, 4),
            }
        )

    doc = {
        "generator": "scripts/gen_projection_golden.py",
        "pyproj_version": pyproj.__version__,
        "proj_version": pyproj.proj_version_str,
        "cases": out_cases,
    }
    target = Path(__file__).resolve().parent.parent / "src-tauri/tests/fixtures/projection_golden.json"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"pyproj {pyproj.__version__} / PROJ {pyproj.proj_version_str}: {len(out_cases)} kasus -> {target}")


if __name__ == "__main__":
    main()
