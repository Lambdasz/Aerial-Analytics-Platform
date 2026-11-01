# Panduan & Mekanisme Spatial Result (Modul 03)

Dokumen ini menjelaskan secara teknis dan mendalam mengenai kontrak data `Spatial Result`, tata cara penulisan JSON yang valid, serta mekanisme persistensi, mutasi, dan siklus hidup (lifecycle) pada **Modul 03 (Aerial Image Map Explorer)** dalam Aerial Analytics Platform.

---

## 1. Konsep & Arsitektur Spatial Result

`Spatial Result` merupakan model data standar platform untuk merepresentasikan seluruh luaran analitik geospasial yang dihasilkan oleh plugin pemrosesan citra drone (seperti deteksi vegetasi, segmentasi tutupan lahan, deteksi pohon individu, dan jalur inspeksi) ke antarmuka peta interaktif Leaflet.

### Peran Spatial Result dalam Pipeline Sistem

```text
┌────────────────────────┐      ┌──────────────────────────┐      ┌─────────────────────────┐
│ Plugin Analitik        │─────▶│ Backend Rust (Module 03) │─────▶│ Frontend Leaflet / UI   │
│ (Python / WASM / Rust) │      │ map_storage/ & IPC Handl │      │ Render Vektor & Popup   │
│ Output: Result GeoJSON │      │ Validasi, Atomik, Kuota  │      │ Styling & Inspeksi Foto │
└────────────────────────┘      └──────────────────────────┘      └─────────────────────────┘
```

1. **Jembatan Antarmuka Plugin & Peta**: Plugin analitik menghasilkan format `LayerPayload` yang kompatibel dengan standar GeoJSON RFC 7946.
2. **Dual-Storage Atomik**: Disimpan di direktori `map_storage/spatial_results/` baik dalam berkas tunggal (`.json`) maupun folder terdekomposisi per fitur (`features/{feature_id}.json`).
3. **Penyelarasan Layer Otomatis**: Setiap penyimpanan hasil spasial secara otomatis mendaftarkan status layer di `map_storage/layers/{layer_id}.json`.

---

## 2. Tata Cara Penulisan & Skema Kontrak JSON

Sebuah berkas `Spatial Result` harus mengikuti struktur `LayerPayload`.

### 2.1. Struktur Tingkat Atas (`LayerPayload`)

```json
{
  "layer_id": "result_vegetation_001",
  "layer_name": "Deteksi Vegetasi (ExG) - Blok Sawit A",
  "display_preference": { ... },
  "spatial_results": [ ... ],
  "updated_at": "2026-10-26T05:32:18.847Z",
  "concurrency_token": "2026-10-26T05:32:18.847Z"
}
```

| Field                | Tipe   |  Wajib   | Keterangan & Batasan                                                                                                                           |
| :------------------- | :----- | :------: | :--------------------------------------------------------------------------------------------------------------------------------------------- |
| `layer_id`           | String |  **Ya**  | Pengenal unik. **Aturan Regex**: `^[A-Za-z0-9_-]{1,64}$`. Tidak boleh mengandung spasi, slash (`/`, `\`), atau `..` (mencegah path traversal). |
| `layer_name`         | String |  **Ya**  | Nama tampilan layer yang dibaca pengguna (1–100 karakter, tidak boleh hanya spasi).                                                            |
| `display_preference` | Object |  **Ya**  | Preferensi visualisasi frontend (Poligon, Titik, Garis, atau Gambar Overlay).                                                                  |
| `spatial_results`    | Array  |  **Ya**  | Daftar fitur spasial (`SpatialResult`) yang memuat geometri dan atribut.                                                                       |
| `updated_at`         | String | Opsional | Timestamp ISO 8601 UTC saat data dibuat atau dimutasi.                                                                                         |
| `concurrency_token`  | String | Opsional | Token konkurensi optimistik untuk mencegah penulisan tumpang tindih (_lost update_).                                                           |

---

### 2.2. Skema Preferensi Tampilan (`display_preference`)

Frontend mendukung 4 mode visualisasi melalui tagged union bertag `"type"`:

#### A. Mode Poligon (`"type": "polygon"`)

Digunakan untuk area segmentasi vegetasi, zonasi tutupan lahan, atau batas AOI.

```json
{
  "type": "polygon",
  "color": "#0074d9",
  "opacity": 0.6,
  "image_url": "/map_storage/images/result_image/result_vegetation_001_1792992738847.jpg",
  "fields": [
    {
      "key": "vegetation_index_type",
      "label": "Jenis Indeks",
      "visible": true,
      "custom": false,
      "kind": "text"
    },
    {
      "key": "mean_greenness_score",
      "label": "Skor Rata-rata",
      "visible": true,
      "custom": false,
      "kind": "number"
    },
    {
      "key": "area_sqm",
      "label": "Luas Area",
      "unit": "m²",
      "visible": true,
      "custom": false,
      "kind": "number"
    }
  ]
}
```

#### B. Mode Titik (`"type": "point"`)

Digunakan untuk deteksi pohon individu, bibit, atau titik anomali.

```json
{
  "type": "point",
  "color": "#2ecc40",
  "icon": "tree",
  "image_url": null,
  "fields": [
    { "key": "tree_id", "label": "ID Pohon", "visible": true, "kind": "text" },
    { "key": "confidence", "label": "Kepercayaan", "visible": true, "kind": "number" },
    {
      "key": "height_est_m",
      "label": "Estimasi Tinggi",
      "unit": "m",
      "visible": true,
      "kind": "number"
    }
  ]
}
```

#### C. Mode Garis (`"type": "line"`)

Digunakan untuk jalur terbang drone, parit/drainase, atau rute inspeksi.

```json
{
  "type": "line",
  "color": "#ff851b",
  "weight": 3.0,
  "fields": [
    { "key": "path_id", "label": "ID Jalur", "visible": true, "kind": "text" },
    {
      "key": "length_m",
      "label": "Panjang Lintasan",
      "unit": "m",
      "visible": true,
      "kind": "number"
    }
  ]
}
```

#### D. Mode Image Overlay (`"type": "image_overlay"`)

Digunakan untuk peta panas (_heatmap_ raster) atau layer overlay bitmap berkoordinat.

```json
{
  "type": "image_overlay",
  "image_url": "/map_storage/runs/001/output_mask_01.png",
  "opacity": 0.75,
  "bounds": [
    [-1.2228, 116.857],
    [-1.2208, 116.8592]
  ]
}
```

_Catatan: `bounds` berformat `[[South, West], [North, East]]` dalam koordinat lintang/bujur (Latitude/Longitude)._

---

### 2.3. Definisi Kolom Presentasi (`FieldDef`)

Setiap item dalam array `fields` mengatur bagaimana atribut ditampilkan di popup peta Leaflet maupun tabel data atribut:

- `key`: Kunci properti yang merujuk pada `properties` di tiap fitur spasial.
- `label`: Label deskriptif yang tampil pada header tabel/popup.
- `unit`: Satuan metrik (misal `m²`, `ha`, `%`, `m`).
- `visible`: Menentukan apakah kolom langsung ditampilkan (`true`) atau disembunyikan secara default (`false`).
- `custom`: Bernilai `true` jika kolom merupakan hasil penambahan manual oleh pengguna melalui modal editor.
- `kind`: Tipe data primitif (`"text"`, `"number"`, `"boolean"`).

---

### 2.4. Fitur Spasial Individual (`SpatialResult`)

Array `spatial_results` berisi sekumpulan objek fitur:

```json
{
  "source": "vegetation_detection_v1",
  "geometry": {
    "type": "Polygon",
    "coordinates": [
      [
        [116.8575, -1.221],
        [116.8582, -1.2208],
        [116.859, -1.2212],
        [116.8575, -1.221]
      ]
    ]
  },
  "properties": {
    "vegetation_index_type": "ExG",
    "mean_greenness_score": 0.68,
    "area_sqm": 1420.5
  },
  "image_url": null,
  "edited_fields": []
}
```

1. **`source`**: String identifikasi modul/algoritma pembuat (misal `"vegetation_detection_v1"`, `"tree_yolo_v8"`).
2. **`geometry`**: Struktur geometri GeoJSON RFC 7946 murni:
   - Sistem Koordinat: **WGS 84 (EPSG:4326)**.
   - Urutan Koordinat: **`[Bujur / Longitude, Lintang / Latitude]`** (bukan `[lat, lon]`).
   - Tipe Geometri: `Point`, `Polygon`, atau `LineString`.
3. **`properties`**: Atribut polimorfik fitur:
   - `TreeProperties`: `tree_id`, `confidence`, `height_est_m`.
   - `VegetationProperties`: `vegetation_index_type`, `mean_greenness_score`, `area_sqm`.
   - `LandCoverProperties`: `land_cover_class`, `confidence`, `area_sqm`.
   - `InspectionPathProperties`: `path_id`, `length_m`.
   - `Generic`: Pasangan kunci-nilai bebas jika plugin menggunakan skema kustom.
4. **`image_url`**: Path foto spesifik untuk fitur tersebut (opsional).
5. **`edited_fields`**: Array nama-nama kunci yang telah diedit secara manual oleh pengguna (misal `["mean_greenness_score"]`).

---

## 3. Mekanisme Penyimpanan & Backend Engine

### 3.1. Format Dual-Storage (Koeksistensi Flat & Dekomposisi)

Backend Modul 03 (`src-tauri/src/modules/module_03/sp_result/`) mendukung dua bentuk penyimpanan di disk:

1. **Format Flat File (`map_storage/spatial_results/{layer_id}.json`)**:
   - Berkas JSON tunggal yang memuat seluruh payload.
   - Cocok untuk data berukuran kecil hingga sedang (ratusan fitur).
2. **Format Folder Terdekomposisi (`map_storage/spatial_results/{layer_id}/`)**:
   - Digunakan saat data sangat besar guna menghindari beban memori tinggi:
     - `{layer_id}/layer.json`: Menyimpan metadata layer dan `display_preference` (dibatasi maksimum 64 KB).
     - `{layer_id}/features/{feature_id}.json`: Tiap fitur disimpan sebagai berkas tersendiri (dibatasi maksimum 64 KB per fitur).
   - Penamaan ID fitur diatur secara bersih melalui sanitasi nama (`feat_0001.json`, `tree_102.json`).

### 3.2. Penulisan Atomik & Integritas Data

Setiap operasi tulis ke disk menggunakan fungsi utilitas `write_atomic`:

1. Konten JSON ditulis terlebih dahulu ke berkas sementara: `temp_{uuid}.tmp`.
2. Dilakukan sinkronisasi flush buffer OS (`flush()`).
3. Berkas sementara di-rename secara atomik menggantikan berkas target (`fs::rename`).
4. **Jaminan**: Tidak akan pernah terjadi berkas JSON korup atau terpotong jika laptop mati mendadak atau aplikasi ditutup paksa saat proses penyimpanan.

### 3.3. Penyelarasan Otomatis Layer (`map_storage/layers/`)

Saat `save_spatial_result` dipanggil, backend secara otomatis membuat atau memperbarui berkas status layer di `map_storage/layers/{layer_id}.json` dengan skema `LayerState`:

```json
{
  "layer_id": "result_vegetation_001",
  "layer_name": "Deteksi Vegetasi (ExG) - Sawit raya",
  "category": "polygon",
  "is_visible": true,
  "opacity": 0.6,
  "z_index": 10,
  "source_type": "spatial_result",
  "source_id": "result_vegetation_001"
}
```

Hal ini memastikan layer baru langsung muncul pada panel pengatur layer di peta tanpa sinkronisasi manual.

---

## 4. Siklus Hidup & Mekanisme Mutasi (Lifecycle Operations)

### 4.1. Operasi Tauri IPC Commands

| Perintah IPC                        |   Tipe Akses    | Deskripsi                                                                                         |
| :---------------------------------- | :-------------: | :------------------------------------------------------------------------------------------------ |
| `save_spatial_result`               | Mutasi (Write)  | Menyimpan data spasial baru secara atomik. Mengirim event `spatial_result:saved`.                 |
| `get_saved_spatial_results`         |   Baca (Read)   | Membaca seluruh spatial result lengkap beserta geometri GeoJSON.                                  |
| `get_saved_spatial_results_summary` |   Baca (Read)   | Mengambil ringkasan ringan (`SpatialResultSummary`) untuk daftar kartu UI (cepat & hemat memori). |
| `get_saved_spatial_result`          |   Baca (Read)   | Mengambil satu spatial result lengkap berdasarkan `layer_id` menggunakan dual-reader.             |
| `update_saved_spatial_result`       | Mutasi (Write)  | Memperbarui field aman (warna, nama, ikon, custom field, foto) tanpa merusak koordinat.           |
| `count_spatial_result_dependents`   |   Baca (Read)   | Menghitung dependensi dan aset foto sebelum memunculkan dialog konfirmasi hapus.                  |
| `delete_saved_spatial_result`       | Mutasi (Delete) | Menghapus layer dengan mekanisme karantina 2-fase. Mengirim event `spatial_result:deleted`.       |
| `check_spatial_result_manual_edits` |   Baca (Read)   | Memeriksa apakah ada fitur yang telah diedit pengguna sebelum ditimpa oleh plugin.                |

---

### 4.2. Mekanisme Pembaruan Aman (Semantic Patching)

Melalui perintah `update_saved_spatial_result`, pengguna dapat mengubah metadata tanpa risiko merusak geometri asli:

1. **Pembaruan Presentasi**: Nama layer, warna heksadesimal (`#RRGGBB`), dan ikon.
2. **Manajemen Foto Layer**: Mengganti atau menghapus foto unggahan (`image_action: "replace" | "delete" | "keep"`). Foto disimpan di `map_storage/images/result_image/{layer_id}_{timestamp}.{ext}` setelah melewati verifikasi _magic bytes_ citra (JPEG/PNG/WebP, batas maks 10 MB).
3. **Audit Edit Manual**: Setiap perubahan pada properti fitur ditandai secara otomatis di array `edited_fields`, sehingga algoritma otomatisasi tidak menimpa modifikasi manual tanpa persetujuan pengguna.
4. **Kontrol Konkurensi Optimistik**: Menggunakan `expected_updated_at` untuk mendeteksi apakah data telah diubah oleh sesi lain sebelum pembaruan diterapkan.

---

### 4.3. Penghapusan Transaksional 2-Fase & Karantina (Quarantine)

Operasi penghapusan layer dirancang aman terhadap kegagalan parsial:

1. **Fase 1 (Karantina)**: Direktori atau berkas spatial result beserta foto terkait di `images/result_image/` dipindahkan ke folder karantina tersembunyi:
   `.trash_sr_{layer_id}_{timestamp}/`
2. **Fase 2 (Deregistrasi Layer)**: Berkas pendaftaran di `map_storage/layers/{layer_id}.json` dihapus.
3. **Rollback Otomatis**: Jika deregistrasi gagal, berkas dari karantina langsung dipulihkan kembali ke tempat semula.
4. **Rekonsiliasi Startup**: Fungsi `cleanup_orphaned_trash` dijalankan otomatis saat aplikasi pertama kali dibuka untuk membersihkan residu folder `.trash_sr_*` yang tertinggal jika terjadi crash pada OS saat proses hapus.

---

## 5. Contoh JSON Lengkap Siap Pakai

### Contoh: Deteksi Pohon Individu (Point Layer)

```json
{
  "layer_id": "result_tree_001",
  "layer_name": "Sensus Pohon Sawit Blok B12",
  "display_preference": {
    "type": "point",
    "color": "#2ecc40",
    "icon": "tree",
    "fields": [
      {
        "key": "tree_id",
        "label": "Nomor Pohon",
        "visible": true,
        "custom": false,
        "kind": "text"
      },
      {
        "key": "confidence",
        "label": "Tingkat Keyakinan",
        "visible": true,
        "custom": false,
        "kind": "number"
      },
      {
        "key": "height_est_m",
        "label": "Tinggi Rata-rata",
        "unit": "m",
        "visible": true,
        "custom": false,
        "kind": "number"
      }
    ]
  },
  "spatial_results": [
    {
      "source": "tree_detector_yolov8",
      "geometry": {
        "type": "Point",
        "coordinates": [116.8582, -1.2208]
      },
      "properties": {
        "tree_id": "TREE_001",
        "confidence": 0.94,
        "height_est_m": 8.5
      }
    },
    {
      "source": "tree_detector_yolov8",
      "geometry": {
        "type": "Point",
        "coordinates": [116.8585, -1.2211]
      },
      "properties": {
        "tree_id": "TREE_002",
        "confidence": 0.91,
        "height_est_m": 7.8
      }
    }
  ],
  "updated_at": "2026-10-26T08:00:00.000Z"
}
```
