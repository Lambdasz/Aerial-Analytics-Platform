# Rencana Implementasi (Revisi): Ganti Input Citra TIFF Menjadi JPG/PNG + Koordinat Area (Module 3)

> **PANDUAN EKSEKUSI (TAHAP 0)**: Dokumen ini disusun sebagai kontrak teknis resmi revisi arsitektur input citra Module 3 (Aerial Image Map Explorer) sesuai arahan pengguna. **TIDAK ADA KODE YANG DIUBAH** sampai user memberikan konfirmasi tertulis bertuliskan **"SETUJU"**.

---

## 1. Konteks & Ringkasan Perubahan Arsitektur

Sebelumnya, input citra orthomosaic pada Module 3 mensyaratkan format TIFF / GeoTIFF dan menggunakan library pembantu eksternal (termasuk Python/rasterio/pyproj).
Fitur tersebut sekarang **DIREVISI TOTAL**:

1. **Format Input Baru**: Hanya menerima berkas `.jpg`, `.jpeg`, dan `.png`. Format TIFF tidak lagi didukung.
2. **Koordinat Area Wajib**: Karena JPG/PNG tidak memiliki metadata georeferensi internal bawaan, pengguna wajib memasukkan koordinat bounding box (WGS84) sebelum citra dapat diimpor ke peta.
3. **Batas Ukuran & Dimensi**:
   - Ukuran file maksimal: **50 MB** (`52.428.800 bytes`).
   - Dimensi gambar maksimal: **16.384 piksel** per sisi.
   - Validasi dilakukan sebelum citra dimuat penuh ke memori (fast-fail via metadata & header).
4. **Pembersihan Total Library & Skrip**:
   - Menghapus dependensi `tiff = "0.11"` dari `src-tauri/Cargo.toml`.
   - Menghapus skrip `scripts/import_geotiff.py` dan virtual environment `.venv_geo`.
   - Menghapus seluruh pemanggilan subprocess Python pada Module 3.
   - Tidak ada library baru yang ditambahkan: cukup menggunakan library bawaan Tauri, Browser Canvas/File API, dan crate `image` yang sudah terpasang.
5. **Penyimpanan Atomic Tunggal**:
   - Citra dan metadata disimpan di `/map_storage/images/{id}/` dengan prinsip 1 berkas JSON atomik (`meta.json` <= 64 KB).
   - Sinkronisasi tunggal ke direktori root `map_storage` tanpa folder bayangan.

---

## 2. Audit Lengkap Dependensi & Lingkungan yang Dihapus

### 2.1 Dependensi Rust (`src-tauri/Cargo.toml`)

| Nama Paket             | Versi  | Status            | Alasan Tindakan                                                                                                                                                            |
| :--------------------- | :----- | :---------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tiff`                 | `0.11` | **DIHAPUS**       | Hanya digunakan untuk membaca IFD dan GeoKey tag TIFF. Karena TIFF tidak lagi didukung, crate ini tidak diperlukan.                                                        |
| `image`                | `0.25` | **DIPERTAHANKAN** | Digunakan untuk membaca dimensi header JPEG/PNG (`image::image_dimensions`) secara instan tanpa decode memori penuh, serta downsampling pratinjau thumbnail `display.jpg`. |
| `serde` / `serde_json` | `1`    | **DIPERTAHANKAN** | Digunakan untuk serialisasi/deserialisasi metadata JSON atomik.                                                                                                            |

### 2.2 Dependensi JavaScript (`package.json`)

- **Hasil Audit**: Bersih. Tidak ada library TIFF (seperti `geotiff.js` atau `utif`) yang terpasang di `package.json`.
- Tidak ada penambahan dependency baru di `package.json`.

### 2.3 Lingkungan Python & Skrip

| File / Direktori                 | Status            | Alasan Tindakan                                                                                           |
| :------------------------------- | :---------------- | :-------------------------------------------------------------------------------------------------------- |
| `scripts/import_geotiff.py`      | **DIHAPUS**       | Skrip pembantu GeoTIFF (rasterio/pyproj) yang digunakan untuk reproyeksi UTM.                             |
| `.venv_geo/`                     | **DIHAPUS TOTAL** | Virtualenv lokal yang memuat rasterio, GDAL, pyproj, Pillow, numpy, dan tifffile.                         |
| `pipeline.rs` (pemanggil Python) | **DIHAPUS**       | Seluruh pemanggilan `std::process::Command` yang mencari `python.exe` atau `.venv_geo` dieliminasi total. |

### 2.4 Temuan & Isolasi Modul Lain

- **Temuan**: Folder `plugins/rgb-vegetation-detection` dan `plugins/rgb-landcover-classification` di root proyek adalah plugin analitik untuk Module 2 (Plugin Manager).
- **Aturan**: Modul lain **TIDAK DISENTUH** sama sekali. Hanya Module 3 yang dibersihkan dari Python.

---

## 3. Desain Batas Ukuran & Validasi File (Fast-Fail)

### 3.1 Konstanta Tunggal Terpusat

Konstanta didefinisikan di satu tempat agar mudah diubah di masa depan:

- **Rust Backend** (`src-tauri/src/modules/module_03/ortho_source/constants.rs`):
  ```rust
  pub const MAX_IMAGE_FILE_SIZE_BYTES: u64 = 50 * 1024 * 1024; // 50 MB (52.428.800 bytes)
  pub const MAX_IMAGE_DIMENSION_PX: u32 = 16_384;              // 16.384 piksel per sisi
  pub const PREVIEW_TARGET_DIMENSION_PX: u32 = 2_048;          // 2048 px untuk thumbnail cepat
  ```
- **Frontend** (`src/modules/module-03/config/mapConfig.ts`):
  ```typescript
  export const MAX_IMAGE_FILE_SIZE_BYTES = 50 * 1024 * 1024; // 50 MB
  export const MAX_IMAGE_DIMENSION_PX = 16384;
  ```

### 3.2 Alur Validasi Cepat (Sebelum Decode Memori)

1. **Cek Ukuran Berkas**:
   - Membaca metadata berkas via `std::fs::metadata(path)?.len()`.
   - Jika `size > 52_428_800 bytes`: Batalkan dan kembalikan pesan error:
     _"Ukuran file {size_mb} MB melebihi batas maksimal 50 MB."_
2. **Cek Magic Bytes (Bukan Sekadar Ekstensi)**:
   - Membaca 8 byte pertama berkas:
     - **JPEG**: Diawali `[0xFF, 0xD8, 0xFF]`
     - **PNG**: Diawali `[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]`
   - Jika magic bytes tidak cocok dengan JPEG atau PNG: Batalkan dan kembalikan pesan error:
     _"Format berkas tidak valid. Hanya berkas JPG dan PNG asli yang diperbolehkan."_
3. **Cek Dimensi dari Header Citra**:
   - Memanggil `image::image_dimensions(path)` yang hanya membaca header file tanpa mengalokasikan array piksel di RAM.
   - Jika `width > 16384` atau `height > 16384`: Batalkan dan kembalikan pesan error:
     _"Dimensi citra {width}x{height} piksel melebihi batas maksimal 16.384 piksel per sisi."_

---

## 4. Desain Form Koordinat Area & Validasi UI (Blueprint.js)

### 4.1 Dialog Impor Citra Orthomosaic

Ketika pengguna menekan tombol **"Impor"** di [ImageGallerySidebar.tsx](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/ImageGallerySidebar.tsx), dialog modal Blueprint.js (`<Dialog title="Impor Citra Orthomosaic (JPG / PNG)" isOpen={...}>`) akan terbuka:

1. **Bagian 1: Pemilihan Berkas**:
   - Tombol _"Pilih Berkas Citra"_ menggunakan `@tauri-apps/plugin-dialog` dengan filter ekstensi `["jpg", "jpeg", "png"]`.
   - Menampilkan kartu info berkas: Nama file, format terverifikasi, ukuran (MB), dan dimensi piksel ($W \times H$).
2. **Bagian 2: Input Bounding Box WGS84**:
   - 4 field input angka menggunakan Blueprint.js `<NumericInput>` dengan `<FormGroup>`:
     - `West (Bujur Barat / Lon Min)`: derajat desimal (contoh: `116.856000`)
     - `South (Lintang Selatan / Lat Min)`: derajat desimal (contoh: `-1.225000`)
     - `East (Bujur Timur / Lon Max)`: derajat desimal (contoh: `116.865000`)
     - `North (Lintang Utara / Lat Max)`: derajat desimal (contoh: `-1.218000`)
   - **Shortcut**: Tombol _"Gunakan Viewport Peta Saat Ini"_, otomatis mengisi koordinat sesuai bounding box tampilan Leaflet yang sedang dilihat pengguna.
3. **Bagian 3: Validasi Form Per-Field**:
   - **Range Lintang**: $-90.0 \le south \le 90.0$ dan $-90.0 \le north \le 90.0$. Jika melanggar: `intent="danger"`, helper text _"Lintang harus berada dalam rentang -90 hingga 90 derajat."_
   - **Range Bujur**: $-180.0 \le west \le 180.0$ dan $-180.0 \le east \le 180.0$. Jika melanggar: `intent="danger"`, helper text _"Bujur harus berada dalam rentang -180 hingga 180 derajat."_
   - **Urutan Relatif**:
     - Jika $south \ge north$: Field South & North diberi `intent="danger"`, helper text _"South (lat min) harus lebih kecil dari North (lat max)."_
     - Jika $west \ge east$: Field West & East diberi `intent="danger"`, helper text _"West (lon min) harus lebih kecil dari East (lon max)."_
4. **Peringatan Rasio Aspek (Aspect Ratio Warning)**:
   - Menghitung rasio aspek piksel citra: $R_{px} = \frac{W}{H}$.
   - Menghitung rasio aspek koordinat geografis dengan koreksi proyeksi cosinus lintang tengah:
     $$lat_{mid} = \frac{south + north}{2}$$
     $$\Delta lon = (east - west) \times \cos(lat_{mid} \times \frac{\pi}{180})$$
     $$\Delta lat = (north - south)$$
     $$R_{geo} = \frac{\Delta lon}{\Delta lat}$$
   - Jika selisih relatif $| \frac{R_{geo}}{R_{px}} - 1 | > 0.35$ (berbeda lebih dari 35%):
     Tampilkan `<Callout intent="warning" icon="warning-sign">`:
     _"Peringatan: Rasio aspek bounding box berbeda signifikan ({selisih}%) dari proporsi piksel citra asli. Citra dapat terlihat terdistorsi di peta."_
5. **Keadaan Tombol "Impor"**:
   - Tombol **DISABLED** (`disabled={!isValid || isSubmitting}`) jika ada salah satu koordinat kosong/NaN, atau melanggar aturan range dan urutan.
   - Tombol **AKTIF** hanya saat berkas lolos validasi ukuran/dimensi dan keempat koordinat valid.

---

## 5. Skema JSON Penyimpanan di `/map_storage/images/`

Setiap citra orthomosaic yang diimpor akan disimpan di direktori terisolasi:
`map_storage/images/{image_id}/`

### 5.1 Struktur Berkas per Citra:

```text
map_storage/images/{image_id}/
├── original.jpg (atau original.png)   # Berkas citra asli
├── display.jpg                        # Versi pratinjau downsample (max 2048 px) untuk render ringan
└── meta.json                          # Metadata lengkap entitas citra (<= 64 KB)
```

### 5.2 Skema `meta.json` (Atomic Entity):

```json
{
  "schema_version": 1,
  "id": "ortho_1792789123456",
  "filename": "orthomosaic_kebun_a.jpg",
  "category": "ortho",
  "extension": "jpg",
  "file_path": "map_storage/images/ortho_1792789123456/original.jpg",
  "size_bytes": 18452300,
  "dimensions": [4096, 3072],
  "bounds": [
    [-1.225, 116.856],
    [-1.218, 116.865]
  ],
  "aspect_ratio": 1.3333,
  "imported_at": "2026-10-24T04:45:00.000Z"
}
```

### 5.3 Pendaftaran Otomatis Layer di `map_storage/layers/`:

Secara otomatis sebuah entitas layer didaftarkan di `map_storage/layers/{image_id}.json`:

```json
{
  "layer_id": "ortho_1792789123456",
  "layer_name": "orthomosaic_kebun_a.jpg",
  "category": "orthomosaic",
  "is_visible": true,
  "opacity": 1.0,
  "z_index": 5,
  "source_type": "image_overlay",
  "source_id": "ortho_1792789123456"
}
```

---

## 6. Daftar Berkas yang Akan Diubah dan Dihapus

### 6.1 Berkas yang Akan Dihapus (Delete)

1. `scripts/import_geotiff.py`
2. `.venv_geo/` (folder environment lokal)
3. `src-tauri/src/modules/module_03/ortho_source/pipeline.rs` (pipeline TIFF lama)
4. `src-tauri/tests/module_03_tiff_pipeline_test.rs`
5. Data TIFF lama di `map_storage/images/` dan `map_storage/layers/`

### 6.2 Berkas Backend yang Akan Dibuat / Diubah (Modify)

1. `src-tauri/Cargo.toml` (Hapus dependensi `tiff = "0.11"`)
2. `src-tauri/Cargo.lock` (Otomatis dibersihkan via `cargo check`)
3. `src-tauri/src/modules/module_03/ortho_source/constants.rs` (Baru: konstanta batas ukuran 50 MB, dimensi 16.384 px)
4. `src-tauri/src/modules/module_03/ortho_source/types.rs` (Update `StoredImageMetadata`, buat `ImportImagePayload`)
5. `src-tauri/src/modules/module_03/ortho_source/image_pipeline.rs` (Baru: validasi magic bytes, file size, image dimensions, downsample preview)
6. `src-tauri/src/modules/module_03/ortho_source/commands.rs` (Ganti `import_tiff` dengan `import_ortho_image`)
7. `src-tauri/src/modules/module_03/ortho_source/store.rs` (Bersihkan alur TIFF, simpan JPG/PNG + meta.json)
8. `src-tauri/src/modules/module_03/ortho_source/mod.rs` (Ekspor modul pipeline baru)
9. `src-tauri/src/modules/module_03/storage_commands.rs` (Re-export command baru)
10. `src-tauri/src/lib.rs` (Registrasi command `import_ortho_image` menggantikan `import_tiff`)
11. `src-tauri/capabilities/default.json` (Perbarui permission command `import_ortho_image`)
12. `src-tauri/tests/module_03_image_pipeline_test.rs` (Baru: pengujian validasi magic bytes, batas ukuran 50 MB, batas dimensi 16.384 px, koordinat WGS84)

### 6.3 Berkas Frontend yang Akan Diubah (Modify)

1. `src/modules/module-03/config/mapConfig.ts` (Tambahkan konstanta 50 MB & 16.384 px)
2. `src/modules/module-03/types/storage.ts` (Perbarui tipe `StoredImageMetadata` & `ImportImageParams`)
3. `src/modules/module-03/services/api.ts` (Ganti `importTiff` dengan `importOrthoImage`)
4. `src/modules/module-03/organisms/ImageGallerySidebar.tsx` (Tambahkan modal dialog form koordinat Blueprint.js, validasi per-field, disabled import button)
5. `src/modules/module-03/molecules/ImportImageModal.tsx` (Baru: komponen modal form koordinat modular & murni)
6. `src/modules/module-03/lib/leaflet/spatialLayerBuilder.ts` (Hapus fallback `.tif` -> `.jpg`, dukung overlay JPG/PNG langsung)
7. `src/modules/module-03/organisms/ManagedMapLayers.tsx` (Render layer overlay orthomosaic JPG/PNG pada peta)

---

## 7. Tahapan Eksekusi Bertahap (Setelah Persetujuan)

- [x] **Tahap 1: Pembersihan Dependensi & Lingkungan (SELESAI)**
  - Hapus `scripts/import_geotiff.py` dan `.venv_geo`.
  - Hapus dependensi `tiff = "0.11"` dari `Cargo.toml`.
  - Bersihkan `Cargo.lock` dengan `cargo check`.
  - Bersihkan data citra TIFF sisa di `map_storage/` dan `public/`.
- [x] **Tahap 2: Backend Rust Pipeline JPG/PNG + Batas Ukuran (SELESAI)**
  - Buat `constants.rs` (50 MB, 16.384 px).
  - Implementasikan `image_pipeline.rs`: validasi magic bytes, batas ukuran (50 MB), dimensi header (16.384 px), simpan `meta.json` atomik dan pratinjau `display.jpg`.
  - Implementasikan command `import_ortho_image` dan update `lib.rs` + `default.json`.
  - Tulis unit test di `module_03_image_pipeline_test.rs` (4/4 tes lulus).
- [x] **Tahap 3: Frontend Modal Dialog Form Koordinat Blueprint.js (SELESAI)**
  - Buat `ImportImageModal.tsx` di `organisms/` dengan field West, South, East, North + validasi per-field dan warning rasio aspek (>35%).
  - Hubungkan ke `ImageGallerySidebar.tsx` dan `api.ts`.
  - Implementasikan auto-zoom (`map.flyToBounds(bounds)`) dan auto-render setelah import berhasil.
- [x] **Tahap 4: Verifikasi & Quality Gates (SELESAI)**
  - `cargo test --test module_03_image_pipeline_test`: 4 passed; 0 failed.
  - `cargo test --test storage_test`: 3 passed; 0 failed.
  - `cargo test --test module_03_storage_crud_test`: 7 passed; 0 failed.
  - `npm run lint`: 0 error, 0 warning.
  - `npm run lint:rust`: 0 error, 0 warning.
  - `npm run format:check`: All files match Prettier.
  - `npm run format:rust:check`: All Rust files formatted.
  - `npm run build`: tsc typecheck + vite build sukses tanpa error.

---

## 9. Revisi Performa & Seleksi Tampilan Citra: Batas Display <= 5 MB, Culling Viewport, & Zoom Threshold >= 16z

### 9.1 Ringkasan Kebutuhan Baru

1. **Tidak Muncul Serempak / Sekaligus**: Citra orthomosaic tidak dimuat atau dirender sekaligus ke peta Leaflet saat peta terbuka atau berada di kejauhan.
2. **Syarat Ukuran Display <= 5 MB**: Berkas pratinjau display (`display.jpg` / `display.png`) yang dirender ke peta wajib memiliki ukuran fisik **maksimal 5 MB** (`5.242.880 bytes`). Berkas yang melebihi 5 MB tidak boleh dimuat ke DOM peta.
3. **Spatial Culling Berbasis Viewport**: Jika pengguna sedang berada jauh dari cakupan area citra, foto tidak akan di-load ke DOM Leaflet. Foto hanya di-load ketika _bounding box_ citra bersilangan (_intersect_) dengan batas pandangan peta saat ini (`map.getBounds().intersects(img.bounds)`).
4. **Zoom Level Threshold >= 16z**: Foto orthomosaic drone hanya muncul pada level ketinggian zoom **16 ke atas** (`zoom >= 16`). Jika level zoom berada di bawah 16 (`zoom < 16`), dipastikan seluruh foto orthomosaic tidak muncul di peta.

---

### 9.2 Rencana Perubahan Teknis

#### A. Backend Rust (`src-tauri/src/modules/module_03/`)

1. **Konstanta Ukuran Display** (`ortho_source/constants.rs`):
   - Tambahkan `pub const MAX_DISPLAY_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024; // 5 MB`
2. **Pipeline Pratinjau Terjamin <= 5 MB** (`ortho_source/image_pipeline.rs`):
   - Perbaiki `generate_display_preview`: Jika citra asli berdimensi besar (misal 13.000+ px), lakukan downsampling secara bertahap (target 2048 px, kompresi JPEG kualitas ~80-85%).
   - Verifikasi ukuran berkas `display.jpg`: Pastikan ukurannya secara ketat $\le 5\text{ MB}$.
   - Simpan metadata `display_size_bytes` pada `StoredImageMetadata` agar frontend dapat memvalidasi sebelum memuat.
3. **Normalisasi Berkas Display yang Sudah Ada**:
   - Downsample `display.jpg` pada folder `map_storage/images/` yang masih berukuran 47.5 MB menjadi berkas kompresi web valid $\le 5\text{ MB}$.

#### B. Frontend React / Leaflet (`src/modules/module-03/`)

1. **Konstanta Zoom & Batas Display** (`config/mapConfig.ts`):
   - Tambahkan `export const MIN_ORTHO_ZOOM = 16;`
   - Tambahkan `export const MAX_DISPLAY_FILE_SIZE_BYTES = 5 * 1024 * 1024;`
2. **Hook / Seleksi Tampilan Citra** (`organisms/SavedOrthoImageLayer.tsx`):
   - **Ketinggian / Zoom**: Gunakan listener `zoomend` atau `useMapZoom()`. Jika `currentZoom < MIN_ORTHO_ZOOM`, kosongkan layer group citra.
   - **Spatial Culling**: Gunakan listener `moveend`. Ambil `const viewBounds = map.getBounds();`.
     Hanya masukkan citra yang memenuhi:
     ```typescript
     const isIntersecting = viewBounds.intersects(L.latLngBounds(img.bounds));
     const isSizeValid =
       !img.display_size_bytes || img.display_size_bytes <= MAX_DISPLAY_FILE_SIZE_BYTES;
     ```
   - **Pencegahan Beban Bersamaan**: Hanya render citra yang aktif di viewport dan memenuhi syarat ukuran $\le 5\text{ MB}$. Saat pengguna menggeser peta menjauh atau melakukan zoom-out di bawah 16z, overlay citra di luar layar otomatis dilepas (_detached_) dari peta untuk membebaskan memori.
3. **Pemberitahuan UI di Galeri** (`organisms/ImageGallerySidebar.tsx`):
   - Berikan badge / tooltip informatif pada kartu galeri jika ukuran display preview tidak valid atau memberikan petunjuk zoom minimal 16z.

---

### 9.3 Rencana Verifikasi & Quality Gates

1. Unit test Rust untuk validasi `MAX_DISPLAY_FILE_SIZE_BYTES` dan fungsi downsampling preview.
2. Pengujian interaktif browser:
   - Pada zoom 15: Verifikasi citra orthomosaic tidak ada di DOM (`.ortho-image-overlay` kosong).
   - Pada zoom 16+ saat viewport berada di Kariangau: Citra orthomosaic termuat.
   - Saat peta digeser jauh (misal ke Samarinda): Citra Kariangau otomatis di-detach dari peta.
3. Pre-commit gates: `npm run lint`, `npm run lint:rust`, `npm run format:check`, `npm run format:rust:check` wajib lolos.

---

## 10. Fitur Baru: CRUD Lengkap Foto Orthomosaic (Module 3)

> **PANDUAN EKSEKUSI TAHAP CRUD**: Dokumen rencana ini memetakan seluruh operasi Create, Read, Update, dan Delete untuk citra orthomosaic Module 3. **TIDAK ADA KODE PRODUKSI YANG DIUBAH** sampai user memberikan konfirmasi bertuliskan **"SETUJU"**.

---

### 10.1 Pola yang Ditemukan di Module 3 & yang Akan Diikuti

1. **Pola Backend Rust**:
   - **Struktur Folder**: Terisolasi di `src-tauri/src/modules/module_03/ortho_source/` (`constants.rs`, `types.rs`, `image_pipeline.rs`, `store.rs`, `commands.rs`, `mod.rs`).
   - **Pemisahan Logika Murni & Efek Samping (Functional Thinking)**:
     - Logika murni: fungsi validasi ID allowlist regex (`validate_ortho_id`), validasi WGS84 (`validate_wgs84_bounds`), kalkulasi rasio aspek, sanitasi input.
     - Efek samping: operasi I/O filesystem terisolasi di `store.rs` dengan penulisan atomik (`write_atomic` via berkas `.tmp` lalu rename) dan sinkronisasi ke in-memory cache `Module03State`.
   - **Validasi ID Ketat (Allowlist Regex & Path Traversal Prevention)**:
     - Validasi ID menggunakan allowlist regex resmi: `^ortho_[0-9]+$`. ID di luar pola ini (misalnya yang mengandung `..`, `/`, `\`, spasi, atau karakter selain `ortho_` + digit angka) langsung ditolak pada tahap validasi murni.
     - Selain regex, canonical path resolution memastikan seluruh akses berkas terkunci rapat di dalam direktori `/map_storage/images/{id}/`.
   - **Penanganan Metadata Rusak (Corrupt `meta.json`)**:
     - Direktori citra dengan berkas `meta.json` korup atau tidak terbaca **TIDAK DI-SKIP / DIABAIKAN BEGITU SAJA**. Item tetap dimuat ke daftar dengan flag `is_corrupted: true` dan label tampilan `"Metadata rusak"`.
     - Pengguna dapat menghapus item rusak ini satu per satu secara tuntas melalui UI galeri untuk membersihkan ruang penyimpanan.
   - **Mekanisme Hapus Tuntas Dua Fase dengan Rollback Transaksional**:
     - Fase 1: Rename folder citra aktif `/map_storage/images/{id}` menjadi folder karantina sementara `/map_storage/images/.trash_{id}_{timestamp}`.
     - Fase 2: Hapus entitas layer terkait (`/map_storage/layers/{id}.json`) dan hapus anotasi yang terikat pada citra ini (`image_id == id`).
     - Fase 3: Jika penghapusan data turunan berhasil, hapus folder karantina sementara secara permanen (`fs::remove_dir_all`).
     - **Rollback**: Jika terjadi kegagalan pada Fase 2 (misal penghapusan layer/anotasi gagal), folder karantina sementara otomatis di-rename kembali ke nama aslinya (`/map_storage/images/{id}`) dan transaksi dibatalkan dengan mengembalikan error transaksional. Tidak ada data yang tertinggal dalam status setengah terhapus.
   - **Penanganan Error Terpusat**:
     - Menggunakan `MapControllerError` (`NotFound`, `InvalidGeometry`, `StorageError`, `TransactionError`) yang dipetakan ke `CommandError::ExecutionFailed`, `CommandError::ValidationFailed`, atau `CommandError::NotFound`.
   - **Registrasi Command**:
     - Didaftarkan pada `src-tauri/src/lib.rs` di dalam macro `tauri::generate_handler![]` melalui modul `map_controller::ortho_source::commands`.
   - **Testing Mandiri**:
     - Unit test di `src-tauri/tests/module_03_ortho_crud_test.rs` menguji seluruh skenario secara deterministik (list terurut, item korup, update immutable fields, delete cascade, delete ID tidak ada, rollback kegagalan di tengah jalan, dan penolakan ID berbahaya).

2. **Pola Frontend React**:
   - **Atomic Design Hierarchy**:
     - `molecules`: Komponen reusable mandiri:
       - `OrthoCardItem.tsx`: Kartu pratinjau citra, detail ukuran, status badge ("Metadata rusak" jika korup), tombol fokus, tombol edit, dan tombol hapus individual.
       - `EditOrthoModal.tsx`: Form dialog Blueprint.js untuk mengedit nama citra dan koordinat bounding box WGS84 dengan validasi murni yang sama persis dengan impor.
       - `DeleteOrthoAlert.tsx`: Dialog konfirmasi bahaya Blueprint.js (`Alert` intent="danger") yang menampilkan nama citra dan jumlah anotasi terikat.
     - `organisms`: Kontainer pengelola:
       - `ImageGallerySidebar.tsx`: Mengelola daftar citra, integrasi modal, pemanggilan command hitung anotasi saat dialog delete dibuka, dan rendering toast.
       - `SavedOrthoImageLayer.tsx`: Komponen reaktif Leaflet yang mendengarkan event global `ortho_image:saved` dan `ortho_image:deleted` untuk menyinkronkan overlay di peta secara langsung.
   - **Blueprint.js UI System & Standar Toast**:
     - Menggunakan komponen resmi Blueprint.js: `Alert`, `Dialog`, `FormGroup`, `InputGroup`, `NumericInput`, `Button`, `Spinner`, `Tag`, `NonIdealState`, dan `OverlayToaster`.
     - Aturan Toast:
       - Sukses Hapus: `intent="success"` (`"Citra '{filename}' beserta layer terkait berhasil dihapus."`)
       - Sukses Update: `intent="success"` (`"Metadata citra '{filename}' berhasil diperbarui."`)
       - Gagal / Error: `intent="danger"` (`"Gagal memproses aksi: {errorMessage}"`)
   - **Hitung Anotasi Real-Time Berdasarkan ID Citra**:
     - Jumlah `{count}` anotasi terikat diambil melalui command backend saat dialog konfirmasi hapus dibuka berdasarkan `image_id == id` (bukan berdasarkan area geografis).
     - Teks dialog secara eksplisit menampilkan: _"Menghapus {count} anotasi yang terikat pada citra ini."_
   - **Pencegahan Klik Ganda & State Loading**:
     - Setiap tombol aksi dinonaktifkan (`disabled={isProcessing}`) saat operasi async sedang berlangsung.
   - **Reaktivitas Peta**:
     - Event `ortho_image:deleted` dilepas secara global saat citra berhasil dihapus sehingga Leaflet overlay langsung dibuang seketika tanpa perlu me-reload halaman.

---

### 10.2 Daftar File yang Dibuat / Diubah

| Jalur Berkas                                               | Status     | Tanggung Jawab & Deskripsi Perubahan                                                                                                                                                                                                         |
| :--------------------------------------------------------- | :--------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/modules/module_03/ortho_source/types.rs`    | **Diubah** | Menambahkan field `updated_at: Option<String>` dan `is_corrupted: Option<bool>` pada `StoredImageMetadata`, payload `UpdateOrthoImagePayload`, dan struct `DeleteOrthoSummary`.                                                              |
| `src-tauri/src/modules/module_03/annotations/types.rs`     | **Diubah** | Menambahkan `image_id: Option<String>` pada struct `AnnotationFeature` agar anotasi terikat langsung pada ID citra orthomosaic.                                                                                                              |
| `src-tauri/src/modules/module_03/ortho_source/store.rs`    | **Diubah** | Menambahkan `validate_ortho_id` (regex `^ortho_[0-9]+$`), `list_ortho_images` (menyertakan item korup), `get_ortho_image`, `update_ortho_image`, `get_ortho_cascade_summary`, dan `delete_ortho_image_single` (mekanisme rename + rollback). |
| `src-tauri/src/modules/module_03/ortho_source/commands.rs` | **Diubah** | Menambahkan thin Tauri commands: `list_ortho_images`, `get_ortho_image`, `update_ortho_image`, `delete_ortho_image`, `get_ortho_cascade_summary`.                                                                                            |
| `src-tauri/src/modules/module_03/state.rs`                 | **Diubah** | Menambahkan metode write-through state untuk sinkronisasi in-memory pasca update dan delete.                                                                                                                                                 |
| `src-tauri/src/lib.rs`                                     | **Diubah** | Mendaftarkan 5 command baru ke dalam `tauri::generate_handler![]`.                                                                                                                                                                           |
| `src-tauri/tests/module_03_ortho_crud_test.rs`             | **Dibuat** | Unit test Rust: verifikasi allowlist regex, list terurut + item korup, delete ID tidak ada, update memproteksi immutable fields, cascade delete tuntas, dan rollback transaksional saat gagal di tengah jalan.                               |
| `src/modules/module-03/types/storage.ts`                   | **Diubah** | Menambahkan tipe `updated_at`, `is_corrupted`, `UpdateOrthoImageParams`, dan `DeleteOrthoSummary`.                                                                                                                                           |
| `src/modules/module-03/services/api.ts`                    | **Diubah** | Menambahkan wrapper typed: `listOrthoImages`, `getOrthoImage`, `updateOrthoImage`, `deleteOrthoImage`, `getOrthoCascadeSummary`.                                                                                                             |
| `src/modules/module-03/molecules/OrthoCardItem.tsx`        | **Dibuat** | Molecule kartu citra ortho: thumbnail, detail ukuran, badge "Metadata rusak" jika korup, tombol fokus, tombol edit, tombol hapus satu per satu, dan proteksi anti double-click.                                                              |
| `src/modules/module-03/molecules/EditOrthoModal.tsx`       | **Dibuat** | Modal dialog Blueprint.js untuk mengedit nama citra dan koordinat bounding box WGS84 dengan validasi murni yang sama persis dengan impor.                                                                                                    |
| `src/modules/module-03/molecules/DeleteOrthoAlert.tsx`     | **Dibuat** | Dialog konfirmasi bahaya Blueprint.js (`Alert` intent="danger") yang memuat nama citra dan teks _"Menghapus {count} anotasi yang terikat pada citra ini."_                                                                                   |
| `src/modules/module-03/organisms/ImageGallerySidebar.tsx`  | **Diubah** | Integrasi `OrthoCardItem`, `EditOrthoModal`, `DeleteOrthoAlert`, toaster notifikasi (sukses = "success", error = "danger"), dan penanganan item korup agar tetap bisa dihapus satu per satu.                                                 |
| `src/modules/module-03/organisms/SavedOrthoImageLayer.tsx` | **Diubah** | Sinkronisasi overlay di peta: memindahkan posisi gambar saat bounds berubah dan melepas overlay dari peta saat menerima event `ortho_image:deleted`.                                                                                         |

---

### 10.3 Daftar Command Tauri (Backend IPC)

1. **`list_ortho_images() -> Result<Vec<StoredImageMetadata>, CommandError>`**
   - **Input**: Kosong.
   - **Output**: Daftar metadata seluruh citra orthomosaic yang tersimpan di `/map_storage/images/`.
   - **Karakteristik & Penanganan Item Korup**:
     - Ringan & Cepat: Hanya membaca berkas `meta.json` di setiap folder `map_storage/images/{id}/`. Tidak membaca atau mendecode piksel gambar ke RAM.
     - **Item Korup Tetap Ditampilkan**: Jika suatu folder memiliki nama yang lolos regex `^ortho_[0-9]+$` tetapi berkas `meta.json` hilang, kosong, atau gagal di-parse JSON-nya:
       - Sistem tidak mengabaikannya/men-skip-nya.
       - Sistem membuat objek fallback `StoredImageMetadata` dengan flag `is_corrupted: Some(true)`, `filename: format!("{id} (Metadata rusak)")`, dan field tanggal dari filesystem metadata.
       - Item ini akan muncul di daftar galeri dengan label `"Metadata rusak"` dan tombol Hapus aktif sehingga pengguna dapat membersihkannya.
     - **Pengurutan**: Diurutkan secara otomatis dari yang terbaru (`imported_at` atau `created_at` secara descending).

2. **`get_ortho_image(id: String) -> Result<StoredImageMetadata, CommandError>`**
   - **Input**: `id` citra (mis. `"ortho_1792805664700"`).
   - **Validasi**:
     - Diperiksa dengan allowlist regex `^ortho_[0-9]+$`. Jika format tidak cocok, kembalikan `CommandError::ValidationFailed("ID citra tidak valid")`.
     - Canonical path check memastikan path berkas berada di dalam direktori penyimpanan yang sah.
   - **Output**: Metadata lengkap citra.
   - **Error**: Mengembalikan `CommandError::NotFound` jika ID tidak ditemukan.

3. **`update_ortho_image(payload: UpdateOrthoImagePayload) -> Result<StoredImageMetadata, CommandError>`**
   - **Input**:
     ```rust
     pub struct UpdateOrthoImagePayload {
         pub id: String,
         pub filename: Option<String>,
         pub west: Option<f64>,
         pub south: Option<f64>,
         pub east: Option<f64>,
         pub north: Option<f64>,
     }
     ```
   - **Aturan Immutabilitas & Validasi**:
     - Field `id`, `imported_at`, dan `schema_version` **BERSIFAT IMMUTABLE (TIDAK BOLEH BERUBAH)**. Nilai asli dari berkas `meta.json` yang lama selalu dipertahankan.
     - Validasi ID: Memenuhi allowlist regex `^ortho_[0-9]+$`.
     - Validasi Nama: `filename` disanitasi dari karakter terlarang dan tidak boleh kosong jika disediakan.
     - Validasi Bounds: Jika koordinat bounding box diubah, fungsi murni yang sama persis (`validate_wgs84_bounds(west, south, east, north)`) dijalankan ulang untuk memastikan rentang valid [-90..90], [-180..180], `south < north`, dan `west < east`.
     - Field `updated_at` diisi dengan timestamp ISO/RFC3339 saat ini (`chrono::Utc::now().to_rfc3339()`).
     - Tulis atomik ke `meta.json` via penulisan berkas `.tmp` lalu rename atomik.
     - Perbarui nama dan koordinat pada berkas layer terkait di `/map_storage/layers/{id}.json` secara sinkron.
   - **Output**: Metadata `StoredImageMetadata` yang telah diperbarui.

4. **`get_ortho_cascade_summary(id: String) -> Result<DeleteOrthoSummary, CommandError>`**
   - **Input**: `id` citra.
   - **Validasi**: Regex `^ortho_[0-9]+$`.
   - **Fungsi**: Membaca dan menghitung data turunan yang terikat pada citra secara cepat saat dialog konfirmasi hapus dibuka:
     - 1 entitas layer di `/map_storage/layers/{id}.json` (apakah ada berkas layer).
     - Jumlah `{count}` anotasi di `/map_storage/annotations/` yang field `image_id` nilainya sama dengan `id` citra ini.
   - **Output**:
     ```rust
     pub struct DeleteOrthoSummary {
         pub id: String,
         pub image_deleted: bool,
         pub layer_deleted: bool,
         pub annotations_deleted_count: usize,
     }
     ```

5. **`delete_ortho_image(id: String) -> Result<DeleteOrthoSummary, CommandError>`**
   - **Input**: `id` citra tunggal (mis. `"ortho_1792805664700"`).
   - **Prinsip**: **HANYA SATU CITRA PER AKSI**. Dilarang menerima array ID, tidak ada bulk/mass delete.
   - **Validasi**:
     - Regex allowlist `^ortho_[0-9]+$`.
     - Verifikasi keberadaan direktori `/map_storage/images/{id}`. Jika direktori tidak ditemukan, kembalikan `CommandError::NotFound("Citra dengan ID tersebut tidak ditemukan")`.
   - **Mekanisme Hapus Tuntas Dua Fase & Rollback Transaksional**:
     - **Langkah 1 (Karantina)**: Rename direktori `/map_storage/images/{id}` ke nama sementara `/map_storage/images/.trash_{id}_{timestamp}`. Operasi rename ini bersifat atomik di level filesystem OS.
     - **Langkah 2 (Hapus Dependensi)**:
       - Hapus berkas layer di `/map_storage/layers/{id}.json` (jika ada).
       - Hapus semua berkas anotasi di `/map_storage/annotations/` yang memiliki `image_id == id`.
     - **Rollback Otomatis Jika Gagal**:
       - Jika salah satu penghapusan dependensi pada Langkah 2 menghasilkan error, sistem segera mengeksekusi rollback: me-rename kembali folder karantina sementara `/map_storage/images/.trash_{id}_{timestamp}` ke nama semula `/map_storage/images/{id}`.
       - Kembalikan error transaksional `CommandError::ExecutionFailed("Penghapusan gagal di tengah proses dan berhasil di-rollback")`. Data pengguna tetap aman dan tidak korup.
     - **Langkah 3 (Pembersihan Permanen)**:
       - Jika Langkah 2 berhasil 100%, hapus folder karantina sementara `/map_storage/images/.trash_{id}_{timestamp}` beserta seluruh isinya (`fs::remove_dir_all`).
       - Sinkronkan cache memori `Module03State`.
   - **Output**: Struct ringkasan `DeleteOrthoSummary`.

---

### 10.4 Skema Perubahan JSON (`meta.json`)

Contoh berkas `map_storage/images/{id}/meta.json` setelah pembaruan:

```json
{
  "schema_version": 1,
  "id": "ortho_1792805664700",
  "filename": "Kariangau_Inspection_Area.jpg",
  "category": "ortho",
  "extension": "jpg",
  "file_path": "/map_storage/images/ortho_1792805664700/original.jpg",
  "size_bytes": 47553460,
  "dimensions": [13695, 15903],
  "thumbnail_url": "/map_storage/images/ortho_1792805664700/display.jpg",
  "bounds": [
    [-1.229988, 116.843119],
    [-1.212955, 116.876078]
  ],
  "aspect_ratio": 0.8611582720241464,
  "display_size_bytes": 946632,
  "imported_at": "2026-10-24T01:34:24.793163300+00:00",
  "updated_at": "2026-10-24T01:50:00.000000000+00:00",
  "is_corrupted": false
}
```

Jika berkas `meta.json` korup di filesystem, payload fallback yang dikirim backend ke frontend adalah:

```json
{
  "schema_version": 1,
  "id": "ortho_1792805664700",
  "filename": "ortho_1792805664700 (Metadata rusak)",
  "category": "ortho",
  "extension": "unknown",
  "file_path": "/map_storage/images/ortho_1792805664700",
  "size_bytes": 0,
  "dimensions": [0, 0],
  "thumbnail_url": "",
  "bounds": [
    [0.0, 0.0],
    [0.0, 0.0]
  ],
  "aspect_ratio": 1.0,
  "display_size_bytes": 0,
  "imported_at": "2026-10-24T01:34:24.793163300+00:00",
  "updated_at": null,
  "is_corrupted": true
}
```

---

### 10.5 Mekanisme Hapus Tuntas & Sinkronisasi Event Reaktif

1. **Jaminan Transaksional Dua Fase & Rollback**:
   - Penghapusan citra adalah operasi multi-file (folder citra di `images/`, berkas layer di `layers/`, dan anotasi terkait di `annotations/`).
   - Dengan alur:
     $$\text{Folder Citra} \xrightarrow{\text{rename}} \text{.trash\_\{id\}} \xrightarrow{\text{hapus}} \text{Layer \& Anotasi} \xrightarrow{\text{sukses}} \text{Hapus Permanen .trash\_\{id\}}$$
     Jika pada langkah penghapusan layer atau anotasi terjadi error, mekanisme rollback mengembalikan folder `.trash_{id}` kembali ke nama folder semula. Tidak ada layer yatim piatu atau citra setengah hilang.
2. **Event Reaktif `ortho_image:deleted`**:
   - Begitu backend mengembalikan sukses pada penghapusan citra, frontend segera mengeksekusi:
     ```typescript
     window.dispatchEvent(
       new CustomEvent("ortho_image:deleted", {
         detail: { id },
       }),
     );
     ```
   - Komponen [SavedOrthoImageLayer.tsx](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/SavedOrthoImageLayer.tsx) mendengarkan event `ortho_image:deleted` dan langsung melepas (_detach_) layer overlay citra dari peta Leaflet. Pengguna langsung melihat gambar hilang dari peta secara instan tanpa perlu memuat ulang peta.
3. **Event Reaktif `ortho_image:saved`**:
   - Jika koordinat bounding box atau nama citra diperbarui melalui modal edit, event `ortho_image:saved` di-dispatch.
   - Peta Leaflet memperbarui posisi bounding box gambar secara real-time dan memanggil `map.flyToBounds(newBounds)`.

---

### 10.6 Desain UI & Interaksi (Blueprint.js)

1. **Molecule `OrthoCardItem.tsx`**:
   - Thumbnail pratinjau citra (jika `is_corrupted: true`, tampilkan placeholder ikon peringatan `warning-sign` dengan latar abu-abu/merah pudar).
   - Label/Badge:
     - Jika normal: Badge biru tag `ORTHO`.
     - Jika korup: Tag merah `intent="danger"` berlabel `"Metadata rusak"`.
   - Nama Citra: Teks tebal. Jika normal, dapat diklik untuk fokus ke koordinat citra di peta.
   - Informasi Detail: Ukuran berkas, dimensi piksel, dan koordinat WGS84.
   - Tombol Aksi di Pojok Kanan Kartu:
     - Tombol **Fokus** (`icon="locate"`, minimal, dinonaktifkan jika `is_corrupted`).
     - Tombol **Edit** (`icon="edit"`, minimal, dinonaktifkan jika `is_corrupted`).
     - Tombol **Hapus** (`icon="trash"`, minimal, `intent="danger"`, **TETAP AKTIF** baik untuk citra normal maupun citra dengan metadata rusak).
   - Seluruh tombol dinonaktifkan (`disabled`) saat operasi async sedang berjalan pada item tersebut (anti klik ganda).

2. **Molecule `EditOrthoModal.tsx`**:
   - Dialog Blueprint.js (`isOpen={isOpen}`, `onClose={onClose}`).
   - Form Fields:
     - Nama Citra: `InputGroup` (validasi wajib diisi, tidak boleh hanya spasi).
     - Koordinat Bounding Box WGS84: West, South, East, North (`NumericInput` dengan ketelitian 6 desimal).
     - Tombol utilitas: _"Gunakan Area Peta Saat Ini"_ untuk mengisi koordinat dari viewport aktif.
     - Banner peringatan dinamis jika perbandingan aspek rasio koordinat berbeda signifikan dengan aspek rasio piksel asli.
   - Proteksi Immutabilitas: ID dan tanggal impor tidak dapat diedit dan ditampilkan sebagai teks label statis.
   - Tombol Aksi: _"Batal"_ dan _"Simpan Perubahan"_ (`intent="primary"`, loading spinner saat mutasi berlangsung).

3. **Molecule `DeleteOrthoAlert.tsx`**:
   - Blueprint.js `<Alert>` konfirmasi bahaya:
     - `intent="danger"`
     - `icon="trash"`
     - `confirmButtonText="Hapus Permanen"`
     - `cancelButtonText="Batal"`
   - Saat dialog dibuka:
     - Komponen secara otomatis memanggil command `get_ortho_cascade_summary(id)`.
     - Spinner loading ditampilkan di dalam dialog hingga data hitungan dependensi selesai dimuat.
   - Teks Konfirmasi:
     _"Apakah Anda yakin ingin menghapus citra **{filename}**? Tindakan ini bersifat permanen dan tidak dapat dibatalkan."_
   - Rincian Dampak Cascade (Daftar Poin):
     - Menghapus folder berkas citra asli dan pratinjau display.
     - Menghapus 1 entitas layer di panel layer peta.
     - Menghapus {count} anotasi yang terikat pada citra ini. (Catatan: `{count}` diperoleh dari hitungan ID citra).
   - Jika item adalah item dengan metadata rusak (`is_corrupted: true`):
     - Teks peringatan disesuaikan: _"Menghapus folder berkas citra yang metadata-nya rusak secara tuntas."_

4. **Standar Notifikasi Toast Blueprint.js**:
   - Menggunakan singleton `OverlayToaster`:
     - **Sukses Hapus Citra**: `showToast("Citra '{filename}' beserta layer terkait berhasil dihapus.", "success")` (`intent="success"`)
     - **Sukses Update Citra**: `showToast("Metadata citra '{filename}' berhasil diperbarui.", "success")` (`intent="success"`)
     - **Gagal / Terjadi Kesalahan**: `showToast("Gagal memproses aksi: {errorMessage}", "danger")` (`intent="danger"`)

---

### 10.7 Analisis Risiko & Mitigasi

1. **Risiko Path Traversal & Injeksi Nama File**:
   - Serangan input ID: Penyerang mengirim ID manipulatif seperti `../../etc/passwd` atau `..\\windows`.
   - **Mitigasi**:
     - Lapisan 1: Allowlist regex murni `^ortho_[0-9]+$`. Pola ini membatasi karakter secara mutlak hanya pada prefix `ortho_` yang diikuti angka numerik. Karakter seperti titik (`.`), slash (`/`), backslash (`\`), dan spasi otomatis ditolak sebelum masuk ke filesystem I/O.
     - Lapisan 2: Validasi path teresolusi (canonical path) memastikan target operasi tetap berada di dalam `map_storage/images/`.
2. **Risiko Berkas `meta.json` Rusak / Korup**:
   - Penanganan di masa lalu hanya melakukan skip dan log di terminal, menyebabkan folder yatim menumpuk dan pengguna tidak bisa membersihkannya.
   - **Mitigasi**: Item korup tetap didaftarkan sebagai `is_corrupted: true`, ditampilkan di UI dengan label khusus, dan memiliki opsi penghapusan mandiri satu per satu.
3. **Risiko Kegagalan I/O Parsial (Cascade Setengah Jalan)**:
   - Folder citra terhapus tetapi penghapusan layer gagal karena permission berkas.
   - **Mitigasi**: Mekanisme dua fase dengan rename ke folder karantina sementara `.trash_{id}_{timestamp}`. Jika penghapusan dependensi gagal, folder karantina langsung di-rollback kembali ke nama aslinya, menjaga keutuhan data.
4. **Risiko Modifikasi Field Terproteksi (Immutable Fields)**:
   - Upaya mengubah `id`, `imported_at`, atau `schema_version` saat update.
   - **Mitigasi**: Logika backend di `update_ortho_image` membaca metadata asli yang tersimpan dan hanya mengizinkan pembaruan pada field yang diperbolehkan (`filename`, `bounds`, `updated_at`). Nilai `id`, `imported_at`, dan `schema_version` dipertahankan secara utuh.

---

### 10.8 Rencana Pengujian Mandiri (Unit Test Rust)

Seluruh pengujian unit diimplementasikan di `src-tauri/tests/module_03_ortho_crud_test.rs` dengan lingkungan direktori pengujian sementara (`tempfile::tempdir`):

1. **Test Validasi ID Regex Allowlist**:
   - Input valid (`"ortho_1792805664700"`) lolos validasi.
   - Input tidak valid (`"../bad_id"`, `"ortho_123/hack"`, `"normal_name"`, `"ortho_abc"`, `""`) ditolak dengan error validasi.
2. **Test List Terurut Descending & Penanganan Metadata Rusak**:
   - Membuat 3 folder citra dengan timestamp berbeda.
   - Salah satu folder sengaja dibuat dengan berkas `meta.json` korup / kosong.
   - Verifikasi hasil `list_ortho_images`:
     - Seluruh item (3 item) tetap ada di dalam daftar.
     - Item diurutkan dari yang terbaru (descending).
     - Item yang rusak memiliki flag `is_corrupted == true` dan label `"Metadata rusak"`.
3. **Test Hapus ID Tidak Ada**:
   - Memanggil `delete_ortho_image("ortho_9999999999999")` pada ID yang tidak tersimpan.
   - Verifikasi mengembalikan error `NotFound` yang jelas.
4. **Test Immutabilitas Field pada Update**:
   - Melakukan update pada citra yang tersimpan.
   - Memverifikasi bahwa `filename` dan `bounds` berubah, `updated_at` bertambah.
   - Memverifikasi secara tegas bahwa field `id`, `imported_at`, dan `schema_version` **tetap identik** dengan nilai awal sebelum update.
5. **Test Mekanisme Hapus Tuntas Normal (No Orphan Policy)**:
   - Menyiapkan folder citra di `images/ortho_123/`, berkas layer di `layers/ortho_123.json`, dan anotasi terikat dengan `image_id: Some("ortho_123")`.
   - Menjalankan `delete_ortho_image("ortho_123")`.
   - Memverifikasi folder citra, berkas layer, dan anotasi terikat terhapus bersih tanpa sisa.
6. **Test Rollback Transaksional Saat Gagal di Tengah Proses Hapus**:
   - Menyiapkan citra dan mensimulasikan kegagalan penghapusan dependensi (misal membuat berkas layer berstatus read-only atau mocking failure).
   - Memverifikasi bahwa proses penghapusan dibatalkan, folder karantina sementara di-rollback kembali ke folder asli `/map_storage/images/ortho_123/`, dan data citra tetap utuh tanpa kehilangan berkas.

---

### 10.9 Checklist Verifikasi CRUD Ortho

- [x] Validasi ID menggunakan allowlist regex `^ortho_[0-9]+$` selain pengecekan path traversal
- [x] `list_ortho_images` membaca semua subfolder tanpa decode piksel (ringan & cepat)
- [x] Berkas `meta.json` yang korup tetap tampil di daftar galeri dengan label `"Metadata rusak"`
- [x] Item dengan metadata rusak dapat dihapus satu per satu secara mandiri
- [x] `get_ortho_image` mengembalikan detail lengkap citra by ID dan menolak ID berbahaya
- [x] `update_ortho_image` memvalidasi bounds menggunakan fungsi murni yang sama (`validate_wgs84_bounds`)
- [x] Field immutable (`id`, `imported_at`, `schema_version`) tidak berubah setelah update metadata
- [x] Field `updated_at` tercatat di `meta.json` setelah update berhasil dilakukan
- [x] `{count}` anotasi pada dialog hapus dihitung secara real-time berdasarkan ID citra (`image_id`) saat dialog dibuka
- [x] Teks konfirmasi dialog hapus menyebutkan _"Menghapus {count} anotasi yang terikat pada citra ini."_
- [x] Mekanisme hapus tuntas dua fase: rename folder ke sementara -> hapus layer & anotasi -> hapus permanen folder sementara
- [x] Rollback rename folder sementara berjalan jika terjadi kegagalan penghapusan dependensi di tengah proses
- [x] Event global `ortho_image:deleted` di-dispatch saat citra berhasil dihapus
- [x] Layer overlay citra di peta Leaflet otomatis hilang saat menerima event `ortho_image:deleted`
- [x] Layer overlay citra di peta Leaflet berpindah posisi secara langsung saat menerima event `ortho_image:saved`
- [x] Notifikasi Toast Blueprint.js muncul: sukses hapus berstatus `"success"`, sukses update berstatus `"success"`, error berstatus `"danger"`
- [x] Tombol aksi dinonaktifkan (`disabled`) saat proses async sedang berjalan (proteksi anti klik ganda)
- [x] Unit test Rust lolos: ID invalid ditolak, list terurut + corrupt meta, delete ID tidak ada, field immutable terlindungi, cascade delete tuntas, dan rollback kegagalan
- [x] Seluruh pre-commit quality gates lolos: `lint`, `lint:rust`, `format:check`, `format:rust:check`, dan `npm run build`

---

## 11. [DIGANTIKAN OLEH BAGIAN 12] Rencana Implementasi: CRUD Spatial Result / sp_result (Module 3)

> **PERHATIAN (DEPRECATED)**: Seluruh arsitektur entitas terpisah `/map_storage/sp_results/` pada Bagian 11 ini telah **DIBATALKAN DAN DIGANTIKAN TOTAL OLEH BAGIAN 12**. Jangan mengacu pada implementasi Bagian 11. Data utama resmi adalah `/map_storage/spatial_results/`.

### 11.0 Keputusan Arsitektur Lama (Dibatalkan)

1. **Pemisahan Ruang Direktori Bersih (No Conflict)**:
   - Direktori penyimpanan khusus: `/map_storage/sp_results/` (terpisah mutlak dari direktori `/map_storage/spatial_results/` yang digunakan oleh plugin analitik eksternal untuk menyimpan layer pohon/tutupan lahan).
   - Perintah `list_sp_results` **HANYA** memindai direktori `/map_storage/sp_results/`. Berkas atau folder layer plugin di `/map_storage/spatial_results/` tidak akan pernah terpindai atau keliru dideteksi sebagai "Data rusak".
2. **SATU Format Berkas & Folder Terpilih**:
   - Setiap entitas hasil pengukuran disimpan dalam 1 subfolder terisolasi:
     - `/map_storage/sp_results/{id}/result.json` (wajib, data hasil pengukuran & geometri, $\le 64$ KB, tanpa field `is_corrupted`).
     - `/map_storage/sp_results/{id}/photo.{ext}` (opsional, berkas foto lapangan terpisah, $\le 10$ MB, format JPG/PNG/WebP).
   - Tidak ada format ganda atau ambiguitas path. Struktur folder per entitas ini memungkinkan penghapusan 2-fase, rollback, dan karantina `.trash_` dilakukan secara atomik pada level folder.

---

### 11.1 Skema Data, Deserialization, & Integritas Tipe

#### 1. Skema JSON Tersimpan di Disk (`StoredSpResult`)

Disimpan pada `/map_storage/sp_results/{id}/result.json` (maksimal 64 KB).
**Perhatian**: Field `is_corrupted` **DIHAPUS** dari skema tersimpan di disk karena status rusak adalah status turunan runtime saat proses pembacaan.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredSpResult {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub measurement_type: String, // "area" | "perimeter" | "distance"
    pub calculated_value: f64,
    pub unit: String,             // "m²" | "m"
    pub source_aoi_id: Option<String>,
    pub source_image_id: Option<String>,
    pub notes: Option<String>,
    pub display_color: String,    // Regex validated: ^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$
    pub geometry: SpatialGeometry,
    pub photo: Option<SpResultPhotoMetadata>, // null jika tidak ada foto
    pub created_at: String,       // ISO-8601 UTC
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpResultPhotoMetadata {
    pub file_name: String,        // mis. "photo.jpg"
    pub content_type: String,     // "image/jpeg" | "image/png" | "image/webp"
    pub size_bytes: u64,          // Maks MAX_RESULT_PHOTO_SIZE_BYTES (10 MB)
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub attached_at: String,      // ISO-8601 UTC
}
```

Contoh berkas `/map_storage/sp_results/sp_1792809800000/result.json`:

```json
{
  "schema_version": 1,
  "id": "sp_1792809800000",
  "name": "Pengukuran Luas Area Blok A",
  "measurement_type": "area",
  "calculated_value": 14250.75,
  "unit": "m²",
  "source_aoi_id": "aoi-1792804000",
  "source_image_id": "ortho_1792805664700",
  "notes": "Hasil ukur batas barat inspeksi drone",
  "display_color": "#2ecc40",
  "geometry": {
    "type": "Polygon",
    "coordinates": [
      [
        [116.8575, -1.221],
        [116.8582, -1.2208],
        [116.859, -1.2212],
        [116.8592, -1.222],
        [116.8586, -1.2228],
        [116.8578, -1.2225],
        [116.857, -1.2218],
        [116.8575, -1.221]
      ]
    ]
  },
  "photo": {
    "file_name": "photo.jpg",
    "content_type": "image/jpeg",
    "size_bytes": 1420580,
    "width": 1920,
    "height": 1080,
    "attached_at": "2026-10-24T02:00:00Z"
  },
  "created_at": "2026-10-24T02:00:00Z",
  "updated_at": null
}
```

#### 2. DTO Runtime Baca Ringan (`SpResultSummary`)

Digunakan oleh antarmuka tabel/kartu galeri melalui command `list_sp_results`. Berkas geometri berat tidak dimuat ke dalam ringkasan list untuk menjaga performa cepat dan responsif:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpResultSummary {
    pub id: String,
    pub name: String,
    pub measurement_type: String,
    pub calculated_value: f64,
    pub unit: String,
    pub display_color: String,
    pub source_aoi_id: Option<String>,
    pub source_image_id: Option<String>,
    pub is_aoi_available: bool,    // false jika AOI sumber sudah terhapus
    pub is_image_available: bool,  // false jika citra sumber sudah terhapus
    pub has_photo: bool,
    pub photo_summary: Option<SpResultPhotoSummary>,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub is_corrupted: bool,        // STATUS TURUNAN SAAT BACA (true jika result.json rusak/tidak valid)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpResultPhotoSummary {
    pub file_name: String,
    pub content_type: String,
    pub size_bytes: u64,
}
```

#### 3. Proteksi Mutlak di Level Tipe pada Update (`UpdateSpResultPayload`)

Sesuai aturan, pembaruan data **DITOLAK DI LEVEL TIPE** jika memuat field yang immutable. Struct Rust dan interface TypeScript dideklarasikan HANYA memuat field yang diperbolehkan untuk diperbarui:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UpdateSpResultPayload {
    pub id: String,
    pub name: Option<String>,
    pub notes: Option<String>,
    pub display_color: Option<String>, // Divalidasi regex hex
}
```

Atribut `#[serde(deny_unknown_fields)]` memastikan jika klien mencoba menyusupkan field immutable (`calculated_value`, `geometry`, `measurement_type`, `unit`, `source_aoi_id`, `source_image_id`, `schema_version`, `created_at`), deserialisasi serde akan **langsung gagal** sebelum eksekusi bisnis.

TypeScript Interface:

```typescript
export interface UpdateSpResultPayload {
  id: string;
  name?: string;
  notes?: string;
  display_color?: string;
}
```

#### 4. Validasi Regex Format Warna (`display_color`)

Fungsi murni:

```rust
pub fn validate_hex_color(color: &str) -> Result<(), String> {
    let re = regex::Regex::new(r"^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$").unwrap();
    if re.is_match(color) {
        Ok(())
    } else {
        Err(format!("Warna '{}' tidak valid. Harus format HEX (#RGB atau #RRGGBB).", color))
    }
}
```

#### 5. Pencegahan Bentrok ID (Collision Prevention)

- Format ID menggunakan prefix dan timestamp epoch millisecond: `^sp_[0-9]+$`.
- Sebelum menulis berkas atomik:
  1. Backend mengecek apakah `storage_dir.join(&id).exists()`.
  2. Jika direktori ID sudah ada, backend otomatis menambahkan counter nano/increment millisecond untuk menjamin keunikan mutlak (atau mengembalikan error `AlreadyExists` jika ID spesifik diminta).
  3. Proses penulisan selalu menggunakan direktori sementara `.tmp_create_{id}_{timestamp}` yang kemudian di-rename secara atomik ke `{id}`.

---

### 11.2 Penanganan Foto Opsional & Lifecycle Berkas

1. **Batas Ukuran & Konstanta**:
   - `pub const MAX_RESULT_PHOTO_SIZE_BYTES: usize = 10 * 1024 * 1024; // 10 MB`
   - Payload foto yang melebihi batas 10 MB langsung ditolak dengan pesan error yang jelas tanpa membebani memori.
2. **Validasi Magic Bytes (Deteksi Konten Murni)**:
   - Validasi tidak hanya mengandalkan ekstensi nama berkas, melainkan memvalidasi byte awal (magic bytes):
     - **JPEG**: byte diawali `[0xFF, 0xD8, 0xFF]`
     - **PNG**: byte diawali `[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]`
     - **WebP**: byte `0..4` bernilai `b"RIFF"` dan byte `8..12` bernilai `b"WEBP"`
   - Berkas selain ketiga format tersebut ditolak.
3. **Penggantian Foto Atomic**:
   - Foto baru ditulis ke berkas sementara di dalam folder hasil: `/map_storage/sp_results/{id}/photo.tmp_{timestamp}.{ext}`.
   - Setelah bytes selesai ditulis dan divalidasi, berkas foto lama (jika ada, misalnya berganti ekstensi dari png ke jpg) dihapus.
   - Berkas sementara di-rename secara atomik menjadi `/map_storage/sp_results/{id}/photo.{ext}`.
   - Metadata `photo` di `result.json` diperbarui secara atomik.
   - Jika proses gagal di tengah jalan, berkas sementara dibersihkan dan berkas foto lama tetap utuh.
4. **Penghapusan Mandiri & Cascade**:
   - Command `remove_sp_result_photo(id)` menghapus berkas fisik `photo.{ext}` dan mengeset `photo: null` di `result.json`.
   - Saat `delete_sp_result` dijalankan, seluruh folder `/map_storage/sp_results/{id}/` (termasuk berkas foto) masuk ke karantina `.trash_` lalu dihapus tuntas, sehingga tidak pernah meninggalkan berkas foto yatim.
5. **Komponen UI `ResultPhoto.tsx` & Lazy Loading**:
   - Komponen Molecule presentasional murni yang menangani 3 status:
     - **Foto Tersedia**: Merender `<img>` dengan atribut `loading="lazy"` (atau asset loader Tauri) dan rasio aspek rapi.
     - **Foto Null / Tidak Ada**: Menampilkan placeholder visual elegan `"No Image"` dengan ikon Blueprint.js `media` / `camera` berlatar abu-abu netral.
     - **Foto Hilang di Disk / Berkas Rusak**: Menangkap kegagalan lewat event `onError` pada elemen gambar dan otomatis beralih menampilkan placeholder peringatan `"Foto Tidak Ditemukan"` dengan ikon `warning-sign` / `issue`.

---

### 11.3 Penanganan Startup, Karantina `.trash_*`, dan Referensi Yatim

1. **Pembersihan Sisa Karantina Saat Startup Backend**:
   - Saat inisialisasi aplikasi pada `Module03State::new()`, sistem mengeksekusi `cleanup_orphaned_trash(&sp_results_dir)`.
   - Fungsi ini memindai direktori `/map_storage/sp_results/` dan menghapus permanen setiap subfolder/berkas yang diawali `.trash_` (sisa dari aplikasi yang terhenti mendadak pada sesi sebelumnya).
2. **Pengecualian Berkas Tersembunyi pada List**:
   - Fungsi `list_sp_results` secara tegas mengabaikan entri yang berawalan titik `.` (seperti `.trash_*` atau `.tmp_*`). Berkas/folder karantina ini tidak akan pernah muncul di daftar dan tidak akan dianggap sebagai data rusak.
3. **Penanganan Referensi Yatim (Orphan Reference Handling)**:
   - Relasi `source_aoi_id` dan `source_image_id` diperlakukan sebagai referensi longgar (_soft reference_).
   - Nilai hasil pengukuran (`calculated_value`) dan bentuk geometri (`geometry`) disimpan secara mandiri di dalam `result.json`.
   - Jika AOI sumber atau Citra Ortho sumber dihapus oleh pengguna:
     - Backend **TIDAK ERROR ATAU CRASH**.
     - Saat membaca ringkasan atau detail, backend mengecek secara non-blocking apakah berkas fisik sumber masih ada di disk:
       - `is_aoi_available = aoi_path.exists()`
       - `is_image_available = image_path.exists()`
     - Di UI: Jika sumber tidak tersedia, UI menampilkan tag netral abu-abu bertuliskan `"AOI: (dihapus)"` atau `"Citra: (dihapus)"` (bukan badge merah error/korup). Tombol pintasan untuk fokus atau membuka sumber dinonaktifkan secara aman dengan tooltip penjelasan: _"Citra/AOI sumber telah dihapus sebelumnya. Hasil pengukuran tetap tersimpan secara mandiri."_
4. **Hubungan Eksplisit dengan `delete_ortho_image`**:
   - Operasi `delete_ortho_image` HANYA menghapus berkas citra drone, layer tampilan citra, dan anotasi terikat.
   - Operasi `delete_ortho_image` **TIDAK MENGHAPUS** hasil pengukuran `sp_result` yang terikat pada citra tersebut.
   - Rasionalitas: Hasil pengukuran spasial (seperti luas area batas lahan atau panjang jalan) merupakan data analitik penting yang harus tetap dapat diakses dan dievaluasi meskipun berkas raster citra drone berukuran besar dihapus atau diganti untuk menghemat ruang disk.

---

### 11.4 Mekanisme Delete 2-Fase & Transaksi Rollback

1. **Prinsip Single-Item (Tanpa Hapus Massal)**:
   - Command `delete_sp_result` hanya menerima 1 ID bertipe `String`. Tidak ada antarmuka hapus massal untuk mencegah kecelakaan data.
2. **Alur Transaksional Dua Fase**:
   $$\text{Folder /map\_storage/sp\_results/\{id\}/} \xrightarrow{\text{rename}} \text{.trash\_sp\_\{id\}\_\{timestamp\}} \xrightarrow{\text{hapus}} \text{Layer Peta di layers/sp\_\{id\}.json} \xrightarrow{\text{sukses}} \text{Hapus Permanen .trash\_}$$
3. **Rollback Otomatis Saat Gagal**:
   - Jika penghapusan layer tampilan `layers/sp_{id}.json` gagal (mis. permission error), folder karantina `.trash_sp_{id}_{timestamp}` segera di-rename kembali ke nama aslinya `/map_storage/sp_results/{id}/`.
   - Data hasil pengukuran dan foto tetap utuh, dan error dikembalikan ke antarmuka pengguna.
4. **Isolasi Sumber (AOI & Citra Tetap Utuh)**:
   - Berkas di `/map_storage/aois/` dan `/map_storage/images/` tidak pernah disentuh dalam proses penghapusan `sp_result`.
5. **Event Reaktif Peta**:
   - Dispatch event global `sp_result:deleted` dengan payload `{ id }`. Layer peta langsung melepas visualisasi hasil secara instan.

---

### 11.5 Daftar Command Tauri (Input & Output)

| Command                      | Parameter Input                                     | Output                  | Deskripsi & Validasi                                                                                                                                                                           |
| :--------------------------- | :-------------------------------------------------- | :---------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list_sp_results`            | `filter_aoi_id: Option<String>`                     | `Vec<SpResultSummary>`  | Memindai folder `/map_storage/sp_results/`. Mengabaikan `.trash_*`. Mengembalikan ringkasan ringan + status turunan `is_corrupted`. Urut descending `created_at`.                              |
| `get_sp_result`              | `id: String`                                        | `StoredSpResult`        | Mengambil detail lengkap 1 hasil (termasuk koordinat geometri & metadata foto). Validasi regex `^sp_[0-9]+$`. Error `NotFound` jika ID tidak ada.                                              |
| `create_sp_result`           | `payload: CreateSpResultPayload`                    | `StoredSpResult`        | Validasi pencegahan bentrok ID. Menghitung nilai secara murni dari geometri sumber (`calculate_area`/`distance`/`perimeter`), menyimpan secara atomik ke folder baru, meregistrasi layer peta. |
| `update_sp_result`           | `payload: UpdateSpResultPayload`                    | `StoredSpResult`        | Memperbarui hanya `name`, `notes`, `display_color` (regex hex), dan `updated_at`. Menolak immutable fields di level tipe via `deny_unknown_fields`.                                            |
| `attach_sp_result_photo`     | `id: String`, `file_name: String`, `bytes: Vec<u8>` | `StoredSpResult`        | Memvalidasi batas 10 MB (`MAX_RESULT_PHOTO_SIZE_BYTES`) dan magic bytes (JPG/PNG/WebP). Menulis foto secara atomik ke folder hasil dan memperbarui `result.json`.                              |
| `remove_sp_result_photo`     | `id: String`                                        | `StoredSpResult`        | Menghapus berkas fisik foto dari folder hasil dan mengosongkan metadata foto (`photo: null`) pada `result.json`.                                                                               |
| `count_sp_result_dependents` | `id: String`                                        | `DeleteSpResultSummary` | Menghitung dependensi layer peta terkait untuk ditampilkan pada dialog konfirmasi hapus.                                                                                                       |
| `delete_sp_result`           | `id: String`                                        | `DeleteSpResultSummary` | Menghapus 1 folder hasil (termasuk berkas foto) secara 2-fase dengan rollback. AOI sumber tetap utuh.                                                                                          |

---

### 11.6 Desain UI & Notifikasi (Blueprint.js)

1. **Molecule `ResultPhoto.tsx`**:
   - Menampilkan pratinjau thumbnail lazy loading (`loading="lazy"`).
   - Props: `photo?: SpResultPhotoSummary | null`, `resultId: string`, `size?: "small" | "medium" | "large"`.
   - Placeholder state:
     - Jika `photo === null`: Placeholder elegan dengan ikon Blueprint `media` / `camera` berlabel teks _"No Image"_.
     - Jika gambar gagal dimuat (`onError`): Fallback otomatis ke placeholder _"Foto Tidak Ditemukan / Rusak"_ dengan ikon `warning-sign`.
2. **Molecule `SpResultCardItem.tsx`**:
   - Menyajikan informasi kartu ringkasan:
     - Thumbnail foto lewat komponen `ResultPhoto`.
     - Nama pengukuran, badge tipe (`Luas Area` hijau, `Keliling` biru, `Jarak` oranye), nilai terformat (mis. `14.250 m²` atau `1,42 ha`).
     - Tag relasi: Tag netral untuk `AOI` dan `Citra`. Jika sumber terhapus, menampilkan label `"AOI: (dihapus)"` bernuansa muted.
     - Jika berstatus `is_corrupted: true`: Tag merah `intent="danger"` berlabel `"Data rusak"`, tombol Fokus dan Edit disabled, tombol Hapus **tetap aktif**.
     - Tombol Aksi: **Fokus** (`icon="locate"`), **Foto** (`icon="camera"`), **Edit** (`icon="edit"`), **Hapus** (`icon="trash"`, `intent="danger"`).
     - Tombol dinonaktifkan (`disabled`) saat aksi async sedang berlangsung untuk mencegah klik ganda.
3. **Molecule `EditSpResultModal.tsx`**:
   - Dialog Blueprint.js (`isOpen`, `onClose`):
     - Form input hanya untuk: Nama Hasil (1..100 karakter), Catatan (opsional, maks 500 karakter), dan Pilihan Warna Tampilan (HEX color picker dengan validasi format regex).
     - Field statis read-only terproteksi: ID, Tipe Ukur, Nilai Hasil Terhitung, Satuan, Tanggal Dibuat, dan Relasi Sumber.
4. **Molecule `AttachPhotoModal.tsx`**:
   - Dialog pemilihan dan pratinjau unggah foto:
     - Validasi sisi klien batas ukuran 10 MB dan format berkas (JPG, PNG, WebP).
     - Pratinjau langsung sebelum disimpan.
     - Tombol ganti foto dan tombol hapus foto mandiri.
5. **Molecule `DeleteSpResultAlert.tsx`**:
   - Blueprint.js `<Alert>` konfirmasi bahaya:
     - `intent="danger"`, `icon="trash"`, `confirmButtonText="Hapus Permanen"`, `cancelButtonText="Batal"`.
     - Mengambil hitungan dependensi lewat `count_sp_result_dependents(id)` saat dibuka.
     - Menampilkan rincian dampak: Menghapus data pengukuran, berkas foto terkait, dan layer tampilan di peta.
     - Penegasan teks: _"AOI sumber dan citra drone tidak akan terhapus."_
6. **Organism `SpResultListPanel.tsx`**:
   - Panel daftar hasil pengukuran dengan filter pencarian nama, filter kategori tipe pengukuran, dan penanganan state (loading, empty, error).
7. **Standar Notifikasi Toast Blueprint.js**:
   - Sukses simpan/update: `showToast("Hasil pengukuran '{name}' berhasil diperbarui.", "success")`
   - Sukses attach foto: `showToast("Foto dokumentasi berhasil dilampirkan.", "success")`
   - Sukses hapus: `showToast("Hasil pengukuran '{name}' beserta berkas terkait berhasil dihapus.", "success")`
   - Gagal/error: `showToast("Gagal memproses aksi: {errorMessage}", "danger")`

---

### 11.7 Rencana Pengujian Unit Rust (`src-tauri/tests/module_03_sp_result_crud_test.rs`)

Seluruh pengujian unit diimplementasikan di `src-tauri/tests/module_03_sp_result_crud_test.rs` dengan direktori pengujian sementara (`tempfile::tempdir`):

1. **Test Validasi ID Regex & Pencegahan Bentrok ID**:
   - ID valid (`"sp_1792809800000"`, `"sp_123"`) lolos validasi.
   - ID tidak valid (`"sp_abc"`, `"../bad"`, `""`, `"ortho_123"`) ditolak.
   - Mencoba membuat entitas baru saat folder ID sudah ada ditolak dengan error `AlreadyExists` atau menghasilkan ID unik baru.
2. **Test List Ringan & Penanganan Berkas Rusak (`is_corrupted` Runtime)**:
   - Membuat 3 entitas pengukuran, salah satunya memiliki berkas `result.json` korup/rusak.
   - Memverifikasi berkas korup tidak memicu panic/crash dan tetap muncul di list dengan flag runtime `is_corrupted == true`.
   - Berkas `result.json` di disk diverifikasi tidak menyimpan field `is_corrupted`.
3. **Test Get ID Tidak Ada**:
   - Memanggil `get_sp_result("sp_999999999")` mengembalikan error `NotFound` yang jelas.
4. **Test Immutabilitas di Level Tipe & Update Field Aman**:
   - Melakukan update pada nama dan catatan.
   - Memverifikasi nilai `calculated_value`, `geometry`, `measurement_type`, `unit`, `source_aoi_id`, `schema_version`, dan `created_at` tetap identik dengan nilai awal sebelum update.
5. **Test Validasi Regex Hex pada `display_color`**:
   - Nilai hex valid (`"#2ecc40"`, `"#fff"`, `"#1A2B3C"`) diterima.
   - Nilai hex tidak valid (`"red"`, `"#12345"`, `"123456"`, `"#gggggg"`, `""`) ditolak dengan pesan error validasi yang tepat.
6. **Test Unggah Foto Opsional & Validasi Magic Bytes**:
   - Mengunggah berkas JPEG murni (magic bytes `[0xFF, 0xD8, 0xFF]`), PNG murni, dan WebP murni berhasil dilampirkan.
   - Mengunggah berkas > 10 MB (`MAX_RESULT_PHOTO_SIZE_BYTES`) ditolak dengan error `PayloadTooLarge`.
   - Mengunggah berkas teks palsu yang dinamai `fake.jpg` (magic bytes salah) ditolak dengan error validasi tipe berkas.
7. **Test Ganti Foto Atomic & Hapus Foto Mandiri**:
   - Mengunggah foto awal $\to$ mengganti dengan foto baru secara atomik $\to$ memverifikasi berkas lama digantikan berkas baru tanpa residu berkas sementara `.tmp_*`.
   - Memanggil `remove_sp_result_photo` $\to$ berkas fisik foto terhapus dan metadata `photo` di `result.json` bernilai `null`.
8. **Test Hapus Tuntas 2-Fase & Proteksi AOI Sumber**:
   - Menyiapkan folder hasil di `sp_results/sp_1/` beserta berkas foto `photo.jpg`, berkas layer di `layers/sp_sp_1.json`, dan AOI di `aois/aoi-1.json`.
   - Menjalankan `delete_sp_result("sp_1")`.
   - Memverifikasi folder hasil dan foto terhapus bersih, berkas layer terhapus bersih, sedangkan berkas AOI di `aois/aoi-1.json` **tetap utuh dan tidak tersentuh**.
9. **Test Rollback Transaksional Saat Gagal Hapus**:
   - Mensimulasikan kegagalan I/O saat menghapus data layer tampilan.
   - Memverifikasi folder karantina `.trash_sp_...` berhasil di-rollback kembali ke nama aslinya dan data hasil beserta foto tetap aman tanpa kehilangan data.
10. **Test Pembersihan Startup `.trash_*` & Penanganan Referensi Yatim**:
    - Membuat sisa folder `.trash_sp_old` di direktori pengujian $\to$ memanggil `cleanup_orphaned_trash` $\to$ memverifikasi sisa folder karantina terhapus tuntas.
    - Menghapus berkas AOI dan citra sumber secara manual $\to$ memanggil `get_sp_result` dan `list_sp_results` $\to$ memverifikasi hasil tetap dapat dibaca normal dengan flag `is_aoi_available == false` dan `is_image_available == false` tanpa error.

---

### 11.8 Checklist Verifikasi Kriteria Selesai (Dibatalkan)

> **STATUS**: Seluruh checklist berikut **DIBATALKAN** seiring dengan pembatalan entitas terpisah `sp_results`. Pekerjaan dialihkan ke Bagian 12 di atas `spatial_results` yang sudah ada.

- [-] (Dibatalkan - digantikan Bagian 12) Folder penyimpanan terpisah `/map_storage/sp_results/` dan tidak memindai layer plugin di `/map_storage/spatial_results/`
- [-] (Dibatalkan - digantikan Bagian 12) SATU format berkas terpilih: folder `/map_storage/sp_results/{id}/` berisi `result.json` ($\le 64$ KB) dan opsional `photo.{ext}` ($\le 10$ MB)
- [-] (Dibatalkan - digantikan Bagian 12) Field `is_corrupted` dihapus dari skema tersimpan `StoredSpResult` dan hanya ada sebagai status turunan runtime saat baca
- [-] (Dibatalkan - digantikan Bagian 12) Item dengan berkas rusak tetap tampil di daftar list dengan label `"Data rusak"` dan tombol hapus tetap aktif
- [-] (Dibatalkan - digantikan Bagian 12) Detail 1 `sp_result` by ID memuat geometri dan metadata lengkap serta menolak ID tidak valid
- [-] (Dibatalkan - digantikan Bagian 12) Create `sp_result` memverifikasi pencegahan bentrok ID dan menghitung nilai secara murni dari geometri sumber
- [-] (Dibatalkan - digantikan Bagian 12) Validasi `display_color` menggunakan regex hex murni `^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$`
- [-] (Dibatalkan - digantikan Bagian 12) `UpdateSpResultPayload` menolak field immutable di level tipe (`serde(deny_unknown_fields)` di Rust & omitted di interface TS)
- [-] (Dibatalkan - digantikan Bagian 12) Foto opsional: `photo: null` jika tidak ada, disimpan terpisah, dan ikut terhapus bersama hasil
- [-] (Dibatalkan - digantikan Bagian 12) Batas foto 10 MB lewat konstanta `MAX_RESULT_PHOTO_SIZE_BYTES` dan validasi magic bytes (JPEG, PNG, WebP)
- [-] (Dibatalkan - digantikan Bagian 12) Penggantian foto berlangsung secara atomik dan penghapusan foto mandiri lewat `remove_sp_result_photo`
- [-] (Dibatalkan - digantikan Bagian 12) Komponen `ResultPhoto` mendukung lazy loading, placeholder elegan `"No Image"`, dan fallback penanganan foto hilang/rusak
- [-] (Dibatalkan - digantikan Bagian 12) Pembersihan sisa folder `.trash_*` berjalan otomatis saat startup aplikasi dan diabaikan saat pemindaian list
- [-] (Dibatalkan - digantikan Bagian 12) Referensi yatim (AOI atau Citra sumber terhapus) ditangani secara anggun tanpa crash/error
- [-] (Dibatalkan - digantikan Bagian 12) Menghapus citra ortho (`delete_ortho_image`) dipastikan TIDAK menghapus `sp_result` yang terikat
- [-] (Dibatalkan - digantikan Bagian 12) Delete hanya menerima 1 ID per aksi (single-item delete)
- [-] (Dibatalkan - digantikan Bagian 12) Dialog konfirmasi Alert `intent="danger"`, `icon="trash"`, menyebut nama hasil dan ringkasan dependensi yang dibersihkan
- [-] (Dibatalkan - digantikan Bagian 12) Mekanisme hapus 2-fase: rename folder ke karantina sementara $\to$ bersihkan layer $\to$ hapus permanen folder sementara
- [-] (Dibatalkan - digantikan Bagian 12) Rollback transaksional memulihkan folder karantina jika terjadi kegagalan di tengah proses hapus
- [-] (Dibatalkan - digantikan Bagian 12) Event global `sp_result:saved` dan `sp_result:deleted` memperbarui tampilan peta secara instan
- [-] (Dibatalkan - digantikan Bagian 12) Tombol aksi dinonaktifkan (`disabled`) selama operasi async berlangsung (proteksi anti klik ganda)
- [-] (Dibatalkan - digantikan Bagian 12) Re-export untuk Module 10 dan modul lain tetap utuh, kontrak antar modul tidak berubah
- [-] (Dibatalkan - digantikan Bagian 12) Seluruh 10 unit test Rust di `module_03_sp_result_crud_test.rs` lolos 100%
- [-] (Dibatalkan - digantikan Bagian 12) Seluruh pre-commit quality gates (`npm run lint`, `npm run lint:rust`, `npm run format:check`, `npm run format:rust:check`, `npm run build`) lolos tanpa error
- [-] (Dibatalkan - digantikan Bagian 12) Tidak ada dependensi baru dan tidak ada Python

---

## 12. Rekonsiliasi & Rencana Implementasi CRUD Spatial Results Berbasis Data Asli (`/map_storage/spatial_results`)

> **STATUS & ATURAN EKSEKUSI**: Bagian ini menggantikan arah implementasi Bagian 11 sesuai keputusan resmi pengguna. **TIDAK ADA KODE YANG DIUBAH** sampai pengguna memberikan persetujuan tertulis bertuliskan **"SETUJU"**.

---

> [!WARNING]
>
> ### PERINGATAN WAJIB BAGI PENGGUNA SEBELUM PENGUJIAN MANUAL
>
> Sebelum menguji operasi hapus/delete secara manual di antarmuka pengguna pada data asli, pengguna **WAJIB** membuat salinan cadangan (_backup_) seluruh direktori `/map_storage/` (terutama subdirektori `/map_storage/spatial_results/` dan `/map_storage/layers/`) ke lokasi aman di luar direktori proyek.

---

### 12.1 Rencana Pembongkaran Kode `sp_results` & Pemakaian Ulang Tab UI

Seluruh infrastruktur entitas baru `sp_results` yang sempat dibangun diusulkan untuk **dihapus bersih**, dan tab antarmuka pengguna (dinamai resmi: **"Hasil Analisis"**) dipakai ulang (_reused_) untuk mengelola data `spatial_results` pengguna.

#### A. Daftar Berkas yang Diusulkan untuk Dihapus / Direfaktor (Menunggu Persetujuan)

| Jalur Berkas                                            | Komponen / Lapisan   | Status Usulan         | Alasan Tindakan                                                                                                                                                                                                                                                                                                                                     |
| :------------------------------------------------------ | :------------------- | :-------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/modules/module_03/sp_result/types.rs`    | Backend Rust (DTO)   | **Dihapus**           | Memuat DTO entitas terpisah `StoredSpResult`, `SpResultSummary`, `CreateSpResultPayload`, dsb. yang tidak lagi digunakan.                                                                                                                                                                                                                           |
| `src-tauri/src/modules/module_03/sp_result/commands.rs` | Backend Rust (IPC)   | **Direfaktor Bersih** | Menghapus command usang (`list_sp_results`, `create_sp_result`, `attach_sp_result_photo`, dsb.). Hanya menyisakan wrapper tipis untuk command resmi: `get_saved_spatial_results`, `get_saved_spatial_results_summary`, `get_saved_spatial_result`, `update_saved_spatial_result`, `count_spatial_result_dependents`, `delete_saved_spatial_result`. |
| `src-tauri/src/modules/module_03/sp_result/store.rs`    | Backend Rust (Store) | **Direfaktor Bersih** | Membuang seluruh logika folder `/sp_results/` (baris 1–618). Memusatkan logika persistensi atomic murni pada `/map_storage/spatial_results/` (dual-reader, safe update JSON patching, transactional delete 2-fase).                                                                                                                                 |
| `src-tauri/tests/module_03_sp_result_crud_test.rs`      | Test Rust            | **Dihapus**           | Dihapus total karena menguji kontrak entitas `sp_results` yang dibatalkan. Digantikan dengan test suite baru `src-tauri/tests/module_03_spatial_results_crud_test.rs`.                                                                                                                                                                              |
| `src/modules/module-03/types/spatialResult.ts`          | Frontend TS          | **Dihapus**           | Tipe frontend untuk entitas baru `StoredSpResult` dihapus. Digantikan oleh ekstensi tipe resmi di `types/map.ts`.                                                                                                                                                                                                                                   |
| `src/modules/module-03/molecules/AttachPhotoModal.tsx`  | Frontend React       | **Dihapus**           | Modal pengunggahan file foto lokal ke subfolder hasil tidak lagi relevan (menggunakan Opsi 1: `image_url`).                                                                                                                                                                                                                                         |
| `src/modules/module-03/lib/photoUrl.ts`                 | Frontend Utility     | **Dihapus**           | Helper resolusi `/map_storage/sp_results/{id}/photo.jpg` tidak lagi diperlukan.                                                                                                                                                                                                                                                                     |
| `src-tauri/src/lib.rs` & `capabilities/default.json`    | Tauri Registration   | **Dibersihkan**       | Menghapus registrasi handler dan permission untuk command `sp_result` yang dieliminasi.                                                                                                                                                                                                                                                             |

#### B. Komponen UI yang Dipakai Ulang (_Reused_) untuk `spatial_results`

1. **`SpResultListPanel.tsx`**: Dipakai ulang sebagai panel utama daftar Spatial Results asli pengguna (menampilkan filter tipe geometri, pencarian nama, dan list kartu).
2. **`SpResultCardItem.tsx`**: Dipakai ulang untuk merender kartu data `spatial_results` (nama layer, badge tipe geometri, jumlah fitur, tag warna, thumbnail/placeholder, tombol Fokus, tombol Edit, dan tombol Hapus).
3. **`EditSpResultModal.tsx`**: Dipakai ulang untuk modal edit aman (hanya mengubah nama layer, warna HEX, dan icon).
4. **`DeleteSpResultAlert.tsx`**: Dipakai ulang untuk dialog konfirmasi bahaya sebelum menghapus 1 spatial result (memanggil `count_spatial_result_dependents`).
5. **`ResultPhoto.tsx`**: Dipakai ulang sebagai komponen penampil foto thumbnail / placeholder "No Image".

---

### 12.2 Audit Mendalam Berkas `/map_storage/spatial_results` Saat Ini

Struktur fisik direktori saat ini:

```
map_storage/spatial_results/
├── test_sr_decomp_01/                 <-- Format Dekomposisi Folder (Unit Test)
│   ├── layer.json                     (Metadata layer: id, nama, display_preference, feature_count)
│   └── features/
│       ├── T-001.json                 (Fitur titik pohon T-001 + geometry + properties)
│       └── T-002.json                 (Fitur titik pohon T-002 + geometry + properties)
├── result_landcover_001.json          <-- Flat File JSON Tunggal (Data Asli Pengguna)
├── result_line_001.json               <-- Flat File JSON Tunggal (Data Asli Pengguna)
├── result_tiff_ortho_001.json         <-- Flat File JSON Tunggal (Data Asli Pengguna)
├── result_tree_001.json               <-- Flat File JSON Tunggal (Data Asli Pengguna)
├── result_tree_002.json               <-- Flat File JSON Tunggal (Data Asli Pengguna)
└── result_vegetation_001.json         <-- Flat File JSON Tunggal (Data Asli Pengguna)
```

#### A. Rincian Format & Isi Setiap Item

| Nama Item                    | Format Penyimpanan                                        | Ukuran                       | Status Asal Data                                                                                        | Ringkasan Isi Data                                                                                                                                                                 |
| :--------------------------- | :-------------------------------------------------------- | :--------------------------- | :------------------------------------------------------------------------------------------------------ | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `test_sr_decomp_01`          | **Dekomposisi Folder** (`layer.json` + `features/*.json`) | 217 B (meta) + 520 B (fitur) | **Unit Test Rust** (`test_spatial_results_atomic_decomposition...` di `module_03_storage_crud_test.rs`) | 2 fitur titik pohon kelapa sawit di koordinat Balikpapan (`[116.858, -1.221]` dan `[116.859, -1.222]`) dengan icon `"tree"` dan warna `#2ECC71`.                                   |
| `result_landcover_001.json`  | **Flat File JSON** (`LayerPayload`)                       | 1.506 B                      | **Data Bawaan (Indikasi Mock/Sampel)**                                                                  | Klasifikasi Tutupan Lahan Sektor Kariangau: 2 poligon tutupan lahan (`bare_soil` 980 m² dan `built_area` 760 m²), warna `#E67E22`, foto `/sample_drone.jpg`.                       |
| `result_line_001.json`       | **Flat File JSON** (`LayerPayload`)                       | 751 B                        | **Data Bawaan (Indikasi Mock/Sampel)**                                                                  | Jalur Inspeksi Kariangau: 1 fitur LineString sepanjang 450,5 meter (tipe jalan tanah), warna `#E74C3C`, foto `/sample_drone.jpg`.                                                  |
| `result_tiff_ortho_001.json` | **Flat File JSON** (`LayerPayload`)                       | 893 B                        | **Data Bawaan (Indikasi Mock/Sampel)**                                                                  | Citra Orthomosaic GeoTIFF Sektor Kariangau: 1 poligon area 724.500 m² dengan preferensi tampilan `image_overlay` dan koordinat bounding box WGS84.                                 |
| `result_tree_001.json`       | **Flat File JSON** (`LayerPayload`)                       | 1.240 B                      | **Data Bawaan (Indikasi Mock/Sampel)**                                                                  | Deteksi Pohon Sektor A: 3 titik pohon individu (`T-001`, `T-002`, `T-003`) dengan estimasi tinggi (8,4 m – 12,5 m), warna `#27AE60`, icon `tree_marker`, foto `/sample_drone.jpg`. |
| `result_tree_002.json`       | **Flat File JSON** (`LayerPayload`)                       | 549 B                        | **Data Bawaan (Indikasi Mock/Sampel)**                                                                  | Deteksi Pohon Titik B: 1 titik pohon individu (`T-004`) tinggi 12,5 m, warna `#27AE60`, icon `tree_marker`, foto `/sample_drone.jpg`.                                              |
| `result_vegetation_001.json` | **Flat File JSON** (`LayerPayload`)                       | 867 B                        | **Data Bawaan (Indikasi Mock/Sampel)**                                                                  | Deteksi Vegetasi (ExG) Sektor Kariangau: 1 poligon area 1.420,5 m² dengan skor kehijauan rata-rata 0,68, warna `#2ECC40`, foto `/sample_drone.png`.                                |

#### B. Mengapa UI Tidak Menampilkan Data Lama Pengguna?

1. **Filter Eksklusif Direktori di Backend (`sp_result/store.rs`)**:
   Fungsi `get_saved_spatial_results()` pada baris 725 memuat:
   ```rust
   if !path.is_dir() {
       continue; // MENGABAIKAN SEMUA BERKAS FLAT .json!
   }
   ```
   Akibatnya seluruh berkas flat `result_*.json` langsung dilewati.
2. **Tab "Hasil Ukur" Membaca Folder yang Keliru**:
   Sidebar panel tab "Hasil Ukur" (`SpResultListPanel.tsx`) memanggil `list_sp_results` yang secara eksklusif memindai folder `/map_storage/sp_results/`, bukan `/map_storage/spatial_results/`.
3. **Tab "Layers" Hanya Membaca Registry `/map_storage/layers/`**:
   Tab "Layers" memanggil `get_saved_layers`. Berkas `result_*.json` belum terdaftar di `/map_storage/layers/`, sehingga tidak terlihat di Layer Manager.

#### C. Catatan: Indikasi Data Contoh / Mock pada Data Lama

Berdasarkan hasil audit data, terdapat indikasi kuat bahwa berkas-berkas di atas merupakan data contoh (_mock dataset_) dari fase pengembangan sebelumnya:

1. **Sisa Dependensi TIFF**: Berkas `result_tiff_ortho_001.json` merujuk ke path `display_preference.image_url: "/map_storage/images/tiff/600fcc9055481e0005e3129e.tif"`. Format GeoTIFF dan library eksternalnya telah dihapus total pada revisi arsitektur sebelumnya sehingga file TIFF ini sudah tidak dapat diproses lagi secara native. Penanganan layer ini harus gagal secara mulus (placeholder pada kartu, tanpa crash/error di peta).
2. **Foto Merujuk Berkas Statis Dummy**: Berkas `result_landcover_001.json`, `result_line_001.json`, `result_tree_001.json`, dan `result_tree_002.json` semuanya merujuk ke `"/sample_drone.jpg"` (berkas statis 1,4 KB di folder `public/`). Begitu pula `result_vegetation_001.json` merujuk ke `"/sample_drone.png"` (340 byte di `public/`).
3. **Koordinat Terfokus di Kariangau**: Seluruh koordinat berada di kluster mock Kariangau Balikpapan (`116.85...`, `-1.22...`).

> **Keputusan Pengguna**:
>
> - Data berkas `result_*.json` tetap **dipertahankan** sebagai data demonstrasi awal aplikasi, dan tidak diubah kecuali lewat operasi UI.
> - Berkas `result_tiff_ortho_001.json`: dipertahankan dan dapat dihapus lewat UI setelah fungsionalitas CRUD selesai.
> - Nama tab di sidebar UI ditetapkan menjadi: **"Hasil Analisis"**.

---

### 12.3 Rancang Bangun CRUD Dual-Reader di Atas `/map_storage/spatial_results`

#### 1. Arsitektur Dual-Reader (Koeksistensi Format)

Backend Rust akan secara transparan membaca kedua bentuk format berkas yang ada:

- **Format Dekomposisi Folder** (`{id}/layer.json` + `{id}/features/*.json`).
- **Format Flat File JSON** (`{id}.json` atau `result_{id}.json` yang memuat struktur `LayerPayload`).
  Keduanya dinormalisasi menjadi model `LayerPayload` yang sama tanpa kehilangan data.

#### 2. Operasi CRUD yang Dirancang

| Operasi                          | Command Rust / API                  | Parameter Input                       | Output                       | Deskripsi & Proteksi Keamanan                                                                                                                                                                                                                                                                                 |
| :------------------------------- | :---------------------------------- | :------------------------------------ | :--------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **List Ringan**                  | `get_saved_spatial_results_summary` | `None`                                | `Vec<SpatialResultSummary>`  | Membaca metadata cepat dari kedua format berkas. Mengembalikan `layer_id`, `layer_name`, `feature_count`, `geom_type` (`Point`/`Polygon`/`Line`/`Overlay`), `display_color`, `icon`, `thumbnail_url`, `format_type`, dan status runtime `is_corrupted`.                                                       |
| **Detail 1 Item**                | `get_saved_spatial_result`          | `layer_id: String`                    | `LayerPayload`               | Memuat seluruh fitur, koordinat GeoJSON lengkap, dan atribut analitik dari layer terkait. Validasi sanitasi ID untuk mencegah path traversal.                                                                                                                                                                 |
| **Update Field Aman**            | `update_saved_spatial_result`       | `payload: UpdateSpatialResultPayload` | `LayerPayload`               | **Hanya memperbarui field presentasi pada JSON asli**: `layer_name`, `display_preference.color`, dan `display_preference.icon`. Seluruh isi lainnya **IMMUTABLE** (dilindungi mekanisme JSON patching semantik). Emit event `spatial_result:saved`.                                                           |
| **Hitung Dampak Hapus**          | `count_spatial_result_dependents`   | `id: String`                          | `DeleteSpatialResultSummary` | Menghitung dependensi sebelum dialog konfirmasi hapus: memeriksa apakah berkas layer di `/map_storage/layers/{id}.json` ada, menghitung jumlah fitur, dan mengonfirmasi bahwa aset foto statis tidak akan terhapus.                                                                                           |
| **Delete 1 per 1 Transaksional** | `delete_saved_spatial_result`       | `layer_id: String`                    | `DeleteSpatialResultSummary` | **Single-Item Delete** dengan karantina 2-fase: rename ke `.trash_sr_*`, hapus layer tampilan di `/map_storage/layers/{layer_id}.json` bila ada, rollback otomatis jika gagal, bersihkan permanen jika sukses. **TIDAK PERNAH menghapus file yang dirujuk `image_url`**. Emit event `spatial_result:deleted`. |

---

### 12.4 Aturan Penamaan ID & Resolusi Bentrok

#### 1. Aturan Penentuan ID

- **Aturan Mutlak**: $\text{ID} = \text{nama folder}$ atau $\text{nama berkas tanpa ekstensi } \texttt{".json"}$, **persis apa adanya** (_as-is_), tanpa menebak, menambah, atau memotong prefix `result_`.
  - Berkas `result_landcover_001.json` $\to$ ID resmi: `"result_landcover_001"`.
  - Berkas `result_line_001.json` $\to$ ID resmi: `"result_line_001"`.
  - Folder `test_sr_decomp_01/` $\to$ ID resmi: `"test_sr_decomp_01"`.
  - Berkas `my_layer.json` $\to$ ID resmi: `"my_layer"`.
- **Independensi Field `layer_id` di Dalam JSON**: Nilai `layer_id` di dalam konten JSON diperlakukan sebagai data payload biasa. Jika terdapat perbedaan antara `layer_id` di dalam JSON dengan nama berkas di disk (misalnya berkas `result_survey.json` memuat `"layer_id": "different_id"`), sistem CRUD storage tetap konsisten menggunakan nama berkas (`"result_survey"`) sebagai kunci identitas entitas.

#### 2. Regex Allowlist ID Ketat

Setiap operasi yang menerima parameter ID wajib memvalidasi input terhadap regex allowlist:
$$\text{Allowlist ID Regex}: \quad \mathbf{\wedge[A-Za-z0-9\_-]{1,64}\$}$$

- **Diterima**: `"result_tree_001"`, `"test_sr_decomp_01"`, `"layer-99"`, `"sp_123"`.
- **Ditolak Keras**: String kosong `""`, string > 64 karakter, karakter spasi, titik (`.`), slash (`/`), backslash (`\`), serta karakter khusus lainnya (`../`, `..\\`, `$`, `;`).
- Pelanggaran validasi langsung mengembalikan error `CommandError::Validation("ID layer tidak valid...")` sebelum operasi I/O disk apa pun dijalankan.

#### 3. Aturan Resolusi Bentrok ID (Folder vs Flat File)

Jika di dalam direktori `/map_storage/spatial_results/` terdapat folder `{id}/` **DAN** flat file `{id}.json` (atau `result_{id}.json`) dengan ID yang sama:

1. **Folder Dekomposisi Diprioritaskan**: Folder `{id}/` dianggap sebagai representasi layer aktif utama.
2. **Flat File Bayangan Diabaikan Saat Baca**: Saat list summary dipindai, hanya data dari folder yang dimasukkan ke daftar agar kartu di UI tidak duplikat.
3. **Pencatatan Peringatan (_Warning Log_)**: Sistem mencatat pesan warning ke log/konsol mengenai adanya berkas flat bayangan.
4. **Sinkronisasi Saat Mutasi/Hapus**: Jika pengguna menghapus layer tersebut, mekanisme hapus 2-fase mengarantina folder `{id}/` sekaligus memindahkan flat file bayangannya ke karantina `.trash_sr_*` agar tidak ada file usang yang tertinggal.

---

### 12.5 Aturan Resolusi Foto Kartu (Opsi 1: `image_url`)

Sesuai keputusan pengguna, mekanisme foto menggunakan **Opsi 1**:

- Tidak membuat berkas `photo.jpg` baru.
- Memanfaatkan field `image_url` yang sudah ada pada data JSON.
- Tampilkan foto jika ada; jika tidak ada, tampilkan placeholder "No Image".
- Penanganan error pemuatan gambar (_broken link_) ditangani via fallback `onError` pada komponen `ResultPhoto`.

#### Aturan Resolusi Foto pada Kartu UI (Hierarki Prioritas Konsisten):

1. **Prioritas 1 (Layer-Level)**:
   Periksa properti `display_preference.image_url` pada level layer (contoh: pada `result_tiff_ortho_001.json`). Jika string tersebut ada dan tidak kosong, gunakan nilai ini sebagai URL thumbnail kartu.
2. **Prioritas 2 (Fitur Pertama / First Feature)**:
   Jika pada level layer tidak ada `image_url`, lakukan iterasi sekuensial pada array `spatial_results`. Ambil nilai `image_url` dari fitur pertama yang memilikinya:
   - Cek `feature.properties.image_url` (seperti pada `result_landcover_001.json` atau `result_tree_001.json`).
   - Cek `feature.image_url` (level fitur langsung).
     Gunakan URL dari fitur pertama yang valid dan tidak kosong.
3. **Prioritas 3 (Tanpa Foto $\to$ Placeholder "No Image")**:
   Jika seluruh fitur tidak memiliki `image_url` (atau bernilai `null`/kosong), kartu mengembalikan `thumbnail_url = null`. Komponen `ResultPhoto` menampilkan placeholder Blueprint `media` berlabel teks _"No Image"_.
4. **Fallback `onError` pada Komponen `ResultPhoto`**:
   Jika URL gambar menghasilkan error pemuatan di browser (misalnya berkas statis hilang atau HTTP 404), event `onError` pada elemen `<img>` otomatis beralih menampilkan placeholder elegan _"No Image / Gagal Memuat"_ tanpa merusak struktur visual kartu.

---

### 12.6 Mekanisme Safe Update Berbasis Patching JSON Asli & Semantik Data

#### A. Klarifikasi Identitas Semantik vs Format Teks

Pembaruan data dilakukan dengan membaca JSON asli sebagai `serde_json::Value` (dinamis), memodifikasi key yang diizinkan, lalu menulis kembali ke disk:

1. **Klaim Semantik**: Seluruh data non-target terbukti **identik secara semantik** (`Value` sama: koordinat GeoJSON float `f64`, boolean, string, array, dan sub-objek properti analitik kustom tidak mengalami perubahan nilai).
2. **Efek ke Urutan Key & Format Teks**:
   - `serde_json::Value` menggunakan `BTreeMap` internal untuk merepresentasikan objek JSON. Ketika diserialisasi kembali dengan `to_string_pretty`, key-key JSON dapat terurut secara leksikografis (alphabetical order) dan whitespace/indentasi distandarisasi ke 2 spasi.
   - **Batasan Cargo.toml**: Fitur seperti `preserve_order` pada crate `serde_json` **TIDAK AKAN DITAMBAHKAN** ke `src-tauri/Cargo.toml` tanpa persetujuan eksplisit dari pengguna. Urutan leksikografis standar serde_json sepenuhnya valid GeoJSON dan aman.

#### B. Alur Patching JSON Asli:

1. Baca berkas JSON sumber dari disk sebagai `serde_json::Value`.
2. Validasi bahwa nilai akar adalah Object JSON (`Value::Object`).
3. Perbarui **HANYA** key yang diizinkan sesuai payload:
   - Jika `payload.name` disediakan (1..100 karakter):
     - Pada flat JSON: perbarui key `"layer_name"`.
     - Pada format folder: perbarui key `"layer_name"` di `layer.json`.
   - Jika `payload.display_color` disediakan:
     - Validasi dengan fungsi murni regex HEX: `^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$`.
     - Jika valid, perbarui nilai string pada `"display_preference"."color"`.
   - Jika `payload.icon` disediakan:
     - Perbarui nilai string pada `"display_preference"."icon"`.
   - Perbarui field ISO-8601 UTC timestamp `"updated_at"`.
4. **SELURUH KEY LAINNYA TIDAK DISENTUH SECARA SEMANTIK**:
   Array `"spatial_results"`, objek `"geometry"`, array `"coordinates"`, `"source"`, serta seluruh kamus `"properties"` analitik dijamin **identik secara semantik** dengan isi aslinya.
5. Tulis kembali perubahan ke disk secara atomik (`write_atomic`).
6. Jika layer terkait terdaftar di `/map_storage/layers/{id}.json`, perbarui pula metadata presentasi layer tersebut.
7. Dispatch event global `spatial_result:saved`.

---

### 12.7 Mekanisme Delete Transaksional & Proteksi Berkas Bersama (`image_url`)

#### A. Proteksi Aset Bersama (`image_url`)

- Operasi `delete_saved_spatial_result` **MUTLAK TIDAK BOLEH** menghapus atau memodifikasi berkas fisik yang dirujuk oleh field `image_url` (seperti `/sample_drone.jpg` atau `/sample_drone.png`).
- Berkas tersebut adalah aset statis bersama (_shared assets_) yang tersimpan di direktori aset publik atau direktori foto bersama, dan mungkin dirujuk oleh layer lain atau modul lain.

#### B. Alur Transaksional Dua Fase & Rollback

1. **Fase 1 (Karantina Sementara)**:
   - Jika target berupa folder: rename folder `{id}` menjadi `.trash_sr_{id}_{timestamp}`.
   - Jika target berupa berkas flat: rename berkas `{id}.json` menjadi `.trash_sr_{id}_{timestamp}.json`.
2. **Fase 2 (Pembersihan Registry Layer Peta)**:
   - Cek apakah berkas tampilan peta `/map_storage/layers/{id}.json` ada. Jika ada, hapus berkas tersebut.
3. **Rollback Otomatis Saat Gagal**:
   - Jika penghapusan berkas layer pada Fase 2 mengalami kegagalan I/O (mis. masalah permission), folder/berkas karantina `.trash_sr_*` **segera di-rename kembali ke nama aslinya**. Transaksi dibatalkan dan data spatial results tetap utuh.
4. **Finalisasi Sukses**:
   - Berkas karantina `.trash_sr_*` dihapus permanen dari disk.
   - Dispatch event global `spatial_result:deleted` dengan payload `{ id }`.

---

### 12.8 Mekanisme Rendering Peta Tanpa Registry Layers & Cara Kerja Tombol Fokus

#### A. Bagaimana Data yang Tidak Terdaftar di `layers/` Dirender di Peta?

- Peta Leaflet Modul 03 memiliki mekanisme rendering mandiri via organism [`SpatialResultLayer.tsx`](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/SpatialResultLayer.tsx) yang memanggil hook [`useSpatialResultLayers.ts`](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/hooks/useSpatialResultLayers.ts).
- Hook ini memanggil backend command `get_saved_spatial_results` secara langsung (yang memindai direktori `/map_storage/spatial_results/`).
- Setiap `LayerPayload` dikonversi menjadi Leaflet LayerGroup oleh [`buildLayerGroupWithPoints`](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/lib/leaflet/spatialLayerBuilder.ts) dan langsung ditambahkan ke instance peta Leaflet (`map.addLayer`) berdasarkan filter Level of Detail (LOD) zoom:
  - Poligon tutupan lahan & vegetasi muncul pada zoom $\ge 15$.
  - Titik pohon individu muncul pada zoom $\ge 17$.
- **Kesimpulan**: Berkas di `/map_storage/layers/` hanyalah registrasi metadata untuk tab sidebar "Layers" (toggle switch visibilitas/opacity). **Data di `spatial_results/` langsung tampil di peta tanpa bergantung pada keberadaan berkas di `/map_storage/layers/`**.
- Saat data diubah atau dihapus, event `spatial_result:saved` dan `spatial_result:deleted` memperbarui peta secara instan.

#### B. Bagaimana Tombol "Fokus" Bekerja?

- Tombol Fokus (`icon="locate"`) pada kartu item bekerja dengan menghitung Bounding Box (BBox) WGS84 dari seluruh fitur yang ada pada layer tersebut:
  1. Untuk fitur **Point**: Ekstrak koordinat `[lat, lng]`.
  2. Untuk fitur **LineString / Polygon**: Iterasi seluruh titik koordinat untuk mencari nilai ekstrem:
     $$\text{South} = \min(\text{lat}), \quad \text{West} = \min(\text{lng}), \quad \text{North} = \max(\text{lat}), \quad \text{East} = \max(\text{lng})$$
  3. Bentuk Leaflet bounds: `L.latLngBounds([[south, west], [north, east]])`.
  4. Panggil animasi kamera peta:
     ```typescript
     map.fitBounds(bounds, {
       padding: [50, 50],
       maxZoom: 18,
       animate: true,
       duration: 1.2,
     });
     ```
  5. Peta meluncur mulus (_fly to bounds_) dan memusatkan pandangan layar tepat ke objek hasil pengukuran.

---

### 12.9 Isolasi Test Murni Tempdir & Perbaikan Kebocoran `test_sr_decomp_01`

#### A. Akar Penyebab Kebocoran `test_sr_decomp_01` ke `map_storage` Asli

Pada test `test_spatial_results_atomic_decomposition_and_size_limits` di [module_03_storage_crud_test.rs baris 225](file:///d:/project/Aerial-Analytics-Platform/src-tauri/tests/module_03_storage_crud_test.rs#L225):

- Pemanggilan `state.save_spatial_result(payload)` memanggil fungsi `sp_result::store::save_spatial_result(payload)`.
- Fungsi store tersebut memanggil `get_spatial_results_dir()` yang memanggil `get_storage_root()`.
- Karena test suite `cargo test` berjalan paralel multi-thread, pembacaan variabel lingkungan `AAP_STORAGE_DIR` rentan _race condition_, sehingga fungsi fallback menulis ke direktori kerja `./map_storage/spatial_results/test_sr_decomp_01` dan `map_storage/layers/test_sr_decomp_01.json`.

#### B. Perbaikan Arsitektur Pengujian

1. **Injeksi Direktori Eksplisit**: Seluruh fungsi persistensi di `sp_result/store.rs` dan metode pada `Module03State` diwajibkan menerima parameter `storage_dir: &Path` atau menggunakan field `self.storage_dir` lokal, **BUKAN** memanggil `get_storage_root()` global.
2. **Isolasi Penuh `tempfile::tempdir`**: Setiap unit test wajib membuat direktori sementara unik yang dioper ke fungsi yang diuji. Tidak ada berkas pengujian yang boleh bocor ke direktori proyek asli.

---

### 12.10 Inventaris Pembersihan Data Dummy/Test (Menunggu Persetujuan Pengguna)

Sesuai instruksi, **TIDAK ADA DATA YANG DIHAPUS SEKARANG**. Berikut adalah daftar lengkap berkas tiruan buatan test/agen yang diusulkan untuk dibersihkan setelah disetujui satu per satu oleh pengguna:

| No  | Jalur Fisik Berkas / Folder                       | Kategori                | Alasan Usulan Pembersihan                                        |
| :-- | :------------------------------------------------ | :---------------------- | :--------------------------------------------------------------- |
| 1   | `/map_storage/spatial_results/test_sr_decomp_01/` | Folder Dekomposisi      | Berkas buatan test yang bocor saat pengujian dekomposisi atomik. |
| 2   | `/map_storage/layers/test_sr_decomp_01.json`      | File Registrasi Layer   | Berkas layer pendamping untuk `test_sr_decomp_01`.               |
| 3   | `/map_storage/sp_results/sp_1792813007663/`       | Folder Hasil Ukur Dummy | Entitas lama buatan agen demo (foto hitam 29 KB, AOI fiktif).    |
| 4   | `/map_storage/layers/sp_sp_1792813007663.json`    | File Registrasi Layer   | Berkas layer pendamping lama dengan bug awalan ganda `sp_sp_`.   |
| 5   | `/map_storage/sp_results/sp_1792813500000/`       | Folder Hasil Ukur Dummy | Entitas lama buatan agen demo (timestamp bulat `...500000`).     |
| 6   | `/map_storage/layers/sp_sp_1792813500000.json`    | File Registrasi Layer   | Berkas layer pendamping lama dengan bug awalan ganda `sp_sp_`.   |
| 7   | `/map_storage/sp_results/sp_1792813800000/`       | Folder Hasil Ukur Dummy | Entitas lama buatan agen demo (timestamp bulat `...800000`).     |
| 8   | `/map_storage/layers/sp_sp_1792813800000.json`    | File Registrasi Layer   | Berkas layer pendamping lama dengan bug awalan ganda `sp_sp_`.   |
| 9   | `/map_storage/sp_results/`                        | Direktori Induk         | Direktori kosong setelah item 3, 5, 7 dibersihkan.               |

_Konfirmasi persetujuan pengguna akan diminta satu per satu sebelum tindakan penghapusan dijalankan._

---

### 12.11 Rencana Test Suite Rust Komprehensif (10 Unit Tests di `tempdir`)

Test suite baru akan dibuat di `src-tauri/tests/module_03_spatial_results_crud_test.rs`. Seluruh test menggunakan `tempfile::tempdir()` murni:

1. **`test_dual_reader_flat_and_decomposed`**:
   - Menyiapkan 1 flat file JSON (`result_tree_001.json`) dan 1 folder dekomposisi (`test_layer/layer.json` + `features/`).
   - Memverifikasi `get_saved_spatial_results` dan list summary berhasil membaca kedua format secara simultan tanpa error.
2. **`test_id_determination_and_mismatched_layer_id`**:
   - Menyiapkan file `result_survey_01.json` yang di dalam kontennya memuat `"layer_id": "different_internal_id"`.
   - Memverifikasi ID entitas resmi yang dikembalikan sistem CRUD storage adalah persis `"result_survey_01"` (berdasarkan nama file tanpa `.json`), bukan menebak atau menggunakan ID internal.
3. **`test_id_collision_folder_precedence`**:
   - Menyiapkan folder `item_a/` dan flat file `item_a.json` secara bersamaan di direktori.
   - Memverifikasi folder `item_a/` diprioritaskan, file flat bayangan diabaikan saat list summary, dan ikut terkarantina saat penghapusan.
4. **`test_corrupted_flat_file_graceful_handling`**:
   - Menulis file `corrupt.json` berisi JSON tidak valid / terpotong.
   - Memverifikasi fungsi list tidak panic/crash dan mengembalikan entri dengan flag runtime `is_corrupted: true`.
5. **`test_safe_update_semantic_identity`**:
   - Menyiapkan berkas layer lengkap dengan geometri kompleks dan custom metadata properties.
   - Memperbarui `layer_name` dan warna `display_preference.color`.
   - Memverifikasi bahwa seluruh field lain (`spatial_results`, `geometry`, `coordinates`, `properties`) terbukti **identik secara semantik** (`Value` sama).
6. **`test_color_hex_regex_validation`**:
   - Menguji validasi warna HEX: nilai `#2ECC71`, `#fff`, `#1A2B3C` lolos; nilai ilegal `red`, `#12345`, `123456`, `#gggggg`, `""` ditolak dengan pesan validasi yang tepat.
7. **`test_delete_preserves_referenced_image_url`**:
   - Menyiapkan layer dengan `image_url: "/sample_drone.jpg"` dan membuat berkas dummy di path aset tersebut.
   - Menjalankan `delete_saved_spatial_result`.
   - Memverifikasi spatial result terhapus bersih, sedangkan berkas fisik gambar yang dirujuk **TETAP UTUH DAN TIDAK DISENTUH**.
8. **`test_transactional_rollback_on_delete_failure`**:
   - Mensimulasikan kegagalan I/O saat menghapus layer tampilan.
   - Memverifikasi berkas karantina `.trash_sr_*` berhasil dipulihkan (_rollback_) ke nama aslinya tanpa kehilangan data.
9. **`test_id_allowlist_and_path_traversal_rejection`**:
   - Menguji input ID ilegal (`../../etc/passwd`, `..\win.ini`, `id with spaces`, `""`, string > 64 karakter) ditolak keras dengan error validasi.
10. **`test_startup_trash_cleanup_and_list_ignores_trash`**:
    - Menyiapkan berkas/folder karantina sisa `.trash_sr_old` di direktori.
    - Memverifikasi bahwa folder karantina otomatis diabaikan saat pemindaian list dan dibersihkan saat fungsi cleanup startup dijalankan.

---

### 12.12 Penundaan Fitur Hasil Ukur Interaktif (Tahap Terpisah)

- Fitur pengukuran interaktif manual di peta (jarak, luas, keliling) **TIDAK DICAMPURADUKKAN** dengan pekerjaan perbaikan CRUD ini.
- Fitur ini ditetapkan sebagai **Tahap Lanjutan Mandiri (Fase Berikutnya)** setelah CRUD spatial results data bawaan selesai, diuji, dan stabil.
- **Desain Masa Depan**: Saat fitur simpan hasil ukur dibangun nanti, hasilnya akan disimpan langsung mengikuti kontrak `LayerPayload` ke dalam `/map_storage/spatial_results/` (misalnya dengan ID `result_measure_{timestamp}.json` dan `source: "measurement"`), sehingga menyatu di dalam arsitektur `spatial_results` tanpa memerlukan entitas atau folder terpisah.

---

### 12.13 Kepatuhan Batasan Proyek & Kontrak Antar Modul

1. **Konsistensi Data & Tanpa Tambahan Dummy**: Seluruh data yang disajikan di UI bersumber dari berkas yang ada di `/map_storage/spatial_results/` (data bawaan `result_*.json` yang terindikasi dataset contoh/mock dari pengembangan awal, dan dipertahankan sebagai data awal sesuai keputusan pengguna). Tidak diklaim sebagai data murni asli, dan tidak ada data dummy baru yang dibuat atau diinjeksi oleh agen.
2. **Tanpa Dependency Baru**: Tidak ada penambahan crate baru di `Cargo.toml` maupun paket baru di `package.json`. Tidak ada perubahan fitur `preserve_order` tanpa persetujuan pengguna.
3. **Tanpa Python**: Seluruh manipulasi dan pembacaan GeoJSON murni native Rust + TypeScript.
4. **Kontrak Antar Modul (Module 10) Terjaga**: Re-export dan signature publik fungsi pada facade `storage.rs` (`save_spatial_result`, `get_saved_spatial_results`, `delete_saved_spatial_result`) tetap dipertahankan sebagai pembungkus tipis tanpa mengubah signature. DTO Module 10 tidak terpengaruh, dan deserialisasi `LayerPayload` tetap kompatibel dengan penambahan key opsional `updated_at`.
5. **Pre-commit Gates Siap Lolos**: Kode baru mematuhi aturan lint clippy, rustfmt, ESLint 9, dan Prettier.

---

### 12.14 Checklist Verifikasi Implementasi (Tahap Eksekusi Mendatang)

- [x] Dual-reader backend berhasil membaca format flat `result_*.json` dan format folder `test_sr_decomp_01`
- [x] 6 Berkas data bawaan (`landcover`, `line`, `tiff_ortho`, `tree_001`, `tree_002`, `vegetation`) tampil utuh di UI
- [x] Layer spasial dari data bawaan berhasil dirender ke peta Leaflet sesuai LOD geometri (zoom level)
- [x] Penanganan layer `result_tiff_ortho_001` gagal dengan mulus (placeholder pada kartu, tanpa crash/error di peta)
- [x] Tombol Fokus berhasil menghitung BBox geometri layer dan memanggil `map.fitBounds`
- [x] List ringan summary mengembalikan informasi cepat tanpa decode geometri penuh
- [x] Detail 1 item memuat koordinat lengkap GeoJSON
- [x] Update aman berbasis JSON patching hanya mengizinkan perubahan `layer_name`, `color` (validasi HEX), dan `icon`
- [x] Immutabilitas semantik koordinat dan data analitik terjamin 100% pada operasi update
- [x] Allowlist regex ID `^[A-Za-z0-9_-]{1,64}$` menolak path traversal dan input berbahaya
- [x] Penentuan ID persis sesuai nama folder atau file tanpa `.json` (termasuk penanganan aman jika `layer_id` di JSON berbeda)
- [x] Hapus 1 per 1 transaksional 2-fase dengan karantina `.trash_sr_*` dan rollback otomatis saat gagal
- [x] Command `count_spatial_result_dependents` menghitung dampak layer sebelum konfirmasi hapus
- [x] Operasi delete dipastikan tidak pernah menyentuh berkas fisik yang dirujuk oleh `image_url`
- [x] Event global `spatial_result:saved` dan `spatial_result:deleted` memperbarui peta dan sidebar secara reaktif
- [x] Komponen `ResultPhoto` menerapkan Opsi 1 (resolusi layer-level vs first-feature, placeholder "No Image", fallback `onError`)
- [x] Tidak ada file `sp_sp_*` setelah pembersihan
- [x] Seluruh test unit Rust menggunakan `tempdir` murni tanpa kebocoran ke `map_storage`
- [x] Pembersihan berkas dummy/test dilaksanakan bertahap sesuai persetujuan pengguna
- [x] Seluruh pre-commit quality gates (`npm run lint`, `npm run lint:rust`, `npm run format:check`, `npm run format:rust:check`, `npm run build`) lolos 100%

---

## 13. Perbaikan Konsistensi Data + "Field" Fleksibel pada Spatial Results (Module 3)

> **PANDUAN EKSEKUSI (TAHAP T0 - REVISI)**: Dokumen ini merupakan spesifikasi arsitektur teknis dan kontrak implementasi resmi hasil revisi komprehensif berdasarkan 14 arahan pengguna. Sesuai aturan wajib, **TIDAK ADA KODE ATAU BERKAS DATA YANG DIUBAH** sampai pengguna memberikan konfirmasi tertulis bertuliskan **"SETUJU"**.

---

### 13.1 Konteks & Ruang Lingkup Masalah

Layanan CRUD untuk `spatial_results` di Module 3 (mendukung koeksistensi format berkas flat `.json` dan format folder dekomposisi pada `/map_storage/spatial_results/`) telah beroperasi. Namun, audit terhadap berkas data riil dan kode menghasilkan temuan inkonsistensi yang perlu diperbaiki:

1. **Inkonsistensi Penyimpanan Foto**: URL gambar tersimpan di 3 tempat berbeda (`display_preference.image_url`, `spatial_results[i].image_url`, `spatial_results[i].properties.image_url`). Pada `result_tree_001.json`, ketiganya tertimpa path unggahan yang sama.
2. **Inkonsistensi Tipe Geometri & UI**: `display_preference.type` bernilai `"point"` pada layer yang sebenarnya berisi geometri `Polygon` dan `LineString`. Jenis geometri untuk kartu dan popup harus diturunkan secara akurat dari geometri fitur aktual.
3. **Pilihan Ikon Bebas vs Kemampuan Renderer**: Input ikon pada modal edit berupa teks bebas, padahal renderer Leaflet saat ini hanya menggambar `circleMarker` dan belum mengevaluasi string ikon secara visual.
4. **Format Timestamp `updated_at`**: Variasi format antara mikrodetik dengan zona `+00:00` dan standar RFC3339 UTC `Z`.
5. **Urutan Kunci JSON (Alphabetical vs Natural)**: Serialisasi bawaan `serde_json::Value` menyusun kunci secara alfabetis (BTreeMap) yang mengubah urutan asli berkas.
6. **Berkas Foto Unggahan Yatim (Orphan Photos)**: Berkas foto hasil unggahan di `/map_storage/images/result_image/{layer_id}_{timestamp}.ext` belum terikat siklus hidup transaksional saat layer dihapus atau diganti.
7. **Kekakuan Atribut (Properties)**: Tabel popup peta saat ini membaca seluruh `properties` fitur tanpa urutan terdefinisi, tanpa unit pengukuran formal, dan tidak memungkinkan pengelolaan label, jenis data (`kind`), visibilitas, penambahan field kustom, atau pengeditan nilai secara aman melalui panel UI.

---

### 13.2 Bagian A: Konsistensi Data & Kode

#### A1. Hirarki Penyimpanan, Resolusi Citra / Foto & Keamanan Jalur Rilis

Terdapat pemisahan tegas antara foto level layer (representasi keseluruhan layer atau citra udara) dengan foto level fitur (foto spesifik hasil deteksi per objek/fitur):

1. **Aturan Penulisan (Writer)**:
   - **Foto Layer** disimpan eksklusif pada `display_preference.image_url`. Operasi "Ganti Foto" atau "Hapus Foto" pada panel edit layer **HANYA** memodifikasi `display_preference.image_url` dan **TIDAK PERNAH** menyentuh `image_url` milik fitur.
   - **Foto Fitur** disimpan pada `spatial_results[i].image_url` (dihasilkan oleh plugin atau pipeline analitik). Backend tidak pernah menulis lagi ke dalam `spatial_results[i].properties.image_url`.
2. **Aturan Pembacaan (Reader Legacy Fallback)**:
   - Untuk fitur lama yang belum dimigrasi, pembaca tetap menerima `properties.image_url` sebagai fallback jika `spatial_results[i].image_url` tidak ada.
3. **Resolusi Thumbnail Kartu Sidebar (Fungsi Murni `resolveCardThumbnail`)**:
   - `Prioritas 1`: Foto layer (`display_preference.image_url`).
   - `Prioritas 2`: Foto fitur pertama yang valid (`spatial_results[0].image_url` atau `properties.image_url`).
   - `Prioritas 3`: Placeholder `"No Image"`.
4. **Resolusi Foto Popup Fitur Peta (Fungsi Murni `resolveFeaturePhoto`)**:
   - `Prioritas 1`: Foto spesifik fitur tersebut (`feature.image_url` atau legacy `feature.properties.image_url`).
   - `Prioritas 2`: Foto layer (`display_preference.image_url`).
   - `Prioritas 3`: Placeholder `"No Image"`.
5. **Standarisasi URL / Path (`resolveImageSrc`) & Keamanan Path Traversal**:
   - URL protokol eksternal dicek **LEBIH DULU** sebelum manipulasi string lokal dilakukan.
   - Deteksi dan penolakan keras terhadap pola path traversal (`..`).
   - Dukungan penuh desktop release build (Tauri): Pada mode desktop (`isTauri()`), berkas lokal `/map_storage/...` dilayani melalui `convertFileSrc` atau protokol aset Tauri (`asset://`) sehingga foto unggahan tetap muncul pada build rilis tanpa dev server.

```typescript
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";

export function resolveImageSrc(rawUrl: string | null | undefined): string | null {
  if (!rawUrl) return null;
  const trimmed = rawUrl.trim().replace(/\\/g, "/");
  if (!trimmed) return null;

  // 1. URL Protokol Eksternal / Data URI / Asset URI dicek LEBIH DULU
  if (/^(https?:\/\/|data:|asset:)/i.test(trimmed)) {
    return trimmed;
  }

  // 2. Tolak keras path traversal ("..")
  if (trimmed.includes("..")) {
    console.warn("Peringatan Keamanan: Path traversal terdeteksi pada image URL:", trimmed);
    return null;
  }

  // 3. Tangani berkas penyimpanan /map_storage/...
  const storageIdx = trimmed.indexOf("map_storage/");
  if (storageIdx !== -1) {
    const relativePath = trimmed.substring(storageIdx); // "map_storage/..."
    if (isTauri()) {
      // Pada Tauri desktop (dev maupun release build), gunakan asset protocol
      return convertFileSrc(relativePath);
    }
    // Pada web browser / dev server, dilayani oleh middleware Vite
    return `/${relativePath}`;
  }

  // 4. Aset publik lokal (/sample_drone.*)
  if (trimmed.startsWith("/")) {
    return trimmed;
  }
  return `/${trimmed}`;
}
```

6. **Pencegahan Path Traversal pada Penyaji Middleware (`vite.config.ts`)**:
   Middleware `serveMapStorage` memverifikasi canonical path untuk memastikan berkas yang diminta tidak keluar dari root `map_storage`:
   ```typescript
   const safePrefix = path.resolve(process.cwd(), "map_storage");
   const requestedPath = path.resolve(safePrefix, rawUrl.replace(/^\//, ""));
   if (!requestedPath.startsWith(safePrefix) || rawUrl.includes("..")) {
     res.statusCode = 403;
     return res.end("Forbidden: Path traversal not allowed");
   }
   ```

#### A2. Geometri UI Diturunkan dari Fitur Aktual & Penanganan `display_preference.type`

1. **Turunan Geometri Murni**:
   - Jenis geometri yang ditampilkan pada kartu UI dan tag informasi diturunkan murni dari array `spatial_results[i].geometry.type`:
     - Jika seluruh fitur bertipe `Point` -> `"Point"` (Label UI: "Titik").
     - Jika seluruh fitur bertipe `Polygon` -> `"Polygon"` (Label UI: "Poligon").
     - Jika seluruh fitur bertipe `LineString` -> `"Line"` (Label UI: "Garis").
     - Jika campuran berbagai tipe geometri -> `"Mixed"` (Label UI: "Campuran").
     - Jika berupa overlay citra -> `"Overlay"` (Label UI: "Overlay").
2. **Koreksi Penulisan `display_preference.type`**:
   - Backend tidak akan pernah menulis nilai `type` yang bertentangan dengan geometri fitur aktual.
   - Penambahan varian `Line` pada enum `LayerDisplayMode` (Rust & TypeScript) untuk mengakomodasi layer garis secara formal:
     ```rust
     #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
     #[serde(tag = "type")]
     pub enum LayerDisplayMode {
         #[serde(rename = "polygon")]
         Polygon {
             color: String,
             #[serde(default = "default_opacity")]
             opacity: f64,
             #[serde(default, skip_serializing_if = "Option::is_none")]
             image_url: Option<String>,
             #[serde(default, skip_serializing_if = "Option::is_none")]
             fields: Option<Vec<FieldDef>>,
         },
         #[serde(rename = "point")]
         Point {
             color: String,
             #[serde(default)]
             icon: String,
             #[serde(default, skip_serializing_if = "Option::is_none")]
             image_url: Option<String>,
             #[serde(default, skip_serializing_if = "Option::is_none")]
             fields: Option<Vec<FieldDef>>,
         },
         #[serde(rename = "line")]
         Line {
             color: String,
             #[serde(default = "default_line_weight")]
             weight: f64,
             #[serde(default, skip_serializing_if = "Option::is_none")]
             image_url: Option<String>,
             #[serde(default, skip_serializing_if = "Option::is_none")]
             fields: Option<Vec<FieldDef>>,
         },
         #[serde(rename = "image_overlay")]
         ImageOverlay {
             image_url: String,
             opacity: f64,
             bounds: [[f64; 2]; 2],
             #[serde(default, skip_serializing_if = "Option::is_none")]
             fields: Option<Vec<FieldDef>>,
         },
     }
     ```

#### A3. Pembuktian Ikon Renderer & Dropdown Terkontrol

1. **Fakta Kode Renderer Saat Ini**:
   - Pada [spatialLayerBuilder.ts:L93-114](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/lib/leaflet/spatialLayerBuilder.ts#L93-L114), fungsi `buildFeatureLayer` untuk titik (`case "Point"`) saat ini **HANYA** merender `L.circleMarker` dengan warna `display.color` (baris 94–101) atau default pin marker `L.marker` (baris 113). Nilai `display.icon` **belum dievaluasi** dalam rendering Leaflet.
   - Pada [store.rs:L148-155](file:///d:/project/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/sp_result/store.rs#L148-L155), `extract_presentation` membaca field `icon` dari `LayerDisplayMode::Point` untuk dimasukkan ke `SpatialResultSummary`.
2. **Opsi Ikon yang Benar-Benar Dikenal & Didukung Renderer**:
   Untuk membuktikan bahwa setiap opsi di dropdown dikenal renderer, pada Tahap T1/T2 `spatialLayerBuilder.ts` akan dilengkapi evaluasi ikon eksplisit:
   - `""`: "(Standar / Titik Bulat)" -> Dirender sebagai `L.circleMarker` (sesuai implementasi eksisting [spatialLayerBuilder.ts:L94](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/lib/leaflet/spatialLayerBuilder.ts#L94)).
   - `"tree_marker"`: "Pohon" -> Dirender menggunakan `L.divIcon` yang memuat SVG pohon hijau ringkas.
3. **Kondisional Visibilitas & Preservasi Nilai Kustom**:
   - Dropdown ikon **HANYA** tampil di UI jika layer bertipe geometri `Point`.
   - Jika berkas lama memuat string ikon lain yang tidak terdaftar di atas, nilai tersebut **tetap dipertahankan** dan ditampilkan sebagai opsi terpilih `"(Kustom) {nilai}"` sehingga tidak tertimpa saat formulir disimpan.

#### A4. Standardisasi Timestamp `updated_at`

1. **Format Standar Baru (Writer)**:
   Setiap kali berkas diperbarui, backend menulis format RFC3339 UTC dengan presisi milidetik berakhiran `Z`:
   `chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)`
   Contoh hasil: `"2026-10-24T07:00:28.626Z"`.
2. **Toleransi Pembacaan (Reader)**:
   Parser pembaca menerima format lama mikrodetik dengan offset `+00:00` (seperti `"2026-10-24T07:00:28.626369+00:00"`) maupun berkas yang belum memiliki `updated_at` (menghasilkan `None`).

#### A5. Urutan Kunci JSON & Usulan Fitur `preserve_order` pada `serde_json`

1. **Penyebab & Dampak**:
   Secara default, `serde_json::Value` menyimpan pasangan key-value pada `std::collections::BTreeMap`. Akibatnya, saat `serde_json::to_string_pretty` dijalankan, seluruh kunci diurutkan secara alfabetis (seperti yang terjadi pada `result_tree_001.json`). Hal ini mengaburkan urutan alami berkas (`layer_id`, `layer_name`, `display_preference`, `spatial_results`, `updated_at`) dan menghasilkan git diff yang tidak perlu pada baris-baris yang tidak diubah.
2. **Usulan Solusi Tanpa Crate Baru**:
   Mengaktifkan cargo feature bawaan `preserve_order` pada `serde_json` di `src-tauri/Cargo.toml`:
   ```toml
   serde_json = { version = "1", features = ["preserve_order"] }
   ```
   Fitur ini mengganti backend peta internal `serde_json::Map` menjadi `indexmap` bawaan `serde_json` tanpa menambah dependensi crate baru ke kode aplikasi.
3. **Status Keputusan**:
   Pengguna menyetujui: `preserve_order = <setuju>`. Fitur `preserve_order` telah diaktifkan pada `src-tauri/Cargo.toml`.

#### A6. Pengelolaan Berkas Foto Unggahan, Pencegahan Berkas Yatim & Pemulihan Startup

1. **Konvensi & Kepemilikan Berkas dengan Regex Ketat**:
   - Berkas foto yang diunggah pengguna disimpan di `/map_storage/images/result_image/`.
   - Suatu berkas foto dianggap dimiliki oleh layer jika dan hanya jika:
     1. Berada langsung di dalam folder `map_storage/images/result_image/`.
     2. Nama berkas cocok persis dengan regex:
        `^{layer_id}_\d{10,}\.(jpg|jpeg|png|webp)$` (case-insensitive ekstensi).
     3. Tidak sedang dirujuk oleh layer lain di dalam `spatial_results/`.
   - Berkas aset publik (`/sample_drone.*`, aset di folder `public/`) **TIDAK PERNAH** disentuh, dipindahkan, atau dihapus.
2. **Penghapusan Transaksional 2-Fase pada `delete_saved_spatial_result`**:
   - `Fase 1 (Karantina)`: Memindahkan folder/file layer DAN seluruh berkas foto unggahan milik layer tersebut ke dalam folder karantina `.trash_sr_{layer_id}_{timestamp}/`.
   - `Fase 2 (Hapus Registrasi)`: Menghapus registrasi layer terkait di `storage/layers/{layer_id}.json`.
   - `Rollback Otomatis`: Jika Fase 2 gagal, seluruh berkas di folder karantina dipulihkan kembali ke lokasi asalnya (termasuk berkas foto unggahan).
   - `Finalisasi`: Jika Fase 2 sukses, folder karantina dihapus permanen.
3. **Pemulihan `.trash_sr_*` saat Startup (`cleanup_orphaned_trash`)**:
   - Saat aplikasi / modul berjalan:
     - Jika registrasi layer masih ada di `storage/layers/{safe_id}.json` (misalnya app crash sebelum fase 2), pulihkan kembali file layer dan foto dari `.trash_sr_*` ke tempat asalnya.
     - Jika registrasi layer sudah tidak ada, hapus permanen folder `.trash_sr_*`.
4. **Penggantian / Penghapusan Foto di Panel Edit**:
   - Ketika pengguna memilih "Hapus Foto" atau mengganti foto dengan berkas baru, berkas lama yang cocok dengan regex kepemilikan layer dihapus **SETELAH** berkas JSON layer berhasil ditulis ke disk secara atomik.
   - Kegagalan penghapusan berkas lama dicatat sebagai log peringatan dan **TIDAK** menggagalkan operasi simpan JSON.
5. **Dependensi Informasi (`count_spatial_result_dependents`)**:
   Struktur `DeleteSpatialResultSummary` diperluas dengan field `photos_count: usize` untuk menginformasikan kepada pengguna berapa berkas foto unggahan yang akan ikut terhapus secara permanen.
6. **Validasi Menyeluruh Unggah Foto (`save_result_image`)**:
   Proses penyimpanan foto menerapkan validasi cepat (fast-fail) sebelum penulisan:
   - `Validasi image_path`: Wajib path berkas valid di disk lokal (`Path::new(src_path).is_file()`). Tolak jika direktori atau symlink tak valid.
   - `Validasi Ukuran`: Memeriksa ukuran berkas via metadata (`metadata.len()`). Batas maksimal adalah **10 MB** (`10.485.760 bytes`).
   - `Validasi Magic Bytes`: Membaca 12 byte pertama:
     - JPEG: `[0xFF, 0xD8, 0xFF]`
     - PNG: `[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]`
     - WebP: Diawali `[0x52, 0x49, 0x46, 0x46]` (RIFF) dan `[0x57, 0x45, 0x42, 0x50]` (WEBP).
   - `Validasi Dimensi`: Membaca header via `image::image_dimensions` tanpa mengalokasikan memori piksel penuh. Batas dimensi maksimal: 8.192 piksel per sisi.

#### A7. Laporan Audit Data Spasial Eksisting (Diff Minimal - Read Only)

Sesuai arahan revisi, usulan diff dibatasi secara **minimal**: hanya memperbaiki `type` pada `landcover`, `line`, dan `vegetation`, serta melakukan de-duplikasi foto pada `tree_001`. **Tidak ada penambahan `updated_at` karangan, tidak ada penulisan ulang timestamp `tree_001`, dan tidak ada penambahan `image_url` level layer pada berkas lain.**

Perbaikan data riil hanya dieksekusi pada Tahap T3 setelah pengguna melakukan backup manual terhadap direktori `map_storage` dan menyetujui per berkas secara tertulis.

##### 1. `result_landcover_001.json`

- **Penyimpangan**: `display_preference.type` bernilai `"point"` padahal seluruh fitur bergeometri `Polygon`.
- **Usulan Diff Minimal**:

```diff
--- map_storage/spatial_results/result_landcover_001.json
+++ map_storage/spatial_results/result_landcover_001.json
@@ -5,3 +5,3 @@
   "display_preference": {
-    "type": "point",
+    "type": "polygon",
     "color": "#E67E22",
```

##### 2. `result_line_001.json`

- **Penyimpangan**: `display_preference.type` bernilai `"point"` padahal geometri adalah `LineString`.
- **Usulan Diff Minimal**:

```diff
--- map_storage/spatial_results/result_line_001.json
+++ map_storage/spatial_results/result_line_001.json
@@ -5,3 +5,3 @@
   "display_preference": {
-    "type": "point",
+    "type": "line",
     "color": "#E74C3C",
```

##### 3. `result_tree_001.json`

- **Penyimpangan**: Foto layer menimpa foto fitur sehingga terjadi duplikasi path unggahan yang sama di dalam `spatial_results[i].image_url` dan `properties.image_url`. Timestamp asli `2026-10-24T07:00:28.626369+00:00` dipertahankan apa adanya.
- **Usulan Diff Minimal**:

```diff
--- map_storage/spatial_results/result_tree_001.json
+++ map_storage/spatial_results/result_tree_001.json
@@ -19,3 +19,2 @@
-      "image_url": "/map_storage/images/result_image/result_tree_001_1792825228627.png",
       "properties": {
         "confidence": 0.93,
         "height_est_m": 12.5,
-        "image_url": "/map_storage/images/result_image/result_tree_001_1792825228627.png",
         "tree_id": "T-001"
       },
@@ -36,3 +35,2 @@
-      "image_url": "/map_storage/images/result_image/result_tree_001_1792825228627.png",
       "properties": {
         "confidence": 0.88,
         "height_est_m": 10.2,
-        "image_url": "/map_storage/images/result_image/result_tree_001_1792825228627.png",
         "tree_id": "T-002"
       },
@@ -53,3 +51,2 @@
-      "image_url": "/map_storage/images/result_image/result_tree_001_1792825228627.png",
       "properties": {
         "confidence": 0.77,
         "height_est_m": 8.4,
-        "image_url": "/map_storage/images/result_image/result_tree_001_1792825228627.png",
         "tree_id": "T-003"
       },
```

##### 4. `result_tree_002.json`

- **Hasil Pemeriksaan**: Geometri `Point` cocok dengan `display_preference.type: "point"`. Tidak ada penyimpangan tipe.
- **Usulan Diff**: **Nihil** (tidak ada perubahan berkas).

##### 5. `result_vegetation_001.json`

- **Penyimpangan**: `display_preference.type` bernilai `"point"` padahal geometri adalah `Polygon`.
- **Usulan Diff Minimal**:

```diff
--- map_storage/spatial_results/result_vegetation_001.json
+++ map_storage/spatial_results/result_vegetation_001.json
@@ -5,3 +5,3 @@
   "display_preference": {
-    "type": "point",
+    "type": "polygon",
     "color": "#2ECC40",
```

---

### 13.3 Bagian B: "Field" Fleksibel

#### B1. Model Data Kompatibel Mundur & Perluasan Jalur Baca

1. **Jalur Baca (List & Detail)**:
   - DTO `LayerPayload` dan setiap varian `LayerDisplayMode` diperluas dengan:
     - `#[serde(default, skip_serializing_if = "Option::is_none")] pub image_url: Option<String>`
     - `#[serde(default, skip_serializing_if = "Option::is_none")] pub fields: Option<Vec<FieldDef>>`
   - Dengan perluasan ini, pemanggilan API detail (`get_saved_spatial_result`) maupun daftar penuh (`get_saved_spatial_results`) otomatis mengembalikan `display_preference.fields` dan `image_url`.
2. **Struktur `FieldDef` dengan Atribut `kind` (Tanpa `deny_unknown_fields`)**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
   pub struct FieldDef {
       pub key: String,
       pub label: String,
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub unit: Option<String>,
       #[serde(default = "default_field_visible")]
       pub visible: bool, // default: true
       #[serde(default)]
       pub custom: bool,  // default: false
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub kind: Option<String>, // "text" | "number" | "boolean"
   }
   ```
   Di frontend TypeScript:
   ```typescript
   export type FieldKind = "text" | "number" | "boolean";

   export interface FieldDef {
     key: string;
     label: string;
     unit?: string | null;
     visible: boolean; // default true
     custom: boolean; // default false
     kind?: FieldKind;
   }
   ```
3. **Resolusi In-Memory Dinamis (Jika `fields` Belum Ada)**:
   Jika berkas belum memiliki `fields`, frontend dan backend menurunkan urutan field di memori dari gabungan kemunculan pertama key dalam `properties` seluruh fitur menggunakan kamus terpusat.
4. **Kamus Konstanta Terpusat (Standard Property Dictionary)**:
   Daftar mapping default label, unit, dan `kind`:

   | Key Teknis              | Label Tampil Default       | Unit Default | Kind Default | Keterangan         |
   | :---------------------- | :------------------------- | :----------- | :----------- | :----------------- |
   | `tree_id`               | "ID Pohon"                 | `null`       | `"text"`     | Modul 7            |
   | `confidence`            | "Keyakinan"                | `null`       | `"number"`   | Modul 7, 8         |
   | `height_est_m`          | "Tinggi Estimasi"          | "m"          | `"number"`   | Modul 7            |
   | `crown_diameter_m`      | "Diameter Tajuk"           | "m"          | `"number"`   | Modul 7            |
   | `health_status`         | "Kondisi"                  | `null`       | `"text"`     | Modul 7            |
   | `detector_run_id`       | "ID Proses Deteksi"        | `null`       | `"text"`     | Modul 7            |
   | `land_cover_class`      | "Kelas Tutupan Lahan"      | `null`       | `"text"`     | Modul 8            |
   | `area_sqm`              | "Luas"                     | "m²"         | `"number"`   | Modul 4, 8         |
   | `path_id`               | "ID Jalur"                 | `null`       | `"text"`     | Jalur Inspeksi     |
   | `path_type` / `type`    | "Jenis Jalan"              | `null`       | `"text"`     | Jalur Inspeksi     |
   | `length_m`              | "Panjang"                  | "m"          | `"number"`   | Jalur Inspeksi     |
   | `surface_condition`     | "Kondisi Permukaan"        | `null`       | `"text"`     | Jalur Inspeksi     |
   | `vegetation_index_type` | "Jenis Indeks Vegetasi"    | `null`       | `"text"`     | Modul 4            |
   | `mean_greenness_score`  | "Skor Kehijauan Rata-rata" | `null`       | `"number"`   | Modul 4            |
   | `plot_id`               | "ID Petak"                 | `null`       | `"text"`     | Template Kerusakan |
   | `damage_level`          | "Tingkat Kerusakan"        | `null`       | `"text"`     | Template Kerusakan |
   | `damage_percent`        | "Persentase Kerusakan"     | "%"          | `"number"`   | Template Kerusakan |
   | `affected_area_sqm`     | "Luas Terdampak"           | "m²"         | `"number"`   | Template Kerusakan |
   | `inspected`             | "Sudah Diperiksa"          | `null`       | `"boolean"`  | Template Kerusakan |

   Fallback untuk key tak terdaftar: label menggunakan `humanize(key)` dan `kind` diturunkan dari tipe nilai fitur pertama (`typeof value`).

5. **Penyelarasan Reserved Keys & Excluded Keys**:
   Satu set konstanta tunggal mendefinisikan kunci yang dilarang dan dikecualikan:
   `RESERVED_KEYS = ["image_url", "photo_url", "thumbnail_url", "image", "edited_fields", "source", "geometry", "properties", "__proto__", "constructor", "prototype"]`.
   Kunci-kunci ini **TIDAK BISA** dibuat sebagai custom field dan **TIDAK PERNAH** ditampilkan sebagai baris data di tabel field maupun popup.
6. **Penanda Edit Manual (`edited_fields`) & Atribut `skip_serializing_if`**:
   - Disimpan pada level fitur: `spatial_results[i].edited_fields: Option<Vec<String>>`.
   - Menggunakan atribut `#[serde(default, skip_serializing_if = "Option::is_none")]` sehingga fitur yang tidak dimodifikasi manual **tidak memunculkan kunci `edited_fields` di JSON**.
   - Struct `SpatialResult` tidak menggunakan `deny_unknown_fields`, sehingga sepenuhnya aman bagi pembaca lama.
7. **Perilaku Overwrite Plugin & Pemeriksaan Edit Manual (Tanpa Mengubah `save_spatial_result`)**:
   - Fungsi `save_spatial_result` **TIDAK DIUBAH** baik signature maupun perilakunya (tetap mempertahankan kontrak untuk modul lain).
   - Ditambahkan command baca-saja `check_spatial_result_manual_edits(layer_id: String) -> Result<ManualEditsCheckResult, MapControllerError>` dan flag `has_manual_edits: bool` pada `SpatialResultSummary`.
   - Dialog konfirmasi pada modul plugin lain (misal Module 10 atau UI runner) hanya dicatat sebagai saran/rekomendasi desain masa depan dan tidak mengubah API backend Module 3.

#### B2. Aturan Validasi (Fungsi Murni Frontend & Backend)

Validasi diterapkan secara identik pada Rust (`pure.rs`) dan TypeScript (`fieldValidation.ts`):

1. **Aturan `key`**:
   - Validasi regex `^[a-z][a-z0-9_]{0,39}$` **HANYA BERLAKU UNTUK FIELD CUSTOM BARU**.
   - Key bawaan plugin yang sudah ada di data diterima apa adanya tanpa penolakan.
   - Unik per layer (tidak boleh duplikat).
   - Tidak boleh termasuk dalam `RESERVED_KEYS`.
2. **Aturan `label`**:
   - Panjang 1..40 karakter setelah di-trim. Wajib teks polos tanpa HTML.
3. **Aturan `unit`**:
   - Panjang 0..12 karakter setelah di-trim (opsional). Wajib teks polos.
4. **Aturan `value` & Preservasi Tipe JSON**:
   - Nilai baru yang diset ke properti fitur **WAJIB menjaga tipe JSON nilai aslinya** (kecuali field custom yang tipenya ditentukan oleh `FieldDef.kind`):
     - Nilai semula `Number` -> nilai baru harus `Number` (finite).
     - Nilai semula `Bool` -> nilai baru harus `Bool`.
     - Nilai semula `String` -> nilai baru harus `String` (&le; 200 karakter).
     - Struktur data kompleks (`Object`/`Array`) ditolak untuk diedit manual.
5. **Batas Field**:
   - Batas maksimal 30 field **HANYA BERLAKU UNTUK FIELD CUSTOM** (`custom: true`). Field bawaan plugin tidak dibatasi oleh kuota ini.

#### B3. Desain Antarmuka Pengguna (`EditSpResultModal.tsx`)

1. **Dimensi Modal & Tata Letak Vertikal (Revisi User: Ramah Layar & Scroll)**:
   Modal disesuaikan menjadi lebar terfokus `style={{ width: "640px", maxWidth: "95vw" }}` dengan layout vertikal alami pada Tab "Field" (`maxHeight: "72vh"`, `overflowY: "auto"`):
   - Tabel manajemen field di atas.
   - Editor nilai per fitur di tengah.
   - Pratinjau popup Leaflet (live preview) terpusat di bawah (maksimal lebar 340px) yang dapat dijangkau pengguna dengan scroll alami ke bawah.
2. **Tab "Umum"**:
   - Form Nama Layer Hasil Analisis.
   - Dropdown Ikon Marker (hanya aktif dan terlihat jika layer bertipe geometri `Point`).
   - Pemilih Warna HEX Tampilan.
   - Seksi Foto Layer: Pratinjau foto, tombol "Ganti Foto", "Hapus Foto" (beralih ke No Image), dan "Batal Ubah".
3. **Tab "Field"**:
   - **Tabel Kelola Field**:
     - Kolom Urutan: Tombol Naik / Turun untuk memindahkan posisi field.
     - Kolom Tampil/Sembunyi: Icon button mata untuk toggle `visible`.
     - Kolom Label: Input inline (1..40 karakter).
     - Kolom Unit: Input inline (0..12 karakter).
     - Kolom Tipe (`kind`): Tag Blueprint.js menunjukkan `Teks`, `Angka`, atau `Boolean`.
     - Kolom Aksi: Tombol Hapus (hanya aktif untuk `custom === true`). Menekan tombol hapus membuka dialog konfirmasi: _"Hapus field {label}? Key '{key}' akan dihapus dari seluruh {n} fitur. Aksi ini tidak dapat dibatalkan."_ Field bawaan plugin (`custom === false`) tidak dapat dihapus, hanya dapat disembunyikan.
     - Tombol "Tambah Field Baru": Membuka form dialog sederhana (Label, Tipe/`kind`, Key otomatis tersinkronisasi dari label dalam format snake_case dan dapat diedit, serta nilai awal opsional untuk seluruh fitur).
   - **Editor Nilai per Fitur**:
     - Pemilih Fitur: Dropdown pencarian fitur. Nama fitur ditentukan berdasarkan prioritas properti identitas: `tree_id` > `path_id` > `plot_id` > `land_cover_class` > `vegetation_index_type` > `"Fitur #{index + 1}"`. Jika jumlah fitur > 50, disediakan paginasi 20 fitur per halaman.
     - Form Nilai Properti: Menampilkan input dinamis sesuai tipe:
       - `kind: "text"` -> `InputGroup` (maksimal 200 karakter).
       - `kind: "number"` -> `NumericInput` (finite number).
       - `kind: "boolean"` -> `HTMLSelect` atau `Switch` (Ya / Tidak).
     - Nilai yang telah diedit manual ditandai dengan badge tag `"diedit manual"`.
   - **Pratinjau Popup Interaktif Langsung**:
     - Menampilkan visualisasi kartu popup persis seperti yang akan tampil di Leaflet saat fitur tersebut diklik.
4. **Eksekusi All-or-Nothing**:
   - Satu tombol footer modal: **"Simpan Perubahan"**.
   - Tombol dinonaktifkan jika formulir dalam keadaan pristine (tidak ada perubahan), sedang memproses penyimpanan (`isSubmitting`), atau terdapat kesalahan validasi.
   - Peringatan konfirmasi jika modal ditutup saat masih ada perubahan yang belum disimpan.

#### B4. Spesifikasi Kontrak Backend & Concurrency Control

1. **Ekstensi `UpdateSpatialResultPayload` (Rust)**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
   #[serde(deny_unknown_fields)]
   pub struct UpdateSpatialResultPayload {
       pub id: String,
       pub name: Option<String>,
       pub display_color: Option<String>,
       pub icon: Option<String>,
       #[serde(default)]
       pub image_action: Option<String>, // "keep" | "delete" | "replace"
       #[serde(default)]
       pub image_path: Option<String>,
       #[serde(default)]
       pub fields: Option<Vec<FieldDef>>,
       #[serde(default)]
       pub feature_edits: Option<Vec<FeatureEdit>>,
       #[serde(default)]
       pub expected_updated_at: Option<String>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
   #[serde(deny_unknown_fields)]
   pub struct FeatureEdit {
       pub index: usize,
       #[serde(default)]
       pub set: std::collections::HashMap<String, serde_json::Value>,
       #[serde(default)]
       pub remove: Vec<String>,
   }
   ```
2. **Kunci yang Diizinkan Ditulis (Whitelist JSON Patching)**:
   - `layer_name`
   - `display_preference.color`
   - `display_preference.icon`
   - `display_preference.image_url`
   - `display_preference.fields`
   - `spatial_results[i].properties[key]`
   - `spatial_results[i].edited_fields`
   - `updated_at`
   - **Kunci Dilarang**: `geometry`, `coordinates`, `type` geometri, dan `source` 100% terkunci dan tidak dapat diubah oleh payload update.
3. **Pemeriksaan Konkurensi Optimistik (`expected_updated_at`)**:
   - Jika berkas memiliki `updated_at`: Bandingkan kesetaraan **string mentah** secara eksak (`file_updated_at == expected_str`).
   - Jika berkas lama belum memiliki `updated_at`: Gunakan token berbasis metadata berkas: `"{file_size}_{mtime_millis}"`. Jika ukuran atau waktu modifikasi berkas di disk berubah, tolak pembaruan dengan pesan error: _"Data hasil analisis telah diperbarui oleh proses lain. Silakan muat ulang data sebelum menyimpan."_
4. **Integritas Format Folder Dekomposisi**:
   Jika pembaruan menyentuh format folder (`layer.json` dan sejumlah `features/{id}.json`), mutasi dilakukan secara atomik melalui snapshot cadangan sementara. Jika penulisan salah satu berkas gagal, seluruh berkas dipulihkan kembali ke kondisi semula.
5. **Event Reaktif**:
   Setelah penyimpanan atomik sukses, backend memancarkan event `spatial_result:saved` yang otomatis memicu refresh pada layer peta, popup aktif, dan daftar kartu sidebar.

#### B5. Popup Peta & Mitigasi Menyeluruh Kerentanan XSS

1. **Resolusi Tampilan Baris Popup (`resolveDisplayFields`)**:
   Fungsi murni tunggal digunakan bersama oleh Popup Leaflet, Kartu Pratinjau, dan Editor Panel:
   - Mengambil definisi field dari `display_preference.fields`. Jika belum ada, menggunakan kamus default dan `humanize(key)`.
   - Menyaring field yang memiliki `visible: true`.
   - Mengabaikan field yang nilainya `null`, `undefined`, atau string kosong.
   - Memformat angka menggunakan locale Indonesia `id-ID` (contoh: `1420.5` dengan unit `"m²"` diformat menjadi `"1.420,5 m²"`).
2. **Pencegahan Serangan XSS (Strict HTML Escaping & Sanitasi URL)**:
   Karena string HTML popup dirangkai untuk Leaflet `bindPopup`, **SEMUA STRING** yang disisipkan ke dalam markup HTML diwajibkan melalui fungsi sanitasi murni `escapeHtml`:
   - `layerName`: di-escape.
   - `source`: di-escape.
   - `label` dan `unit`: di-escape.
   - `value`: dikonversi ke string lalu di-escape.
   - `display_color`: divalidasi format HEX (`^#[0-9a-fA-F]{3,8}$`).
   - `imageUrl`: disanitasi agar tidak memuat skema berbahaya (seperti `javascript:`) dan di-escape tanda kutipnya pada atribut `src`.

```typescript
export function escapeHtml(unsafe: string | null | undefined): string {
  if (!unsafe) return "";
  return String(unsafe)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}
```

---

### 13.4 Rencana Pengujian Komprehensif (10 Skenario Tempdir)

Seluruh pengujian backend menggunakan direktori sementara (`tempdir`) dan **TIDAK MENYENTUH** direktori `map_storage` asli. Keempat berkas template dari Module 3 `data_template` disalin ke `tempdir` sebagai fixture dasar:

1. **Test 1 (`resolveDisplayFields`)**:
   - Memverifikasi resolusi turunan in-memory pada file tanpa skema `fields`.
   - Memverifikasi pemakaian skema `fields` yang tersimpan (termasuk atribut `kind`).
   - Memverifikasi penambahan otomatis key baru plugin di urutan akhir.
   - Memverifikasi field dengan `visible: false` tidak dirender di popup.
2. **Test 2 (Validasi Field & Reserved Keys)**:
   - Memvalidasi penolakan key duplikat, key ilegal (karakter huruf besar/simbol), dan reserved keywords (`image_url`, `__proto__`, `edited_fields`, dll.).
   - Memvalidasi penolakan label > 40 karakter dan penolakan jika jumlah field > 30.
3. **Test 3 (Edit Nilai Properti & Preservasi Tipe)**:
   - Menguji edit teks, angka, boolean.
   - Memverifikasi **penolakan perubahan tipe nilai asli** (misal memasukkan string ke field yang aslinya number).
   - Memverifikasi penolakan index fitur di luar batas (`out-of-bounds`).
   - Memverifikasi penolakan edit manual pada struktur data kompleks (objek/array).
   - Memverifikasi pencatatan key yang diubah pada `edited_fields` (dengan verifikasi `skip_serializing_if`).
   - Memverifikasi semantik `geometry`, `coordinates`, dan `source` tetap identik 100%.
4. **Test 4 (Hapus Field)**:
   - Field `custom: true` berhasil dihapus dari `fields` dan seluruh `properties` fitur.
   - Field `custom: false` (bawaan plugin) ditolak jika dihapus (hanya boleh `visible: false`).
5. **Test 5 (Concurrency Lock via String Mentah & Metadata Token)**:
   - Uji penolakan jika `expected_updated_at` tidak cocok dengan string berkas.
   - Uji penolakan untuk berkas tanpa `updated_at` jika token ukuran+mtime berkas berubah.
6. **Test 6 (Isolasi Foto Layer vs Fitur)**:
   - Ganti/Hapus foto layer hanya memodifikasi `display_preference.image_url`.
   - `image_url` milik masing-masing fitur tidak berubah.
   - Berkas fisik lama di `result_image/` dihapus hanya jika memenuhi regex kepemilikan layer `^{id}_\d{10,}\.(jpg|jpeg|png|webp)$`.
   - Aset publik (`/sample_drone.*`) tidak pernah dihapus.
7. **Test 7 (Karantina & Rollback Foto saat Hapus Layer)**:
   - Menghapus layer ikut mengkarantina foto unggahan milik layer tersebut.
   - Mensimulasikan kegagalan fase 2; memverifikasi rollback memulihkan berkas layer dan foto unggahan.
8. **Test 8 (Toleransi Format Legacy)**:
   - Berkas dengan timestamp mikrodetik `+00:00` terbaca normal.
   - Berkas yang hanya memiliki `properties.image_url` tetap menampilkan thumbnail.
   - Nilai ikon `""` atau kustom tidak rusak.
   - Kunci tak dikenal di JSON tetap utuh setelah operasi update (JSON patching).
9. **Test 9 (Integritas Format Folder Dekomposisi)**:
   - Mensimulasikan kegagalan I/O di tengah mutasi lintas file (`layer.json` dan `features/*.json`); memverifikasi seluruh folder kembali ke kondisi awal (all-or-nothing).
10. **Test 10 (Frontend Pure Functions & Quality Gates)**:
    - Verifikasi typecheck TypeScript murni (`npm run build`: `tsc && vite build`) dan linter (`npm run lint`).
    - Verifikasi clippy dan formatting Rust (`npm run lint:rust`, `npm run format:rust:check`).

---

### 13.5 Analisis Berkas Acuan Template & Pemindahan ke Module 3

Keempat berkas template di `map_storage/templates/` disalin ke folder internal Module 3 di `src-tauri/src/modules/module_03/data_template/` (bukan di `map_storage`). **Setelah proses penyalinan berhasil diverifikasi, berkas lama dan folder `map_storage/templates/` dihapus** agar root `map_storage` tetap bersih.

#### Penambahan Atribut `kind` pada 4 Template Acuan:

1. **`template_crop_damage.json`**:
   - `plot_id`: `"kind": "text"`
   - `damage_level`: `"kind": "text"`
   - `damage_percent`: `"kind": "number"`
   - `affected_area_sqm`: `"kind": "number"`
   - `inspected`: `"kind": "boolean"`
2. **`template_inspection_path.json`**:
   - `path_id`: `"kind": "text"`
   - `path_type`: `"kind": "text"`
   - `length_m`: `"kind": "number"`
   - `surface_condition`: `"kind": "text"`
3. **`template_landcover_polygon.json`**:
   - `land_cover_class`: `"kind": "text"`
   - `confidence`: `"kind": "number"`
   - `area_sqm`: `"kind": "number"`
   - `catatan`: `"kind": "text"`
4. **`template_tree_detection.json`**:
   - `tree_id`: `"kind": "text"`
   - `confidence`: `"kind": "number"`
   - `height_est_m`: `"kind": "number"`
   - `crown_diameter_m`: `"kind": "number"`
   - `health_status`: `"kind": "text"`
   - `detector_run_id`: `"kind": "text"`

#### Ketidaksesuaian Template vs Kode Saat Ini:

1. `template_inspection_path.json`: Menggunakan `"display_preference": { "type": "line", "color": "#E74C3C" }`. Penyesuaian: Menambahkan varian `Line { color: String, weight: f64, ... }` pada `LayerDisplayMode`.
2. `template_crop_damage.json`: Properti kustom tidak cocok dengan 5 varian kaku `SpatialProperties`. Penyesuaian: Menambahkan varian tangkapan `Generic(serde_json::Map<String, serde_json::Value>)` pada akhir enum `SpatialProperties`.

---

### 13.6 Pengecekan Sisa TIFF di Seluruh Repositori

Hasil audit terhadap seluruh direktori kerja:

1. **Direktori `map_storage/images/tiff`**: Dipastikan **TIDAK ADA**. Folder di dalam `map_storage/images/` saat ini hanya `raster`, `raster_1792817972627`, dan `result_image`.
2. **Pencarian Berkas `.tif` / `.tiff`**: Dipastikan tidak ada berkas fisik GeoTIFF di dalam `map_storage/` maupun `public/`.
3. **Penanganan Graceful Rujukan TIFF**:
   Pada [spatialLayerBuilder.ts:L81, L170-172](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/lib/leaflet/spatialLayerBuilder.ts#L170-L172) dan [popupHtml.ts:L34](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/lib/popupHtml.ts#L34), rujukan berkas berekstensi `.tif` atau `.tiff` ditangani secara halus (_graceful fallback_) dengan menampilkan placeholder "Format TIFF (No Image)" tanpa membekukan atau merusak peta Leaflet.

---

### 13.7 Keputusan Pengguna

1. **Test Fungsi Murni Frontend (`vitest`)**:
   - **Keputusan Pengguna**: **SETUJU** (`vitest sebagai devDependency = <setuju>`). Ditambahkan sebagai `devDependency` pada frontend untuk menguji fungsi murni TypeScript (`resolveImageSrc`, `resolveDisplayFields`, `formatFieldValue`, `escapeHtml`, validasi).
2. **Fitur `preserve_order` pada `serde_json`**:
   - **Keputusan Pengguna**: **SETUJU** (`preserve_order = <setuju>`). Diaktifkan pada `src-tauri/Cargo.toml` agar urutan kunci berkas JSON tetap natural.
3. **Status Cadangan `map_storage`**:
   - **Konfirmasi Pengguna**: `Backup map_storage sudah saya buat: <ya>`.

---

### 13.8 Daftar Berkas yang Akan Dibuat atau Dimodifikasi

#### Berkas Baru:

1. `src-tauri/src/modules/module_03/data_template/` (folder 4 berkas template acuan ber-`kind`).
2. `src/modules/module-03/lib/fieldResolution.ts` (fungsi murni `resolveDisplayFields`, `formatFieldValue`, `resolveImageSrc`).
3. `src/modules/module-03/lib/fieldValidation.ts` (fungsi murni validasi key, label, unit, value, dan `escapeHtml`).
4. `src/modules/module-03/molecules/FieldManagerTable.tsx` (tabel kelola urutan, label, unit, visibilitas, tipe `kind`, dan hapus custom field).
5. `src/modules/module-03/molecules/FeatureValuesEditor.tsx` (pemilih fitur dan formulir pengeditan nilai terikat tipe).
6. `src/modules/module-03/molecules/PopupPreviewCard.tsx` (pratinjau langsung kartu popup fitur terpilih).
7. `src/modules/module-03/lib/__tests__/pureFunctions.test.ts` (unit test vitest untuk fungsi murni frontend).

#### Berkas Dimodifikasi:

1. `src-tauri/Cargo.toml`:
   - Penambahan `features = ["preserve_order"]` pada `serde_json`.
2. `src-tauri/src/modules/module_03/types/sp_result_type.rs`:
   - Penambahan varian `Line` pada `LayerDisplayMode`.
   - Penambahan varian `Generic` pada `SpatialProperties`.
   - Preservasi sisa key via `#[serde(default, flatten)] pub extra: serde_json::Map<String, serde_json::Value>` pada semua properti bertipe.
   - Penambahan `pub edited_fields: Option<Vec<String>>` pada `SpatialResult` dengan `skip_serializing_if`.
   - Penambahan `concurrency_token: Option<String>` pada `LayerPayload`.
3. `src-tauri/src/modules/module_03/sp_result/types.rs`:
   - Penambahan `FieldDef` (dengan `kind`, default `visible: true`, default `custom: false`, tanpa `deny_unknown_fields`) dan `FeatureEdit`.
   - Pembaruan `UpdateSpatialResultPayload` (`fields`, `feature_edits`, `expected_updated_at`).
   - Penambahan `has_manual_edits: bool` dan `concurrency_token: String` pada `SpatialResultSummary`.
   - Penambahan `photos_count: usize` pada `DeleteSpatialResultSummary`.
   - Penambahan struct `ManualEditsCheckResult`.
4. `src-tauri/src/modules/module_03/sp_result/pure.rs`:
   - Fungsi murni validasi field kustom, deteksi regex kepemilikan foto, turunan `geom_type`, dan validasi tipe nilai JSON.
5. `src-tauri/src/modules/module_03/sp_result/store.rs`:
   - Pembaruan JSON patching `update_spatial_result_in_dir` (isolasi foto layer vs fitur, concurrency check string mentah / token metadata, patching fields & values menjaga tipe JSON).
   - Pembaruan `delete_saved_spatial_result_in_dir` (karantina dan rollback foto unggahan milik layer via regex).
   - Pemulihan `.trash_sr_*` saat startup (`cleanup_orphaned_trash`).
   - Pembaruan `count_spatial_result_dependents_in_dir` (menghitung jumlah foto).
   - Implementasi `check_spatial_result_manual_edits_in_dir`.
6. `src-tauri/src/modules/module_03/sp_result/commands.rs` & `src-tauri/src/lib.rs`:
   - Penambahan command `check_spatial_result_manual_edits`.
7. `src-tauri/tests/module_03_spatial_results_crud_test.rs`:
   - 10 skenario pengujian komprehensif menggunakan tempdir, round-trip 4 template dan 5 berkas riil, serta pemulihan startup trash.
8. `src-tauri/tauri.conf.json`:
   - Konfigurasi `assetProtocol` dan CSP `img-src` untuk akses lokal foto unggahan di build rilis.
9. `src/modules/module-03/types/map.ts` & `src/modules/module-03/types/spatialResult.ts`:
   - Sinkronisasi DTO tipe `FieldDef`, `FeatureEdit`, `LayerDisplayMode`, dan token konkurensi.
10. `src/modules/module-03/lib/popupHtml.ts`:
    - Penerapan `resolveDisplayFields`, `resolveImageSrc`, dan sanitasi `escapeHtml` menyeluruh. Penghapusan penanganan khusus TIFF (fallback via `onError`).
11. `src/modules/module-03/molecules/EditSpResultModal.tsx`:
    - Rombak tata letak dengan modal 960px, Blueprint.js Tabs ("Umum" dan "Field"), dropdown ikon Point, dan all-or-nothing save dengan pembaruan token konkurensi.
12. `src/modules/module-03/molecules/SpResultCardItem.tsx`:
    - Sinkronisasi geometri label ("Titik", "Poligon", "Garis", "Campuran", "Overlay").
13. `src/modules/module-03/lib/leaflet/spatialLayerBuilder.ts`:
    - Dukungan pewarnaan garis untuk `display.type === "line"`, evaluasi visual ikon marker, dan penghapusan penanganan khusus TIFF.
14. `vite.config.ts`:
    - Pengamanan middleware `serveMapStorage` terhadap path traversal `..` dengan `decodeURIComponent` dan verifikasi `safePrefix + path.sep`.

---

### 13.9 Tahapan Pengerjaan & Syarat Berhenti

- **Tahap T0 (SELESAI DENGAN DOKUMEN REVISI INI)**:
  Penyusunan Bagian 13 lengkap pada `implementasi.md`. **BERHENTI DAN MENUNGGU USER MENGETIK "SETUJU"**.
- **Tahap T1 (Backend Rust) - SELESAI**:
  - Fitur `preserve_order` diaktifkan pada `serde_json` di `src-tauri/Cargo.toml`.
  - Penyalinan 4 berkas template ber-`kind` ke `src-tauri/src/modules/module_03/data_template/` dan pembersihan folder lama `map_storage/templates/`.
  - Penambahan varian `Line` pada `LayerDisplayMode` dan varian `Generic` pada `SpatialProperties`.
  - Preservasi 100% sisa properti via `#[serde(default, flatten)] extra` pada seluruh struct properti dan `SpatialResult`.
  - `FieldDef` tersimpan bebas dari `deny_unknown_fields` (hanya DTO payload yang memakainya); default `visible: true`, `custom: false`.
  - `save_spatial_result` dipertahankan 100% tanpa perubahan signature/perilaku; command baca-saja `check_spatial_result_manual_edits` dan flag `has_manual_edits: bool` pada `SpatialResultSummary` telah diimplementasikan dan didaftarkan di `src-tauri/src/lib.rs`.
  - Pemulihan `.trash_sr_*` saat startup (`cleanup_orphaned_trash`): jika registrasi layer ada, dipulihkan; jika tidak, dihapus permanen.
  - Validasi regex key hanya berlaku untuk field custom baru; field bawaan plugin diterima apa adanya; batas 30 hanya untuk field custom.
  - Concurrency token dikembalikan di `list` dan `detail`, serta diverifikasi ketat saat `update`.
  - Pengujian menyeluruh: 21 skenario test lulus 100% di `src-tauri/tests/module_03_spatial_results_crud_test.rs` (termasuk round-trip 4 template dan 5 berkas riil).
  - Pre-commit quality gates backend: `npm run lint:rust` (clippy) dan `npm run format:rust:check` (rustfmt) lolos 100% tanpa warning/diff.
  - **STATUS**: SELESAI. Berhenti untuk pelaporan sebelum melangkah ke Tahap T2.
- **Tahap T2 (Frontend TypeScript & React) - SELESAI**:
  - Implementasi fungsi murni `fieldResolution.ts` (`resolveDisplayFields`, `formatFieldValue`, `resolveImageSrc`, `resolveCardThumbnail`, `resolveFeaturePhoto`, `getFeatureDisplayName`) dan `fieldValidation.ts` (`escapeHtml`, `isValidHexColor`, `validateCustomFieldKey`, `validateFieldLabel`, `validateFieldUnit`, `validateCustomFieldsCount`, `validatePropertyValueType`).
  - Implementasi komponen presentasi: `FieldManagerTable.tsx`, `FeatureValuesEditor.tsx`, `PopupPreviewCard.tsx`, dan modal 960px `EditSpResultModal.tsx` (Blueprint.js Tabs, Point-only marker icon dropdown, optimistik concurrency token, deteksi unsaved changes).
  - Integrasi popup Leaflet aman XSS di `popupHtml.ts` dan penghapusan bypass khusus TIFF di `spatialLayerBuilder.ts` & `popupHtml.ts`.
  - Konfigurasi keamanan Vite middleware (`decodeURIComponent` + `safePrefix + path.sep` anti-traversal) dan Tauri `assetProtocol.scope` + CSP `img-src` di `tauri.conf.json`.
  - Unit test `pureFunctions.test.ts` via Vitest: 34 skenario test lulus 100%.
  - Pre-commit quality gates frontend & backend: `npm run lint`, `npm run lint:rust`, `npm run format:check`, `npm run format:rust:check`, `npm run build` (tsc + vite) lolos 100%.
  - **STATUS**: SELESAI. Berhenti untuk pelaporan sebelum melangkah ke Tahap T3.
- **Tahap T3 (Perbaikan Data Riil) - SELESAI**:
  1. Konfirmasi backup manual direktori `map_storage` oleh pengguna telah terverifikasi.
  2. Persetujuan tertulis pengguna untuk eksekusi diff minimal per berkas telah diterima.
  3. Eksekusi penerapan diff minimal:
     - `result_landcover_001.json`: `display_preference.type` dikoreksi dari `"point"` menjadi `"polygon"`.
     - `result_line_001.json`: `display_preference.type` dikoreksi dari `"point"` menjadi `"line"`.
     - `result_vegetation_001.json`: `display_preference.type` dikoreksi dari `"point"` menjadi `"polygon"`.
     - `result_tree_001.json`: De-duplikasi foto layer dari level fitur berhasil dibersihkan; foto layer pada `display_preference.image_url` dan timestamp asli tetap terjaga utuh.
     - `result_tree_002.json`: Dipertahankan tipe `"point"` (tanpa perubahan).
  4. Verifikasi bahwa seluruh 5 berkas terbaca sempurna oleh dual-reader backend, test suite Rust, dan layer Leaflet peta.
  - **STATUS**: SELESAI.

---

### 13.10 Kriteria Selesai (Dapat Diverifikasi 100%)

- [x] Popup peta dan pratinjau di panel modal menampilkan field yang identik (label, unit, urutan, visibilitas).
- [x] Judul dan isi field dapat diubah, ditambah (custom), disembunyikan, dan dihapus (custom).
- [x] Nilai yang diedit ditandai badge "diedit manual"; geometri, koordinat, dan `source` tetap terkunci 100%.
- [x] Ganti/Hapus Foto layer tidak menimpa `image_url` milik fitur.
- [x] Delete layer tidak meninggalkan berkas foto unggahan yatim; aset publik (`/sample_drone.*`) tidak tersentuh.
- [x] Jenis geometri dan ikon marker di UI konsisten dengan data sebenarnya dan didukung renderer.
- [x] Tidak ada celah XSS pada popup peta (seluruh teks input melalui fungsi `escapeHtml`).
- [x] Data lama terbaca lancar tanpa migrasi paksa; tidak ada data riil yang diubah tanpa persetujuan tertulis pengguna.
- [x] Seluruh pre-commit quality gates (`npm run lint`, `npm run lint:rust`, `npm run format:check`, `npm run format:rust:check`, `npm run build`) lolos 100% tanpa dependensi baru atau Python.
