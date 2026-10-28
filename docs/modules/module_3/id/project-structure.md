# Struktur Proyek: Modul 03 (Aerial Image Map Explorer)

Dokumen ini menyediakan inventarisasi struktural komprehensif seluruh berkas, folder, dan jalur penyimpanan yang terkait dengan **Modul 03 (Aerial Image Map Explorer)**.

---

## 1. Backend Rust Source Tree (`src-tauri/src/modules/module_03/`)

Kode sumber backend untuk Modul 03 diorganisir secara modular di bawah `src-tauri/src/modules/module_03/`. Logika bisnis dipartisi ke dalam submodul domain terdedikasi:

```text
src-tauri/src/modules/module_03/
├── mod.rs                        # Modul fasad utama, deklarasi submodul, dan pub use re-export
├── storage.rs                    # Primitif penyimpanan filesystem langsung dan inisialisasi folder
├── storage_commands.rs           # Fasad perintah Tauri IPC yang diekspor ulang untuk operasi storage
├── storage_utils.rs              # Utilitas resolusi path (get_storage_root, sanitize_filename)
├── error.rs                      # Definisi enum MapControllerError dan OrthoImportError
├── state.rs                      # Cache in-memory write-through Module03State dan sinkronisasi
├── types.rs                      # Struct Rust bersama yang digunakan lintas domain Modul 03
├── aoi/                          # Domain Area of Interest (AOI)
│   ├── mod.rs                    # Definisi submodul dan ekspor publik
│   ├── commands.rs               # Handler perintah Tauri IPC untuk operasi AOI
│   ├── store.rs                  # Persistensi filesystem, CRUD, dan kalkulasi ulang metrik
│   ├── types.rs                  # Struct data AOI (AoiPolygon, AoiFeature, AoiMetrics)
│   └── builder.rs                # Konstruksi poligon geometris murni dan perhitungan metrik
├── annotations/                  # Domain Anotasi Spasial
│   ├── mod.rs                    # Definisi submodul dan ekspor publik
│   ├── commands.rs               # Handler perintah Tauri IPC untuk anotasi
│   ├── store.rs                  # Persistensi filesystem untuk anotasi di map_storage/annotations/
│   └── types.rs                  # AnnotationFeature, AnnotationProperties, dan bentuk GeoJSON
├── geometry/                     # Perhitungan & validasi Geometri Geospasial
│   ├── mod.rs                    # Definisi submodul, ekspor, dan unit test
│   ├── calculations.rs           # Luas Shoelace, keliling geodesik, dan matematika titik tengah
│   └── validation.rs             # Validator rentang koordinat WGS84 dan topologi poligon
├── layers/                       # Metadata Map Layer dan state visualisasi
│   ├── mod.rs                    # Definisi submodul dan ekspor publik
│   ├── commands.rs               # Handler perintah Tauri IPC untuk state layer
│   ├── store.rs                  # Persistensi filesystem untuk layer di map_storage/layers/
│   ├── types.rs                  # LayerState, LayerType, dan properti tampilan
│   └── pure.rs                   # Fungsi transformasi murni pada koleksi layer
├── ortho_source/                 # Mesin GeoTIFF ortomosaik & pembuatan tile raster
│   ├── mod.rs                    # Definisi submodul dan ekspor publik
│   ├── commands.rs               # Perintah Tauri IPC untuk impor orto, daftar, dan inspeksi
│   ├── store.rs                  # Manajemen direktori citra, meta.json, dan cascade delete
│   ├── types.rs                  # StoredImageMetadata, ImportDualModePayload, JobRecord
│   ├── instance_lock.rs          # Kunci konkurensi proses (.app.lock) dan pengawal single-instance
│   ├── job_manager.rs            # Pelacakan job latar belakang aktif dan registri CancellationToken
│   ├── geotiff_reader.rs         # Parsing header GeoTIFF murni Rust, tag, dan deteksi CRS
│   ├── projection.rs             # Matematika konversi koordinat UTM ke WGS84
│   ├── tiler.rs                  # Generator tile piramida mendalam (tingkat zoom 10-18)
│   └── image_pipeline.rs         # Supervisor impor latar belakang asinkron untuk ingesti dual-mode
├── photos/                       # Domain penanda foto tangkapan drone
│   ├── mod.rs                    # Definisi submodul dan ekspor publik
│   ├── commands.rs               # Perintah Tauri IPC untuk daftar penanda foto
│   ├── store.rs                  # Pembaca filesystem untuk foto drone
│   └── types.rs                  # PhotoMetadata, DroneImageMetadata, koordinat GPS
└── sp_result/                    # Domain inspeksi Hasil Spasial
    ├── mod.rs                    # Definisi submodul dan ekspor publik
    ├── commands.rs               # Perintah Tauri IPC untuk CRUD hasil analisis spasial
    ├── types.rs                  # SpatialResultItem, FeatureEditPayload, DisplayPreference
    ├── reader.rs                 # Pemindaian disk, parsing JSON, dan ingesti pohon direktori
    ├── writer.rs                 # Serialisasi atomik dan penulisan disk untuk hasil analisis
    ├── patch.rs                  # Penambalan properti in-place, preferensi tampilan, dan sinkronisasi foto
    └── cleanup.rs                # Manajemen karantina (.trash_*) dan penghapusan bertingkat
```

### Purpose of Backend Files

| Directory / File      | Domain / Scope      | Responsibilities & Architectural Role                                              |
| :-------------------- | :------------------ | :--------------------------------------------------------------------------------- |
| `mod.rs`              | Fasad Utama         | Mendeklarasikan 10 submodul, menerapkan lint crate, dan mengekspos API publik      |
| `storage.rs`          | Primitif Storage    | Memastikan pembuatan direktori akar storage (`aois/`, `images/`, dll.)             |
| `storage_commands.rs` | Fasad IPC           | Mengekspos `get_storage_root_path` dan mengekspor ulang command domain storage     |
| `storage_utils.rs`    | Resolusi Path       | Menghitung akar penyimpanan kanonikal dan menyaring path yang tidak aman           |
| `error.rs`            | Pemodelan Error     | Memetakan error domain ke `CommandError` lintas batas Tauri IPC                    |
| `state.rs`            | Cache Memori        | Cache thread-safe `Arc<RwLock<HashMap>>` dengan sinkronisasi write-through ke disk |
| `types.rs`            | Model Bersama       | Struct dasar bersama (misalnya `ImportBoundsPayload`)                              |
| `aoi/*`               | Submodul AOI        | Membuat, menghitung ulang metrik, memvalidasi, dan menyimpan poligon AOI pengguna  |
| `annotations/*`       | Anotasi             | Mengelola anotasi vektor (titik, garis, poligon) dengan batasan ukuran 64 KB       |
| `geometry/*`          | Matematika Geometri | Perhitungan murni untuk luas geodesik, keliling, dan kotak pembatas (bounds)       |
| `layers/*`            | State Layer         | Mengontrol visibilitas layer, opasitas, dan persistensi urutan tumpukan layer      |
| `ortho_source/*`      | Ortomosaik          | Validasi GeoTIFF, pemotongan tile dual-mode, pelacakan job, dan lock proses        |
| `photos/*`            | Foto Drone          | Menginspeksi dan mendaftarkan penanda titik tangkapan foto drone ber-geotag        |
| `sp_result/*`         | Hasil Spasial       | Memuat hasil deteksi/klasifikasi, menambal properti, dan menyinkronkan foto        |

---

## 2. Frontend React Source Tree (`src/modules/module-03/`)

Aplikasi frontend untuk Modul 03 berada di dalam `src/modules/module-03/`, terstruktur ketat mengikuti prinsip **Desain Atomik**:

```text
src/modules/module-03/
├── index.ts                      # Titik masuk modul yang mengekspor ulang komponen utama dan hook
├── REQUIREMENTS.md               # Spesifikasi kebutuhan fitur
├── RULES_MAP_FR.md               # Panduan pengkodean frontend (Leaflet, state, desain atomik)
├── atoms/                        # Primitif UI atomik terkecil
│   └── StatusBadge.tsx           # Lencana status yang menampilkan status online, aktif, atau sinkron
├── molecules/                    # Komposisi komponen UI sederhana
│   ├── AoiCardItem.tsx           # Kartu AOI individual di sidebar dengan aksi zoom/hapus
│   ├── AoiCreateForm.tsx         # Formulir inline untuk memberi nama dan gaya draf poligon AOI
│   ├── AoiDetailModal.tsx        # Modal yang menampilkan metrik rinci (luas, keliling, koordinat)
│   ├── DeleteAoiDialog.tsx       # Dialog konfirmasi untuk penghapusan AOI
│   ├── DeleteConfirmDialog.tsx   # Dialog konfirmasi generik untuk tindakan destruktif
│   ├── EditAoiModal.tsx          # Modal untuk mengedit nama dan properti AOI yang ada
│   ├── EditSpResultModal.tsx     # Editor komprehensif untuk hasil spasial dan sinkronisasi foto
│   ├── FeatureDetailCard.tsx     # Kartu inspektor properti fitur di sidebar
│   ├── LayerCardItem.tsx         # Item layer di sidebar dengan sakelar visibilitas/opasitas
│   ├── MapSearchBar.tsx          # Kolom pencarian input untuk menemukan koordinat atau fitur
│   ├── MeasureResultCard.tsx     # Kartu melayang yang menampilkan pengukuran jarak/luas aktif
│   ├── OrthoCardItem.tsx         # Kartu ortomosaik yang menampilkan thumbnail, dimensi, dan aksi
│   ├── OrthoDeleteModal.tsx      # Modal ringkasan penghapusan cascade dan konfirmasi
│   ├── OrthoDetailModal.tsx      # Penampil metadata teknis untuk berkas ortomosaik
│   ├── OrthoEditModal.tsx        # Modal untuk menyesuaikan nama dan bounds citra ortomosaik
│   ├── OrthoUploadModal.tsx      # Modal dual-mode untuk mengunggah dan memotong tile ortomosaik
│   ├── PhotoCardItem.tsx         # Kartu thumbnail untuk foto drone ber-geotag individual
│   ├── ResultPhoto.tsx           # Komponen gambar terstandarisasi dengan penanganan fallback
│   └── SpatialResultItem.tsx     # Node pohon yang merepresentasikan layer hasil analisis spasial
├── organisms/                    # Bagian UI kompleks yang tersusun dari molekul dan atom
│   ├── AoiToolbar.tsx            # Toolbar kiri yang mengelola pembuatan AOI, daftar, dan alat bantu
│   ├── DrawingControlBar.tsx     # Lapisan kontrol gambar Geoman untuk kanvas peta
│   ├── ImageGallerySidebar.tsx   # Panel sidebar penelusuran foto hasil tangkapan drone ber-geotag
│   ├── LayerManager.tsx          # Hierarki layer, slider opasitas, dan daftar anotasi
│   ├── ManagedMapLayers.tsx      # Perender layer Leaflet deklaratif untuk vektor dan raster
│   ├── MapCanvas.tsx             # Kontainer peta interaktif Leaflet dan pengaturan tile
│   ├── MapLeftPanel.tsx          # Kontainer sidebar kiri bertab (AOI, Layer, Hasil, Orto)
│   ├── MapViewControlBar.tsx     # Toolbar peta melayang (zoom, pusatkan, ukur, peta dasar)
│   ├── OrthoImportPanel.tsx      # Panel unggah ortomosaik dan pemantau progres job
│   ├── SpatialResultDetail.tsx   # Tampilan inspeksi rinci untuk hasil spasial yang dipilih
│   └── SpatialResultTree.tsx     # Tampilan pohon hierarkis hasil klasifikasi spasial
├── pages/                        # Komposisi tingkat halaman
│   └── MapPage.tsx               # Halaman utama yang memuat kanvas peta, kontrol, dan laci kiri
├── hooks/                        # Custom hook React yang membungkus state dan efek samping
│   ├── useAOI.ts                 # Hook siklus hidup state lengkap untuk poligon Area of Interest
│   ├── useAnnotations.ts         # Hook yang mengelola anotasi vektor buatan pengguna
│   ├── useImageMarkers.ts        # Hook yang memuat dan menyimpan cache titik GPS foto drone
│   ├── useLeafletGeoman.ts       # Hook integrasi yang menghubungkan event toolbar Leaflet-Geoman
│   └── useSpatialResultLayers.ts # Hook yang mengambil, memformat, dan mengaktifkan hasil spasial
├── services/                     # Lapisan abstraksi IPC dan jaringan
│   ├── api.ts                    # 38 fungsi bertipe yang membungkus pemanggilan Tauri invokeMap
│   └── error.ts                  # Pembungkus invokeMap terstandarisasi dan normalisasi error
├── types/                        # Antarmuka domain TypeScript dan definisi tipe
│   ├── aoi.ts                    # Definisi entitas AOI dan geometri
│   ├── index.ts                  # Re-export terpusat untuk semua tipe
│   ├── layers.ts                 # Antarmuka LayerState, LayerType, dan visibilitas
│   ├── map.ts                    # Konfigurasi bounds, koordinat, dan tampilan peta Leaflet
│   ├── ortho.ts                  # Metadata ortomosaik, payload unggah, dan rekaman job
│   └── spatialResult.ts          # Hasil spasial, properti fitur, dan gaya tampilan
├── lib/                          # Fungsi pembantu algoritmik murni dan utilitas
│   ├── aoiCalculations.ts        # Matematika luas, keliling, dan bounding box di sisi client
│   ├── fieldResolution.ts        # Resolusi atribut, penamaan tampilan, dan resolusi URL gambar
│   ├── geoJson.ts                # Normalisasi GeoJSON dan konversi koordinat
│   ├── measurement.ts            # Perhitungan pengukuran dinamis selama proses menggambar
│   ├── popupBuilder.ts           # Pembangun popup HTML untuk penanda peta dan poligon
│   └── leaflet/                  # Pembangun geometri khusus Leaflet
│       ├── aoiLayerBuilder.ts    # Merender lapisan poligon Leaflet interaktif untuk AOI
│       └── spatialLayerBuilder.ts # Mengonversi fitur GeoJSON menjadi layer vektor Leaflet bergaya
├── config/                       # Konfigurasi statis
│   └── mapConfig.ts              # Penyedia tile basemap (OSM, Satelit) dan zoom bawaan
├── store/                        # Penyimpan state lintas komponen
│   └── ImageSyncContext.tsx      # Penyedia konteks penyorotan gambar aktif yang tersinkronisasi
└── docs/                         # Spesifikasi internal dan dokumen referensi
    ├── Proyek.md                 # Salinan spesifikasi proyek PBL 2026
    ├── README_UMUM.md            # Gambaran umum visualisasi aerial
    └── README_modul3.md          # Catatan teknis kebutuhan Modul 03
```

### Purpose of Frontend Categories

| Folder / Layer | Responsibility         | Key Rules & Constraints                                            |
| :------------- | :--------------------- | :----------------------------------------------------------------- |
| `atoms/`       | Elemen UI dasar        | Bersifat stateless murni; menerima props dan merender UI           |
| `molecules/`   | Gabungan dari atom     | Berfokus pada satu tugas (misal kartu item, formulir modal)        |
| `organisms/`   | Komponen gabungan      | Mengoordinasikan hook state dengan molekul anak; memancarkan aksi  |
| `pages/`       | Tata letak halaman     | Menyusun organisme menjadi alur kerja layar penuh yang responsif   |
| `hooks/`       | Bisnis & State Peta    | Membungkus state React, callback termemoize, dan panggilan IPC     |
| `services/`    | Transportasi Tauri IPC | Semua komunikasi diarahkan melalui `invokeMap` di dalam `error.ts` |
| `lib/`         | Algoritma Murni        | Fungsi utilitas deterministik tanpa elemen UI atau efek samping    |

---

## 3. Storage Filesystem Tree (`map_storage/`)

Modul 03 bergantung pada direktori penyimpanan lokal terdedikasi bernama `map_storage/` di akar workspace. Semua operasi penulisan menerapkan penguncian single-instance dan semantik penulisan-ganti atomik:

```text
map_storage/
├── .app.lock                     # Berkas kunci kontrol konkurensi yang memuat PID instans aktif
├── .jobs/                        # Rekaman status pekerjaan impor di latar belakang
│   └── {job_id}.json             # Progres pekerjaan, status, timestamp, dan catatan error
├── aois/                         # Definisi Area of Interest yang tersimpan
│   └── {aoi_id}.json             # Geometri poligon GeoJSON, properti, dan metrik luas
├── annotations/                  # Anotasi vektor buatan pengguna
│   └── {annotation_id}.json      # Fitur GeoJSON dengan catatan, tag, dan kategori
├── images/                       # Direktori penyimpanan raster ortomosaik
│   └── {image_id}/               # Direktori yang didedikasikan untuk ortomosaik tertentu
│       ├── meta.json             # Metadata citra (bounds, resolusi, ukuran berkas, status)
│       ├── raw.tif               # Berkas raster GeoTIFF asli
│       └── tiles/                # Struktur tile piramida zoom mendalam
│           └── {z}/{x}/{y}.png   # Tile raster peta slippy yang dibuat untuk Web Mercator
├── layers/                       # Konfigurasi layer peta
│   └── {layer_id}.json           # Visibilitas layer, opasitas, tipe, dan pengikatan sumber
├── photos/                       # Foto tangkapan drone ber-geotag
│   └── {photo_id}.json           # Metadata foto, ketinggian, yaw, pitch, dan koordinat GPS
├── spatial_results/              # Hasil deteksi spasial & klasifikasi
│   └── {result_id}.json          # Definisi hasil berkas tunggal atau manifes
│   └── {result_id}/              # Direktori hasil spasial multi-fitur
│       ├── manifest.json         # Metadata hasil, versi skema, dan preferensi tampilan
│       ├── result_image/         # Citra hasil yang diekstrak atau terkait
│       │   └── {image_name}.png  # Citra raster atau thumbnail overlay
│       └── features/             # Fitur-fitur terdeteksi individual
│           └── {feature_id}.json # Fitur GeoJSON dengan bounding box, kelas, dan keyakinan
├── .trash_{timestamp}_{id}/      # Direktori karantina yang menunggu penghapusan permanen
└── temp_{uuid}.tmp               # Berkas buffer penulisan atomik sebelum diganti nama
```

### Storage Paths Lifecycle & Ownership Matrix

| Path Pattern                         | Format      | Producer (Writer)       | Consumer (Reader)       | Retention & Lifecycle Rules                                                      |
| :----------------------------------- | :---------- | :---------------------- | :---------------------- | :------------------------------------------------------------------------------- |
| `map_storage/.app.lock`              | Teks (PID)  | `instance_lock.rs`      | Semua instans           | Dibuat saat peluncuran; dihapus saat keluar bersih; dicek via heartbeat PID      |
| `map_storage/.jobs/{job_id}.json`    | JSON        | `image_pipeline.rs`     | `commands.rs`, UI       | Dibuat saat impor dimulai; diperbarui saat ada progres; disimpan untuk riwayat   |
| `map_storage/aois/{id}.json`         | JSON        | `aoi/store.rs`          | `aoi/store.rs`, UI      | Disimpan saat dibuat; diperbarui saat diedit; dihapus saat aksi hapus pengguna   |
| `map_storage/annotations/{id}.json`  | JSON        | `annotations/store.rs`  | `annotations/store.rs`  | Disimpan saat dibuat; ukuran dibatasi 64 KB; dihapus saat aksi pengguna          |
| `map_storage/images/{id}/meta.json`  | JSON        | `ortho_source/store.rs` | `ortho_source/store.rs` | Dibuat saat impor; dibersihkan saat penghapusan citra bertingkat (cascade)       |
| `map_storage/images/{id}/raw.tif`    | GeoTIFF     | `image_pipeline.rs`     | Generator tile          | Diingesti saat impor; dipertahankan sebagai sumber raster asli                   |
| `map_storage/images/{id}/tiles/*`    | PNG         | `tiler.rs`              | Leaflet TileLayer       | Dihasilkan saat pemotongan tile mendalam; di-cache untuk tampilan offline        |
| `map_storage/layers/{id}.json`       | JSON        | `layers/store.rs`       | `layers/store.rs`       | Disimpan saat pembuatan layer; dipulihkan otomatis atau dibersihkan saat startup |
| `map_storage/photos/{id}.json`       | JSON        | Eksternal / Plugin      | `photos/store.rs`       | Dibaca oleh galeri foto; hanya dibaca selama eksplorasi standar                  |
| `map_storage/spatial_results/*.json` | JSON        | Plugin / `writer.rs`    | `reader.rs`, UI         | Diingesti dari plugin; properti dapat diedit melalui patch UI                    |
| `map_storage/.trash_*`               | Direktori   | `ortho_source/store.rs` | `cleanup.rs`            | Folder karantina; dihapus permanen setelah sinkronisasi layer tuntas             |
| `map_storage/temp_*`                 | Berkas Temp | `storage.rs`            | Berkas target           | Buffer atomik efemeral; langsung diubah namanya ke berkas tujuan akhir           |
