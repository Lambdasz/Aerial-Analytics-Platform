# Keputusan Arsitektur, Batasan Sumber Daya & Audit: Modul 03

Dokumen ini mencatat keputusan arsitektur mendasar, batasan sumber daya, mekanisme resolusi tampilan gambar, matriks audit 25 temuan, dan isu teknis terbuka untuk **Modul 03 (Aerial Image Map Explorer)**.

---

## 1. Architectural Decisions: Pure Rust GeoTIFF Engine

Pilihan desain kunci dalam Modul 03 adalah mengganti pustaka geospasial native C/C++ (seperti GDAL atau libgeotiff) dengan pipeline **100% Rust murni** berbasis crate `tiff` dan `image`:

| Criterion                           | Pure Rust Engine (`tiff` + `image`)                          | Native C/C++ Binding (`GDAL`)                                              | Rationale & Architectural Choice                                          |
| :---------------------------------- | :----------------------------------------------------------- | :------------------------------------------------------------------------- | :------------------------------------------------------------------------ |
| **Portabilitas & Kompilasi Silang** | Mengompilasi mulus di Windows, Linux, dan macOS              | Membutuhkan toolchain C++ rumit, pustaka bersama, dan header dev GDAL      | Memungkinkan build CI yang seragam tanpa dependensi platform native       |
| **Keamanan Memori & Isolasi Error** | Menegakkan jaminan memori Rust; bebas risiko buffer overflow | Rentan terhadap korupsi memori atau segmentation fault di pustaka C        | Mencegah eksploitasi memori saat mem-parsing berkas GeoTIFF tak tepercaya |
| **Ukuran Biner**                    | Menambahkan jejak statis minimal pada aplikasi desktop akhir | Membengkakkan ukuran biner secara signifikan dan perlu menyertakan DLL/.so | Menjaga ukuran installer distribusi tetap ringkas dan efisien             |
| **Cakupan Fitur**                   | Tag TIFF dasar, piramida tile, dan proyeksi UTM/WGS84        | Dukungan penuh untuk ratusan proyeksi dan format eksotis                   | Modul 03 berfokus ketat pada ortomosaik RGB drone (WGS84 & UTM)           |

---

## 2. Resource Limits & Technical Constraints

Untuk menjaga performa UI yang responsif dan melindungi sumber daya sistem pada workstation standar, Modul 03 menerapkan batasan ketat berikut:

| Resource Constraint              | Configured Threshold     | Enforcement Mechanism                                    | Failure Response / Mitigation                                           |
| :------------------------------- | :----------------------- | :------------------------------------------------------- | :---------------------------------------------------------------------- |
| **Batas Atas Ukuran JSON**       | 64 KB (65.536 byte)      | Divalidasi di `aoi/store.rs` dan `annotations/store.rs`  | Mengembalikan error `InvalidGeometry` jika ukuran melebihi batas        |
| **Batas Dimensi Impor Cepat**    | 4.096 x 4.096 piksel     | Divalidasi di `geotiff_reader.rs`                        | Mengarahkan pengguna memilih Mode B (Auto Tiling) untuk piramida dalam  |
| **Ambang Batas Ingesti Sinkron** | Ukuran berkas 5 MB       | Diperiksa di `ortho_source/commands.rs`                  | Berkas yang lebih besar dialihkan melalui tugas asinkron Tokio di latar |
| **Tingkat Zoom Tile**            | Level 10 hingga Level 18 | Ditegakkan di generator piramida Web Mercator `tiler.rs` | Tile di luar rentang ini dipotong untuk mencegah pemborosan disk        |
| **Kunci Single-Instance**        | 1 Proses Utama Aktif     | Ditegakkan via `map_storage/.app.lock`                   | Proses sekunder berjalan dalam mode baca-saja (`ACCESS_DENIED`)         |

---

## 3. Image Display URL & Storage Root Determination

Resolusi aset citra (thumbnail ortomosaik, potongan foto deteksi) berbeda secara mendasar antara lingkungan pengembangan dan rilis paket aplikasi desktop:

| Execution Environment                    | Resolution Mechanism                                                  | Code Implementation Reference                                                       | Verification Status                                     |
| :--------------------------------------- | :-------------------------------------------------------------------- | :---------------------------------------------------------------------------------- | :------------------------------------------------------ |
| **Server Pengembangan Vite**             | Middleware filesystem lokal mencegat permintaan `/map_storage`        | Dikonfigurasi di `vite.config.ts` (baris 24); dilayani dari akar workspace          | **Terverifikasi** di browser lokal dan Tauri dev        |
| **Aplikasi Desktop Terpaket (Produksi)** | IPC mengambil akar storage kanonikal, lalu meresolusi protokol kustom | `get_storage_root_path()` + `convertFileSrc("asset://...")` di `fieldResolution.ts` | **Belum Terverifikasi** (Biner paket rilis belum diuji) |

---

## 4. Audit Findings Matrix (25 Items from audit.md)

Tabel berikut mencatat status dan bukti verifikasi untuk seluruh 25 temuan arsitektur yang diidentifikasi dalam `audit.md`:

| Audit ID  | Location & Scope                   | Description of Finding                                        | Resolution Status | Verification Evidence / Reference                                     |
| :-------- | :--------------------------------- | :------------------------------------------------------------ | :---------------- | :-------------------------------------------------------------------- |
| **DC-01** | `hooks/useMeasurement.ts`          | Hook pengukuran mati tidak diimpor di mana pun                | **Selesai**       | Berkas telah dihapus dari repositori                                  |
| **DC-02** | `hooks/useAnnotations.ts`          | Hook belum terintegrasi ke LayerManager                       | **Terbuka**       | Berkas dipertahankan; menunggu integrasi ke LayerManager              |
| **DC-03** | `organisms/ManagedMapLayers.tsx`   | Kondisi mock `flight_session_01` tidak pernah cocok           | **Terbuka**       | Percabangan mock masih ada di kode; menunggu pembersihan              |
| **DC-04** | `organisms/MapSyncHandler.tsx`     | Komponen tak terlihat mati mencari marker kosong              | **Selesai**       | Berkas telah dihapus dari repositori                                  |
| **DC-05** | `molecules/MapSearchBar.tsx`       | State query pencarian kosmetik; tidak difilter                | **Terbuka**       | Query pencarian lokal belum terhubung ke induk                        |
| **DC-06** | `services/api.ts`                  | Wrapper mati `reorderLayerStack`, `filterLayersByCategory`    | **Selesai**       | Fungsi mati telah dihapus dari `api.ts`                               |
| **DC-07** | `src-tauri/.../type.rs`            | Berkas stub kosong 230 byte dengan allow dead_code            | **Selesai**       | Berkas telah dihapus dari `module_03/`                                |
| **DC-08** | `src-tauri/.../aoi/pure.rs`        | Duplikat klon 100% dari `aoi/builder.rs`                      | **Selesai**       | Duplikat `pure.rs` dihapus; `builder.rs` dipertahankan                |
| **DC-09** | `src-tauri/.../layers/commands.rs` | Perintah IPC `reorder_layer_stack_cmd` mati                   | **Selesai**       | Perintah dihapus dari `lib.rs` dan `commands.rs`                      |
| **DC-10** | `src-tauri/.../logperson1.md`      | Log personal & aturan usang di sumber Rust                    | **Selesai**       | Berkas telah dihapus dari `src-tauri/`                                |
| **DC-11** | `config/mapConfig.ts`              | Basemap `positron` mati tanpa API key                         | **Selesai**       | Entri positron dihapus dari `mapConfig.ts`                            |
| **DC-12** | `src/modules/module-03/index.ts`   | Re-export tak terpakai (`AoiDemoToolbar`, alias)              | **Selesai**       | Alias dibersihkan pada commit `f24e05c` & `bf5220d`                   |
| **RV-01** | `geometry/validation.rs`           | Duplikasi validasi poligon antar fungsi                       | **Terbuka**       | Kedua fungsi validasi saat ini masih dipertahankan                    |
| **RV-02** | `aoi/commands.rs`                  | Round-trip IPC ganda (`create_aoi_cmd` lalu `save_aoi`)       | **Terbuka**       | Kedua perintah masih terdaftar di IPC                                 |
| **RV-03** | `leaflet/spatialLayerBuilder.ts`   | Panggilan `isValidWgs84Bounds` redundan saat render           | **Terbuka**       | Pengecekan masih dipertahankan di layer builder                       |
| **RV-04** | `molecules/OrthoCardItem.tsx`      | Gambar fallback error mengarah ke foto drone dummy            | **Selesai**       | Diganti dengan komponen No Image `ResultPhoto` bersih                 |
| **DS-01** | `map_storage/photos/`              | Panggilan paralel 4x `useImageMarkers(500)` ke storage kosong | **Selesai**       | Polling dihapus dari organisme utama                                  |
| **DS-02** | `state.rs`                         | Cache in-memory `Arc<RwLock>` duplikat vs disk langsung       | **Terbuka**       | Cache in-memory `Module03State` masih dipertahankan                   |
| **DS-03** | `map_storage/layers/`              | Direktori kosong vs pembuatan layer dinamis                   | **Terbuka**       | Tata letak disk masih mempertahankan folder `layers/`                 |
| **DS-04** | `store/ImageSyncContext.tsx`       | Konteks lintas komponen vs state URL lokal                    | **Terbuka**       | Penyedia konteks masih dipertahankan di dalam tree                    |
| **FP-01** | `organisms/AoiToolbar.tsx`         | Komponen monolitik 797 baris memadukan form & hitungan        | **Selesai**       | Didekomposisi jadi 330 baris; ekstraksi form & modal rincian          |
| **FP-02** | `organisms/LayerManager.tsx`       | Komponen monolitik 790 baris dengan formulir inline           | **Terbuka**       | Sebagian dibersihkan (634 baris); menunggu ekstraksi modal            |
| **FP-03** | `sp_result/store.rs`               | Berkas monolitik 1.542 baris menggabungkan semua tugas        | **Selesai**       | Didekomposisi jadi `reader.rs`, `writer.rs`, `patch.rs`, `cleanup.rs` |
| **FP-04** | `hooks/useSpatialResultLayers.ts`  | Mutasi Leaflet imperatif di dalam hook                        | **Terbuka**       | Pola adapter Leaflet imperatif masih dipertahankan                    |
| **FP-05** | `LayerManager.tsx` & `commands.rs` | Round-trip IPC untuk boolean `is_visible` & opasitas          | **Selesai**       | Dikonversi ke pembaruan klien React fungsional immutable              |

---

## 5. Open Technical Issues ("Isu Terbuka")

1. **Risiko Path Traversal pada `delete_saved_image` (Perintah Legacy)**:
   - _Status_: Terbuka.
   - _Analisis_: Di `src-tauri/src/modules/module_03/ortho_source/store.rs` (baris 597), `legacy_dir = get_images_dir().join(category).join(&safe_id)`. Meskipun `safe_id` telah disanitasi melalui `sanitize_filename`, parameter string `category` belum diperiksa terhadap sekuens traversal (`..`). Penyerang berpotensi menyuplai string kategori berbahaya.
2. **Ketiadaan Pengawal Instans pada `cancel_import_job`**:
   - _Status_: Terbuka.
   - _Analisis_: Di `src-tauri/src/modules/module_03/ortho_source/commands.rs` (baris 178), `cancel_import_job` tidak memiliki `require_primary_instance()?`. Instans sekunder baca-saja dapat memutasi `.jobs/{job_id}.json` secara langsung di disk, sementara `CancellationToken` in-memory instans utama tetap tidak dibatalkan.
3. **Inspeksi Path Berkas Tanpa Batasan pada `inspect_geotiff_header`**:
   - _Status_: Terbuka.
   - _Analisis_: Di `src-tauri/src/modules/module_03/ortho_source/commands.rs` (baris 236), `inspect_geotiff_header` menerima string path berkas lokal apa pun. Meskipun memeriksa `path.is_file()`, fungsi ini tidak menegakkan batasan direktori atau ekstensi berkas, sehingga berupaya mendekode berkas sistem apa pun sebagai TIFF.
4. **Verifikasi Protokol Aset Produksi**:
   - _Status_: Belum Terverifikasi.
   - _Analisis_: Pemuatan tile ortomosaik dan thumbnail melalui protokol kustom `convertFileSrc` belum di-benchmark atau diverifikasi di dalam biner paket rilis produksi (`--no-bundle` di CI).
5. **Isu Repositori Terbuka Lainnya**:
   - _Advisory Cargo Audit_: Peringatan kerentanan pada dependensi crate `zip`.
   - _Golden Test Proyeksi_: Konversi proyeksi UTM ke WGS84 belum memiliki pengujian verifikasi dataset golden otomatis.
   - _Lingkungan CI_: Kompatibilitas Ubuntu 24.04 dalam alur kerja GitHub Actions.
   - _Benchmark Penggunaan Memori_: Tabel konsumsi memori puncak selama pemotongan tile mendalam saat ini ditandai **Belum Terverifikasi** karena ketiadaan berkas log mentah hasil pengujian runtime.
