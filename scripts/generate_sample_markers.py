#!/usr/bin/env python3
"""
Pembangkit Data Marker Foto Drone Deterministik (Module 3)
Lokasi: Kariangau, Balikpapan (sesuai citra orthomosaic OAM 600fcc9055481e0005e312a1)
"""

import json
import os
import random

SEED = 42
TOTAL_MARKERS = 200

# Bounding box Kariangau untuk lintasan terbang (di dalam batas orthomosaic)
LAT_MIN = -1.2240
LAT_MAX = -1.2190
LON_MIN = 116.8565
LON_MAX = 116.8625
ALTITUDE_M = 120.0

def generate_markers(count=TOTAL_MARKERS):
    rng = random.Random(SEED)
    markers = []
    
    # Buat pola flight grid: baris garis terbang dari selatan ke utara bolak-balik
    num_flight_lines = 10
    points_per_line = count // num_flight_lines
    
    idx = 1
    for line_idx in range(num_flight_lines):
        # Longitude tetap untuk satu garis terbang
        lon_ratio = line_idx / max(1, num_flight_lines - 1)
        lon = LON_MIN + lon_ratio * (LON_MAX - LON_MIN)
        
        # Bolak-balik: garis genap ke utara (heading 0), garis ganjil ke selatan (heading 180)
        direction_north = (line_idx % 2 == 0)
        heading = 0.0 if direction_north else 180.0
        
        for pt_idx in range(points_per_line):
            lat_ratio = pt_idx / max(1, points_per_line - 1)
            if not direction_north:
                lat_ratio = 1.0 - lat_ratio
            
            lat = LAT_MIN + lat_ratio * (LAT_MAX - LAT_MIN)
            
            # Tambahkan deviasi kecil deterministik (jitter) agar realistis seperti drone nyata
            lat_jitter = (rng.random() - 0.5) * 0.0001
            lon_jitter = (rng.random() - 0.5) * 0.0001
            
            image_id = f"img_kariangau_{idx:03d}"
            file_name = f"DJI_20260914_{idx:04d}.JPG"
            
            markers.append({
                "image_id": image_id,
                "file_name": file_name,
                "latitude": round(lat + lat_jitter, 6),
                "longitude": round(lon + lon_jitter, 6),
                "altitude_agl": ALTITUDE_M,
                "heading_deg": heading,
                "thumbnail_url": "/drone_ortho.jpg"
            })
            idx += 1
            
    return markers

def main():
    markers = generate_markers()
    output_path = os.path.join(
        os.path.dirname(__file__),
        "..",
        "src-tauri",
        "resources",
        "sample-data",
        "module3",
        "markers.json"
    )
    output_path = os.path.normpath(output_path)
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(markers, f, indent=2, ensure_ascii=False)
        f.write("\n")
        
    print(f"Berhasil menghasilkan {len(markers)} markers ke {output_path}")

if __name__ == "__main__":
    main()
