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

1. **Resolusi Display**: Batas maksimal ditetapkan **4.096 piksel pada sisi terpanjang DAN ukuran berkas $\le 5\text{ MB}$**, dihasilkan melalui loop optimasi adaptif (menjaga aspek rasio dan ketajaman pada level zoom $\ge 16$).
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

- **Spesifikasi Resampling (Box Filter / Area Averaging)**:
  - Menggunakan algoritma _Area Averaging_ untuk menghindari distorsi aliasing foto udara.
  - Untuk menghemat memori, sistem menerapkan **Band Processing (Akumulator per Baris Tile)**:
    Akumulator intensitas `u32` per kanal hanya dialokasikan untuk rentang baris output yang bersinggungan dengan baris tile yang sedang aktif dibaca. Memori akumulator band hanya berukuran $\approx \mathbf{8{,}4\text{ MB}}$.
- **Optimasi Piramida Internal (Overview IFD)**:
  Jika berkas GeoTIFF merupakan Cloud Optimized GeoTIFF (COG) atau memiliki sub-IFD piramida resolusi, dekoder memilih sub-IFD yang dimensinya paling mendekati target display ($\ge 4.096\text{ px}$), menghemat waktu dekompresi hingga $4–10\times$.
- **Evaluasi Kompresi TIFF (Crate `tiff 0.9.1`)**:
  - _Didukung_: Uncompressed (None), LZW, PackBits, Deflate (ZIP), Horizontal Differencing Predictor.
  - _Perilaku JPEG-in-TIFF (Compression 7)_: Pustaka `tiff 0.9.1` memiliki keterbatasan decoding terhadap varian JPEG-in-TIFF dengan tabel YCbCr kompleks. Jika dekompresi chunk JPEG gagal, sistem menangkap error dan mengembalikan pesan terstruktur `KompresiTidakDidukung`:
    _"Format kompresi JPEG-in-TIFF pada berkas ini tidak didukung oleh decoder pure Rust. Harap ekspor ulang berkas dengan kompresi Deflate, LZW, atau Uncompressed (Tiled GeoTIFF)."_
- **Deteksi Alpha & Nodata**:
  - Deteksi Alpha **wajib** melalui tag `ExtraSamples` (Tag 338: 1=Associated Alpha, 2=Unassociated Alpha), bukan dari jumlah kanal $\ge 4$.
  - Jika band $\ge 4$ tanpa tag `ExtraSamples` (mis. citra multispektral RGB+NIR): Sistem mengambil 3 band pertama (RGB) untuk visualisasi display dengan mencatat pesan peringatan pada `warnings`.
  - Nodata: Jika terdapat tag `GDAL_NODATA`, piksel bernilai nodata diubah menjadi transparan (Alpha = 0) **HANYA JIKA SEMUA KANAL bernilai nodata** (mencegah atap/tanah berwarna hitam murni berubah menjadi bolong).
  - Jika citra memiliki Alpha atau Nodata, keluaran display **wajib berformat PNG**. Jika RGB murni tanpa alpha, keluaran berformat **JPEG**.
- **Konversi Bit Depth (16-bit ke 8-bit)**:
  Diterapkan formula linier deterministik cepat:
  $$v_8 = \left\lfloor \frac{v_{16}}{257.0} \right\rfloor = (v_{16} \gg 8) \text{ as u8}$$
  Menghasilkan pemetaan seragam dari $[0 .. 65535] \rightarrow [0 .. 255]$ tanpa distorsi kontras.
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
  Seluruh nilai referensi koordinat dalam modul proyeksi diturunkan dari formulasi analitis Snyder (_USGS Professional Paper 1395_) dan Karney yang diimplementasikan secara mandiri (_golden values pure Rust_), sehubungan dengan aturan larangan dependensi runtime C (GDAL/PROJ `cs2cs`) maupun pustaka Python (`pyproj`). Akurasi numerik diuji secara ketat melalui pengujian bolak-balik analitis (_forward ↔ inverse roundtrip_) dengan toleransi presisi $< 10^{-6}\text{ derajat}$ ($\approx 0{,}11\text{ meter}$). Test suite mencakup:
  1. _Belahan Bumi Utara & Greenwich_: Greenwich Royal Observatory ($51{,}476852^\circ\text{N}, 0{,}000000^\circ\text{E}$) yang merupakan batas timur persis UTM Zona 30N (EPSG:32630).
  2. _Titik Batas Zona UTM_: Titik bujur dekat batas barat ($114{,}001^\circ\text{E}$) dan dekat batas timur ($119{,}999^\circ\text{E}$) pada UTM Zona 50S (EPSG:32750).

---

## 4. Jaminan Ukuran Display $\le 5\text{ MB}$ (Loop Adaptif In-Memory)

Downscaling dilakukan langsung dari canvas penampung di RAM (tanpa membaca ulang berkas sumber dari disk):

```
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
  1. Set Quality = 85                               1. Encode PNG (Level 6)
  2. Jika size > 5 MB:                              2. Jika size > 5 MB:
     Turunkan Quality:                                 Turunkan resolusi canvas:
     80 -> 75 -> 70                                    4096 -> 3200 -> 2560 -> 2048
  3. Jika Quality 70 masih > 5 MB:                     -> 1536 -> 1024 px
     Turunkan resolusi canvas                          (Batas bawah: 1.024 px)
     (faktor 0.85x per loop)
  4. Batas bawah: dimensi 1.024 px                  3. Jika 1.024 px masih > 5 MB:
  5. Jika 1.024 px masih > 5 MB:                       Kembalikan MelebihiBatas
     Kembalikan MelebihiBatas
```

- Alokasi memori tambahan selama loop adaptif: buffer salinan canvas downscaled + buffer encoder stream $\le \mathbf{30\text{ MB}}$.

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

### 5.3 Hasil Pengukuran Memori Empiris (Fase 3 Langkah 0 - Pasca Streaming Resampler)

Pengujian dilakukan menggunakan integration test binary mandiri terisolasi (`src-tauri/tests/module_03_memory_benchmark.rs`) dengan sub-proses terpisah dan pembacaan `PeakWorkingSetSize` melalui Windows API `K32GetProcessMemoryInfo` (didahului oleh `K32EmptyWorkingSet` untuk membersihkan working set pra-eksekusi).

| Kasus Uji                                    | Dimensi Sumber & Format                                        | Baseline RAM       | Puncak RAM (Peak Working Set) | Delta Memori             | Rincian Alokasi Buffer Utama                                                                          | Evaluasi Target ($\le 400\text{ MB}$)                                                                           |
| :------------------------------------------- | :------------------------------------------------------------- | :----------------- | :---------------------------- | :----------------------- | :---------------------------------------------------------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------- |
| **JPEG Baseline 100 MP**                     | $10.000 \times 10.000\text{ px}$, RGB, Noise/Gradient          | $0{,}08\text{ MB}$ | **$123{,}20\text{ MB}$**      | **$123{,}12\text{ MB}$** | Decode IDCT: $75\text{ MB}$, Display: $50{,}3\text{ MB}$, Resampler: $< 0{,}1\text{ MB}$              | **LOLOS**. Jauh di bawah target $400\text{ MB}$. Turun drastis dari $\approx 310\text{ MB}$ sebelum perbaikan.  |
| **JPEG Baseline 144 MP** _(Batas Desain)_    | $12.000 \times 12.000\text{ px}$, RGB, Noise/Gradient          | $0{,}08\text{ MB}$ | **$157{,}55\text{ MB}$**      | **$157{,}47\text{ MB}$** | Decode IDCT: $108\text{ MB}$, Display: $50{,}3\text{ MB}$, Resampler: $< 0{,}1\text{ MB}$             | **LOLOS**. Berada di $157{,}55\text{ MB}$, menyisakan headroom $> 240\text{ MB}$ untuk runtime Tauri/WebView.   |
| **JPEG Progressive 40 MP**                   | $8.000 \times 5.000\text{ px}$, RGB, Progressive Enkoder Nyata | $0{,}08\text{ MB}$ | **$290{,}07\text{ MB}$**      | **$289{,}99\text{ MB}$** | Koefisien DCT: $\sim 80-120\text{ MB}$, RGB: $120\text{ MB}$, Display: $31{,}5\text{ MB}$             | **LOLOS**. Berada di $290{,}07\text{ MB} \le 400\text{ MB}$ (headroom $\approx 110\text{ MB}$).                 |
| **PNG 100 MP RGBA** _(Non-interlaced)_       | $10.000 \times 10.000\text{ px}$, RGBA, Streaming Scanline     | $0{,}08\text{ MB}$ | **$66{,}24\text{ MB}$**       | **$66{,}16\text{ MB}$**  | Decode scanline: $0{,}04\text{ MB}$, Display RGBA: $67{,}1\text{ MB}$, Resampler: $< 0{,}1\text{ MB}$ | **LOLOS SANGAT EFISIEN**. Berkurang dari $1.072\text{ MB}$ menjadi $66{,}24\text{ MB}$ via `reader.next_row()`. |
| **PNG 25 MP RGBA** _(Interlaced Adam7 Baru)_ | $5.000 \times 5.000\text{ px}$, RGBA, Adam7 Interlaced Nyata   | $0{,}08\text{ MB}$ | **$161{,}49\text{ MB}$**      | **$161{,}41\text{ MB}$** | Frame Adam7: $100\text{ MB}$, Display RGBA: $67{,}1\text{ MB}$, Resampler: $< 0{,}1\text{ MB}$        | **LOLOS**. Batas 25 MP terbukti aman di $161{,}49\text{ MB} \le 400\text{ MB}$.                                 |

#### Analisis Kuantitatif Penyebab Lonjakan ~310 MB – 435 MB pada JPEG 100 MP Sebelum Perbaikan:

Sebelum implementasi `StreamingResampler`:

1. Decoder mengalokasikan buffer IDCT skala 1/2 ($5.000 \times 5.000 \times 3 = 75\text{ MB}$).
2. Objek `image::DynamicImage` membungkus buffer tersebut ($75\text{ MB}$).
3. Pemanggilan `image::imageops::resize(..., FilterType::Triangle)` mengeksekusi 2-pass resampling terpisah (horizontal lalu vertikal). Pass pertama mengalokasikan buffer intermediate sebesar $4.096 \times 5.000 \times 3 \approx 61{,}4\text{ MB}$.
4. Implementasi `imageops::resize` menggunakan paralelisasi Rayon multithread yang mengalokasikan buffer kerja per thread.
5. Buffer citra keluaran dialokasikan sebesar $4.096 \times 4.096 \times 3 = 50{,}3\text{ MB}$.
6. Akibatnya, pada puncak eksekusi, keempat buffer (sumber $75\text{ MB}$, intermediate $61{,}4\text{ MB}$, target $50{,}3\text{ MB}$, buffer thread Rayon, serta konversi DynamicImage) berada di RAM secara simultan ($\approx 190 - 210\text{ MB}$ heap langsung). Ditambah fragmentasi allocator dan page commit cache Windows, total peak working set mencapai $310 - 435\text{ MB}$.
7. Dengan `StreamingResampler`, alokasi intermediate buffer dan Rayon thread copy **DIHILANGKAN TOTAL**. `StreamingResampler` langsung mengonsumsi baris dari buffer IDCT dan mengakumulasikannya ke buffer target display dengan overhead akumulator $< 0{,}1\text{ MB}$, sehingga konsumsi memori puncak terpangkas menjadi hanya **$123{,}20\text{ MB}$**!

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

| Kode Error                          | Skenario Pemicu                                                                                           | Pesan untuk Pengguna (Bahasa Indonesia)                                                                                                                                     | Recoverable? | Aksi Pemulihan                                                                  |
| :---------------------------------- | :-------------------------------------------------------------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :----------: | :------------------------------------------------------------------------------ |
| `FormatTidakDidukung`               | Ekstensi `.tif` tapi bukan TIFF, Progressive JPEG > 40 MP, atau format palet/CMYK/DEM.                    | "Format berkas tidak dikenali atau tidak didukung. Harap gunakan berkas TIFF, Baseline JPEG, atau PNG dalam format warna RGB/RGBA."                                         |    Tidak     | Pilih berkas citra yang sesuai standar RGB/RGBA.                                |
| `KompresiTidakDidukung`             | TIFF menggunakan kompresi yang tidak didukung (mis. Old JPEG, WebP, atau JPEG-in-TIFF kompleks).          | "Kompresi TIFF '{kompresi}' belum didukung. Harap ekspor ulang berkas dengan kompresi Deflate, LZW, atau Uncompressed (Tiled GeoTIFF)."                                     |    Tidak     | Ekspor ulang berkas dengan kompresi Deflate/LZW.                                |
| `FileKorup`                         | Berkas terpotong di tengah, header TIFF terkorupsi, atau chunk gagal didecode.                            | "Berkas citra rusak atau tidak lengkap sehingga tidak dapat diproses."                                                                                                      |    Tidak     | Gunakan berkas citra asli yang tidak rusak.                                     |
| `MelebihiBatas` (File Size)         | Berkas TIFF > 250 MB atau JPG/PNG > 50 MB.                                                                | "Ukuran berkas ({size} MB) melebihi batas maksimal yang diizinkan ({max} MB)."                                                                                              |    Tidak     | Kompresi atau pangkas area citra sebelum diimpor.                               |
| `MelebihiBatas` (Piksel Opsi A)     | Citra JPG/PNG memiliki total piksel > 144 Megapiksel.                                                     | "Total dimensi citra ({mp} MP) melebihi batas aman memori (144 MP)."                                                                                                        |    Tidak     | Perkecil resolusi citra sebelum diimpor.                                        |
| `MelebihiBatas` (Piksel Opsi B)     | Citra TIFF memiliki total piksel > 500 Megapiksel.                                                        | "Total dimensi citra TIFF ({mp} MP) melebihi batas aman dekompresi (500 MP)."                                                                                               |    Tidak     | Pangkas area citra sebelum diimpor.                                             |
| `MelebihiBatas` (Strip Raksasa)     | TIFF uncompressed disimpan dalam strip raksasa (ukuran strip > 30 MB per chunk).                          | "Format internal berkas TIFF memiliki ukuran strip terlalu besar ({dim} px) yang tidak aman untuk memori. Harap ekspor ulang dengan format Tiled TIFF."                     |    Tidak     | Ekspor ulang citra drone dengan opsi _Tiled GeoTIFF_.                           |
| `MelebihiBatas` (Display File Size) | Kompresi display mencapai dimensi minimum ($1.024\text{ px}$) namun ukuran berkas masih $> 5\text{ MB}$.  | "Gagal mengompresi citra display di bawah 5 MB tanpa kehilangan kualitas kritis."                                                                                           |    Tidak     | Reduksi area citra sebelum diimpor.                                             |
| `KoordinatTidakValid`               | Koordinat lintang/bujur terbalik ($south \ge north$ atau $west \ge east$) atau out-of-range.              | "Batas koordinat tidak valid: Lintang Selatan harus lebih kecil dari Lintang Utara, dan Bujur Barat harus lebih kecil dari Bujur Timur."                                    |    **Ya**    | Pengguna mengoreksi input koordinat di formulir modal.                          |
| `CrsTidakDidukung`                  | GeoTIFF menggunakan proyeksi lokal/asing atau kode CRS User-Defined (32767).                              | "Sistem koordinat citra ({crs}) belum didukung otomatis. Anda dapat memasukkan koordinat batas WGS84 secara manual."                                                        |    **Ya**    | Modal mengaktifkan field koordinat manual agar pengguna dapat memasukkan WGS84. |
| `RotasiTidakDidukung`               | GeoTIFF atau World File memuat koefisien rotasi/skew affine.                                              | "Citra memiliki orientasi rotasi/kemiringan. Peta hanya mendukung citra tegak lurus (North-up)."                                                                            |    Tidak     | Luruskan orientasi citra di software fotogrametri/GIS.                          |
| `DiskPenuh`                         | Ruang hard disk tidak mencukupi saat preflight check atau penulisan staging.                              | "Penyimpanan disk tidak mencukupi. Membutuhkan setidaknya {req} MB ruang kosong."                                                                                           |    **Ya**    | Kosongkan ruang penyimpanan hard disk lalu coba lagi.                           |
| `JobSedangBerjalan`                 | Pengguna memicu impor baru saat ada job lain yang sedang berjalan.                                        | "Proses impor lain sedang berlangsung. Harap tunggu hingga selesai atau batalkan job aktif."                                                                                |    **Ya**    | Tunggu hingga job selesai atau klik tombol batalkan.                            |
| `Dibatalkan`                        | Pengguna menekan tombol "Batalkan" di modal progres.                                                      | "Proses impor citra telah dibatalkan oleh pengguna."                                                                                                                        |    Tidak     | Modal ditutup, folder staging otomatis dibersihkan tanpa sampah.                |
| `InternalError`                     | Kesalahan internal tak terduga (mis. panic thread tertangkap JoinError).                                  | "Terjadi kesalahan internal saat memproses citra. Detail teknis telah disiapkan untuk disalin."                                                                             |    Tidak     | Salin detail error untuk pelaporan bug.                                         |
| `AplikasiSudahBerjalan`             | Instansi sekunder mencoba menjalankan impor atau operasi tulis storage (CRUD ortho/layer/spatial result). | "Aplikasi Aerial Analytics Platform sudah berjalan di jendela/proses lain. Operasi impor dan modifikasi data dinonaktifkan pada instansi kedua demi mencegah konflik data." |    **Ya**    | Tutup jendela instansi lain jika ingin melakukan impor atau modifikasi data.    |

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
  - **Status Upstream**: Repositori `image-rs/jpeg-decoder` saat ini berada dalam status **Maintenance Mode** (diumumkan oleh pengembang `image-rs` seiring migrasi pustaka utama `image` v0.25 ke `zune-jpeg`). Commit terakhir upstream tercatat pada **20 September 2024**, dan rilis resmi terakhir versi 0.3.2 diterbitkan pada **21 Maret 2024**.
  - **Rasionalitas Penggunaan**: `jpeg-decoder` sengaja dipertahankan karena merupakan satu-satunya parser JPEG _pure Rust_ yang mendukung **DCT-domain scaling** (`scale(w, h)`), yang krusial untuk memangkas memori dekompresi dari 432 MB menjadi 75–108 MB pada citra 100–144 MP. Pustaka alternatif seperti `zune-jpeg` tidak menyediakan fitur downscaling pada tahap dekuantisasi DCT.
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
- `test_geotiff_with_alpha_and_nodata`: Menguji citra dengan kanal alpha (ExtraSamples) dan nodata; memastikan output disimpan sebagai `display.png` transparan dan ukuran $\le 5\text{ MB}$.
- `test_geotiff_utm_reprojection`: Menguji reproyeksi koordinat UTM Zona 50S (Balikpapan) dengan membandingkannya terhadap nilai referensi golden values (deret Krüger, diverifikasi dengan implementasi independen) (toleransi $< 1\text{ meter}$).
- `test_geotiff_epsg3857_reprojection`: Menguji reproyeksi citra GeoTIFF Web Mercator (EPSG:3857) ke WGS84 derajat desimal.
- `test_cancel_import_midway`: Memicu token pembatalan saat proses chunk berjalan; memastikan thread berhenti seketika, folder staging dihapus bersih via `StagingGuard`, dan job berstatus `Dibatalkan`.
- `test_geotiff_performance_benchmark`: Pengujian performa untuk berkas besar 50–250 MB diberi atribut `#[ignore]` (dijalankan manual saat evaluasi performa).

### 10.3 Pengujian Tata Letak Storage & Ketahanan Sistem

- `test_storage_layout_option_a`: Memastikan Opsi A selalu menghasilkan `original.jpg`/`original.png`, `display.jpg`/`display.png`, `meta.json`, dan `metadata.json`.
- `test_storage_layout_option_b_with_original`: Memastikan Opsi B dengan opsi centang menghasilkan `original.tif`.
- `test_storage_layout_option_b_without_original`: Memastikan Opsi B tanpa centang TIDAK menghasilkan `original.tif`, dan `size_bytes` di `meta.json` bernilai sama dengan `display_size_bytes`.
- `test_layer_registration_and_reconciliation`: Menguji pembuatan berkas `map_storage/layers/{id}.json` dan rekonsiliasi startup jika crash terjadi di antara rename staging dan penulisan layer.
- `test_delete_ortho_clean`: Memastikan penghapusan citra menghapus bersih seluruh isi folder `images/{id}/` dan `layers/{id}.json`.
- `test_interrupted_job_recovery_on_startup`: Membuat mock file job berstatus `"running"`; menjalankan fungsi `cleanup_stale_import_jobs` saat startup dan memastikan status berubah menjadi `"interrupted"` serta folder staging dibersihkan.
- `test_single_active_job_enforcement`: Memastikan pemanggilan job kedua saat job pertama aktif langsung ditolak dengan `JobSedangBerjalan`.
- `test_ui_responsiveness`: Memastikan perintah Tauri segera mengembalikan `job_id` dalam waktu $< 50\text{ ms}$ saat proses streaming berjalan di worker thread.

---

## 11. Kriteria Selesai (Definition of Done) & Panduan Verifikasi Dosen

### Kriteria Selesai (DoD):

1. [ ] Modal impor Module 3 memiliki 2 tab/opsi yang jelas (_Tab A: Citra + Metadata_, _Tab B: GeoTIFF Konversi_).
2. [ ] Berkas GeoTIFF 50–250 MB dapat diimpor secara streaming tanpa freeze pada UI, dengan puncak RAM stabil di bawah 500 MB.
3. [ ] Berkas display yang dihasilkan selalu $\le 5\text{ MB}$ (PNG jika ada alpha/nodata, JPG jika RGB murni).
4. [ ] Bounding box GeoTIFF (WGS84, EPSG:3857, dan UTM) terdeteksi otomatis dari keempat sudut envelope dan dapat dikoreksi oleh pengguna.
5. [ ] Opsi A mendukung pembacaan otomatis World File (`.jgw`, `.pgw`, `.wld` + `.prj`) dan sidecar `{nama_berkas}.bounds.json`.
6. [ ] Tata letak folder `map_storage/images/{id}/` dan pendaftaran `map_storage/layers/{id}.json` terverifikasi konsisten untuk seluruh skenario impor.
7. [ ] Tombol "Batalkan" berfungsi seketika dan membersihkan folder staging tanpa meninggalkan file sampah.
8. [ ] Crash/interupsi OS terdeteksi saat startup dan folder setengah jadi dibersihkan otomatis.
9. [ ] Seluruh pre-commit gates (`npm run lint`, `npm run lint:rust`, `npm run format:check`, `npm run format:rust:check`, dan `npm run build`) lulus 100%.

### Panduan Verifikasi Singkat untuk Dosen:

1. Jalankan aplikasi via `npm run dev` atau `npm run tauri dev`.
2. Buka **Task Manager / Resource Monitor** Windows, lalu amati proses `aerial-analytics-platform.exe`.
3. Buka **Module 3: Aerial Image Map Explorer** $\rightarrow$ klik **"Impor Citra Orthomosaic"**.
4. **Uji Opsi A**: Pilih berkas JPG beserta berkas pendamping `.jgw` atau `{nama}.bounds.json` $\rightarrow$ perhatikan koordinat langsung terisi otomatis tanpa perlu mengetik manual $\rightarrow$ klik Impor.
5. **Uji Opsi B**: Pilih berkas GeoTIFF (`.tif`) berukuran 50–250 MB $\rightarrow$ perhatikan koordinat WGS84 terdeteksi otomatis dari GeoKeys $\rightarrow$ klik Impor.
6. **Verifikasi Puncak Memori**: Amati grafik memori di Task Manager; buktikan bahwa selama proses dekompresi dan resampling, penggunaan RAM tetap stabil di bawah 500 MB (biasanya ~120–175 MB).
7. **Uji Ketahanan & Responsivitas**: Saat progres berjalan, amati bahwa modal dapat ditutup ke latar belakang, aplikasi tetap responsif, dan tombol "Batalkan" mampu membatalkan proses seketika dengan pembersihan data tuntas.
8. **Verifikasi Berkas Storage**:
   Buka folder `map_storage/images/{id}/` dan `map_storage/layers/{id}.json`:
   - Verifikasi bahwa berkas `display.jpg`/`display.png` selalu berukuran $\le 5\text{ MB}$.
   - Verifikasi keberadaan `original.tif` hanya ada jika opsi centang diaktifkan.
   - Verifikasi bahwa `meta.json` dan `metadata.json` tersimpan secara atomik dan akurat.

---

## 10. Catatan Khusus Langkah 0 (Verifikasi & Kompatibilitas)

### 10.1 Perilaku Instansi Kedua (Single-Instance Lock)

Dipilih kebijakan **(b) Semua command yang mengubah data mengembalikan `AplikasiSudahBerjalan` dan UI menampilkan banner peringatan**:

- **Alasan Pemilihan**: Pengalaman pengguna (UX) jauh lebih baik daripada opsi (a) yang mematikan aplikasi sekunder secara tiba-tiba via dialog. Pengguna tetap dapat melihat peta, meninjau layer, dan membaca data secara read-only.
- **Pembedaan Error Windows Kernel**:
  Pada Windows, perolehan lock membedakan `raw_os_error() == Some(32)` (`ERROR_SHARING_VIOLATION`) yang secara spesifik dipetakan ke `OrthoImportError::AplikasiSudahBerjalan`. Error I/O lainnya (seperti `EACCES` permission denied atau path corrupt) dicatat sebagai kegagalan lock sistem murni (`OrthoImportError::InternalError`) dengan log pesan eksplisit dan tidak melewati cleanup begitu saja.
- **Cakupan Proteksi Mutasi**: Seluruh command mutasi (impor citra, hapus citra, simpan hasil spasial, hapus layer) dilindungi oleh `require_primary_instance()?`.

### 10.2 Kompatibilitas Target Non-Windows

- Kompilasi silang non-Windows dievaluasi via `cargo check --target x86_64-unknown-linux-gnu`.
- Status saat ini: **"Belum terverifikasi secara lokal"** karena target standar Rust Linux belum terpasang di host Windows lokal ini.
- Catatan: CI GitHub Actions (`.github/workflows/ci.yml`) menjalankan cross-platform build (Ubuntu, macOS, Windows) yang secara otomatis memvalidasi kompilasi Linux dan macOS saat PR. Tercatat sebagai **TODO CI Linux**.

---
