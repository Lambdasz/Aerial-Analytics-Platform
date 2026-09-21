# Kontrak Modul 7: Tree Detection & Counting

> Untuk: tim Modul 2 (Plugin Manager), 3 (Map Explorer), 9 (Area & Plot), 10 (Temporal), dan 11 (Dashboard).
> Status: **draf v0.1.0, belum final**. Bagian [Perlu disepakati](#6-perlu-disepakati) menunggu jawaban
> dari tim terkait. Nama field di dokumen ini sama persis dengan yang dipakai kode.

Dokumen ini menjelaskan apa yang Modul 7 **terima** dan **hasilkan**. Modul lain tidak perlu
membaca kode Python Modul 7 untuk memakainya.

## 1. Cara Modul 7 dipanggil

Modul 7 tidak menyediakan fungsi yang dipanggil langsung oleh Rust atau React. Plugin ini
dijalankan Modul 2 sebagai **proses terpisah**, sesuai template `plugins/template`:

```text
python main.py --healthcheck                                  → exit 0 sehat / 1 tidak sehat
python main.py --input <payload.json> --output <result.json>  → exit 0 selesai / 1 gagal
```

Semua pertukaran data lewat file JSON yang tervalidasi skema di `schemas/`:

```text
M1 (gambar+metadata) ─┐
M3 (AOI) ─────────────┼─► M2 buat execution_payload.json ─► Modul 7 ─► execution_result.json
                      │                                             ├─► <id>_tree_points.geojson
                      │                                             └─► <id>_plugin_output.json
                      └──────────────────── M3 / M9 / M10 / M11 membaca hasil lewat M2
```

Kalau plugin gagal (gambar hilang, GeoJSON rusak, dan sebagainya), plugin **tetap** menulis
`result.json` dengan `status: "failure"` dan pesan di `error`, lalu keluar dengan kode 1.

## 2. Input (dari Modul 1, 2, dan 3)

Mengikuti `schemas/execution_payload.schema.json`. Field yang dipakai Modul 7:

| Field payload                         | Wajib  | Sumber    | Dipakai untuk                                                                 |
| :------------------------------------ | :----: | :-------- | :---------------------------------------------------------------------------- |
| `execution_id`                        |   ya   | M2        | nama file artefak & `execution_id` hasil                                      |
| `session_id`                          | tidak  | M1/M2     | `project_id` di metadata hasil                                                |
| `output_dir`                          |   ya   | M2        | folder tempat artefak ditulis                                                 |
| `target.image_path`                   |   ya   | M1        | gambar RGB (`.jpg` `.jpeg` `.png` `.tif` `.tiff`)                             |
| `target.metadata.gps` `{lat, lon}`    | **ya** | M1        | posisi pusat gambar (WGS84)                                                   |
| `target.metadata.resolution` `[w, h]` | **ya** | M1        | ukuran gambar dalam piksel                                                    |
| `target.metadata.gsd_m`               | tidak* | M1        | meter per piksel                                                              |
| `target.metadata.rotation_deg`        | tidak  | M1        | rotasi searah jarum jam dari utara ke tepi atas gambar, default `0`           |
| `target.metadata.image_id`            | tidak  | M1        | id gambar; default: nama file tanpa ekstensi                                  |
| `aoi.geometry`                        | tidak  | M3        | GeoJSON `Polygon`/`MultiPolygon` WGS84; hanya pohon di dalamnya yang dihitung |
| `parameters.*`                        |   ya   | M2 (form) | lihat tabel parameter di bawah                                                |

\* Kalau `gsd_m` tidak ada, dipakai parameter `gsd_cm` (default 3 cm/px).

**Parameter** (`parameters.json`, dirender Modul 2 sebagai form):

| `name`                 | Tipe        | Default  | Keterangan                                        |
| :--------------------- | :---------- | :------- | :------------------------------------------------ |
| `confidence_threshold` | number 0–1  | `0.5`    | deteksi di bawah nilai ini dibuang                |
| `density_unit`         | select      | `per_ha` | `per_ha` atau `per_m2`                            |
| `detector`             | select      | `stub`   | model deteksi; `stub` = placeholder (0 deteksi)   |
| `iou_threshold`        | number 0–1  | `0.5`    | batas tumpang tindih untuk deduplikasi antar-tile |
| `tile_size`            | number (px) | `1024`   | ukuran tile inferensi                             |
| `gsd_cm`               | number      | `3.0`    | cadangan kalau metadata tidak punya `gsd_m`       |

## 3. Output

### 3.1 `execution_result.json` (dibaca Modul 2, lalu diteruskan ke modul lain)

`status`: `success`, `warning` (selesai tapi ada catatan, misalnya detector `stub`), atau `failure`.

**Metrik** (`metrics`):

| Key                      | Tipe    | Arti                                                                         |
| :----------------------- | :------ | :--------------------------------------------------------------------------- |
| `tree_count`             | integer | jumlah pohon setelah filter confidence, **di dalam AOI** kalau AOI diberikan |
| `avg_confidence`         | float   | rata-rata confidence pohon yang dihitung (0–1)                               |
| `removed_low_confidence` | integer | deteksi yang dibuang karena di bawah threshold                               |
| `trees_outside_aoi`      | integer | deteksi di luar AOI (`0` kalau tanpa AOI)                                    |
| `analysis_area_m2`       | float   | luas AOI, atau luas tapak gambar kalau tanpa AOI                             |
| `tree_density`           | float   | `tree_count / analysis_area_m2` dalam satuan `tree_density_unit`             |
| `tree_density_unit`      | string  | `per_ha` atau `per_m2`                                                       |

**Artefak** (`artifacts`):

| Key             | Format                 | Isi                                      |
| :-------------- | :--------------------- | :--------------------------------------- |
| `tree_points`   | `application/geo+json` | titik tiap pohon (lihat 3.2)             |
| `plugin_output` | `application/json`     | hasil lengkap `PluginOutput` (lihat 3.3) |

### 3.2 `tree_points`: GeoJSON FeatureCollection

Satu `Feature` per pohon yang dihitung. Koordinat `[longitude, latitude]` dalam EPSG:4326 (RFC 7946).

```json
{
  "type": "FeatureCollection",
  "features": [
    {
      "type": "Feature",
      "id": "DJI_0042-deepforest-0001",
      "geometry": { "type": "Point", "coordinates": [116.8535, -1.2445] },
      "properties": {
        "detection_id": "DJI_0042-deepforest-0001",
        "confidence": 0.93,
        "area_m2": 18.4,
        "bbox_px": [1990.0, 1490.0, 2010.0, 1510.0]
      }
    }
  ]
}
```

- `detection_id` berformat `{image_id}-{model_name}-{nomor 4 digit}`. Deteksi yang ditambahkan
  manual lewat Detection Review diawali `manual-`.
- `area_m2` adalah perkiraan luas kotak tajuk (`luas bbox piksel × gsd_m²`).
- `bbox_px` = `[x_min, y_min, x_max, y_max]` dalam piksel gambar asli, dipakai untuk menggambar kotak
  di atas foto.

### 3.3 `plugin_output`: struktur lengkap

Sesuai model `PluginOutput` di laporan progres Modul 7 (`treedet/models.py`):

```json
{
  "status": "success",
  "detections": [
    {
      "detection_id": "...",
      "bbox": { "x_min": 0, "y_min": 0, "x_max": 0, "y_max": 0 },
      "confidence": 0.93,
      "center": { "latitude": -1.2445, "longitude": 116.8535 },
      "area_m2": 18.4
    }
  ],
  "statistics": {
    "total_trees": 42,
    "avg_confidence": 0.81,
    "counted_at": "2026-09-21T09:15:00+00:00",
    "density_value": 5.03,
    "density_unit": "per_ha",
    "area_m2": 83470.2,
    "removed_low_confidence": 7,
    "threshold_used": 0.5,
    "plot_id": "aoi",
    "trees_outside_area": 5
  },
  "geojson": "<string berisi tree_points di atas>",
  "metadata": {
    "plugin_version": "0.1.0",
    "model_name": "deepforest",
    "model_version": null,
    "executed_at": "2026-09-21T09:15:00+00:00",
    "device": "cpu",
    "image_id": "DJI_0042",
    "project_id": "session_20260921_01",
    "processing_time_s": 2.1,
    "warnings": []
  }
}
```

Status `PluginOutput` dipetakan ke skema resmi: `success → success`, `partial → warning`, `failed → failure`.

## 4. Per modul pemakai

| Modul                 | Yang dipakai                                                                       | Catatan                                                                                                                                                                                                                                                                                                                           |
| :-------------------- | :--------------------------------------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **M2** Plugin Manager | CLI di bagian 1; `manifest.json`, `parameters.json`, `inputs.json`, `outputs.json` | semua lolos validasi `check-jsonschema` terhadap `schemas/`                                                                                                                                                                                                                                                                       |
| **M3** Map Explorer   | artefak `tree_points`                                                              | layer_hint `point_marker`. Contoh M3 di `docs/examples/spatial-result.example.json` memakai format `spatial_results`; M3 cukup mengubah tiap `Feature` menjadi `{source, geometry, properties}` dengan `source = "tree-detection-counting"`. `height_est_m` **tidak** tersedia karena citra RGB tunggal tidak memberi info tinggi |
| **M9** Area & Plot    | `tree_points` atau `plugin_output.detections[].center`                             | hitung per plot dengan point-in-polygon terhadap `Plot.rings` milik M9. Alternatif: M2 menjalankan Modul 7 dengan `aoi` = satu plot, lalu `tree_count` = pohon di plot tersebut                                                                                                                                                   |
| **M10** Temporal      | metrik `tree_count` dari dua eksekusi (tanggal berbeda)                            | bandingkan hanya jika `confidence_threshold`, `detector`, dan AOI sama                                                                                                                                                                                                                                                            |
| **M11** Dashboard     | semua `metrics` + `plugin_output.statistics`                                       | label & satuan tersedia di `outputs.json`                                                                                                                                                                                                                                                                                         |

## 5. Fungsi publik di dalam plugin

Untuk sesama pengembang Python atau penguji, paket `treedet` mengekspor fungsi-fungsi dari laporan
progres Modul 7. Semuanya murni, kecuali `detect_trees`.

| Fitur                   | Fungsi                                                                                     |
| :---------------------- | :----------------------------------------------------------------------------------------- |
| 1 Tree Detection        | `detect_trees(input_data, model_config) -> DetectionOutput`                                |
| 2 Tree Counting         | `count_trees(detections, image_id, counted_at) -> CountOutput`                             |
| 3 Confidence Filtering  | `filter_by_confidence(detections, threshold) -> FilteredOutput`                            |
| 4 Tree Location Mapping | `map_tree_locations(detections, image_metadata) -> LocationOutput`                         |
| 5 Area-Based Counting   | `count_trees_in_area(tree_points, plot_boundary) -> AreaCountOutput`                       |
| 6 Tree Density          | `calculate_tree_density(tree_count, area_m2, unit="per_ha") -> DensityOutput`              |
| 7 Detection Review      | `apply_review(detections, corrections) -> ReviewOutput`                                    |
| 8 Plugin Output         | `build_plugin_output(detections, count, density, locations, run_metadata) -> PluginOutput` |

Perbedaan dari laporan progres, beserta alasannya:

- `manifest.json` memakai format resmi `schemas/plugin.schema.json`. Skema itu menolak field
  `input_schema`, `output_schema`, dan `config` dari lampiran laporan, sehingga isinya dipindah
  ke `inputs.json`, `outputs.json`, dan `parameters.json`.
- `count_trees` menerima `image_id` dan `counted_at` sebagai argumen. Tanpa itu, `image_id` tidak
  bisa diketahui saat deteksi kosong, dan membaca jam di dalam fungsi membuatnya tidak murni.
- `TreeDetection.center` dan `area_m2` bernilai `null` sebelum Fitur 4 dijalankan, karena detector
  hanya tahu posisi piksel.
- `image_id` dan `project_id` tidak ada di skema payload resmi, jadi diambil dari
  `target.metadata.image_id` (atau nama file) dan `session_id`.
- Status `partial`/`failed` dipetakan ke `warning`/`failure` sesuai skema hasil resmi.

## 6. Perlu disepakati

|  #  | Pertanyaan                                                                                                                                                                                     | Dengan    | Usulan Modul 7                                                                         |
| :-: | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :-------- | :------------------------------------------------------------------------------------- |
|  1  | Apakah M1 bisa mengisi `gsd_m`, `rotation_deg`, dan `image_id` di `target.metadata`? GSD bisa dihitung dari ketinggian relatif + focal length + ukuran sensor; rotasi dari `gimbal_yaw_degree` | M1, M2    | ya, dengan nama field persis seperti tabel input                                       |
|  2  | Hitung per plot: agregasi titik di M9, atau M2 menjalankan Modul 7 per plot?                                                                                                                   | M2, M9    | agregasi di M9 (satu eksekusi untuk semua plot)                                        |
|  3  | Detection Review butuh tempat menyimpan koreksi user dan cara mengirimnya ke plugin. Ini perlu Tauri command baru di core                                                                      | M2        | M2 menambah command simpan koreksi; Modul 7 menerima path file koreksi lewat parameter |
|  4  | Siapa yang mengubah GeoJSON ke format `spatial_results` M3?                                                                                                                                    | M3        | M3 (plugin tetap mengeluarkan GeoJSON standar)                                         |
|  5  | Model deteksi yang dipakai (klasik OpenCV atau deep learning seperti DeepForest)?                                                                                                              | dosen/tim | ditentukan Modul 7; tidak mengubah kontrak ini                                         |
|  6  | Plugin ditaruh di repo ini (`plugins/tree_detection/`) atau repo terpisah? `CONTRIBUTING.md` menyebut plugin analitik ada di repo terpisah                                                     | tim       | sementara di repo ini, mengikuti `plugins/mock`                                        |

## 7. Riwayat versi

| Versi | Tanggal    | Perubahan                                                         |
| :---- | :--------- | :---------------------------------------------------------------- |
| 0.1.0 | 2026-09-21 | Draf awal: kerangka plugin, kontrak input/output, detector `stub` |
