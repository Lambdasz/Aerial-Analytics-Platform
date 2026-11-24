# Panduan Arsitektur & Rekap Refactor Modul 03 (Aerial Image Map Explorer)

**Platform:** Aerial Analytics Platform (AAP) — Tauri 2 + React 19  
**Fokus Modul:** Modul 03 (Map Explorer, Orthomosaic Ingestion, AOI, & Geodesic Geometry)  
**Status Kode:** Terverifikasi 100% Lulus Linting (`Clippy -D warnings`), Format (`rustfmt`), dan Pengujian Unit (`cargo test`, 41 tests di Modul 03 & 464 tests di workspace).

---

## 1. Ringkasan Eksekutif (Baca Ini Dulu)

Dokumen ini adalah **panduan alur kerja teknis resmi** untuk backend Modul 03. Seluruh arsitektur backend telah direfaktor secara radikal untuk memenuhi prinsip _Clean Architecture_, _Single Responsibility Principle_ (SRP), dan _Functional Programming_ (pemisahan murni antara komputasi matematika dan I/O fisik).

Untuk menjaga keterbacaan kode tingkat tinggi (_human-readable code_) dan memudahkan audit akademis/dosen, seluruh penamaan berkas di `src-tauri/src/modules/module_03/` dipecah ke dalam **4 Pilar Penamaan Eksplisit**:

```
                       ┌──────────────────────────────────────────────┐
                       │           Lapisan UI (React 19)              │
                       └──────────────────────┬───────────────────────┘
                                              │ Tauri Invoke (IPC)
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│ 1. PILAR `_cmd.rs` (Jembatan IPC Tauri)                                                      │
│    - File entrypoint `#[tauri::command]`.                                                   │
│    - Tugas: Membongkar DTO Request, memanggil logic & I/O, mengembalikan DTO Response.       │
└──────────────────────┬──────────────────────────────────────────────┬───────────────────────┘
                       │ Validasi & Komputasi                         │ Simpan / Baca
                       ▼                                              ▼
┌──────────────────────────────────────────────┐  ┌───────────────────────────────────────────┐
│ 2. PILAR `_logic.rs` (Otak Matematika Murni) │  │ 3. PILAR `_io.rs` (Akses Fisik & Storage) │
│    - PURE FUNCTION (Tanpa I/O, No Side-Eff)  │  │    - Operasi disk, decoders, SQLite/JSON  │
│    - Deterministik, 100% mudah di-unit-test  │  │    - Mutasi file atomik, staging guard    │
└──────────────────────┬───────────────────────┘  └───────────────────┬───────────────────────┘
                       │                                              │
                       └──────────────────────┬───────────────────────┘
                                              │ Mengoperasikan
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│ 4. PILAR `_struct.rs` (Kontrak Data & Model Entitas)                                        │
│    - Definisi Struct Serde (`Serialize`/`Deserialize`), DTO Request/Response (Envelope)    │
│    - Struktur domain GeoJSON (Coordinate, Point, Polygon, AoiFeature, Metadata)             │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Karakteristik 4 Pilar:

1. **`*_struct.rs` (Data Contracts):** Berisi definisi tipe data murni, entitas domain GeoJSON, dan struktur DTO Request/Response dengan anotasi `#[serde(rename_all = "camelCase")]`. Tidak memuat logika bisnis maupun akses file.
2. **`*_logic.rs` (Pure Math & Business Logic):** Berisi algoritma perhitungan matematis, validasi koordinat, proyeksi spasial, dan pemrosesan piksel. **DILARANG KERAS** melakukan operasi I/O (tidak boleh `std::fs`, tidak boleh membaca network/disk). Fungsi di sini berstatus _Pure Functions_.
3. **`*_io.rs` (Input/Output & Hardware Access):** Mengisolasi semua interaksi fisik dengan sistem operasi: pembacaan decoder biner, penulisan atomik (`write_atomic`), streaming file ke disk, RAII Staging Guard, serta manajemen lock instance proses.
4. **`*_cmd.rs` (IPC Command Boundary):** Menjadi gerbang penghubung tipis antara frontend dan backend. Hanya bertugas memvalidasi envelope payload dari Tauri, memanggil fungsi `_logic`, meneruskan hasil ke `_io`, lalu mengembalikan response.

---

## 2. Alur Pipeline Import Gambar (Orthomosaic Ingestion)

Proses impor citra udara dibagi menjadi dua orkestrator asinkron:

- **Opsi A (`orchestrator_option_a.rs`):** Untuk citra drone standar (JPEG / PNG) dengan pasangan georeferensi eksternal (World File `.jgw`/`.pgw`/`.wld`, Sidecar JSON `.json`, atau input manual koordinat).
- **Opsi B (`orchestrator_option_b.rs`):** Untuk citra GeoTIFF profesional (`.tif`/`.tiff`) yang menyimpan metadata proyeksi dan koordinat di dalam tag biner IFD internal.

```
                              ALUR PIPELINE OPSI A (JPEG / PNG)

  [Input Citra Drone] ──> [Tahap 1: Validasi Fast-Fail] ──> [Tahap 2: Deteksi Bounding Box]
     (JPG/PNG + WF)          (pipeline_validation_logic)         (world_file_parser_logic)
                                                                            │
  ┌─────────────────────────────────────────────────────────────────────────┘
  │
  ▼
[Tahap 3: Staging Guard RAII] ──> [Tahap 4: Decoding & Resampling] ──> [Tahap 5: Kompresi Display]
  (StagingGuard & Preflight)        (jpeg_decoder / image_resampling)     (adaptive_encoder_io)
                                                                                   │
  ┌────────────────────────────────────────────────────────────────────────────────┘
  │
  ▼
[Tahap 6: Atomic Finalization] ──> [map_storage/images/<id>/] & [map_storage/layers/<id>.json]
  (Point of No Return Rename)
```

### A. Tahapan Eksekusi Pipeline Opsi A (`orchestrator_option_a.rs`)

1. **Tahap 1 — Validasi Fast-Fail (`pipeline_validation_logic.rs`):**
   - Memeriksa keberadaan berkas fisik dan membaca _magic bytes_ header (menolak berkas non-image sebelum menyentuh RAM).
   - Memeriksa batas aman piksel maksimal ($144\text{ MP}$ untuk baseline citra drone).
   - Melakukan deteksi instan apakah JPEG bertipe Progressive via header marker `SOF2` (`0xC2`). Jika Progressive $> 40\text{ MP}$, sistem langsung menolak tanpa mengalokasikan buffer piksel besar untuk mencegah _Out-of-Memory_ (OOM).

2. **Tahap 2 — Ekstraksi Georeferensi & Deteksi Bounding Box:**
   - **Pencarian Kandidat World File (`world_file_reader_io.rs`):** Mencari ekstensi pasangan seperti `.jgw`, `.jpgw`, `.pgw`, `.pngw`, `.tfw`, `.wld`.
   - **Parsing Parameter Afinitas (`world_file_parser_logic.rs`):** Membaca parameter affine 6-baris $(A, D, B, E, C, F)$ dengan _half-pixel shift_ ke tepi terluar citra.
   - **Toleransi Rotasi Drone:** Ambang batas rotasi dilonggarkan ke `1e-5` untuk mengantisipasi _roundoff error_ floating-point dari WebODM atau Pix4D.
   - **Graceful Fallback Manual Bounds:** Jika World File atau Sidecar JSON tidak ditemukan, rusak, atau gagal dibaca, pipeline **TIDAK CRASH**, melainkan otomatis beralih menggunakan koordinat `payload.manual_bounds` (jika disediakan user) dan mencatat _pipeline warning_.

3. **Tahap 3 — Isolasi Staging Guard RAII & Preflight Disk:**
   - Mengalokasikan direktori sementara di `.staging/<job_id>/` melalui struct RAII `StagingGuard`. Jika terjadi error atau pembatalan di tengah jalan, seluruh direktori staging otomatis dibersihkan saat guard keluar dari scope (_drop_).
   - Melakukan `check_disk_space_preflight` untuk memastikan ruang hard disk mencukupi sebelum memproses.
   - Jika `save_original: true`, citra asli disalin secara streaming ber-buffer via `stream_copy_with_cancellation`.

4. **Tahap 4 — Dekompresi & Resampling Hemat Memori (`image_resampling_logic.rs`):**
   - **Penanganan Progressive JPEG (Bypass IDCT Scale):** Pada berkas JPEG Progressive, pemanggilan `decoder.scale()` sengaja dilewati karena pustaka `jpeg-decoder` menolak scaling DCT bertahap (`ProgressiveWithScale`). Citra didekode penuh, lalu dialirkan baris-demi-baris ke `StreamingResampler` (algoritma _Area Averaging / Box Filter_).
   - **Penanganan Baseline JPEG (IDCT Scale):** Citra di-downscale langsung pada domain frekuensi DCT ke resolusi target terdekat (1/2, 1/4, atau 1/8) melalui `decode_jpeg_with_idct_scale` di `jpeg_decoder_wrapper_io.rs`. Ukuran buffer dikunci menggunakan dimensi aktual `(actual_w, actual_h)` hasil dekuantisasi untuk mencegah _buffer size mismatch_. Format Grayscale (`L8`) otomatis diekspansi ke 3-channel RGB.
   - **Normalisasi PNG Palette & 16-Bit:** Decoder PNG dikonfigurasi dengan transformasi in-memory `png::Transformations::normalize_to_color8()`. Citra bertipe `ColorType::Indexed` otomatis diubah ke RGB/RGBA 8-bit, dan citra sensor `BitDepth::Sixteen` otomatis di-_strip_ ke 8-bit tanpa ditolak.
   - **Streaming PNG Non-Interlaced:** Dibaca baris-demi-baris melalui `reader.next_row()`, mengonsumsi RAM hanya $< 50\text{ KB}$ per baris (bukan ratusan megabyte). Untuk PNG interlaced (Adam7), batas aman dibatasi $\le 25\text{ MP}$.

5. **Tahap 5 — Kompresi Adaptif Berkas Display (`adaptive_encoder_io.rs`):**
   - Memproduksi berkas `display.jpg` atau `display.png` berukuran ringan ($\le 5\text{--}10\text{ MB}$) agar Leaflet dapat merendernya secara instan di antarmuka web.
   - Jika citra memiliki transparansi (alpha channel), disimpan sebagai PNG dengan kompresi adaptif baris _banded_.
   - Jika citra solid (RGB), dieksekusi kompresi adaptif JPEG berjenjang (_Quality Ladder_: $q=85 \rightarrow 75 \rightarrow 65$) hingga batas ukuran berkas display terpenuhi.
   - Penulisan berkas dibungkus dalam `CancellableWriter` yang merespons `CancellationToken` pengguna secara instan.

6. **Tahap 6 — Finalisasi Atomik (Point of No Return):**
   - Menulis `metadata.json` (parameter teknis, EXIF drone, bounding box WGS84, CRS, dan daftar akumulasi warnings).
   - Menulis `meta.json` (`StoredImageMetadata`).
   - Melakukan pemindahan direktori secara atomik: `fs::rename(&staging_dir, &final_image_dir)`. Ini adalah _Point of No Return_; setelah tahap ini, data resmi tersimpan di `map_storage/images/<id>/`.
   - Menonaktifkan guard: `staging_guard.disarm()`.
   - Mendaftarkan metadata layer ke `map_storage/layers/<id>.json` via `register_ortho_layer`.
   - Mengubah status job menjadi `completed` (100%).

---

### B. Tahapan Eksekusi Pipeline Opsi B (`orchestrator_option_b.rs`)

1. **Validasi GeoTIFF Header & IFD (`geotiff_reader_io.rs`):**
   - Memeriksa _magic bytes_ TIFF: Little Endian (`II\x2a\x00`), Big Endian (`MM\x00\x2a`), atau BigTIFF 64-bit (`\x2b\x00`).
   - Membaca direktori tag IFD (Image File Directory) pertama tanpa meload data piksel.
2. **Ekstraksi Georeferensi Internal:**
   - Membaca tag `ModelPixelScaleTag` (33550), `ModelTiepointTag` (33922), atau `ModelTransformationTag` (34264).
   - Mengekstrak informasi proyeksi dari `GeoKeyDirectoryTag` (34735).
   - Menghitung koordinat WGS84 (EPSG:4326) melalui pure functions di `geospatial_projection_logic.rs`. Jika tag georeferensi GeoTIFF rusak, sistem menyediakan fallback ke `payload.manual_bounds`.
3. **Dekompresi Streaming Tile/Strip & Penanganan Kontras 16-Bit:**
   - Membaca data piksel raster per strip atau per tile secara streaming.
   - Citra sensor 16-bit diregangkan (_contrast stretch_) menggunakan pemetaan rentang persentil min-max (_2nd–98th percentile mapping_) ke ruang warna 8-bit standar.
   - Mendeteksi border hitam (_nodata black border_) secara otomatis dan mengonversinya menjadi transparansi (_straight alpha channel_) tanpa menimbulkan _dark fringe_ pada tepian kanvas.
4. **Resampling & Kompresi Adaptif:**
   - Dialirkan ke `StreamingResampler` untuk menghasilkan layer visualisasi display.
   - Disimpan via `adaptive_encode_display` dan difinalisasi secara atomik ke `map_storage/images/<id>/`.

---

## 3. Alur Pembuatan AOI & Geometri Spasial

Modul 03 memisahkan secara ketat pembuatan poligon Area of Interest (AOI) antara eksekusi matematika geodesik dan manipulasi disk:

```
                            ALUR PEMBUATAN AOI

   [User di UI Leaflet]
            │ Klik tombol "Simpan AOI"
            ▼
   [aoi_ipc_cmd.rs] ──> save_new_aoi_cmd(SaveNewAoiRequest)
            │
            ├─► 1. Cek Instance Lock: require_primary_instance()? (process_instance_lock_io.rs)
            │
            ├─► 2. Validasi Geometri Murni (Pure Function)
            │      └─► validate_polygon(&polygon)? (geometry_validation_logic.rs)
            │
            ├─► 3. Kalkulasi Geodesik WGS84 Murni (Pure Functions)
            │      ├─► calculate_area(&polygon) (spherical_excess_logic.rs)
            │      └─► calculate_perimeter(&polygon) (spherical_excess_logic.rs)
            │
            ├─► 4. Konstruksi Entitas Domain Murni (Pure Function)
            │      └─► create_aoi(...) -> AoiFeature (aoi_builder_logic.rs)
            │
            ├─► 5. Persistensi Atomik ke Disk (I/O)
            │      └─► save_aoi(aoi_feature) -> map_storage/aois/<id>.json (aoi_storage_io.rs)
            │
            ▼
   [AoiFeatureResponse] ──> Dikembalikan ke UI React via Tauri IPC Envelope
```

### Bukti Pure Functions (Bahan Penjelasan Akademis/Dosen)

Sebuah fungsi berstatus **Pure Function** jika memenuhi dua syarat mutlak:

1. **Deterministik:** Untuk input argumen yang sama, selalu menghasilkan nilai return yang identik.
2. **Zero Side-Effects:** Tidak melakukan pembacaan/penulisan file, tidak mengubah variabel global, tidak mengakses network, dan tidak bergantung pada state luar.

Berikut daftar _Pure Functions_ utama di Modul 03 yang dapat ditunjukkan kepada dosen:

| Nama Fungsi           | Berkas Sumber                                                                                                                                                              | Bukti Bebas Side-Effect & Logika Matematika                                                                                                           |
| :-------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `validate_coordinate` | [`geometry_validation_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/geometry_validation_logic.rs#L28) | Memeriksa koordinat bukan NaN/Infinite, longitude $[-180, 180]$, latitude $[-90, 90]$. Murni komparasi aritmatika.                                    |
| `validate_polygon`    | [`geometry_validation_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/geometry_validation_logic.rs#L64) | Memastikan poligon memiliki $\ge 4$ koordinat, cincin tertutup (_first == last_), dan bebas dari titik kolinear degenerasi.                           |
| `calculate_distance`  | [`haversine_distance_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/haversine_distance_logic.rs#L25)   | Implementasi rumus Haversine pada bola bumi WGS84 ($R = 6.371.000\text{ m}$) menggunakan fungsi trigonometri murni (`sin`, `cos`, `atan2`).           |
| `calculate_area`      | [`spherical_excess_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/spherical_excess_logic.rs#L20)       | Menggunakan Teorema Ekses Sferis Girard (_Girard's Spherical Excess Theorem_). Menghitung luas permukaan lengkung bumi tanpa proyeksi silindris semu. |
| `calculate_perimeter` | [`spherical_excess_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/spherical_excess_logic.rs#L61)       | Menjumlahkan jarak busur lingkaran besar (_great-circle distance_) antar-verteks poligon secara fungsional.                                           |
| `create_aoi`          | [`aoi_builder_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/aoi/aoi_builder_logic.rs#L12)                      | Merakit struct `AoiFeature` GeoJSON secara instan di RAM dari parameter yang dioper tanpa menyentuh file system.                                      |
| `reorder_layer_stack` | [`layer_state_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/layers/layer_state_logic.rs#L13)                   | Memanipulasi urutan _z-index_ stack layer peta menggunakan operasi murni array mutasi lokal.                                                          |

---

## 4. Peta Jalan Arsitektur Sistem (Markdown Flowchart)

Berikut adalah diagram alur komprehensif sistem Modul 03 yang memetakan keterkaitan antar-komponen mulai dari interaksi UI, perutean IPC, komputasi _Pure Logic_, orkestrasi pipeline, operasi _I/O_, hingga penyimpanan fisik dan _render_ Leaflet:

```text
========================================================================================================================
                                     PETA JALAN ARSITEKTUR SISTEM MODUL 03 (AAP)
========================================================================================================================

 [ 1. LAPISAN FRONTEND (React 19 / TypeScript) ]
   │
   ├── Komponen Kanvas & Tools : MapCanvas (aoiPane, orthoPane, overlayPane), AoiToolbar, MapSearchBar
   ├── Form & Dialog           : AoiModal, ImportImageModal (Tab Opsi A & Tab Opsi B), AnnotationModal
   └── IPC Client Service      : src/modules/module-03/services/api.ts
         │
         │ Invoke Tauri IPC (Membawa DTO Request CamelCase)
         ▼
 [ 2. PINTU GERBANG IPC TAURI (*_cmd.rs & *_struct.rs) ]
   │
   ├── aoi_ipc_cmd.rs          ──► create_aoi_cmd, save_new_aoi_cmd, get_saved_aois, delete_saved_aoi
   ├── geometry_ipc_cmd.rs     ──► measure_distance, measure_area, measure_perimeter, validate_polygon_cmd
   ├── ortho_import_ipc_cmd.rs ──► execute_import_option_a, execute_import_option_b, cancel_import_job
   └── layer_ipc_cmd.rs        ──► get_initial_map_layers, set_layer_visibility, reorder_layer_stack
         │
         ├─────────────────────────────────┬─────────────────────────────────┐
         │ Panggilan Validasi & Perhitungan│ Delegasi Tugas Impor Asinkron   │ Operasi I/O File Langsung
         ▼                                 ▼                                 ▼
 ┌───────────────────────────────┐ ┌───────────────────────────────┐ ┌───────────────────────────────┐
 │ 3. PILAR `_logic.rs`          │ │ 4. PIPELINE ORCHESTRATION     │ │ 5. PILAR `_io.rs`             │
 │    (Pure Math - No Side-Eff)  │ │    (Asynchronous Workers)     │ │    (Physical Access & Storage)│
 ├───────────────────────────────┤ ├───────────────────────────────┤ ├───────────────────────────────┤
 │ • geometry_validation_logic   │ │ • orchestrator_option_a.rs    │ │ • aoi_storage_io.rs           │
 │   - validate_coordinate()     │ │   - Alur Impor Citra Drone    │ │   - save_aoi() atomik         │
 │   - validate_polygon()        │ │     (JPG/PNG + World File /   │ │   - get_saved_aois()          │
 │                               │ │      Sidecar / Manual)        │ │   - delete_saved_aoi()        │
 │ • spherical_excess_logic      │ │                               │ │                               │
 │   - calculate_area() (Girard) │ │ • orchestrator_option_b.rs    │ │ • jpeg_decoder_wrapper_io.rs  │
 │   - calculate_perimeter()     │ │   - Alur Impor Citra Sensor   │ │   - decode IDCT downscale     │
 │                               │ │     (GeoTIFF tile/strip +     │ │   - bypass progressive scan   │
 │ • haversine_distance_logic    │ │      kontras 16-bit)          │ │                               │
 │   - calculate_distance()      │ │                               │ │ • geotiff_reader_io.rs        │
 │                               │ │ [Dipandu Modul Pendukung]:    │ │   - baca tag IFD biner        │
 │ • geospatial_projection_logic │ │   ├── pipeline_validation_... │ │   - dekompresi tile/strip     │
 │   - EPSG:4326 <-> 3857, UTM   │ │   ├── image_resampling_logic  │ │                               │
 │                               │ │   ├── world_file_parser_logic │ │ • adaptive_encoder_io.rs      │
 │ • aoi_builder_logic           │ │   └── adaptive_encoder_io     │ │   - kompresi display layer    │
 │   - create_aoi() GeoJSON      │ │                               │ │   - CancellableWriter guard   │
 │                               │ │                               │ │                               │
 │ • layer_state_logic           │ │                               │ │ • job_lifecycle_io.rs         │
 │   - reorder_layer_stack()     │ │                               │ │   - StagingGuard RAII         │
 │   - filter_layers_by_categ()  │ │                               │ │                               │
 │                               │ │                               │ │ • ortho_storage_io.rs         │
 │ • ortho_classification_logic  │ │                               │ │   - register_ortho_layer()    │
 │   - classify_image_ext()      │ │                               │ │   - tulis metadata teknis     │
 └───────────────┬───────────────┘ └───────────────┬───────────────┘ └───────────────┬───────────────┘
                 │                                 │                                 │
                 └─────────────────────────────────┼─────────────────────────────────┘
                                                   │
                                                   │ Tulis Atomik (Point of No Return)
                                                   ▼
 [ 6. PENYIMPANAN FISIK HARD DISK (map_storage/) ]
   │
   ├── map_storage/aois/<id>.json             ──► Poligon AOI GeoJSON tervalidasi
   ├── map_storage/images/<id>/original.*     ──► Salinan citra drone asli resolusi penuh
   ├── map_storage/images/<id>/display.jpg    ──► Citra display ringan (<= 5-10 MB) untuk web
   ├── map_storage/images/<id>/metadata.json  ──► Metadata teknis, EXIF, bounding box WGS84, warnings
   ├── map_storage/images/<id>/meta.json      ──► StoredImageMetadata untuk konsumsi aplikasi
   └── map_storage/layers/<id>.json           ──► Registrasi layer ortomosaik aktif
         │
         │ Dibaca & Dirender oleh Antarmuka
         ▼
 [ 7. VISUALISASI KANVAS PETA LEAFLET ]
   ├── tilePane    (z-index: 200) : Basemap OpenStreetMap / Satelit
   ├── orthoPane   (z-index: 250) : Layer Citra Ortomosaik (display.jpg / display.png)
   ├── aoiPane     (z-index: 350) : Poligon AOI (pointer-events: stroke selektif)
   └── overlayPane (z-index: 400) : Marker Foto Drone & Anotasi Lapangan
========================================================================================================================
```

### Detail Alur 1: Pipeline Impor Ortomosaik (Opsi A & B)

```text
[Input Citra Drone] (JPG / PNG / GeoTIFF)
       │
       ▼
[Tahap 1: Validasi Fast-Fail] (pipeline_validation_logic.rs)
       ├── Cek magic bytes berkas (bukan file korup / ekstensi palsu)
       ├── Cek batas dimensi (< 144 MP)
       └── Deteksi JPEG Progressive SOF2 (tolak instan jika > 40 MP untuk cegah OOM)
       │
       ▼
[Tahap 2: Deteksi Georeferensi]
       ├── Opsi A (world_file_parser_logic.rs):
       │     ├── Cari kandidat World File (.jgw/.pgw/.wld)
       │     ├── Parse parameter afinitas (toleransi rotasi 1e-5 untuk drone)
       │     └── FALLBACK: Jika World File korup/hilang, pakai payload.manual_bounds
       └── Opsi B (geotiff_reader_io.rs + geospatial_projection_logic.rs):
             ├── Ekstrak tag internal ModelPixelScale, ModelTiepoint, GeoKeyDirectory
             └── Reproyeksi koordinat piksel raster ke WGS84 EPSG:4326
       │
       ▼
[Tahap 3: Persiapan Staging RAII] (job_lifecycle_io.rs)
       ├── Buat direktori sementara .staging/<job_id>/ melalui StagingGuard
       ├── Preflight cek sisa ruang hard disk
       └── Salin streaming berkas asli jika save_original: true (dengan token pembatalan)
       │
       ▼
[Tahap 4: Decoding & Resampling Hemat Memori] (image_resampling_logic.rs)
       ├── Progressive JPEG : Bypass IDCT scale -> Decode penuh -> Stream ke StreamingResampler
       ├── Baseline JPEG    : IDCT downscale via jpeg_decoder_wrapper_io -> Ikat (actual_w, actual_h)
       ├── PNG Palette/16bit: Normalisasi in-memory via png::Transformations::normalize_to_color8()
       └── PNG Streaming    : Baca per baris via reader.next_row() (RAM < 50 KB per baris)
       │
       ▼
[Tahap 5: Kompresi Adaptif Display Layer] (adaptive_encoder_io.rs)
       ├── Evaluasi channel transparansi (alpha channel)
       ├── Jika transparan -> Kompresi PNG banded
       ├── Jika RGB solid  -> Loop JPEG Quality Ladder (q=85 -> q=75 -> q=65) hingga <= 5-10 MB
       └── Penulisan dikawal CancellableWriter (dapat dibatalkan seketika oleh user)
       │
       ▼
[Tahap 6: Finalisasi Atomik & Registrasi]
       ├── Tulis metadata.json (bounds WGS84, CRS, EXIF kamera, akumulasi warnings)
       ├── Tulis meta.json (StoredImageMetadata)
       ├── POINT OF NO RETURN: Atomic rename fs::rename(.staging/ -> map_storage/images/<id>/)
       ├── Nonaktifkan guard: staging_guard.disarm()
       ├── Daftarkan layer ke map_storage/layers/<id>.json
       └── Simpan JobRecord status: "completed" (100%)
```

### Detail Alur 2: Pembuatan & Simpan Area of Interest (AOI)

```text
[User di Peta Leaflet] ──► Menggambar poligon & klik tombol "Simpan AOI"
       │
       ▼
[Lapisan IPC] (aoi_ipc_cmd.rs::save_new_aoi_cmd)
       │
       ├──► 1. Cek Process Instance Lock (process_instance_lock_io.rs)
       │       Pastikan instance aplikasi pemanggil memegang primary lock.
       │
       ├──► 2. Validasi Geometri Murni (geometry_validation_logic.rs)
       │       [PURE FUNCTION] validate_polygon(&request.polygon)
       │       - Titik verteks >= 4
       │       - Cincin koordinat tertutup (titik awal == titik akhir)
       │       - Bebas dari nilai NaN / Infinite
       │
       ├──► 3. Hitung Luas Geodesik WGS84 (spherical_excess_logic.rs)
       │       [PURE FUNCTION] calculate_area(&polygon)
       │       - Teorema Ekses Sferis Girard pada bola bumi WGS84 (R = 6.371.000 m)
       │       - Menghasilkan luas akurat dalam meter persegi (m²)
       │
       ├──► 4. Hitung Keliling Geodesik WGS84 (spherical_excess_logic.rs)
       │       [PURE FUNCTION] calculate_perimeter(&polygon)
       │       - Penjumlahan jarak Haversine busur lingkaran besar antar-verteks
       │       - Menghasilkan keliling akurat dalam meter (m)
       │
       ├──► 5. Konstruksi Entitas Domain GeoJSON (aoi_builder_logic.rs)
       │       [PURE FUNCTION] create_aoi(...)
       │       - Merakit struct AoiFeature GeoJSON murni di RAM tanpa akses I/O
       │
       └──► 6. Persistensi Atomik ke Disk (aoi_storage_io.rs)
               [I/O OPERATION] aoi_storage_io::save_aoi(aoi_feature)
               - Tulis aman via berkas sementara + rename atomik
               - Lokasi: map_storage/aois/<id>.json
       │
       ▼
[Respon DTO Envelope] (AoiFeatureResponse { aoi }) ──► Dikembalikan ke UI React
```

---

## 5. Daftar Berkas & Tanggung Jawab (Tabel Ringkas Modul 03)

Berikut adalah daftar lengkap seluruh berkas di `src-tauri/src/modules/module_03/` setelah standardisasi 4 pilar arsitektur:

### A. Submodul AOI (`aoi/`)

| Nama Berkas                                                                                                                                       | Kategori  | Tanggung Jawab & Fungsi Utama                                                                                             |
| :------------------------------------------------------------------------------------------------------------------------------------------------ | :-------- | :------------------------------------------------------------------------------------------------------------------------ |
| [`aoi_ipc_cmd.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/aoi/aoi_ipc_cmd.rs)             | `_cmd`    | Endpoint Tauri command untuk AOI: `save_new_aoi_cmd`, `create_aoi_cmd`, `save_aoi`, `get_saved_aois`, `delete_saved_aoi`. |
| [`aoi_builder_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/aoi/aoi_builder_logic.rs) | `_logic`  | Pure function `create_aoi` untuk merakit GeoJSON `AoiFeature` di memori RAM.                                              |
| [`aoi_storage_io.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/aoi/aoi_storage_io.rs)       | `_io`     | Persistensi disk atomik: membaca dan menulis berkas `map_storage/aois/{id}.json`.                                         |
| [`aoi_ipc_struct.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/aoi/aoi_ipc_struct.rs)       | `_struct` | Definisi DTO Request/Response (Envelope Pattern) untuk operasi AOI.                                                       |
| [`aoi_entity_struct.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/aoi/aoi_entity_struct.rs) | `_struct` | Definisi entitas model domain: `AoiFeature`, `AoiProperties`, `Polygon`, `Coordinate`.                                    |

### B. Submodul Geometri (`geometry/`)

| Nama Berkas                                                                                                                                                                | Kategori  | Tanggung Jawab & Fungsi Utama                                                                                               |
| :------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :-------- | :-------------------------------------------------------------------------------------------------------------------------- |
| [`geometry_ipc_cmd.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/geometry_ipc_cmd.rs)                       | `_cmd`    | Endpoint Tauri command kalkulasi geometri: `measure_distance`, `measure_area`, `measure_perimeter`, `validate_polygon_cmd`. |
| [`geometry_validation_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/geometry_validation_logic.rs)     | `_logic`  | Pure functions validasi koordinat (`validate_coordinate`) dan poligon tertutup (`validate_polygon`).                        |
| [`spherical_excess_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/spherical_excess_logic.rs)           | `_logic`  | Pure functions komputasi luas geodesik (`calculate_area` via Girard's theorem) dan keliling (`calculate_perimeter`).        |
| [`haversine_distance_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/haversine_distance_logic.rs)       | `_logic`  | Pure function jarak lengkung busur bumi WGS84 (`calculate_distance` via Haversine).                                         |
| [`geospatial_projection_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/geospatial_projection_logic.rs) | `_logic`  | Pure functions transformasi proyeksi koordinat EPSG:4326, EPSG:3857, dan UTM Zone 1--60 N/S.                                |
| [`geometry_ipc_struct.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/geometry_ipc_struct.rs)                 | `_struct` | Definisi DTO Request/Response untuk pengukuran geodesik.                                                                    |

### C. Submodul Ingesti Ortomosaik (`ortho_source/` & `ortho_source/pipeline/`)

| Nama Berkas                                                                                                                                                                         | Kategori      | Tanggung Jawab & Fungsi Utama                                                                    |
| :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :------------ | :----------------------------------------------------------------------------------------------- |
| [`orchestrator_option_a.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/pipeline/orchestrator_option_a.rs)         | `Orkestrator` | Alur eksekusi asinkron impor Opsi A (JPEG/PNG + World File / Sidecar / Manual Fallback).         |
| [`orchestrator_option_b.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/pipeline/orchestrator_option_b.rs)         | `Orkestrator` | Alur eksekusi asinkron impor Opsi B (GeoTIFF tile/strip streaming + kontras 16-bit).             |
| [`pipeline_validation_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/pipeline/pipeline_validation_logic.rs) | `_logic`      | Validasi fast-fail berkas citra, cek dimensi, cek batas WGS84, dan preflight ruang disk.         |
| [`image_resampling_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/pipeline/image_resampling_logic.rs)       | `_logic`      | Decoding PNG streaming per baris, normalisasi palette 16-bit, dan pure box-filter resampling.    |
| [`adaptive_encoder_io.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/pipeline/adaptive_encoder_io.rs)             | `_io`         | Kompresi adaptif display layer (JPEG ladder & PNG banded) dengan pembungkus `CancellableWriter`. |
| [`jpeg_decoder_wrapper_io.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/jpeg_decoder_wrapper_io.rs)              | `_io`         | Wrapper decoder JPEG dengan dekuantisasi skala IDCT, isolasi progressive scan, dan ekspansi L8.  |
| [`geotiff_reader_io.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/geotiff_reader_io.rs)                          | `_io`         | Pembaca tag GeoTIFF (ModelTiepoint, PixelScale, GeoKey) dan dekompresi raster berjenjang.        |
| [`world_file_parser_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/world_file_parser_logic.rs)              | `_logic`      | Parsing affine 6-parameter World File dengan toleransi rotasi longgar `1e-5`.                    |
| [`streaming_resampler_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/streaming_resampler_logic.rs)          | `_logic`      | Engine resampling streaming per baris dengan proteksi batas memori ketat dan straight alpha.     |
| [`process_instance_lock_io.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/process_instance_lock_io.rs)            | `_io`         | File-locking OS (`app.lock`) untuk mencegah konkurensi mutasi antar-proses desktop.              |
| [`job_lifecycle_io.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/job_lifecycle_io.rs)                            | `_io`         | Struct RAII `StagingGuard` untuk pembersihan otomatis staging folder dan manajemen status job.   |
| [`ortho_storage_io.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/ortho_storage_io.rs)                            | `_io`         | Registrasi atomik layer citra ortomosaik ke `map_storage/layers/{id}.json`.                      |
| [`ortho_import_ipc_cmd.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/ortho_import_ipc_cmd.rs)                    | `_cmd`        | Endpoint Tauri commands untuk ingesti ortomosaik, pembatalan job, dan kueri status.              |
| [`ortho_import_ipc_struct.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/ortho_import_ipc_struct.rs)              | `_struct`     | Definisi DTO Request/Response impor citra Opsi A dan Opsi B.                                     |
| [`ortho_metadata_struct.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/ortho_source/ortho_metadata_struct.rs)                  | `_struct`     | Definisi model metadata ortomosaik (`StoredImageMetadata`, `TechnicalMetadata`).                 |

### D. Submodul Anotasi & Layer State (`annotations/` & `layers/`)

| Nama Berkas                                                                                                                                                             | Kategori | Tanggung Jawab & Fungsi Utama                                                               |
| :---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :------- | :------------------------------------------------------------------------------------------ |
| [`annotation_ipc_cmd.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/annotations/annotation_ipc_cmd.rs)             | `_cmd`   | Endpoint Tauri commands operasi anotasi spasial (simpan, list, hapus).                      |
| [`annotation_builder_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/annotations/annotation_builder_logic.rs) | `_logic` | Pure function `create_annotation` untuk merakit entitas anotasi GeoJSON.                    |
| [`annotation_storage_io.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/annotations/annotation_storage_io.rs)       | `_io`    | Penyimpanan berkas anotasi atomik di `map_storage/annotations/{id}.json`.                   |
| [`layer_ipc_cmd.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/layers/layer_ipc_cmd.rs)                            | `_cmd`   | Endpoint Tauri commands manajemen layer peta (reorder, opacity, visibility).                |
| [`layer_state_logic.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/layers/layer_state_logic.rs)                    | `_logic` | Pure functions pengaturan state susunan layer (`reorder_layer_stack`, `set_layer_opacity`). |
| [`layer_storage_io.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/layers/layer_storage_io.rs)                      | `_io`    | Membaca dan menulis konfigurasi state layer ke disk.                                        |

---

## 6. Ringkasan Kepatuhan Standar Kualitas

Seluruh berkas yang terdaftar di atas telah lulus gerbang pengujian kualitas (_quality gates_):

1. **Zero Warnings pada Clippy:** `npm run lint:rust` (`cargo clippy --all-targets -D warnings`) bersih 100%.
2. **Kerapian Format:** `npm run format:rust:check` (`cargo fmt --check`) sinkron dengan panduan gaya repositori.
3. **Pengujian Fungsional:** 41 unit tests di Modul 03 dan 464 unit tests di seluruh workspace Rust berhasil lulus 100%.
4. **Backward Compatibility:** Lapisan fasad (`image_pipeline.rs`) tetap dipertahankan agar tidak merusak modul turunan lain yang mengonsumsi endpoint internal.
   7
