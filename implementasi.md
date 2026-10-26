# Dokumen Desain & Rencana Implementasi: Fitur Impor Citra Orthomosaic Dual-Mode (Opsi A & Opsi B)

> **STATUS DOKUMEN: TAHAP 0 (REVISI ARSITEKTUR KOMPREHENSIF TAHAP 0.2)**  
> Dokumen ini merupakan spesifikasi arsitektur teknis resmi hasil revisi lengkap berdasarkan seluruh parameter evaluasi.  
> **TIDAK ADA KODE YANG DIUBAH / DITULIS** sebelum dokumen ini disetujui secara tertulis oleh pengguna dengan kata **"SETUJU"**.

---

## 1. Ringkasan Revisi Keputusan Arsitektur & Semantik Penyimpanan

### 1.1 Riwayat & Alasan Revisi

- **Keputusan Lama**: Format TIFF dihapus dari Module 3 dan digantikan oleh JPG/PNG murni dengan input koordinat bounding box manual. Alasan saat itu adalah untuk mengeliminasi dependensi runtime C (GDAL/libvips) dan skrip Python (`.venv_geo`).
- **Keputusan Baru (DIREVISI)**: **Format TIFF / GeoTIFF RESMI KEMBALI DIDUKUNG** melalui jalur konversi streaming _pure Rust_ mandiri tanpa dependensi runtime eksternal.
- **Rasionalitas Teknis**:
  1. _Standar Industri Fotogrametri Drone_: Citra orthomosaic drone (Pix4D, WebODM, Agisoft) lazimnya diekspor sebagai GeoTIFF dengan tag koordinat geospasial tertanam (_embedded GeoKeys_).
  2. _Eliminasi Friksi Pengguna_: Mengharuskan pengguna mengonversi TIFF ke JPG secara manual di luar aplikasi dan mengetik koordinat desimal menimbulkan friksi UX yang tinggi dan rawan kesalahan manusia (_human error_).
  3. _Kelayakan Pure Rust_: Dengan ekosistem crate Rust modern (`tiff 0.9.1` berbasis _chunked/tiled streaming_), pemrosesan berkas GeoTIFF 50–250 MB dapat dilakukan secara efisien dalam batas memori **200–500 MB RAM** tanpa memerlukan runtime C/Python atau instalasi sistem apa pun.

### 1.2 Parameter Desain & Semantik Penyimpanan File (Syarat Mutlak)

1. **Resolusi Display**: Batas maksimal ditetapkan **4.096 piksel pada sisi terpanjang**. Batas ukuran berkas ditetapkan terpisah: **$\le 5\text{ MB}$ untuk JPEG** dan **$\le 10\text{ MB}$ untuk PNG Alpha**, dihasilkan melalui loop optimasi adaptif kuadratik dengan tangga dimensi `[4096, 3584, 3072, 2560, 2304, 2048, 1792, 1536, 1280, 1024]` (lantai keras tetap $1.024\text{ px}$). Peringatan $< 2.048\text{ px}$ otomatis dicatat di metadata dan banner UI.
2. **Opsi "Simpan TIFF Asli"**: Ditetapkan **default TIDAK dicentang (mati)** di UI dengan teks peringatan: _"Tanpa berkas asli, citra tidak dapat di-render ulang dengan resolusi lebih tinggi di masa mendatang"_.
3. **Struktur Penyimpanan Fisik (`map_storage/images/{image_id}/`)**:
   Setiap citra yang diimpor disimpan dalam 1 folder terisolasi = 1 entitas data atomik.
   Contoh ID: `ortho_1792850000000`.
   ```text
   map_storage/images/{image_id}/
   ├── display.jpg (atau display.png)   <- Berkas yang dirender di peta Leaflet
   ├── meta.json                        <- Metadata utama (kompatibel penuh dengan Layer Manager)
   ├── metadata.json                    <- Metadata teknis detail (CRS, GeoKeys, EXIF, dll.)
   └── original.tif (atau .jpg/.png)    <- Berkas asli (kondisional)
   ```
4. **Semantik Field `size_bytes` di `meta.json`**:
   - Jika berkas asli ada di disk (`original.jpg`, `original.png`, atau `original.tif` jika dicentang): `size_bytes` mencatat ukuran fisik berkas asli tersebut.
   - Jika berkas asli TIDAK disimpan (Opsi B tanpa centang): `size_bytes` mencatat ukuran fisik berkas display yang benar-benar ada di disk (`display_size_bytes`) agar inspeksi disk, backup, dan kuota storage akurat dan tidak menunjuk ke file fiktif.
   - Ukuran berkas sumber asli historis selalu tercatat permanen di `metadata.json` (`source_file_size_bytes`).
5. **Nilai Field `extension` di `meta.json`**:
   Mengikuti ekstensi berkas utama yang tersimpan di disk:
   - Jika `original` disimpan: `"jpg"`, `"png"`, atau `"tif"`.
   - Jika `original` tidak disimpan (Opsi B tanpa centang): `"jpg"` atau `"png"` (mengikuti format berkas `display` fisik yang ada di disk).
6. **Normalisasi Nama Berkas Asli**:
   - `.jpeg`, `.JPG` $\rightarrow$ dinormalisasi menjadi `original.jpg`.
   - `.png`, `.PNG` $\rightarrow$ dinormalisasi menjadi `original.png`.
   - `.tiff`, `.TIF` $\rightarrow$ dinormalisasi menjadi `original.tif`.
7. **Perlakuan Berkas Sidecar Georeferensi**:
   Berkas sidecar (`.jgw`, `.pgw`, `.wld`, `.prj`, `{nama}.bounds.json`) **TIDAK DISALIN** ke folder `images/{image_id}/`. Seluruh hasil parsing bounds WGS84 dan kode CRS sumber sudah tercatat secara permanen di dalam `meta.json` dan `metadata.json`.

---

## 2. Desain Arsitektur Sistem

### 2.1 Arsitektur Job, Registry Atomik, dan Pembatalan (Cancel)

Untuk menjamin UI tidak pernah berstatus _"Not Responding"_ saat memproses citra besar, seluruh pemrosesan berat dipindahkan ke thread pekerja asinkron:

```
+---------------------------------------------------------------------------------------+
| FRONTEND (React 19 / Blueprint.js)                                                    |
| [Modal Impor: Tab Opsi A / Tab Opsi B] ---> invoke("start_import_ortho_job", payload) |
|         ^                                              |                              |
|         | listen("ortho_import_progress")              v (Mengembalikan job_id        |
|         | (filter by job_id)                              dalam < 50ms)               |
|         | listen("ortho_import_finished")      [State: Active Job di Store/Modal]     |
|         | listen("ortho_import_failed")                |                              |
|         +--- invoke("cancel_import_job", {job_id}) <---+ [Tombol Batalkan]            |
+--------------------------------------------------------|------------------------------+
                                                         | IPC
+--------------------------------------------------------v------------------------------+
| BACKEND (Rust / Tauri 2)                                                              |
| Tauri Command Handler (Non-blocking):                                                 |
|   ├── 1. Preflight Disk Check: hitung estimasi ruang disk (display + original + margin)|
|   ├── 2. Mutex JobRegistry (Atomik): jika ada job berstatus "running", tolak          |
|   │      dengan error JobSedangBerjalan                                               |
|   ├── 3. Alokasikan job_id (UUID v4) & CancellationToken                              |
|   ├── 4. Tulis atomik metadata job: map_storage/.jobs/{job_id}.json                   |
|   └── 5. tauri::async_runtime::spawn_blocking(move || run_import_worker(...))         |
|                                                                                       |
| Worker Thread (Staging, Streaming & Drop Guard):                                      |
|   ├── Inisialisasi StagingGuard (otomatis bersihkan staging jika thread panic/gagal)  |
|   ├── Buat folder staging sementara: map_storage/images/.staging_{job_id}/            |
|   ├── [Tahap Salin Asli]: jika Opsi A atau Opsi B (centang): streaming copy per blok, |
|   │     cek token tiap blok, ikut persentase progres                                  |
|   ├── [Tahap Resampling & Display]:                                                   |
|   │     - Opsi A: DCT-domain scaling (JPEG) atau scanline streaming (PNG)             |
|   │     - Opsi B: Tiled chunk streaming via tiff 0.9.1 -> Box Filter Area Averaging  |
|   │     Cek token pembatalan sebelum & sesudah tahap encode display                   |
|   ├── [Tahap Loop Adaptif]: optimasi display <= 5 MB (JPG quality/scale, PNG downscale)|
|   ├── Tulis meta.json & metadata.json ke dalam folder staging                         |
|   ├── POINT OF NO RETURN:                                                             |
|   │     1. fs::rename .staging_{job_id}/ ke map_storage/images/{image_id}/            |
|   │        (Loop retry dengan exponential backoff untuk Windows Defender/indexer)     |
|   │     2. Registrasi atomik layer: map_storage/layers/{image_id}.json                |
|   ├── Nonaktifkan StagingGuard (disarm)                                               |
|   └── Tulis status "completed" di .jobs/{job_id}.json & emit event selesai            |
+---------------------------------------------------------------------------------------+
```

### 2.2 Penanganan Crash Startup, Single-Instance Lock, dan Retensi Job

- **Single-Instance Lock & Urutan Startup Multi-Instance**:
  Penguncian instance aplikasi dilakukan secara murni pada `map_storage/.app.lock` (`share_mode(0)` di Windows kernel, serta advisory `fs4::FileExt::try_lock_exclusive` pada platform non-Windows):
  1. _Instansi Primer (Pemegang Kunci)_: Saat pertama kali dibuka, instansi ini berhasil memperoleh kunci eksklusif, lalu menjalankan pembersihan startup (`cleanup_stale_import_jobs()`) dan rekonsiliasi layer (`reconcile_storage_layers()`).
  2. _Instansi Sekunder (Kunci Tidak Didapat)_: Jika pengguna membuka jendela aplikasi kedua saat instansi pertama masih berjalan, kernel/OS menolak perolehan kunci dengan error terstruktur `OrthoImportError::AplikasiSudahBerjalan`.
  3. _Aturan Proteksi Instansi Sekunder_: **Instansi kedua TIDAK BOLEH menjalankan `cleanup_stale_import_jobs` maupun `reconcile_storage_layers`**, karena menjalankan pembersihan tersebut akan menghancurkan folder staging `.staging_*` dan menandai job aktif instansi pertama sebagai terputus (_interrupted_). Instansi kedua mencatat log peringatan ramah dan berjalan dalam mode baca aman tanpa mengalami crash.
- **Deteksi Crash Startup Tanpa Ambang Batas Waktu**:
  Hanya oleh instansi primer: saat aplikasi baru dibuka (`setup()` di `src-tauri/src/lib.rs`), **SEMUA job di `map_storage/.jobs/` yang masih berstatus `"running"` langsung dinyatakan `interrupted` (terputus/crash)**. Karena aplikasi baru saja di-boot, tidak mungkin ada pekerja latar belakang dari sesi sebelumnya yang masih berjalan.
- **Pembersihan Folder Staging Sisa Crash**:
  Semua folder staging temporer `map_storage/images/.staging_*` yang ditinggalkan oleh job yang crash langsung dihapus secara rekursif saat startup.
- **Folder Staging Diabaikan oleh Listing**:
  Fungsi listing citra di `store.rs` (`list_ortho_images`) memfilter entri folder berawalan titik (`if folder_name.starts_with('.') { continue; }`), sehingga folder `.staging_*` dan `.trash_*` tidak pernah terbaca sebagai entitas citra rusak.
- **Kebijakan Retensi Berkas Job**:
  Direktori `map_storage/.jobs/` menyimpan **maksimal 20 berkas job terakhir**. Saat startup, berkas job diurutkan berdasarkan tanggal (`created_at`) dan berkas di luar 20 teratas dihapus otomatis.
- **Interval Heartbeat Disk vs Event UI**:
  - Heartbeat status job ditulis ke disk `map_storage/.jobs/{job_id}.json` secara atomik setiap **3 – 5 detik** (hanya untuk keperluan pemulihan UI jika modal ditutup lalu dibuka kembali via `get_import_job_status`).
  - Event progres IPC `ortho_import_progress` dikirim ke antarmuka pengguna setiap **150 milidetik** secara in-memory agar animasi progress bar mulus tanpa membebani disk I/O.
- **Batas Konkurensi**:
  Maksimal **1 job impor aktif** pada satu waktu. Panggilan `start_import_ortho_job` baru saat ada job berstatus `"running"` akan langsung mengembalikan error `JobSedangBerjalan`.

### 2.3 Registrasi Layer Manager & Rekonsiliasi Konsistensi

- **Penanggung Jawab Penulisan Layer**:
  Berkas `map_storage/layers/{image_id}.json` ditulis oleh **jalur backend worker impor citra** (bukan frontend) menggunakan fungsi atomik `crate::modules::module_03::layers::store::save_layer`.
- **Urutan Transaksi**:
  1. _Rename staging_: `fs::rename(&staging_dir, &target_dir)`.
  2. _Tulis layer JSON atomik_: `map_storage/layers/{image_id}.json` ditulis melalui berkas temporer `.tmp` lalu di-rename.
  3. _Finalisasi job_: Status job diubah menjadi `"completed"`.
- **Rekonsiliasi Startup Saat Terjadi Crash**:
  Fungsi `reconcile_storage_layers()` dijalankan saat startup aplikasi untuk merekonsiliasi kemungkinan ketidaksinkronan:
  - **Kasus A: Folder citra `images/{id}/` ada, tetapi `layers/{id}.json` belum ada (crash di antara langkah 1 dan 2)**:
    Sistem secara otomatis mendaftarkan ulang (_auto-heal / auto-register_) layer default pada `map_storage/layers/{id}.json` menggunakan nama dan ID dari `meta.json`.
  - **Kasus B: Berkas `layers/{id}.json` ada, tetapi folder citra `images/{id}/` tidak ada (layer yatim)**:
    Sistem menghapus berkas layer yatim tersebut agar tidak merender layer hantu di Leaflet.

### 2.4 Penanganan Kolisi ID & Penghapusan Bersih

- **Format ID**: `ortho_{epoch_ms}`. Jika folder tujuan sudah ada (kasus duplikasi timestamp milidetik), sistem menambahkan counter berurutan `ortho_{epoch_ms}_{counter}` hingga menemukan path yang belum ada di disk.
- **CRUD Hapus Citra Ortho (1 per 1)**:
  Fungsi `delete_ortho_image_single` di `store.rs`:
  1. Memindahkan folder `images/{id}/` ke `.trash_{id}_{timestamp}/` secara atomik.
  2. Menghapus berkas layer `map_storage/layers/{id}.json`.
  3. Menghapus anotasi terikat (`image_id == id`).
  4. Menghapus permanen seluruh folder `.trash_{id}_{timestamp}/` (mencakup `original.tif`/`original.jpg`, `display.*`, `meta.json`, dan `metadata.json`).
  5. Jika terdapat error di tengah proses, folder karantina dipulihkan kembali (_rollback_).

---

## 3. Spesifikasi Teknis Opsi A & Opsi B

### 3.1 Opsi A: Citra + Metadata (JPG / PNG)

- **Sumber Georeferensi (Urut Prioritas)**:
  1. **World File (`.jgw` / `.pgw` / `.wld` + `.prj`)**:
     Membaca 6 parameter transformasi affine:
     - Garis 1 ($A$): Ukuran piksel arah $X$.
     - Garis 2 ($D$): Suku rotasi arah $Y$.
     - Garis 3 ($B$): Suku rotasi arah $X$.
     - Garis 4 ($E$): Ukuran piksel arah $Y$ (bernilai negatif).
     - Garis 5 ($C$): Koordinat $X$ tengah piksel kiri atas.
     - Garis 6 ($F$): Koordinat $Y$ tengah piksel kiri atas.
       _Validasi Rotasi_: Jika parameter $B \neq 0.0$ atau $D \neq 0.0$ (melebihi toleransi $10^{-7}$), tolak dengan error `RotasiTidakDidukung`.
       _Dukungan `.prj`_: Mengekstrak `AUTHORITY["EPSG", "..."]` atau nama WKT baku (`WGS_1984`, `Pseudo-Mercator`, `UTM`). Jika CRS bukan WGS84, konversi via modul murni `projection.rs`. Jika CRS tidak dikenali, kembalikan `CrsTidakDidukung` dengan input manual.
  2. **Sidecar JSON (`{nama_berkas}.bounds.json`)**:
     Format sidecar JSON baku:
     ```json
     {
       "schema_version": 1,
       "bounds": {
         "west": 116.850648,
         "south": -1.253814,
         "east": 116.859893,
         "north": -1.247529
       },
       "crs": "EPSG:4326"
     }
     ```
     _Aturan CRS_: Hanya `"EPSG:4326"` yang diizinkan pada sidecar JSON.
  3. **Input Manual Form**: Pengguna memasukkan 4 titik koordinat di modal.
- **Peran EXIF GPS & Orientasi**:
  Hanya berupa **satu titik koordinat posisi kamera drone** ($lat, lon, altitude$) saat foto diambil. **TIDAK BOLEH digunakan sebagai bounding box**. EXIF hanya dimanfaatkan untuk ekstraksi metadata kamera drone, tanggal terbang, dan opsi tombol _"Pusatkan Peta ke Lokasi Drone"_.
  _Penanganan EXIF Orientation != 1_: Citra orthomosaic drone terikat secara spasial pada grid koordinat North-up. Jika EXIF memuat tag `Orientation != 1` (mis. rotasi 90°/180°/270°), rotasi bitmap otomatis **DIABAIKAN** demi mencegah rusaknya kesesuaian koordinat georeferensi, dan peringatan non-blocking dicatat ke dalam `TechnicalMetadata.warnings`.
- **Evaluasi Decoder: `jpeg-decoder` vs `zune-jpeg`**:
  - `jpeg-decoder` (v0.3.2, Maret 2024, repo aktif di bawah `image-rs`) dipilih secara sadar karena menyediakan fitur **DCT-domain scaling** (`scale(w, h)`). Fitur ini memangkas ukuran buffer langsung di tahap dekuantisasi/IDCT ke skala 1/2, 1/4, atau 1/8, sehingga citra 100–144 MP tidak pernah dialokasikan dalam resolusi penuh di RAM.
  - `zune-jpeg` sengaja tidak dipilih karena dirancang untuk dekompresi penuh berbasis throughput SIMD (AVX2/NEON) dan **tidak mendukung arbitrary DCT-domain downscaling saat dekuantisasi**. Penggunaan `zune-jpeg` pada citra 144 MP akan memaksakan alokasi buffer piksel utuh sebesar $12.000 \times 12.000 \times 3 = 432\text{ MB}$ sebelum downsample, menyebabkan lonjakan RAM yang melanggar batas pagu memori.
- **Penanganan Kasus Tepi (Edge Cases) Format**:
  - _JPEG Arithmetic Coding (SOF9 / SOF10)_: Ditolak seketika pada pembacaan header segmen SOF dengan error `FormatTidakDidukung` (_"JPEG dengan pengkodean arithmetic (SOF9/SOF10) tidak didukung. Harap simpan citra dengan Huffman coding standar."_).
  - _JPEG 12-bit (Sample Precision = 12)_: Ditolak dengan error `FormatTidakDidukung` (_"JPEG 12-bit tidak didukung. Hanya JPEG 8-bit yang didukung."_).
  - _JPEG CMYK / YCCK (Jumlah Komponen = 4)_: Ditolak dengan error `FormatTidakDidukung` (_"JPEG dengan 4 komponen warna (CMYK/YCCK) tidak didukung. Harap konversi ke format RGB."_).
  - _PNG 16-bit Depth_: Ditolak pada Opsi A dengan error `FormatTidakDidukung` (_"PNG 16-bit tidak didukung pada Opsi A. Harap gunakan format 8-bit atau gunakan Opsi B."_).
  - _PNG Palette (Indexed Color)_: Ditolak pada Opsi A dengan error `FormatTidakDidukung` (_"PNG Palette (Indexed Color) tidak didukung. Harap konversi ke RGB/RGBA."_).
  - _PNG Interlaced (Adam7)_: Membutuhkan alokasi frame penuh di memori untuk merekonstruksi 7-pass Adam7 sebelum disuapkan per baris ke resampler. Diberi batas aman ketat **$\le 25\text{ Megapiksel}$** (mis. $5.000 \times 5.000\text{ px}$). Jika melebihi 25 MP, ditolak seketika dengan error `MelebihiBatas` (_"PNG interlaced (Adam7) dengan resolusi ... melebihi batas aman 25 MP karena memerlukan alokasi frame penuh di memori. Harap simpan citra sebagai PNG non-interlaced atau JPEG."_). Klaim "didukung penuh tanpa batas khusus" pada draf awal telah resmi diperbaiki.
- **Mitigasi Memori & Deteksi Progressive JPEG**:
  - Deteksi SOF marker: Jika terdeteksi marker `0xFF, 0xC2` (Progressive JPEG) dan total piksel $> 40\text{ MP}$, tolak dengan error `MelebihiBatas` (_"JPEG progressive dengan resolusi ... melebihi batas aman 40 MP. Harap simpan citra sebagai baseline JPEG atau PNG."_).
  - Batas maksimal total piksel Opsi A: **144 Megapiksel** (mis. $12.000 \times 12.000\text{ px}$) dan ukuran berkas maksimal **50 MB**.
- **Validasi Rasio Aspek Bbox Manual**:
  Pada input koordinat manual di UI, rasio aspek bounding box dihitung dalam ruang proyeksi Web Mercator:
  $$\text{Aspect}_{\text{bbox}} = \frac{(east - west) \cdot \cos(lat_{mid})}{north - south}$$
  Jika selisih rasio aspek terhadap citra ($W / H$) melebihi **5%**, UI menampilkan peringatan non-blocking (Callout peringatan kuning) tanpa memblokir tombol impor. Tombol _"Isi dari Viewport Peta"_ dilengkapi atribut eksplisit `type="button"` agar tidak pernah memicu pengiriman form secara otomatis.
- **Penyimpanan Berkas Asli**: Selalu disalin ke `map_storage/images/{id}/original.{ext}` dengan nama normalisasi (`original.jpg` atau `original.png`).

### 3.2 Opsi B: TIFF -> Konversi (GeoTIFF)

- **Model Memori & Pemilihan Arsitektur Resampling (Pilihan Sadar Opsi B)**:
  - _Karakteristik Masukan vs Pemrosesan_: Decoder TIFF (`tiff 0.9.1`) mengembalikan data raster dalam bentuk blok tile ($TileWidth \times TileHeight$, lazimnya $512 \times 512\text{ px}$) untuk Tiled TIFF, atau strip horizontal ($W \times rows\_per\_strip$) untuk Stripped TIFF. Sementara itu, `StreamingResampler` dirancang murni berbasis aliran baris horizontal (`push_row_u8` / `push_row_u16`).
  - _Evaluasi Dua Pendekatan Arsitektur_:
    1. **Opsi (i) - Merakit Strip Tile-Row Penuh (DIPILIH SECARA SADAR)**:
       Sistem membaca satu baris tile horizontal penuh (mis. $\lceil W / 512 \rceil$ blok tile) dan merakitnya ke dalam buffer memori sementara selebar citra dengan tinggi 512 baris ($W \times 512$). Setelah satu baris tile terakit penuh, 512 baris disuapkan secara sekuensial per baris ke `resampler.push_row()`. Buffer strip ini dialokasikan satu kali dan digunakan kembali (_reused_) untuk baris tile berikutnya.
    2. **Opsi (ii) - Menambah API `push_block(x, y, w, h)` dengan Akumulator Band 2D (DITOLAK)**:
       Mengharuskan penampung akumulator 2D per kolom tile dengan pelacakan kontribusi fraksional koordinat $X$ dan $Y$ non-integer. Menambah kompleksitas state-machine secara signifikan tanpa manfaat memori yang berarti pada batas ukuran citra yang didukung.
  - _Kalkulasi Matematis Puncak Memori Kasus Terburuk (Citra 500 MP / Lebar 22.000 px)_:
    - **Buffer Strip Tile-Row ($22.000 \times 512\text{ px}$)**:
      - Data 8-bit RGBA: $22.000 \times 512 \times 4\text{ byte} = 45{,}05\text{ MB} \approx 43{,}0\text{ MiB}$.
      - Data 16-bit RGBA: $22.000 \times 512 \times 8\text{ byte} = 90{,}11\text{ MB} \approx 85{,}9\text{ MiB}$.
    - **Buffer Output Display ($4.096 \times 4.096\text{ px}$)**:
      - Display RGBA: $4.096 \times 4.096 \times 4\text{ byte} = 67{,}11\text{ MiB}$.
      - Display RGB: $4.096 \times 4.096 \times 3\text{ byte} = 50{,}33\text{ MiB}$.
    - **Akumulator Aktif `StreamingResampler` (2 Baris Aktif f64, RGBA 4.096 px)**:
      - 2 baris output aktif $\times (4.096 \times 4 \text{ kanal} \times 8\text{ byte} + 4.096 \times 8\text{ byte bobot}) = 327.680\text{ bytes} \approx \mathbf{0{,}26\text{ MB} - 0{,}31\text{ MiB}}$.
    - **Puncak Memori Total Kasus Terburuk (16-bit RGBA 500 MP)**:
      $$\text{Total RAM} \approx 85{,}9\text{ MiB (Strip)} + 67{,}11\text{ MiB (Display)} + 0{,}31\text{ MiB (Resampler)} + 25\text{ MiB (Heap/Decompress)} \approx \mathbf{178{,}3\text{ MiB}}$$
      Puncak ini **sangat aman di bawah batas pagu $400\text{ MiB}$** ($\approx 44{,}6\%$ dari batas maksimum yang diizinkan).
  - _Penanganan TIFF Stripped_:
    Pada Stripped TIFF, data raster disimpan per strip horizontal memanjang ($W \times rows\_per\_strip$, dengan $rows\_per\_strip$ umumnya 1 hingga 32 baris). Buffer strip untuk $22.000 \times 32$ hanya berukuran $\approx 2{,}8\text{ MiB}$ (8-bit) atau $\approx 5{,}6\text{ MiB}$ (16-bit). Membaca TIFF stripped langsung mengisi buffer strip dan disuapkan ke `push_row()`, sehingga performa dan efisiensi memorinya bahkan lebih optimal daripada tiled TIFF.
- **Optimasi Piramida Internal (Overview IFD)**:
  Jika berkas GeoTIFF merupakan Cloud Optimized GeoTIFF (COG) atau memiliki sub-IFD piramida resolusi, dekoder memilih sub-IFD yang dimensinya paling mendekati target display ($\ge 4.096\text{ px}$), menghemat waktu dekompresi hingga $4–10\times$.
- **Evaluasi Kompresi TIFF (Crate `tiff 0.9.1`)**:
  - _Didukung Penuh_: Uncompressed (None / ID 1), LZW (ID 5), PackBits (ID 32773), Deflate / Adobe ZIP (ID 8), PKZIP Deflate (ID 32946), Horizontal Differencing Predictor.
  - _Dukungan JPEG-in-TIFF (Compression 7 - Terverifikasi Penuh)_:
    Dukungan JPEG-in-TIFF telah diverifikasi secara numerik dan fungsional terhadap citra nyata OAM Balikpapan (273 Megapiksel, YCbCr 4:2:0 tiled 512x512 dengan shared JPEGTables DHT/DQT, selisih numerik terhadap `jpeg-decoder` independen adalah 0,0000) serta varian sintetis RGB 4:4:4.
    _Batasan yang Terverifikasi_:
    1. Layout: Tiled TIFF atau Stripped TIFF ber-PlanarConfiguration 1 (Chunky).
    2. Format Warna: RGB (Photometric 2) dan YCbCr (Photometric 6).
    3. Batasan: Old JPEG (Compression 6 / TIFF 6.0 usang tanpa JPEGTables) dan PlanarConfiguration 2 (separate planes) tidak didukung dan ditolak secara aman dengan pesan jelas.
  - _Kompresi Tak Didukung (Ditolak Eksplisit)_: CCITT Group 3/4 Fax (ID 3, 4), JBIG (ID 34661), LERC, WebP, dan JPEG 2000.
- **Deteksi Alpha & Nodata**:
  - Deteksi Alpha **wajib** melalui tag `ExtraSamples` (Tag 338: 1=Associated Alpha, 2=Unassociated Alpha), bukan dari jumlah kanal $\ge 4$.
  - Jika band $\ge 4$ tanpa tag `ExtraSamples` (mis. citra multispektral RGB+NIR): Sistem mengambil 3 band pertama (RGB) untuk visualisasi display dengan mencatat pesan peringatan pada `warnings`.
  - Nodata: Jika terdapat tag `GDAL_NODATA`, piksel bernilai nodata diubah menjadi transparan (Alpha = 0) **HANYA JIKA SEMUA KANAL bernilai nodata** (mencegah atap/tanah berwarna hitam murni berubah menjadi bolong).
  - Jika citra memiliki Alpha atau Nodata, keluaran display **wajib berformat PNG**. Jika RGB murni tanpa alpha, keluaran berformat **JPEG**.
- **Konversi Bit Depth 16-bit ke 8-bit (Percentile Stretch $p_{0{,}5} - p_{99{,}5}$ Deterministik)**:
  - Data sensor drone 16-bit lazimnya tidak memanfaatkan rentang penuh 16-bit (mis. sensor 10-bit atau 12-bit disimpan dalam kontainer 16-bit sehingga intensitas maksimum $\le 4.095$, yang jika di-shift kanan $8$ akan tampil hitam pekat). Sebaliknya, min-max stretch rentan terhadap satu piksel derau/hot-pixel.
  - _Metode Deterministik_:
    1. Mengumpulkan histogram 16-bit (65.536 bins) dari sampel tile deterministik ($N \ge 9$ tile yang tersebar merata di grid koordinat citra, bukan acak).
    2. Mengecualikan piksel yang bernilai nodata atau memiliki alpha = 0.
    3. Menghitung persentil kumulatif $p_{0{,}5}$ (batas bawah $lo$) dan $p_{99{,}5}$ (batas atas $hi$) secara **bersama/shared** untuk seluruh kanal warna (satu pasang $[lo, hi]$ yang sama untuk RGB guna menjaga keseimbangan warna dan mencegah distorsi tinting warna).
    4. _Pengecualian_: Jika data terdeteksi sudah full-range ($lo \le 50$ dan $hi \ge 65.400$), stretch dilewati dan diterapkan pemetaan linier langsung ($v \gg 8$).
    5. Fungsi `push_row_u16` menerima parameter $(lo, hi)$ hasil analisis untuk memetakan nilai $[lo .. hi] \rightarrow [0 .. 255]$:
       $$v_8 = \left\lfloor \frac{\text{clamp}(v_{16}, lo, hi) - lo}{hi - lo} \times 255.0 + 0{,}5 \right\rfloor \text{ as u8}$$
    6. Metode peregangan kontras, nilai $lo$, dan nilai $hi$ dicatat secara transparan pada `metadata.json` dan daftar `warnings`.
- **Matriks Format Sampel**:
  - RGB 8-bit / 16-bit $\rightarrow$ **Didukung**.
  - RGBA 8-bit / 16-bit $\rightarrow$ **Didukung** (output PNG).
  - Grayscale 8-bit / 16-bit $\rightarrow$ **Didukung** (direplikasi ke R=G=B).
  - Palette / Indexed Color $\rightarrow$ **Ditolak** (`FormatTidakDidukung`).
  - CMYK $\rightarrow$ **Ditolak** (`FormatTidakDidukung`).
  - Float32 / DEM $\rightarrow$ **Ditolak** (`FormatTidakDidukung`).
- **Batas Maksimal Piksel Opsi B**:
  Maksimal **500 Megapiksel** (mis. $25.000 \times 20.000\text{ px}$) untuk mencegah _decompression bomb_ dari file TIFF yang terkompresi sangat tinggi.

### 3.3 Georeferensi & Reproyeksi Koordinat (Modul Bersama `projection.rs`)

- **PENTING**: **Hanya koordinat bounding box yang direproyeksikan, bukan piksel raster citra**.
- **Perhitungan Envelope 4 Sudut**:
  Koordinat empat sudut citra dihitung: $(0, 0)$, $(W, 0)$, $(0, H)$, dan $(W, H)$.
  Keempat sudut dikonversi ke koordinat WGS84 derajat desimal, lalu envelope diambil:
  $$\text{south} = \min(lats), \quad \text{north} = \max(lats), \quad \text{west} = \min(lons), \quad \text{east} = \max(lons)$$
- **Offset Setengah Piksel (`RasterPixelIsArea` vs `RasterPixelIsPoint`)**:
  Jika tag `GTRasterTypeGeoKey` bernilai 2 (`RasterPixelIsPoint`), terapkan pergeseran batas setengah piksel ($0{,}5 \times \text{pixel\_scale}$) agar georeferensi akurat.
- **Deteksi Rotasi**:
  Jika `ModelTransformationTag` memuat nilai rotasi/shear ($M_{01} \neq 0$ atau $M_{10} \neq 0$), tolak dengan `RotasiTidakDidukung`.
- **Cakupan Reproyeksi Murni**:
  - `EPSG:4326` $\rightarrow$ Langsung digunakan (identitas).
  - `EPSG:3857` (Web Mercator) $\rightarrow$ Rumus invers bola mercator murni:
    $$lon = \frac{x}{a} \times \frac{180}{\pi}, \quad lat = \left( 2 \arctan\left(\exp\left(\frac{y}{a}\right)\right) - \frac{\pi}{2} \right) \times \frac{180}{\pi}$$
  - `UTM North / South WGS84` (EPSG:32601–32660, 32701–32760) $\rightarrow$ Rumus invers Snyder/Karney dengan penanganan _False Northing_ $10.000.000\text{ m}$ untuk belahan bumi selatan (misal EPSG:32750 UTM Zona 50S Balikpapan).
  - Kode CRS `32767` (User-Defined) atau di luar daftar di atas $\rightarrow$ Kembalikan `CrsTidakDidukung` dengan input manual (`is_recoverable = true`).
- **Asal Nilai Referensi Proyeksi & Pengujian Batas Zona**:
  Implementasi proyeksi (`projection.rs`) murni Rust tanpa dependensi runtime C/Python. Nilai referensi uji TIDAK diketik manual: dihasilkan oleh `scripts/gen_projection_golden.py` memakai pyproj 3.8.0 / PROJ 9.8.1 ke `src-tauri/tests/fixtures/projection_golden.json` (14 kasus), dan dibaca oleh tes Rust (forward dan inverse, toleransi $< 0{,}5\text{ m}$). Pyproj hanya dipakai saat membangkitkan golden (dev-time), bukan runtime. Test suite mencakup:
  1. _Belahan Bumi Utara & Greenwich_: Greenwich Royal Observatory ($51{,}476852^\circ\text{N}, 0{,}000000^\circ\text{E}$) yang merupakan batas timur persis UTM Zona 30N (EPSG:32630).
  2. _Titik Batas Zona UTM_: Titik bujur dekat batas barat ($114{,}001^\circ\text{E}$) dan dekat batas timur ($119{,}999^\circ\text{E}$) pada UTM Zona 50S (EPSG:32750).

---

## 4. Jaminan Ukuran Display $\le 5\text{ MB}$ (JPEG) & $\le 10\text{ MB}$ (PNG Alpha) (Loop Adaptif In-Memory)

Downscaling dilakukan langsung dari canvas penampung di RAM (selalu resize dari kanvas display asli di memori, bukan downscale berulang dari hasil resize sebelumnya, untuk menjaga kualitas):

### 4.1 Keputusan Desain Batas Ukuran Display & Tangga Dimensi

- **`DISPLAY_MAX_BYTES_JPEG = 5 MB`**: Format JPEG dengan kompresi lossy berkuantisasi DCT sangat efisien pada foto udara, mampu mencapai resolusi $4.096\text{ px}$ pada ukuran $\le 5\text{ MB}$.
- **`DISPLAY_MAX_BYTES_PNG_ALPHA = 10 MB`**: Citra orthomosaic dengan transparansi (nodata/border) wajib menggunakan format PNG lossless untuk menjaga ketajaman alpha mask. Foto udara berentropi kanopi/vegetasi riil yang tinggi pada resolusi $2.048\text{ px}$ secara fisik menghasilkan ukuran $\approx 11{,}2\text{ MB}$ dengan kompresor zlib/Deflate standar, sehingga tidak mungkin mencapai $2.048\text{ px}$ dalam batas $5\text{ MB}$. Oleh karena itu, batas PNG ber-alpha ditetapkan $10\text{ MB}$.
- **Tangga Dimensi Baru (Sisi Terpanjang)**:
  `[4096, 3584, 3072, 2560, 2304, 2048, 1792, 1536, 1280, 1024]`.
  Lantai keras batas bawah tetap **$1.024\text{ px}$**.
- **Estimasi Kuadratik Melompat Langsung**:
  Setelah encode pertama, estimasi dimensi baru dihitung dari rasio ukuran berkas:
  $$\text{dim}_{\text{baru}} \approx \text{dim}_{\text{saat\_ini}} \times \sqrt{\frac{\text{TargetBytes}}{\text{CurrentBytes}}}$$
  Sistem langsung melompat ke anak tangga terdekat dari estimasi tersebut, lalu melakukan encode verifikasi (iterasi berlanjut hingga hasil muat dalam batas ukuran atau lantai keras $1.024\text{ px}$ tercapai; tidak ada batas 3 iterasi).
- **Peringatan Resolusi Rendah ($< 2.048\text{ px}$)**:
  Jika resolusi akhir display jatuh ke anak tangga $< 2.048\text{ px}$ (misalnya $1.792\text{ px}$ pada citra OAM Balikpapan), sistem otomatis menyimpan peringatan di metadata layer (`warnings`) dan menampilkan banner kuning di UI:
  _"Resolusi tampilan dibatasi ke {W}x{H} untuk menghemat memori. Detail pada zoom tinggi mungkin berkurang."_

```text
+---------------------------------------------------------------------------------+
| Canvas Buffer di RAM (Resolusi Maksimal Awal 4.096 px)                          |
+---------------------------------------------------------------------------------+
                                       |
                   +-------------------+-------------------+
                   |                                       |
            [Citra RGB Murni]                      [Citra Alpha / Nodata]
                   |                                       |
                   v                                       v
         [Jalur Encode JPEG]                      [Jalur Encode PNG]
                   |                                       |
   1. Target <= 5 MB                                1. Target <= 10 MB
   2. Set Quality = 85                              2. Encode PNG awal (4.096 px)
   3. Jika size > 5 MB:                             3. Jika size > 10 MB:
      Turunkan Quality:                                Hitung estimasi kuadratik:
      80 -> 75 -> 70                                   dim ~ sqrt(10MB / size)
   4. Jika Quality 70 masih > 5 MB:                    Lompat ke anak tangga terdekat:
      Turunkan resolusi via tangga dimensi             [3584, 3072, 2560, 2304, 2048, 1792, ...]
      (sampai muat / lantai)                           (sampai muat / lantai)
   5. Batas bawah: dimensi 1.024 px                 4. Batas bawah: dimensi 1.024 px
   6. Jika 1.024 px masih > 5 MB:                   5. Jika 1.024 px masih > 10 MB:
      Kembalikan MelebihiBatas                         Kembalikan MelebihiBatas
```

- Alokasi memori tambahan selama loop adaptif: buffer salinan canvas downscaled + buffer encoder stream $\le \mathbf{30\text{ MB}}$.

### 4.2 Keterbatasan Arsitektur Single-Image Display

- Display ortofoto $2.048\text{ px}$ (atau $1.792\text{ px}$) yang di-resample dari citra sumber beresolusi raksasa (~$20.000\text{ px}$, 273 MP) secara inheren tidak akan tajam pada tingkat zoom yang sangat dalam (misal zoom level 20–22 di Leaflet).
- Ini adalah **batasan desain (_by-design_)** dari arsitektur _single-image overlay display_ yang memprioritaskan konsumsi RAM minimal ($\le 400\text{ MB}$) tanpa backend tile server.
- Solusi jangka panjang untuk ketajaman penuh pada zoom mendalam adalah pembuatan piramida ubin peta (_XYZ Map Tiles / Deep Zoom Tiles_), yang merupakan fitur masa depan di luar lingkup Module 3 saat ini.

---

## 5. Anggaran Memori Realistis & Estimasi Waktu

### 5.1 Model Alokasi Memori Puncak (Worst-Case Analysis)

#### A. Opsi A (JPG 144 Megapiksel):

- Buffer decode setelah IDCT scale 1/2 ($6.000 \times 6.000 \times 3\text{ B}$): **$108\text{ MB}$**.
- Buffer display output target ($4.096 \times 4.096 \times 3\text{ B}$): **$50{,}3\text{ MB}$**.
- Buffer perantara encoder JPEG: **$15\text{ MB}$**.
- Base process runtime Tauri / WebView: **$60\text{ MB}$**.
- **Puncak Memori Opsi A Realistis (Baseline JPEG 144 MP)**: $\approx \mathbf{233\text{ MB RAM}}$ (stabil di bawah pagu $500\text{ MB}$).
- **Kasus Khusus Progressive JPEG 40 MP (Batas Maksimal Progressive yang Diizinkan)**:
  - Buffer koefisien DCT penuh seluruh blok ($6.324 \times 6.324 \times 3 \text{ komponen} \times 2 \text{ B}$ koefisien DCT): **$\approx 240\text{ MB}$**.
  - Buffer rekonstruksi piksel RGB penuh: **$120\text{ MB}$**.
  - Buffer display target ($4.096 \times 4.096 \times 3\text{ B}$): **$50{,}3\text{ MB}$**.
  - Base process runtime Tauri / WebView: **$60\text{ MB}$**.
  - **Puncak Memori Kasus Terburuk Progressive 40 MP**: $\approx \mathbf{470{,}3\text{ MB RAM}}$ (berada tepat di bawah pagu mutlak $500\text{ MB}$; JPEG progressive $> 40\text{ MP}$ ditolak seketika via fast-fail untuk mencegah OOM).

#### B. Opsi B (GeoTIFF 250 MB Streaming):

- Buffer canvas display RGBA ($4.096 \times 4.096 \times 4\text{ B}$): **$67{,}1\text{ MB}$**.
- Akumulator Box Filter per baris tile ($4.096 \times 128 \times 4 \times 4\text{ B}$): **$8{,}4\text{ MB}$**.
- Buffer single chunk tile 16-bit ($512 \times 512 \times 8\text{ B}$): **$2{,}1\text{ MB}$**.
- Salinan file asli (streaming copy buffer per 64 KB): **$0{,}06\text{ MB}$**.
- Buffer perantara encoder PNG/JPG: **$25\text{ MB}$**.
- Base process runtime Tauri / WebView: **$60\text{ MB}$**.
- **Puncak Memori Opsi B Realistis**: $\approx \mathbf{163\text{ MB RAM}}$ (jauh di bawah batas pagu $200 - 500\text{ MB}$).

### 5.2 Matriks Estimasi Ukuran Berkas & Waktu Eksekusi

_(Catatan: Seluruh angka waktu dan memori di bawah adalah hasil estimasi teoritis yang akan divalidasi lewat pengujian empiris)_

| Ukuran Berkas Sumber |                  Tipe & Layout                   |   Estimasi Puncak RAM   | Estimasi Waktu (Dev / opt-level 3) | Estimasi Waktu (Release Mode) |                Resolusi & Ukuran Display Akhir                 |
| :------------------: | :----------------------------------------------: | :---------------------: | :--------------------------------: | :---------------------------: | :------------------------------------------------------------: |
|      **50 MB**       | Tiled GeoTIFF ($10.000 \times 8.000\text{ px}$)  | $\approx 125\text{ MB}$ |        $2 - 4\text{ detik}$        |    $< 1{,}5\text{ detik}$     |   $4.096 \times 3.277\text{ px}$ ($\le 2{,}8\text{ MB}$ JPG)   |
|      **100 MB**      | Tiled GeoTIFF ($15.000 \times 11.000\text{ px}$) | $\approx 145\text{ MB}$ |        $4 - 7\text{ detik}$        |   $2 - 3{,}5\text{ detik}$    |   $4.096 \times 3.004\text{ px}$ ($\le 3{,}9\text{ MB}$ JPG)   |
|      **250 MB**      | Tiled GeoTIFF ($22.000 \times 16.000\text{ px}$) | $\approx 175\text{ MB}$ |       $9 - 15\text{ detik}$        |     $4 - 7\text{ detik}$      | $3.200 \times 2.327\text{ px}$ ($\le 4{,}8\text{ MB}$ PNG/JPG) |

### 5.3 Hasil Pengukuran Memori & Waktu Empiris

Setiap skenario diukur di SUBPROSES terpisah (fixture dibangun di proses induk, di luar pengukuran) lewat `module_03_memory_benchmark.rs`. Mode build: `cargo test` (dev; crate uji opt-level 0, png/tiff/jpeg/image opt-level 3, crate aplikasi `aerial-analytics-platform` opt-level 0 default). Seluruh fixture GeoTIFF diverifikasi dekompresinya secara independen menggunakan Pillow 13.0.0 dan tifffile 2026.10.15.

| Skenario                                | Baseline WS |    Peak WS |   Delta WS | Peak Pagefile | Waktu (dev) | Display akhir          | Ukuran display |
| :-------------------------------------- | ----------: | ---------: | ---------: | ------------: | ----------: | :--------------------- | -------------: |
| JPEG baseline 100 MP                    |    0,11 MiB | 155,71 MiB | 155,60 MiB |    148,12 MiB |     13,52 s | 4096×4096              |    4.652.972 B |
| JPEG baseline 144 MP                    |    0,11 MiB | 159,88 MiB | 159,77 MiB |    159,15 MiB |     15,10 s | 4096×4096              |    4.697.448 B |
| JPEG 4:2:0 40 MP                        |    0,11 MiB | 177,01 MiB | 176,90 MiB |    176,15 MiB |      7,46 s | 4096×2560              |    4.257.760 B |
| JPEG progresif 40 MP                    |    0,11 MiB | 292,20 MiB | 292,09 MiB |    291,36 MiB |      7,62 s | 4096×2560              |    4.257.760 B |
| PNG opak 100 MP                         |    0,11 MiB | 131,88 MiB | 131,77 MiB |    130,64 MiB |     17,83 s | 4096×4096              |    1.565.999 B |
| PNG interlaced 25 MP                    |    0,11 MiB | 163,33 MiB | 163,22 MiB |    162,55 MiB |      9,80 s | 4096×4096              |      898.285 B |
| GeoTIFF RGBA 8-bit 8000×6000 (95 MB)    |    0,11 MiB |  99,85 MiB |  99,74 MiB |     98,17 MiB |     22,12 s | 4096×3072 (56,6% opak) |    7.330.662 B |
| GeoTIFF RGBA 16-bit 6000×4500 (115 MB)  |    0,11 MiB | 137,05 MiB | 136,94 MiB |    135,15 MiB |     60,44 s | 3072×2304 (56,6% opak) |    8.416.404 B |
| GeoTIFF RGBA 16-bit 22000×4000 (140 MB) |    0,11 MiB |  27,21 MiB |  27,11 MiB |     25,52 MiB |     23,52 s | 4096×745 (56,7% opak)  |    2.998.878 B |

#### Rincian Waktu per Tahap Pipeline (Pengukuran `Instant` Aktual)

Pengukuran waktu per tahap pada `execute_import_option_b` mengungkap distribusi durasi secara presisi:

- **GeoTIFF RGBA 8-bit ($8.000 \times 6.000\text{ px}$)**:
  - `header_georef`: $6\text{ ms}$
  - `sampling_16bit`: $0\text{ ms}$ (dilewati karena 8-bit)
  - `decode_resample`: $15.078\text{ ms}$ ($15{,}08\text{ s}$)
  - `encode_display`: $9.262\text{ ms}$ ($9{,}26\text{ s}$)
  - `write_register`: $5\text{ ms}$
  - Total: $24.357\text{ ms}$ ($24{,}36\text{ s}$)
- **GeoTIFF RGBA 16-bit ($6.000 \times 4.500\text{ px}$)**:
  - `header_georef`: $10\text{ ms}$
  - `sampling_16bit`: $1.520\text{ ms}$ ($1{,}52\text{ s}$, berhenti tepat saat batas $1.500\text{ ms}$ terpenuhi)
  - `decode_resample`: $16.532\text{ ms}$ ($16{,}53\text{ s}$)
  - `encode_display`: **$50.853\text{ ms}$ ($50{,}85\text{ s}$, $\approx 73\%$ total durasi)**
  - `write_register`: $3\text{ ms}$
  - Total: $68.927\text{ ms}$ ($68{,}93\text{ s}$)

**Mengapa `encode_display` 16-bit memakan 50,85 detik di dev build?**
Pada citra 16-bit RGBA berentropi tinggi, tangga adaptif pertama kali mencoba encode dimensi $4.096\text{ px}$ ($4096 \times 3072$) menghasilkan berkas $> 10\text{ MiB}$. Estimasi kuadratik melompat ke $3.072\text{ px}$, melakukan resampling box filter dari kanvas asli, lalu melakukan encode Deflate ulang hingga menghasilkan $8{,}02\text{ MiB}$ ($8.416.404\text{ B}$). Pada build unoptimized (`dev`), dua kali kompresi Deflate beresolusi tinggi + resample memakan $50{,}85\text{ s}$. Sebaliknya, citra OAM nyata Balikpapan ($273\text{ MP}$) adalah RGB JPEG-in-TIFF tanpa alpha, sehingga dienkode sebagai **JPEG** (`display.jpg`) dalam waktu $< 0{,}5\text{ s}$ per iterasi (total $6{,}91\text{ s}$).

#### Penjelasan Kuantitatif Memori Kasus $22.000 \times 4.000\text{ px}$ (Hanya 27,21 MiB)

Rendahnya puncak RAM ($27{,}21\text{ MiB}$) pada citra lebar $22.000\text{ px}$ disebabkan oleh geometri kanvas display:

1. **Aspek Rasio Sangat Pipih ($5{,}5 : 1$)**: Kanvas display dibatasi sisi terpanjang $4.096\text{ px}$, sehingga dimensi display akhir hanya **$4.096 \times 745\text{ px}$**.
2. **Ukuran Buffer Display Kecil**: Buffer RGBA display hanya berukuran $4.096 \times 745 \times 4\text{ B} = \mathbf{11{,}64\text{ MiB}}$ (dibandingkan $48{,}00\text{ MiB}$ pada citra $4.096 \times 3.072$).
3. **Akumulator Resampler**: Baris target resampler hanya 745 baris, memangkas memori bobot box filter sebesar $4\times$.
4. **Buffer Strip Dekompresi**: Satu strip ($22.000 \times 64\text{ baris} \times 4 \times 2\text{ B}$) memakan $\mathbf{11{,}26\text{ MiB}}$.
5. **Total Teoritis**: $11{,}64\text{ MiB (display)} + 11{,}26\text{ MiB (strip)} + \approx 2\text{ MiB (resampler/overhead)} \approx \mathbf{25\text{ MiB}}$, tepat selaras dengan hasil terukur **$27{,}21\text{ MiB}$**.

---

## 6. Contoh Nyata `meta.json` dan `metadata.json`

### 6.1 Contoh `meta.json` (Kompatibel Penuh 100% dengan Leaflet & LayerManager)

```json
{
  "schema_version": 2,
  "id": "ortho_1792850000000",
  "filename": "DJI_0042_ortho",
  "category": "ortho",
  "extension": "jpg",
  "file_path": "/map_storage/images/ortho_1792850000000",
  "size_bytes": 48234496,
  "dimensions": [12000, 9000],
  "thumbnail_url": "/map_storage/images/ortho_1792850000000/display.jpg",
  "bounds": [
    [-1.253814, 116.850648],
    [-1.247529, 116.859893]
  ],
  "aspect_ratio": 1.3333333333,
  "display_size_bytes": 2841600,
  "is_corrupted": false,
  "created_at": "2026-10-25T18:01:00Z",
  "updated_at": null
}
```

### 6.2 Contoh `metadata.json` Opsi A (JPG + World File / EXIF)

```json
{
  "schema_version": 1,
  "image_id": "ortho_1792850000000",
  "import_mode": "A",
  "source_format": "jpeg",
  "source_filename": "DJI_0042_ortho.jpg",
  "source_dimensions": [12000, 9000],
  "display_dimensions": [4000, 3000],
  "source_file_size_bytes": 48234496,
  "display_file_size_bytes": 2841600,
  "display_format": "jpeg",
  "bounds": {
    "south": -1.253814,
    "west": 116.850648,
    "north": -1.247529,
    "east": 116.859893,
    "source": "world_file"
  },
  "source_crs": "EPSG:4326",
  "exif_summary": {
    "camera_make": "DJI",
    "camera_model": "FC300X",
    "captured_at": "2026-10-24T09:15:30Z",
    "gps_latitude": -1.250671,
    "gps_longitude": 116.855271,
    "gps_altitude_agl_m": 120.5
  },
  "tiff_technical_details": null,
  "imported_at": "2026-10-25T18:01:00Z",
  "warnings": []
}
```

### 6.3 Contoh `metadata.json` Opsi B (GeoTIFF Konversi, TIFF Asli Tidak Disimpan)

```json
{
  "schema_version": 1,
  "image_id": "ortho_1792850000001",
  "import_mode": "B",
  "source_format": "geotiff",
  "source_filename": "kariangau_orthomosaic_utm50s.tif",
  "source_dimensions": [20111, 13583],
  "display_dimensions": [3200, 2161],
  "source_file_size_bytes": 188743680,
  "display_file_size_bytes": 3845120,
  "display_format": "png",
  "bounds": {
    "south": -1.253814,
    "west": 116.850648,
    "north": -1.247529,
    "east": 116.859893,
    "source": "geotiff"
  },
  "source_crs": "EPSG:32750",
  "exif_summary": null,
  "tiff_technical_details": {
    "bands": 4,
    "bit_depth": 8,
    "sample_format": "uint8",
    "compression": "deflate",
    "layout": "tiled",
    "chunk_dimensions": [512, 512],
    "total_chunks": 1080,
    "planar_configuration": "chunky",
    "photometric_interpretation": "rgb",
    "has_alpha": true,
    "nodata_value": 0.0,
    "software": "OpenDroneMap 3.2"
  },
  "imported_at": "2026-10-25T18:02:00Z",
  "warnings": [
    "Koordinat batas area (bounding box) berhasil dikonversi dari EPSG:32750 (UTM Zona 50S) ke WGS84 derajat desimal.",
    "Citra display di-downscale ke resolusi 3.200 px agar memenuhi batas ukuran berkas display <= 5 MB."
  ]
}
```

---

## 7. Matriks Penanganan Error Lengkap

| Kode Error                          | Skenario Pemicu                                                                                                       | Pesan untuk Pengguna (Bahasa Indonesia)                                                                                                                                     | Recoverable? | Aksi Pemulihan                                                                  |
| :---------------------------------- | :-------------------------------------------------------------------------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :----------: | :------------------------------------------------------------------------------ |
| `FormatTidakDidukung`               | Ekstensi `.tif` tapi bukan TIFF, Progressive JPEG > 40 MP, atau format palet/CMYK/DEM.                                | "Format berkas tidak dikenali atau tidak didukung. Harap gunakan berkas TIFF, Baseline JPEG, atau PNG dalam format warna RGB/RGBA."                                         |    Tidak     | Pilih berkas citra yang sesuai standar RGB/RGBA.                                |
| `KompresiTidakDidukung`             | TIFF menggunakan kompresi yang benar-benar tidak didukung (mis. CCITT Fax T.4/T.6, JBIG, LERC, JPEG 2000, atau WebP). | "Metode kompresi TIFF '{kompresi}' belum didukung oleh decoder pure Rust. Harap gunakan kompresi Deflate, LZW, JPEG, PackBits, atau Uncompressed (Tiled GeoTIFF)."          |    Tidak     | Ekspor ulang berkas dengan kompresi Deflate/LZW/JPEG.                           |
| `FileKorup`                         | Berkas terpotong di tengah, header TIFF terkorupsi, atau chunk gagal didecode.                                        | "Berkas citra rusak atau tidak lengkap sehingga tidak dapat diproses."                                                                                                      |    Tidak     | Gunakan berkas citra asli yang tidak rusak.                                     |
| `MelebihiBatas` (File Size)         | Berkas TIFF > 250 MB atau JPG/PNG > 50 MB.                                                                            | "Ukuran berkas ({size} MB) melebihi batas maksimal yang diizinkan ({max} MB)."                                                                                              |    Tidak     | Kompresi atau pangkas area citra sebelum diimpor.                               |
| `MelebihiBatas` (Piksel Opsi A)     | Citra JPG/PNG memiliki total piksel > 144 Megapiksel.                                                                 | "Total dimensi citra ({mp} MP) melebihi batas aman memori (144 MP)."                                                                                                        |    Tidak     | Perkecil resolusi citra sebelum diimpor.                                        |
| `MelebihiBatas` (Piksel Opsi B)     | Citra TIFF memiliki total piksel > 500 Megapiksel.                                                                    | "Total dimensi citra TIFF ({mp} MP) melebihi batas aman dekompresi (500 MP)."                                                                                               |    Tidak     | Pangkas area citra sebelum diimpor.                                             |
| `MelebihiBatas` (Strip Raksasa)     | TIFF uncompressed disimpan dalam strip raksasa (ukuran strip > 30 MB per chunk).                                      | "Format internal berkas TIFF memiliki ukuran strip terlalu besar ({dim} px) yang tidak aman untuk memori. Harap ekspor ulang dengan format Tiled TIFF."                     |    Tidak     | Ekspor ulang citra drone dengan opsi _Tiled GeoTIFF_.                           |
| `MelebihiBatas` (Display File Size) | Kompresi display mencapai dimensi minimum ($1.024\text{ px}$) namun ukuran berkas masih $> 5\text{ MB}$.              | "Gagal mengompresi citra display di bawah 5 MB tanpa kehilangan kualitas kritis."                                                                                           |    Tidak     | Reduksi area citra sebelum diimpor.                                             |
| `KoordinatTidakValid`               | Koordinat lintang/bujur terbalik ($south \ge north$ atau $west \ge east$) atau out-of-range.                          | "Batas koordinat tidak valid: Lintang Selatan harus lebih kecil dari Lintang Utara, dan Bujur Barat harus lebih kecil dari Bujur Timur."                                    |    **Ya**    | Pengguna mengoreksi input koordinat di formulir modal.                          |
| `CrsTidakDidukung`                  | GeoTIFF menggunakan proyeksi lokal/asing atau kode CRS User-Defined (32767).                                          | "Sistem koordinat citra ({crs}) belum didukung otomatis. Anda dapat memasukkan koordinat batas WGS84 secara manual."                                                        |    **Ya**    | Modal mengaktifkan field koordinat manual agar pengguna dapat memasukkan WGS84. |
| `RotasiTidakDidukung`               | GeoTIFF atau World File memuat koefisien rotasi/skew affine.                                                          | "Citra memiliki orientasi rotasi/kemiringan. Peta hanya mendukung citra tegak lurus (North-up)."                                                                            |    Tidak     | Luruskan orientasi citra di software fotogrametri/GIS.                          |
| `DiskPenuh`                         | Ruang hard disk tidak mencukupi saat preflight check atau penulisan staging.                                          | "Penyimpanan disk tidak mencukupi. Membutuhkan setidaknya {req} MB ruang kosong."                                                                                           |    **Ya**    | Kosongkan ruang penyimpanan hard disk lalu coba lagi.                           |
| `JobSedangBerjalan`                 | Pengguna memicu impor baru saat ada job lain yang sedang berjalan.                                                    | "Proses impor lain sedang berlangsung. Harap tunggu hingga selesai atau batalkan job aktif."                                                                                |    **Ya**    | Tunggu hingga job selesai atau klik tombol batalkan.                            |
| `Dibatalkan`                        | Pengguna menekan tombol "Batalkan" di modal progres.                                                                  | "Proses impor citra telah dibatalkan oleh pengguna."                                                                                                                        |    Tidak     | Modal ditutup, folder staging otomatis dibersihkan tanpa sampah.                |
| `InternalError`                     | Kesalahan internal tak terduga (mis. panic thread tertangkap JoinError).                                              | "Terjadi kesalahan internal saat memproses citra. Detail teknis telah disiapkan untuk disalin."                                                                             |    Tidak     | Salin detail error untuk pelaporan bug.                                         |
| `AplikasiSudahBerjalan`             | Instansi sekunder mencoba menjalankan impor atau operasi tulis storage (CRUD ortho/layer/spatial result).             | "Aplikasi Aerial Analytics Platform sudah berjalan di jendela/proses lain. Operasi impor dan modifikasi data dinonaktifkan pada instansi kedua demi mencegah konflik data." |    **Ya**    | Tutup jendela instansi lain jika ingin melakukan impor atau modifikasi data.    |

---

## 8. Catatan Teknis & Evaluasi Crate

### 8.1 Evaluasi Panic Profil Proyek

- Pada `src-tauri/Cargo.toml`, profil release dikonfigurasi dengan:
  ```toml
  [profile.release]
  panic = "abort"
  ```
- **Konsekuensi Arsitektur**: Pada profil release, panic pada thread pekerja tidak dapat ditangkap oleh `catch_unwind` dan guard `Drop` (`StagingGuard`) tidak akan dipanggil karena proses langsung diaborsi oleh OS.
- **Jaring Pengaman Utama**: Oleh karena itu, **Startup Cleanup (`cleanup_stale_import_jobs` dan `reconcile_storage_layers`) adalah jaring pengaman utama yang mutlak** untuk membersihkan folder `.staging_*` dan merekonsiliasi layer jika proses mati mendadak. Pada profil dev dan test, `panic = "unwind"` tetap berlaku normal.

### 8.2 Konfigurasi Linter (`clippy.toml`)

Untuk mencegah penegakan `#![deny(clippy::unwrap_used)]` memblokir pengujian unit/integrasi pada folder `tests/`, ditambahkan konfigurasi:

```toml
# clippy.toml
allow-unwrap-in-tests = true
```

### 8.3 Pemilihan Crate Pure Rust & Versi

- `tiff = "0.9.1"`: Pure Rust, dipelihara oleh tim `image-rs`.
- `kamadak-exif = "0.6.1"`: Pure Rust, parser EXIF terisolasi.
- `jpeg-decoder = "=0.3.2"`: Pure Rust, versi di-pin secara ketat (`=0.3.2`).
  - **Status Upstream** (kutipan verbatim README `jpeg-decoder-0.3.2` dari registry cargo lokal): "**WARNING: This crate is in maintenance mode as _image-rs_ seeks to move over to the zune-jpeg crate. However, since other Rust JPEG crates do not support lossless mode, we will continue to welcome PRs related to that functionality.**" Tanggal commit/rilis upstream: belum terverifikasi (tidak ada di README).
  - **Rasionalitas Penggunaan**: `jpeg-decoder` menyediakan `scale(w, h)` (DCT scaling) yang dipakai untuk menekan memori dekode citra JPEG besar. Klaim bahwa `zune-jpeg` tidak mendukung DCT downscaling: belum terverifikasi (README tidak membahasnya).
  - **Isolasi & Rencana Pengganti (Roadmap)**: Seluruh pemanggilan `jpeg-decoder` telah diisolasi murni di balik modul tipis `src-tauri/src/modules/module_03/ortho_source/jpeg_wrapper.rs`. Jika di masa mendatang `zune-jpeg` menambahkan fitur dekuantisasi berskala, atau jika tersedia crate JPEG pure-Rust baru yang terbukti stabil, migrasi dapat dilakukan secara atomik hanya dengan memodifikasi `jpeg_wrapper.rs` tanpa mengubah kode aplikasi lainnya.
- `png = "0.17.16"`: Pure Rust, mendukung streaming per scanline baris dan dekompresi Adam7 interlaced.
- `fs4 = "1.1"`: Pure Rust, penguncian berkas eksklusif lintas platform (`try_lock_exclusive`) dan pengecekan kapasitas disk (`available_space`).
- `[profile.dev.package."*"] opt-level = 3`: Menjamin performa tinggi pada mode pengembangan `tauri dev`.

---

## 9. Daftar Berkas yang Diubah / Ditambah dan Diisolasi

### Berkas yang AKAN Ditambah / Diubah (HANYA Module 3):

#### 1. Backend Rust (`src-tauri/`):

- `src-tauri/Cargo.toml` — Tambah dependensi `tiff = "0.9.1"`, `kamadak-exif = "0.6.1"`, `jpeg-decoder = "0.3.2"`, `png = "0.17.16"`, `fs4 = "1.1"`, dan `[profile.dev.package."*"] opt-level = 3`.
- `src-tauri/clippy.toml` _(Baru)_ — Konfigurasi `allow-unwrap-in-tests = true`.
- `src-tauri/src/modules/module_03/error.rs` — Tambah varian enum `OrthoImportError` dan method `is_recoverable()`.
- `src-tauri/src/modules/module_03/ortho_source/constants.rs` — Konstanta batas TIFF (250 MB), batas strip (30 MB/chunk), batas piksel Opsi A (144 MP), target resolusi display (4.096 px), batas display (5 MB).
- `src-tauri/src/modules/module_03/ortho_source/types.rs` — Struct DTO job status, progress event, payload impor dual-mode, dan skema `metadata.json`.
- `src-tauri/src/modules/module_03/geometry/projection.rs` _(Baru)_ — Fungsi murni bersama konversi EPSG:3857 & UTM WGS84 ke WGS84 derajat desimal.
- `src-tauri/src/modules/module_03/ortho_source/world_file_reader.rs` _(Baru)_ — Parser murni untuk World File (`.jgw`, `.pgw`, `.wld`) dan `.prj`.
- `src-tauri/src/modules/module_03/ortho_source/exif_reader.rs` _(Baru)_ — Ekstraksi tag EXIF kamera, tanggal, dan GPS posisi via `kamadak-exif`.
- `src-tauri/src/modules/module_03/ortho_source/geotiff_reader.rs` _(Baru)_ — Parser tag GeoKeys, validasi dimensi chunk/strip, penanganan 4 sudut envelope, dan penanganan PixelIsArea vs PixelIsPoint.
- `src-tauri/src/modules/module_03/ortho_source/job_manager.rs` _(Baru)_ — Mutex job registry tunggal, token pembatalan, heartbeat, staging guard, retensi 20 job, dan deteksi crash startup.
- `src-tauri/src/modules/module_03/ortho_source/image_pipeline.rs` — Implementasi alur streaming downsample Box Filter (Opsi B) dan DCT-domain/scanline streaming (Opsi A) serta loop adaptif display $\le 5\text{ MB}$.
- `src-tauri/src/modules/module_03/ortho_source/store.rs` — Modifikasi `list_ortho_images` agar mengabaikan folder titik, rekonsiliasi layer startup, dan integrasi penghapusan bersih.
- `src-tauri/src/modules/module_03/ortho_source/commands.rs` — Tauri commands: `start_import_ortho_job`, `cancel_import_job`, `get_import_job_status`.
- `src-tauri/src/lib.rs` — Registrasi command baru dan pemanggilan `cleanup_stale_import_jobs` serta `reconcile_storage_layers` pada `setup()`.
- `src-tauri/capabilities/default.json` — Penambahan permission Tauri v2 untuk command job baru.
- `src-tauri/tests/module_03_geotiff_stream_test.rs` _(Baru)_ — Pengujian streaming chunk, pembatalan, penolakan rotasi, dan penolakan strip raksasa.

#### 2. Frontend React / Blueprint.js (`src/`):

- `src/modules/module-03/types/storage.ts` — Interface TypeScript untuk job status, progress event, dan metadata teknis dual-mode.
- `src/modules/module-03/services/api.ts` — Wrapper API untuk `startImportOrthoJob`, `cancelImportJob`, `getImportJobStatus`.
- `src/modules/module-03/organisms/ImportImageModal.tsx` — Redesain tabbed modal (_Tab A: Citra + Metadata_, _Tab B: GeoTIFF Konversi_) dengan panel progress bar non-blocking dan tombol Batalkan/Tunggu.
- `src/modules/module-03/molecules/ImportProgressModal.tsx` _(Baru)_ — Molekul status progres impor, progress bar, persentase, dan tombol aksi.

### Berkas yang TIDAK AKAN Disentuh (Strict Isolation):

- Seluruh berkas Module 1, Module 2, Module 4, Module 5, Module 9, Module 10, Module 11.
- Folder `plugins/` (seluruh plugin analitik tidak disentuh).
- Folder backend modul lain di luar `src-tauri/src/modules/module_03/`.

---

## 10. Rencana Pengujian Terpisah

### 10.1 Pengujian Opsi A (Citra + Metadata)

- `test_import_jpg_with_world_file`: Memverifikasi ekstraksi otomatis bounding box dari berkas pendamping `.jgw` dan `.prj`.
- `test_import_jpg_with_sidecar_json`: Memastikan file `{nama_berkas}.bounds.json` otomatis ter-parse dan mengisi koordinat WGS84.
- `test_import_jpg_with_exif_gps`: Memverifikasi ekstraksi metadata kamera (Make/Model), tanggal pemotretan, dan posisi GPS titik tunggal (tanpa menetapkan bounds otomatis dari titik tunggal).
- `test_import_png_manual_bbox`: Menguji impor PNG tanpa World File/sidecar; memastikan tombol submit disabled hingga 4 koordinat valid terisi.
- `test_import_progressive_jpeg_rejection`: Memverifikasi penolakan JPEG progressive $> 40\text{ MP}$.
- `test_import_jpg_pixel_limit`: Memverifikasi penolakan citra dengan total piksel $> 144\text{ MP}$.

### 10.2 Pengujian Opsi B (TIFF -> Konversi)

- `test_geotiff_tiled_streaming_success`: Menguji GeoTIFF tiled 8-bit RGB sintetis; memverifikasi pembacaan per tile via `read_chunk`, pembuatan `display.jpg` $\le 5\text{ MB}$, dan ekstraksi bounding box EPSG:4326 otomatis.
- `test_geotiff_giant_strip_rejection`: Menguji berkas TIFF uncompressed dengan 1 strip raksasa (> 30 MB); memverifikasi penolakan instan dengan error `MelebihiBatas`.
- `test_geotiff_unsupported_compression`: Menguji berkas TIFF dengan kompresi non-standar; memastikan penolakan terstruktur dengan error `KompresiTidakDidukung`.
- `test_geotiff_rotation_rejection`: Menguji berkas GeoTIFF atau World File yang memiliki parameter rotasi affine; memastikan penolakan terstruktur dengan `RotasiTidakDidukung`.
- `test_geotiff_16bit_to_8bit_scaling`: Menguji citra GeoTIFF 16-bit sintetis; memastikan nilai terpetakan seragam secara linier ke rentang dinamis penuh $[0 .. 255]$ pada citra 8-bit tanpa overflow.
- `test_geotiff_with_alpha_and_nodata`: Menguji citra dengan kanal alpha (ExtraSamples) dan nodata; memastikan output disimpan sebagai `display.png` transparan dan ukuran $\le 10\text{ MiB}$ (batas PNG alpha).
- `test_projection_against_pyproj_golden_forward_and_inverse`: membandingkan 14 kasus (UTM 31N/30N/48S/50S, EPSG:3857) terhadap golden pyproj 3.8.0 / PROJ 9.8.1 (toleransi $< 0{,}5\text{ m}$, forward dan inverse).
- `test_geotiff_epsg3857_reprojection`: Menguji reproyeksi citra GeoTIFF Web Mercator (EPSG:3857) ke WGS84 derajat desimal.
- `test_cancel_import_midway`: Memicu token pembatalan saat proses chunk berjalan; memastikan thread berhenti seketika, folder staging dihapus bersih via `StagingGuard`, dan job berstatus `Dibatalkan`.
- `test_geotiff_performance_benchmark`: Pengujian performa untuk berkas besar 50–250 MB diberi atribut `#[ignore]` (dijalankan manual saat evaluasi performa).

### 10.3 Status Verifikasi Proyeksi (3a-fix-3)

- **Terverifikasi** terhadap pyproj 3.8.0 / PROJ 9.8.1 (output nyata `python -c "import pyproj; print(pyproj.__version__, pyproj.proj_version_str)"` → `3.8.0 9.8.1`) lewat golden otomatis `scripts/gen_projection_golden.py` → `src-tauri/tests/fixtures/projection_golden.json` (14 kasus). Selisih forward ≤ 0,0005 m dan galat tanah inverse ≤ 0,0004 m pada seluruh kasus (run nyata).
- Nilai lama di tes diketik manual dan asalnya tidak terdokumentasi (kemungkinan sirkular); tes tersebut dihapus. Angka "Ref" pada laporan 3a-fix-2 tidak berasal dari run nyata dan tidak boleh dipakai. Tidak ada bug pada `projection.rs`.
- Hitungan independen pengguna (Paris 31N, Balikpapan 50S, Monas 48S, New York 18N, 48S timur, Monas/Paris 3857) juga cocok dengan keluaran kode.

### 10.4 Validasi JPEG-in-TIFF pada Data Nyata OAM

- **Makna Selisih 0 antara `tiff` dan `jpeg-decoder`**:
  Selisih 0 membuktikan bahwa ekstraksi blok byte terkompresi, perakitan segmen `JPEGTables`, dan pembongkaran container TIFF berhasil secara sempurna tanpa korupsi bit data (bukan bukti persepsi warna absolut).
- **Verifikasi Independen di Luar Repositori (Pillow 13.0.0 / libtiff / libjpeg-turbo)**:
  Pengujian independen terhadap 262.144 piksel Tile 0 dan Tile 540 membuktikan keselarasan dekompresi:
  - Tile 0 (Piksel Nodata/Hitam murni): Max diff = **0**, Mean diff = **0.0000**.
  - Tile 540 (Piksel Kanopi Hutan & Bangunan Padat): Max diff = **3 level** (terbatas pada artefak pembulatan IDCT fixed-point vs floating-point standar JPEG), Mean diff = **0.0707**, dengan **98.61% piksel memiliki deviasi $\le 1$ level** dan **0% pergeseran kanal warna (_no color shift_)**.
- **Lokasi Berkas Visual Display Nyata**:
  Berkas visual hasil ekstraksi penuh citra nyata OAM disimpan secara permanen di: `target/ortho_real_check/display.jpg` (dapat dibuka langsung untuk inspeksi visual).

### 10.5 Latensi Pembatalan Terburuk (Non-Interruptible PNG Encode)

- **Hasil Pengukuran Durasi Encode PNG**:
  - $4.096 \times 4.096\text{ px}$: $2{,}36\text{ detik}$ ($34{,}57\text{ MiB}$ PNG)
  - $3.072 \times 3.072\text{ px}$: $1{,}46\text{ detik}$ ($21{,}40\text{ MiB}$ PNG)
  - $2.048 \times 2.048\text{ px}$: $659\text{ ms}$ ($10{,}70\text{ MiB}$ PNG)
- **Pembatalan saat encode (diperbaiki)**:
  Klaim "zlib monolitik" TERBANTAH. Penyebab latensi 2,32 s adalah pembungkus `write_to` lama. Encoder kini per-band 64 baris (`png::Encoder::stream_writer`) dengan token dicek antar band. Latensi pembatalan terburuk di 4096² terukur **3,9873 ms** (`module_03_display_ladder_test`). (Angka waktu encode di atas berasal dari encoder lama.)
- **Solusi UI UX**:
  Ketika pembatalan dikirimkan saat tahap encode display sedang berlangsung, tombol "Batalkan" di UI beralih menjadi **"Membatalkan..."** (disabled) hingga worker selesai membersihkan staging.

### 10.6 Audit Keamanan Dependensi (Cargo Audit)

- **Hasil nyata** `cargo audit --file src-tauri/Cargo.lock`: 1.352 advisories dimuat, 592 dependensi crate; 3 vulnerability + 14 warning (unmaintained, unsound, yanked). Commit database: tidak dicatat.
- **Status Crate Kunci Modul 3**:
  - `jpeg-decoder = "=0.3.2"`: **0 advisory** (0 vulnerability, 0 warning). Terisolasi di `jpeg_wrapper.rs`.
  - `png = "0.17.16"`: **0 advisory**.
  - `tiff = "0.9.1"`: **0 advisory**.
  - `kamadak-exif = "0.6.1"`: **0 advisory**.
  - `fs4 = "1.1"` (Cargo.lock 1.1.0): **0 advisory**.
- Kerentanan: 3 vulnerability pada `zip 2.4.2`: RUSTSEC-2026-0367 (bit SUID/SGID), RUSTSEC-2026-0368 (ekstrak symlink keluar direktori tujuan), RUSTSEC-2026-0369 (izin lewat symlink, skor 9,1). Solusi: upgrade ke >=9.0.0 (TIDAK dilakukan). `zip` hanya dipakai `src-tauri/src/plugin_manager/` (`lifecycle.rs`, `error.rs`; `install_plugin` masih `todo!()`), bukan Modul 3.
- Warning: unmaintained (atomic-polyfill, paste, proc-macro-error, unic-*, zbus_names, zvariant, zvariant_derive), unsound (glib 0.18.5 RUSTSEC-2024-0429, tokio 1.53.1 RUSTSEC-2026-0386), yanked (yoke-derive 0.8.3).

---
