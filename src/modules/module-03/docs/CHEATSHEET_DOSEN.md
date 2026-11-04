# Cheat Sheet Pertanggungjawaban Teknis & Sidang Modul 03 (Map Explorer)

Dokumen ini berisi panduan Tanya-Jawab (Q&A) teknis untuk menjawab pertanyaan penguji/dosen terkait arsitektur perangkat lunak, komputasi spasial, performa memori, integritas data, dan pemrosesan citra pada **Modul 03 (Aerial Image Map Explorer)**.

---

### Q1: Di mana logika perhitungan matematika (luas, perimeter, dan jarak) dilakukan?

**Jawab:**
Seluruh komputasi matematika resmi dilakukan di backend Rust pada submodul `geometry`:

- **Luas Poligon:** [`spherical_excess.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/spherical_excess.rs) (`calculate_area`) menggunakan metode _Spherical Excess_ WGS84 ($R = 6.371.008,8\text{ m}$). Algoritma ini menjumlahkan luas ring luar dan mengurangi luas ring lubang (_holes_).
- **Jarak Dua Titik:** [`haversine.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/haversine.rs) (`calculate_distance`) menggunakan formula trigonometri _Haversine_ lingkaran besar (_great-circle distance_).
- **Keliling Poligon:** [`spherical_excess.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/spherical_excess.rs) (`calculate_perimeter`) dengan iterasi _sliding window_ segmen batas berpasangan via Haversine.
- **Konversi & Reproyeksi Koordinat:** [`projection.rs`](file:///home/bhisma/Documents/GitHub/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/projection.rs) mendukung konversi murni EPSG:4326, EPSG:3857, dan UTM WGS84 Zona 1–60 Utara/Selatan (formulasi invers Snyder/Karney) secara mandiri (_Pure Rust_) tanpa bergantung pada pustaka C eksternal atau dependensi berat seperti GDAL/PROJ.

---

### Q2: Mengapa di file TypeScript/TSX masih ditemukan rumus matematika seperti `isPointInPolygon` dan `calculateGeodesicAreaMeters`? Apakah frontend melakukan komputasi berat?

**Jawab:**
Tidak, frontend **sama sekali tidak melakukan komputasi berat (_heavy lifting_)**. Rumus matematika di TypeScript berbobot $O(N)$ (hanya beberapa puluhan titik) dan sengaja ditempatkan di frontend untuk kebutuhan UI/UX responsif:

1. **Hit-Testing Responsif (`isPointInPolygon`):** Menggunakan algoritma _Ray-Casting (Jordan Curve Theorem)_ di `lib/geoJson.ts` untuk mengatasi batasan CSS `pointer-events: stroke`. Saat pengguna mengklik area dalam poligon di luar citra ortomosaik, browser langsung mengevaluasi poligon mana yang terpilih dalam hitungan sub-milidetik (<1 ms) tanpa perlu memicu _round-trip_ IPC ke backend Rust pada setiap klik mouse.
2. **Instant UI Feedback (`calculateGeodesicAreaMeters` & `calculateLineLengthMeters`):** Menampilkan estimasi luas dan keliling pada kartu modal secara instan saat pengguna masih menggambar atau mengedit batas poligon di kanvas peta.
3. **Z-Order Rendering Sorting (`estimatePolygonArea`):** Mengurutkan urutan render layer poligon pada Leaflet secara dinamis (poligon berukuran lebih kecil ditempatkan di layer atas agar tidak tertutup poligon besar).
4. **Footprint Kamera Drone (`estimatePhotoBounds`):** Menghitung kotak visual cakupan kamera drone berdasarkan ketinggian terbang $AGL$ ($2 \cdot alt \cdot \tan(HFOV / 2)$).

Seluruh angka metrik resmi yang disimpan ke dalam storage atau basis data tetap dihitung dan divalidasi oleh Rust di backend.

---

### Q3: Mana sajakah fungsi di backend yang tergolong sebagai _Pure Function_? Mengapa hal ini penting?

**Jawab:**
Fungsi murni (_Pure Functions_) adalah fungsi yang deterministik (input sama selalu menghasilkan output sama), tidak menghasilkan efek samping (_side-effect_), dan tidak melakukan I/O (tanpa membaca disk, jaringan, atau state global). Fungsi-fungsi ini terkonsentrasi di:

- **`geometry/`**:
  - `haversine::calculate_distance`
  - `spherical_excess::calculate_area`
  - `spherical_excess::calculate_perimeter`
  - `projection::wgs84_to_epsg3857`
  - `projection::epsg3857_to_wgs84`
  - `projection::wgs84_to_utm`
  - `projection::utm_to_wgs84`
  - `projection::project_to_wgs84`
  - `projection::compute_wgs84_envelope_from_corners`
  - `validation::validate_coordinate`
  - `validation::validate_polygon`
  - `validation::validate_fixed_ring`
  - `validation::validate_dynamic_ring`
  - `validation::SpatialGeometry::validate`
- **`ortho_source/pure.rs`**:
  - `classify_image_extension`
  - `calculate_aspect_ratio_difference`
- **`sp_result/pure.rs`**:
  - `validate_spatial_payload`
  - `validate_spatial_result_id`
  - `validate_name`
  - `validate_notes`
  - `validate_hex_color`
  - `validate_photo_magic_bytes`
  - `calculate_verified_value` (Menghitung ulang verifikasi metrik luas, keliling, atau jarak)
  - `is_reserved_key`
  - `is_owned_result_photo`
  - `validate_custom_field_key`
  - `validate_field_label`
  - `validate_field_unit`
  - `validate_custom_fields_count`
  - `validate_property_value_type`
- **`layers/pure.rs`**:
  - `toggle_layer_visibility`
  - `set_layer_opacity`
  - `reorder_layer_stack`
  - `filter_layers_by_category`
- **`aoi/builder.rs` & `annotations/pure.rs`**:
  - `aoi::create_aoi`
  - `annotations::create_annotation`

**Pentingnya:** Menerapkan prinsip _Functional Programming_, mempermudah pengujian unit terisolasi (_100% deterministic unit tests_ tanpa perlu mock database/filesystem), serta menjamin keandalan sistem dari _race conditions_ dan _concurrency bugs_.

---

### Q4: Bagaimana alur pemrosesan (_pipeline_) citra ortomosaik dan GeoTIFF dirancang?

**Jawab:**
Pipeline pemrosesan citra dipecah secara modular di bawah folder `ortho_source/pipeline/` dan diorkestrasi dalam 6 tahapan terstruktur layaknya resep masakan dari atas ke bawah:

1. **Tahap 1 (Fast-Fail & Preflight):** Validasi header magic bytes berkas, pembatasan ukuran berkas (maks 50 MB), batas dimensi aman (maks 144 MP), pembatasan JPEG progressive (>40 MP), resolusi koordinat georeferensi (Opsi A: World File $\rightarrow$ Sidecar $\rightarrow$ Manual; Opsi B: GeoTIFF internal Geokeys $\rightarrow$ Manual), dan pengecekan kapasitas sisa ruang hard disk (`check_disk_space_preflight`).
2. **Tahap 2 (Streaming Copy Asli):** Menyalin berkas citra asli ke folder _staging_ secara streaming dengan buffer 64 KB, dilengkapi pengecekan token pembatalan (`CancellationToken`) agar dapat dibatalkan instan oleh user.
3. **Tahap 3 (Dekompresi & Downscaling Efisien):**
   - **JPEG:** Menggunakan dekompresi parsial DCT (_Discrete Cosine Transform_) melalui `jpeg-decoder` sehingga decoding langsung dilakukan pada skala target (2048px).
   - **PNG:** Menggunakan streaming row-by-row decoding via `png::Reader::next_row()` yang disalurkan langsung ke `StreamingResampler`.
   - **GeoTIFF:** Mendeteksi peregangan kontras 16-bit (_percentile stretch_ $p_{0.5} - p_{99.5}$) dan memeriksa keberadaan sub-IFD piramida overview internal untuk dekompresi instan.
4. **Tahap 4 (Loop Adaptif Display):** Mengompresi citra display secara bertingkat hingga ukuran file $\le 5\text{ MiB}$ (JPEG) atau $\le 10\text{ MiB}$ (PNG transparan).
5. **Tahap 5 (Staging RAII & Registrasi Atomik):** Menulis metadata teknis secara atomik (`write_atomic`), merename folder staging ke folder penyimpanan final (`fs::rename`), dan mendaftarkan layer ke `map_storage/layers/{id}.json`.
6. **Tahap 6 (Penyelesaian):** Mengubah status job menjadi 100% selesai dan mengembalikan DTO respon.

---

### Q5: Bagaimana sistem mencegah kehabisan memori (RAM Out-Of-Memory) saat memproses citra berukuran puluhan hingga ratusan megapixel?

**Jawab:**
Sistem menerapkan tiga strategi mitigasi memori:

1. **IDCT Scaling (JPEG):** Pada JPEG beresolusi raksasa, pustaka `jpeg-decoder` membaca koefisien DCT secara parsial sehingga citra langsung didekode ke ukuran display (maksimal 2048px). Sistem tidak pernah mengalokasikan buffer piksel mentah penuh untuk citra 100+ megapixel, memotong konsumsi RAM hingga 75%.
2. **Row-by-Row Streaming (PNG):** Membaca baris piksel satu per satu (`next_row()`) ke dalam `StreamingResampler` berbasis Box Filter, menekan kebutuhan RAM puncak dari ~400 MB menjadi kurang dari 50 KB per iterasi baris.
3. **IFD Overview Acceleration (GeoTIFF):** Jika berkas GeoTIFF memiliki piramida sub-IFD internal, decoder langsung melompat ke resolusi tinjauan (_overview_) tanpa mendekompresi base raster berukuran gigabyte.

---

### Q6: Bagaimana integritas file terjamin jika aplikasi crash atau proses dibatalkan di tengah pemrosesan citra?

**Jawab:**
Menggunakan pola **Staging RAII Guard** (`StagingGuard`) dan **Atomic Write**:

- Seluruh berkas sementara ditulis ke direktori isolasi `.staging/<job_id>`.
- Jika proses dibatalkan oleh pengguna atau terjadi _panic/crash_, _drop handler_ Rust (`StagingGuard::drop`) secara otomatis menghapus direktori staging dari hard disk.
- Seluruh berkas metadata (`metadata.json`, `meta.json`, dan file layer) ditulis menggunakan helper `write_atomic` yang menulis ke berkas sementara `.tmp` terlebih dahulu sebelum dilakukan _atomic replace/rename_.
- Pemindahan direktori staging ke direktori permanen `map_storage/images/<id>` hanya terjadi di _Point of No Return_ via pemanggilan primitif OS `fs::rename()`. Hal ini menjamin tidak ada folder citra korup, setengah jadi, atau sisa artefak gagal yang tertinggal di sistem.

---

### Q7: Mengapa digunakan _DTO & Envelope Pattern_ pada Tauri Commands? Mengapa tidak mengembalikan array mentah?

**Jawab:**
Seluruh komunikasi IPC distandarisasi menggunakan objek permintaan bertipe tunggal (`*Request`) dan objek respons terbungkus (`*Response`):

1. **Mencegah Bug Urutan Argumen (_Fragile Telescoping Arguments_):** Menghindari kesalahan fatal akibat tertukarnya urutan parameter bertipe sama (misal `latitude` dan `longitude` yang keduanya bertipe `f64`).
2. **Forward Compatibility (Envelope Pattern):** Format respons seperti `{ aois: [...], total: 5 }` memungkinkan backend menambahkan metadata tambahan (seperti durasi query, pagination cursor, atau status sinkronisasi) di masa depan tanpa merusak kontrak kode (_breaking change_) pada frontend.
3. **Type Safety via Serde:** Menggunakan anotasi `#[serde(rename_all = "camelCase")]` untuk menjembatani konvensi penamaan Rust (`snake_case`) dan TypeScript (`camelCase`) secara otomatis saat waktu kompilasi (_compile-time_).
