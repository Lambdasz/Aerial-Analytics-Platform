# Requirements — Modul 9: Plot Area Analytics (terintegrasi di Modul 3)

> Dokumen kebutuhan (functional & non-functional requirements) untuk **Module 9 — Area & Plot Analytics** pada Aerial Analytics Platform.
> Dokumen ini **bukan** daftar dependency. Sumber kebenaran instalasi tetap `package.json` (`npm ci`) dan `src-tauri/Cargo.toml`.

## 0. Informasi Dokumen

| Item                 | Isi                                                      |
| -------------------- | -------------------------------------------------------- |
| Modul                | Module 9 — Area & Plot Analytics ("Plot Area Analytics") |
| Pemilik              | Kelompok Modul 9                                         |
| Versi aplikasi acuan | `1.0.0` (CHANGELOG `[1.0.0] — 2026-10-29`)               |
| Versi dokumen        | 0.2 (draf — status diperbarui setelah implementasi)      |
| Tanggal              | 25 November 2026                                         |
| Status               | Draf, menunggu review kelompok dan Project Lead          |
| Lokasi               | `src/modules/module-09/REQUIREMENTS.md`                  |

**Legenda status**

| Simbol | Arti                                    |
| ------ | --------------------------------------- |
| ✅     | Sudah ada di kode dan bisa diverifikasi |
| 🔶     | Sebagian ada, perlu penyesuaian         |
| 🔲     | Target, belum dikerjakan                |
| ⏸️     | Ditunda, menunggu keputusan pihak lain  |

**Prinsip perubahan:** perubahan seminimal mungkin. Backend, skema database, dan kontrak dengan modul lain **tidak diubah**, kecuali disebut eksplisit di dokumen ini.

---

## 1. Pendahuluan

### 1.1 Tujuan

Modul 9 menyediakan sistem analitik spasial untuk **mendefinisikan plot** (batas lahan berbentuk poligon) dan **merangkum hasil analisis plugin di dalam tiap plot**, sehingga pengguna dapat membandingkan dan meranking plot berdasarkan indikator tertentu.

### 1.2 Ruang Lingkup

**Termasuk:**

- Membuat, mengimpor, mengganti nama, dan menghapus plot.
- Menghitung luas dan keliling plot.
- Menjalankan plugin analisis (Modul 4–8) per plot dan menyimpan indikatornya.
- Statistik, perbandingan, ranking, dan ringkasan per plot.
- Menampilkan plot sebagai layer di peta Modul 3.

**Tidak termasuk:**

- Algoritma analisis citra (tanggung jawab plugin Modul 4–8).
- Eksekusi dan isolasi plugin (Modul 2).
- Penyimpanan citra, orthomosaic, dan AOI (Modul 1 dan 3).
- Dashboard, laporan, dan ekspor data (Modul 11).
- Analisis perubahan antarwaktu (Modul 10).

### 1.3 Istilah

| Istilah              | Arti                                                                                                     |
| -------------------- | -------------------------------------------------------------------------------------------------------- |
| **Plot**             | Batas lahan bernama yang disimpan Modul 9 (tabel `plot`), dipakai sebagai unit analisis.                 |
| **AOI**              | _Area of Interest_ yang digambar di Map Explorer (Modul 3). AOI bisa dijadikan plot.                     |
| **Indikator**        | Satu nilai metrik hasil plugin untuk satu plot, misalnya `coverage_percentage` (tabel `plot_indicator`). |
| **Orthomosaic**      | Citra udara gabungan yang sudah ber-georeferensi (punya `bounds`), disimpan Modul 3.                     |
| **Slice**            | Potongan citra orthomosaic di dalam batas plot. Plot besar bisa terdiri dari beberapa slice.             |
| **Functional core**  | Bagian kode berupa fungsi murni (tanpa I/O, tanpa mengubah state luar).                                  |
| **Imperative shell** | Bagian kode yang melakukan I/O: database, file, pemanggilan plugin, event UI.                            |

---

## 2. Konteks Integrasi

### 2.1 Posisi di Aplikasi

UI resmi Modul 9 adalah tab **"Plots"** di sidebar kiri **Map Explorer (Modul 3)**.

- Komponen: `src/modules/module-09/PlotAnalyticsPanel.tsx`
- Dipasang di: `src/modules/module-03/organisms/MapLeftPanel.tsx` (tab `plots`)

Halaman terpisah `Module09Page.tsx` (menu "Area & Plot Analytics") beserta editor gambarnya `PlotMapEditor.tsx` **sudah dihapus**. Tab Plots adalah satu-satunya UI Modul 9 (lihat ISU-9.1).

### 2.2 Dependensi

| Modul                        | Yang dipakai Modul 9                                                                                                                           |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| M1 — Image & Project Manager | `project_id` aktif, citra dan sesi terbang                                                                                                     |
| M2 — Plugin System           | `start_plugin_job`, status job, `StandardJobResultDto.metrics`, `CommandError`                                                                 |
| M3 — Map Explorer            | AOI tersimpan (`getSavedAois`), orthomosaic (`get_ortho_image`, `listOrthoImages`), pemotong raster `clip_orthomosaic_for_plot`, layer spasial |
| M4–M8 — Plugin analisis      | Metrik yang menjadi indikator plot                                                                                                             |

### 2.3 Konsumen Hasil

| Modul                       | Yang dibaca                                                                                      |
| --------------------------- | ------------------------------------------------------------------------------------------------ |
| M3 — Map Explorer           | Layer plot dari `publish_plots_layer`                                                            |
| M11 — Dashboard & Reporting | Tabel `plot` dan `plot_indicator`, dibaca langsung (dashboard, laporan HTML, ekspor CSV/GeoJSON) |

### 2.4 Alur Utama

```
[M3] Gambar AOI ──┐
                  ├─► [M9] Plot (validasi + luas/keliling) ─► tabel `plot`
[File] GeoJSON ───┘
                              │
                              ▼
[M3] Orthomosaic ─► [M3] clip per plot ─► slice ─► [M2] jalankan plugin per slice
                                                        │
                              ┌─────────────────────────┘
                              ▼
          [M9] gabung metrik antar-slice (pure) ─► tabel `plot_indicator`
                              │
              ┌───────────────┼──────────────────┐
              ▼               ▼                  ▼
   Statistik/Perbandingan   Ranking      Ringkasan per plot
              │
              ▼
   [M3] layer plot di peta        [M11] dashboard & laporan
```

---

## 3. Keputusan Desain

| ID   | Keputusan                                                                                                                                                                                 | Alasan                                                               |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| KD-1 | Tab **Plots** di Modul 3 menjadi **UI resmi** Modul 9. Fitur baru hanya ditambahkan di sini.                                                                                              | Plot adalah data spasial dan paling alami dikelola langsung di peta. |
| KD-2 | Kode tetap di folder `src/modules/module-09/` (frontend) dan `src-tauri/src/modules/module_09/` (backend). Modul 3 hanya menampung panel.                                                 | Kepemilikan kode jelas, file Modul 3 hampir tidak tersentuh.         |
| KD-3 | Backend, command Tauri, dan skema tabel `plot` / `plot_indicator` **tidak berubah**.                                                                                                      | Modul 11 bergantung pada tabel ini.                                  |
| KD-4 | `Module09Page.tsx` dan menunya **dihapus**. Fitur yang belum ada di tab Plots (Refresh, centang semua, konfirmasi hapus) dipindahkan lebih dulu; fitur gambar plot tidak dipindah (KD-5). | Menghilangkan UI ganda (ISU-9.1).                                    |
| KD-5 | Plot dibuat dari AOI Modul 3, bukan dari editor gambar terpisah. `PlotMapEditor` sudah dihapus bersama `Module09Page` (ISU-9.1).                                                          | Menghindari dua alat gambar yang sama di satu peta.                  |
| KD-6 | Untuk sementara, analisis per plot **hanya dari orthomosaic**.                                                                                                                            | Hanya jalur ini yang benar-benar memotong citra per plot (ISU-9.4).  |

---

## 4. Kebutuhan Fungsional

Format: **ID** · kebutuhan · status · lokasi kode · cara verifikasi.

### FR-9.1 Plot Definition — _Create or import plot boundaries_

| ID       | Kebutuhan                                                                                                                                                                           | Status | Lokasi kode                                                                                                                                                | Verifikasi                                                                                       |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| FR-9.1.1 | Pengguna dapat membuat plot dari satu atau beberapa AOI tersimpan di Modul 3. Tab Plots menampilkan jumlah AOI yang belum dijadikan plot.                                           | ✅     | `PlotAnalyticsPanel.tsx`, `createPlotsFromAois()` / `createPlot()` di `plotCommands.ts`, kolom `source_aoi_id`                                             | Uji manual UM-1                                                                                  |
| FR-9.1.2 | Pengguna dapat **mengimpor file GeoJSON** (Feature / FeatureCollection Polygon) dari tab Plots. Fitur yang tidak valid dilewati dan dilaporkan, bukan menggagalkan seluruh impor.   | ✅     | tombol _Import GeoJSON_ di `PlotAnalyticsPanel.tsx` → `importPlots()` → `import_plots_into_project` → `plot::parse_plots()` + `layer::import_candidates()` | Unit test `plot.rs` + uji manual UM-2                                                            |
| FR-9.1.3 | Pengguna dapat **mengganti nama** plot dari tab Plots. Nama kosong ditolak.                                                                                                         | ✅     | `RenamePlotDialog.tsx` → `renamePlot()` → `rename_plot`                                                                                                    | Unit test `empty_names_are_rejected` + UM-3                                                      |
| FR-9.1.4 | Pengguna dapat menghapus plot setelah konfirmasi. Indikator milik plot itu ikut terhapus.                                                                                           | ✅     | dialog konfirmasi di `PlotAnalyticsPanel.tsx` → `delete_plot`, `delete_plot_indicators`                                                                    | Unit test `project_filter_and_plot_deletion`                                                     |
| FR-9.1.5 | Setiap geometri plot divalidasi: GeoJSON `Polygon`, urutan `[lon, lat]`, koordinat dalam rentang WGS84, ring ditutup otomatis, ring pertama = batas luar, ring berikutnya = lubang. | ✅     | `polygon_from_geometry()` di `module_09/plots.rs`                                                                                                          | Unit test `invalid_geometry_is_rejected`, `rejects_swapped_coordinates`, `rejects_unclosed_ring` |
| FR-9.1.6 | Plot terikat ke proyek aktif (`project_id`) dan hanya ditampilkan untuk proyek tersebut. Plot tanpa proyek tetap terlihat.                                                          | ✅     | `list_plots(project_id)`                                                                                                                                   | Unit test `project_filter_and_plot_deletion`                                                     |

### FR-9.2 Area Calculation — _Calculate plot area and perimeter_

| ID       | Kebutuhan                                                                                                                                     | Status | Lokasi kode                         | Verifikasi                                 |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------- | ------ | ----------------------------------- | ------------------------------------------ |
| FR-9.2.1 | Luas (m² dan ha) dan keliling (m) dihitung **secara geodesik** setiap kali plot disimpan, lalu disimpan di kolom `area_m2` dan `perimeter_m`. | ✅     | `metrics()` di `module_09/plots.rs` | Unit test `polygon_is_closed_and_measured` |
| FR-9.2.2 | Luas dan keliling ditampilkan di daftar plot dan ringkasan plot dengan satuan yang sesuai.                                                    | ✅     | `formatArea()`, `formatPerimeter()` | Uji manual UM-1                            |

### FR-9.3 Analysis by Plot — _Aggregate analytical results within individual plot boundaries_

| ID       | Kebutuhan                                                                                                                                                                                                                                                    | Status                                                                        | Lokasi kode                                                                           | Verifikasi                                                                                   |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| FR-9.3.1 | Pengguna memilih plot, plugin, dan orthomosaic. Sistem memotong orthomosaic mengikuti batas tiap plot, menjalankan plugin **per slice secara berurutan**, lalu menyimpan hasil per plot.                                                                     | ✅                                                                            | `analyse_plots_from_ortho` → `analyse_plots_from_ortho_impl()` di `ortho_analysis.rs` | Uji manual UM-4                                                                              |
| FR-9.3.2 | Metrik antar-slice digabung menjadi satu nilai per plot: metrik hitungan (`*count*`, `*pixel*`, `*total*`) dijumlahkan; metrik rasio/persentase dirata-rata dengan bobot jumlah piksel; persentase coverage dihitung ulang dari jumlah piksel bila tersedia. | ✅                                                                            | `aggregate_slice_metrics()` (pure, gaya fungsional sejak NFR-9.1.9)                   | Unit test `test_aggregate_slice_metrics_*` (4 test)                                          |
| FR-9.3.3 | Plot yang tidak beririsan dengan orthomosaic **ditolak per plot** dengan pesan jelas, tanpa menghentikan plot lainnya.                                                                                                                                       | ✅                                                                            | `ClipperError::OutOfBounds` → `failed_plots`                                          | Uji manual UM-5                                                                              |
| FR-9.3.4 | Sumber **session image** pada dialog analisis dinonaktifkan sementara (atau diberi peringatan), karena belum memotong citra per plot.                                                                                                                        | ✅ (tombol _Flight Session_ nonaktif + peringatan; default _Map Orthomosaic_) | `SESSION_SOURCE_ENABLED = false` di `AnalysePlotsDialog.tsx`                          | Uji manual UM-6                                                                              |
| FR-9.3.5 | Progres analisis (plot ke-n, slice ke-m, persen) ditampilkan ke pengguna, dan ringkasan akhir memuat jumlah plot berhasil, plot gagal beserta alasannya, dan total slice.                                                                                    | ✅                                                                            | event `plot-analysis-progress`, `PlotAnalysisOutcome`                                 | Uji manual UM-4                                                                              |
| FR-9.3.6 | Hasil indikator disimpan sebagai nilai **terbaru** per kombinasi (plot, plugin, indikator), lengkap dengan satuan default bila dikenali.                                                                                                                     | ✅                                                                            | `save_indicators()`, `default_unit()`                                                 | Unit test `save_upserts_latest_value_per_key`, `normalisation_keeps_scalars_and_drops_nulls` |

### FR-9.4 Plot Statistics — _Calculate summary statistics for each plot_

| ID       | Kebutuhan                                                                                                                                                             | Status                                                                                             | Lokasi kode                                                                                                 | Verifikasi            |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | --------------------- |
| FR-9.4.1 | Untuk setiap plot, sistem menampilkan seluruh indikator terakhirnya, dikelompokkan per plugin, beserta satuan.                                                        | ✅ (kartu plot menampilkan 2 indikator + "+N more"; daftar lengkap per plugin di ringkasan FR-9.7) | `list_plot_indicators`, `formatIndicator()`, `latestFor()`, `indicatorsByPlugin()`, `PlotSummaryDialog.tsx` | Uji manual UM-7, UM-8 |
| FR-9.4.2 | Untuk indikator yang dipilih, sistem menghitung statistik antarplot: **jumlah plot, rata-rata, minimum, maksimum, dan simpangan baku**. Dihitung dengan fungsi murni. | ✅ (simpangan baku populasi; satu nilai → 0)                                                       | `indicatorStats()` di `plotAnalytics.ts`, ditampilkan di `PlotRankingCard.tsx`                              | Unit test baru UT-1   |

### FR-9.5 Multi-Plot Comparison — _Compare indicators among different plots_

| ID       | Kebutuhan                                                                                                                                                              | Status | Lokasi kode                                                        | Verifikasi      |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------ | --------------- |
| FR-9.5.1 | Pengguna memilih satu indikator, lalu sistem menampilkan nilai indikator tersebut untuk semua plot yang memilikinya, berdampingan dalam satu daftar beserta satuannya. | ✅     | `PlotRankingCard.tsx`, `comparisonEntries()` di `plotAnalytics.ts` | Uji manual UM-7 |
| FR-9.5.2 | Plot yang belum punya nilai untuk indikator tersebut tidak ikut dibandingkan, dan tidak dianggap bernilai 0.                                                           | ✅     | filter `value !== null` di `comparisonEntries()`                   | Uji manual UM-7 |

### FR-9.6 Plot Ranking — _Rank plots according to selected analytical indicators_

| ID       | Kebutuhan                                                                                                                                                                                                              | Status | Lokasi kode                                                                                                                                    | Verifikasi          |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- |
| FR-9.6.1 | Plot diurutkan berdasarkan indikator terpilih dan diberi peringkat.                                                                                                                                                    | ✅     | `rankEntries()` di `plotAnalytics.ts` (menyalin array, _stable sort_)                                                                          | Uji manual UM-7     |
| FR-9.6.2 | Pengguna dapat memilih **arah urutan**: tertinggi dulu atau terendah dulu. Indikator "makin kecil makin baik" (`yellowing_pct`, `browning_pct`, `moderate_stress_pct`, `severe_stress_pct`) default-nya terendah dulu. | ✅     | `rankEntries()`, `defaultDirection()`, `LOWER_IS_BETTER` di `plotAnalytics.ts`; toggle _Highest first / Lowest first_ di `PlotRankingCard.tsx` | Unit test baru UT-2 |
| FR-9.6.3 | Indikator yang namanya sama tetapi berasal dari plugin berbeda **tidak tercampur**. Pilihan indikator dibedakan per plugin.                                                                                            | ✅     | kunci `pluginId::indicator` di `indicatorOptions()`; di layer peta `ind_<plugin>_<indicator>` (`layer::indicator_property_key()`)              | Unit test baru UT-3 |

### FR-9.7 Plot Summary — _Generate a structured analytical summary for each plot_

| ID       | Kebutuhan                                                                                                                                                    | Status                                | Lokasi kode                                     | Verifikasi      |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------- | ----------------------------------------------- | --------------- |
| FR-9.7.1 | Dari tab Plots, pengguna dapat membuka **ringkasan per plot** berisi: nama, luas, keliling, tanggal dibuat, dan indikator terakhir dikelompokkan per plugin. | ✅ (klik nama plot atau ikon dokumen) | `PlotSummaryDialog.tsx`, `indicatorsByPlugin()` | Uji manual UM-8 |
| FR-9.7.2 | Plot tanpa hasil analisis menampilkan pesan "belum ada hasil analisis", bukan tabel kosong.                                                                  | ✅ ("No analysis results yet")        | `PlotSummaryDialog.tsx`                         | Uji manual UM-8 |

> Ekspor ringkasan (CSV/JSON/GeoJSON) **tidak** menjadi kebutuhan Modul 9, karena sudah disediakan Modul 11.

### FR-9.8 Plot Layer di Peta

| ID       | Kebutuhan                                                                                                                                    | Status                                                 | Lokasi kode                                              | Verifikasi      |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ | -------------------------------------------------------- | --------------- |
| FR-9.8.1 | Semua plot dalam satu proyek dapat dipublikasikan sebagai satu layer di Map Explorer, sehingga bisa ditampilkan, disembunyikan, dan di-klik. | ✅                                                     | `publish_plots_layer` → `layer::plot_layer_payload()`    | Uji manual UM-9 |
| FR-9.8.2 | Popup plot menampilkan nama, luas (ha), dan keliling (m).                                                                                    | ✅                                                     | properti `plot_name`, `area_ha`, `perimeter_m`           | Uji manual UM-9 |
| FR-9.8.3 | Popup plot juga menampilkan indikator terakhir hasil plugin.                                                                                 | ✅ (satu field per plugin + indikator, beserta satuan) | `layer::plot_properties()`, `layer::plot_layer_fields()` | Uji manual UM-9 |

---

## 5. Kebutuhan Non-Fungsional

### NFR-9.1 Functional Thinking

| ID         | Kebutuhan                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Status                                                              | Bukti / Verifikasi                                                                                                                                                           |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| NFR-9.1.1  | Logika perhitungan dipisahkan dari I/O dengan pola **functional core / imperative shell**. Fungsi perhitungan tidak boleh mengakses database, file, jaringan, atau event UI.                                                                                                                                                                                                                                                                                                                 | ✅                                                                  | `ortho_analysis.rs` (bagian `FUNCTIONAL CORE` dan `IMPERATIVE SHELL`); `polygon_from_geometry`, `metrics`, `normalise_indicators`, `aggregate_slice_metrics` ditandai _Pure_ |
| NFR-9.1.2  | Fungsi murni selalu menghasilkan output yang sama untuk input yang sama, sehingga bisa diuji tanpa database atau aplikasi berjalan.                                                                                                                                                                                                                                                                                                                                                          | ✅                                                                  | Unit test memakai input statis atau SQLite in-memory                                                                                                                         |
| NFR-9.1.3  | Data input **tidak diubah di tempat** (immutability). Fungsi menerima referensi dan mengembalikan nilai baru, misalnya `aggregate_slice_metrics(&[Value]) -> Value`. Di frontend, state diperbarui dengan salinan baru (`new Set(...)`, spread), bukan mutasi.                                                                                                                                                                                                                               | ✅ (`withMember()` / `toggleMember()` untuk state `Set`)            | Code review                                                                                                                                                                  |
| NFR-9.1.4  | Transformasi koleksi **wajib** memakai **higher-order function** (`map`, `filter`, `reduce`/`fold`, `collect`, `flat_map`, `zip`), bukan loop `for` yang mengubah variabel luar.                                                                                                                                                                                                                                                                                                             | ✅                                                                  | Code review + pemeriksaan NFR-9.1.8; target untuk FR-9.4.2, FR-9.6.2, FR-9.6.3                                                                                               |
| NFR-9.1.5  | Kegagalan direpresentasikan sebagai nilai (`Result<T, E>` / `Option<T>` di Rust, `CommandError { code, message }` ke frontend), bukan `panic!`, `todo!()`, atau `unwrap()` pada data dari luar.                                                                                                                                                                                                                                                                                              | ✅                                                                  | `plot_api.rs` kini mengembalikan `Err` yang menyebut command pengganti (ISU-9.2)                                                                                             |
| NFR-9.1.6  | **Percabangan sebagai ekspresi, bukan `if/else`.** Kode Modul 9 tidak memakai `if / else` maupun rantai `else if`. Penggantinya: **Rust** → `match` (termasuk _match guard_), kombinator `Option`/`Result` (`map`, `and_then`, `filter`, `ok_or`, `unwrap_or`, `unwrap_or_else`), operator `?`, dan `if let` tanpa `else` untuk satu pola; **TypeScript** → tabel pemetaan (`Record<K, V>` / objek fungsi), ternary sederhana satu tingkat untuk nilai, operator `??` / `?.`, dan HOF array. | ✅ (sisa `if` hanya _match guard_ Rust)                             | Pemeriksaan NFR-9.1.8 + code review                                                                                                                                          |
| NFR-9.1.7  | **Tanpa mutasi.** Kode Modul 9 tidak memakai `let mut`, `push`/`insert` ke koleksi lokal, atau penugasan ulang variabel (`let` → `const` di TypeScript). Akumulasi dilakukan dengan `fold`/`reduce`, rekursi, atau `collect`. Pengecualian hanya untuk kebutuhan API pihak luar (misalnya builder `rusqlite`/Tauri) dan wajib diberi komentar alasannya.                                                                                                                                     | ✅ (pengecualian berkomentar: `useRef` di `AnalysePlotsDialog.tsx`) | Pemeriksaan NFR-9.1.8 + code review                                                                                                                                          |
| NFR-9.1.8  | **Cakupan aturan NFR-9.1.6–9.1.7** adalah semua kode milik Modul 9: `src-tauri/src/modules/module_09/`, `src-tauri/src/commands/plots.rs`, `src-tauri/src/plot.rs`, dan `src/modules/module-09/`. Kode modul lain (M2, M3, dst.) **tidak** diubah. Kepatuhan diperiksa sebelum commit dengan pencarian pola terlarang (lihat 7.4).                                                                                                                                                           | ✅ (0 baris)                                                        | Bagian 7.4                                                                                                                                                                   |
| NFR-9.1.9  | **Refactor `aggregate_slice_metrics()`** di `ortho_analysis.rs` agar memenuhi NFR-9.1.4, 9.1.6, dan 9.1.7 (saat ini masih memakai `let mut`, `for`, dan `if/else`). **Perilaku tidak boleh berubah**: keempat unit test `test_aggregate_slice_metrics_*` harus tetap lolos tanpa diubah.                                                                                                                                                                                                     | ✅                                                                  | Unit test yang ada + pemeriksaan 7.4                                                                                                                                         |
| NFR-9.1.10 | Kode Modul 9 yang lain (`plots.rs`, `indicators.rs`, `commands/plots.rs`, `plot.rs`, sisa `ortho_analysis.rs`, dan semua komponen di `src/modules/module-09/`) di-refactor bertahap mengikuti aturan yang sama, dengan unit test yang ada sebagai jaring pengaman. Setiap refactor tidak boleh mengubah perilaku.                                                                                                                                                                            | ✅ (unit test lolos; uji manual 7.3 belum dijalankan)               | Unit test yang ada + uji manual bagian 7.3                                                                                                                                   |

**Contoh penerapan NFR-9.1.6–9.1.7** (arah urutan ranking, FR-9.6.2):

```ts
// ❌ Tidak sesuai: if/else + mutasi array asli
if (direction === "desc") {
  list.sort((a, b) => b.value - a.value);
} else {
  list.sort((a, b) => a.value - b.value);
}

// ✅ Sesuai: tabel pemetaan + fungsi murni, data asli tidak berubah
const comparators: Record<Direction, Comparator> = {
  desc: (a, b) => b.value - a.value,
  asc: (a, b) => a.value - b.value,
};
const rankPlots =
  (direction: Direction) =>
  (entries: readonly Entry[]): Entry[] =>
    [...entries].sort(comparators[direction]);
```

```rust
// ❌ Tidak sesuai
let mut sum = 0.0;
for v in values { if v.is_finite() { sum += v; } }

// ✅ Sesuai
let sum: f64 = values.iter().filter(|v| v.is_finite()).sum();

// ✅ Percabangan dengan match, bukan if/else
let kind = match key {
    k if k.contains("pct") || k.contains("percent") => MetricKind::Ratio,
    k if k.contains("count") || k.contains("pixel") => MetricKind::Count,
    _ => MetricKind::Ratio,
};
```

### NFR-9.2 Kontrak Geometri

| ID        | Kebutuhan                                                                                                                                                | Status |
| --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| NFR-9.2.1 | Geometri plot mengikuti **GeoJSON RFC 7946**, `Polygon`, CRS **WGS84 (EPSG:4326)**, urutan **`[longitude, latitude]`**, sama dengan kontrak AOI Modul 3. | ✅     |
| NFR-9.2.2 | Perhitungan luas dan keliling dilakukan di backend (Rust), bukan di frontend.                                                                            | ✅     |

### NFR-9.3 Ketahanan & Penanganan Error

| ID        | Kebutuhan                                                                                                                                                       | Status |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| NFR-9.3.1 | Kegagalan satu plot (geometri tidak valid, di luar citra, plugin gagal, timeout) **tidak menghentikan** analisis plot lainnya dan tidak membuat aplikasi crash. | ✅     |
| NFR-9.3.2 | Setiap job plugin per slice punya batas waktu (saat ini 120 detik) dan dapat dibatalkan.                                                                        | ✅     |
| NFR-9.3.3 | Pesan error ke pengguna menyebut nama plot dan penyebabnya.                                                                                                     | ✅     |

### NFR-9.4 Performa & Sumber Daya

| ID        | Kebutuhan                                                                                                                                 | Status          |
| --------- | ----------------------------------------------------------------------------------------------------------------------------------------- | --------------- |
| NFR-9.4.1 | Slice diproses **berurutan** dan file slice dihapus setelah diproses, supaya pemakaian RAM dan disk tidak bertambah seiring jumlah slice. | ✅              |
| NFR-9.4.2 | File sementara ditaruh di direktori staging sementara yang terhapus otomatis setelah analisis selesai.                                    | ✅ (`tempfile`) |

### NFR-9.5 Kualitas Kode & Kesiapan Open Source

| ID        | Kebutuhan                                                                                                                          | Status                                                                                             |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| NFR-9.5.1 | Kode lolos `npm run lint`, `npm run lint:rust`, `npm run format:check`, `npm run format:rust:check`.                               | ✅ wajib sebelum commit                                                                            |
| NFR-9.5.2 | Semua unit test lolos: `cargo test --manifest-path src-tauri/Cargo.toml`.                                                          | ✅ wajib sebelum commit                                                                            |
| NFR-9.5.3 | Command Tauri baru (bila ada) diletakkan di `src-tauri/src/commands/` dan didaftarkan di `lib.rs`; logika di `modules/module_09/`. | aturan `AGENTS.md`                                                                                 |
| NFR-9.5.4 | Bagian `## Module 9` di rustdoc `src-tauri/src/lib.rs` diperbarui jika ada perubahan perilaku.                                     | aturan `AGENTS.md`                                                                                 |
| NFR-9.5.5 | Tidak ada kode mati atau kontrak menyesatkan di repo (lihat ISU-9.2, ISU-9.3).                                                     | 🔶 (ISU-9.3 selesai; `plot_api.rs` sudah `#[deprecated]`, menunggu persetujuan Lead untuk dihapus) |
| NFR-9.5.6 | Tersedia data contoh untuk mencoba fitur tanpa data pribadi: `test-data/contoh-plot.geojson`.                                      | ✅                                                                                                 |

---

## 6. Kontrak Data & Command

> Bagian ini mendokumentasikan kontrak yang **sudah ada**. Tidak ada perubahan skema.

### 6.1 Tabel `plot`

| Kolom                      | Tipe          | Keterangan                                      |
| -------------------------- | ------------- | ----------------------------------------------- |
| `id`                       | TEXT PK       | UUID plot                                       |
| `project_id`               | TEXT          | Proyek pemilik (boleh kosong)                   |
| `session_id`               | TEXT          | Sesi terbang terkait (opsional)                 |
| `name`                     | TEXT NOT NULL | Nama plot                                       |
| `geometry`                 | TEXT NOT NULL | GeoJSON `Polygon` (WGS84)                       |
| `source_aoi_id`            | TEXT          | ID AOI Modul 3 asal plot (bila dibuat dari AOI) |
| `area_m2`                  | REAL NOT NULL | Luas geodesik (m²)                              |
| `perimeter_m`              | REAL NOT NULL | Keliling geodesik (m)                           |
| `created_at`, `updated_at` | TEXT NOT NULL | Waktu dibuat / diubah                           |

### 6.2 Tabel `plot_indicator`

| Kolom                  | Tipe                                    | Keterangan                                  |
| ---------------------- | --------------------------------------- | ------------------------------------------- |
| `id`                   | TEXT PK                                 | UUID baris                                  |
| `plot_id`              | TEXT NOT NULL                           | Plot pemilik                                |
| `plugin_id`            | TEXT NOT NULL                           | Plugin penghasil                            |
| `execution_id`         | TEXT                                    | ID eksekusi (opsional)                      |
| `indicator`            | TEXT NOT NULL                           | Nama metrik, misalnya `coverage_percentage` |
| `unit`                 | TEXT                                    | Satuan, misalnya `%`, `m²`, `px`            |
| `value` / `text_value` | REAL / TEXT                             | Nilai numerik atau teks                     |
| `created_at`           | TEXT NOT NULL                           | Waktu disimpan                              |
| —                      | `UNIQUE(plot_id, plugin_id, indicator)` | Satu nilai terbaru per kombinasi            |

### 6.3 Command Tauri yang Dipakai

| Command                     | Wrapper frontend (`plotCommands.ts`) | Fungsi                                                   |
| --------------------------- | ------------------------------------ | -------------------------------------------------------- |
| `create_plot`               | `createPlot()`                       | Simpan plot baru (validasi + luas/keliling)              |
| `list_plots`                | `listPlots()`                        | Daftar plot per proyek                                   |
| `get_plot_by_id`            | —                                    | Ambil satu plot                                          |
| `rename_plot`               | `renamePlot()`                       | Ganti nama plot                                          |
| `delete_plot`               | `deletePlot()`                       | Hapus plot beserta indikatornya                          |
| `import_plots_into_project` | `importPlots()`                      | Impor GeoJSON ke proyek                                  |
| `save_plot_indicators`      | `savePlotIndicators()`               | Simpan indikator satu run                                |
| `list_plot_indicators`      | `listPlotIndicators()`               | Baca indikator                                           |
| `publish_plots_layer`       | `publishPlotsLayer()`                | Publikasikan plot sebagai layer peta                     |
| `analyse_plots_from_ortho`  | `analysePlotsFromOrtho()`            | Analisis plot dari orthomosaic (clip + slice + agregasi) |

Event: `plot-analysis-progress` (`PlotAnalysisProgressEvent`).

---

## 7. Rencana Verifikasi

### 7.1 Unit Test yang Sudah Ada

Jalankan: `cargo test --manifest-path src-tauri/Cargo.toml`

| File                                                | Jumlah | Test                                                                                                                                                                                                                                                           | Menguji                      |
| --------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- |
| `src-tauri/src/modules/module_09/plots.rs`          | 4      | `polygon_is_closed_and_measured`, `invalid_geometry_is_rejected`, `crud_roundtrip`, `empty_names_are_rejected`                                                                                                                                                 | FR-9.1.3–9.1.5, FR-9.2.1     |
| `src-tauri/src/modules/module_09/indicators.rs`     | 3      | `normalisation_keeps_scalars_and_drops_nulls`, `save_upserts_latest_value_per_key`, `project_filter_and_plot_deletion`                                                                                                                                         | FR-9.1.4, FR-9.1.6, FR-9.3.6 |
| `src-tauri/src/plot.rs`                             | 8      | `reads_valid_polygon`, `each_plot_gets_distinct_uuid`, `plot_without_name_gets_default_name`, `rejects_unclosed_ring`, `rejects_swapped_coordinates`, `rejects_non_json_text`, `skips_non_polygon_geometry_without_error`, `errors_when_no_valid_plot_remains` | FR-9.1.2, FR-9.1.5           |
| `src-tauri/src/modules/module_09/ortho_analysis.rs` | 4      | `test_aggregate_slice_metrics_empty`, `_single_slice`, `_weighted_average`, `_tree_counts`                                                                                                                                                                     | FR-9.3.2                     |
| `src-tauri/src/modules/module_09/layer.rs`          | 4      | `properties_carry_plot_metrics_and_own_indicators`, `same_indicator_from_two_plugins_gets_two_fields`, `import_outcome_reports_why_nothing_was_stored`, `layer_id_depends_on_project`                                                                          | FR-9.1.2, FR-9.6.3, FR-9.8.3 |
| `src-tauri/src/plot_api.rs`                         | 1      | `legacy_functions_fail_without_panicking`                                                                                                                                                                                                                      | NFR-9.1.5, ISU-9.2           |

### 7.2 Unit Test yang Perlu Ditambah

| ID   | Untuk     | Kasus uji minimal                                                                                                                                      |
| ---- | --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| UT-1 | FR-9.4.2  | Daftar kosong; satu nilai (simpangan baku = 0); beberapa nilai dengan rata-rata/min/maks/simpangan yang sudah dihitung manual; nilai `null` diabaikan. |
| UT-2 | FR-9.6.2  | Urutan menurun dan menaik; nilai sama (urutan stabil); indikator "makin kecil makin baik" default menaik; input asli tidak berubah setelah diurutkan.  |
| UT-3 | FR-9.6.3  | Dua plugin dengan nama indikator sama menghasilkan dua pilihan terpisah dan nilainya tidak tercampur.                                                  |
| UT-4 | NFR-9.1.9 | Keempat test `test_aggregate_slice_metrics_*` dijalankan **sebelum dan sesudah** refactor tanpa diubah; hasilnya harus sama-sama lolos.                |

> Fungsi untuk UT-1 s.d. UT-3 sudah dibuat sebagai fungsi murni di `src/modules/module-09/plotAnalytics.ts` (`indicatorStats`, `rankEntries`, `defaultDirection`, `indicatorOptions`, `comparisonEntries`) dan kasus minimal di atas sudah dicek dengan skrip lokal. Repo belum punya test runner frontend, sehingga file test-nya belum ditambahkan; test runner tetap disepakati kelompok mengikuti konvensi repo. UT-4 ✅: keempat test lolos sebelum dan sesudah refactor tanpa diubah.

### 7.3 Skenario Uji Manual

Data: `test-data/contoh-plot.geojson` (BLOK_A1, BLOK_A2, …) dan satu orthomosaic yang menutupi area tersebut.

| ID    | Langkah                                                                                                  | Hasil yang diharapkan                                                                                                                            | Untuk                  |
| ----- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------- |
| UM-1  | Buka Map Explorer → gambar 2 AOI → tab Plots → jadikan plot.                                             | 2 plot muncul dengan luas dan keliling; jumlah "AOI belum jadi plot" berkurang.                                                                  | FR-9.1.1, FR-9.2       |
| UM-2  | Tab Plots → Import GeoJSON → pilih `contoh-plot.geojson`. Ulangi dengan file berisi satu fitur rusak.    | Semua plot valid tersimpan; fitur rusak dilaporkan sebagai _skipped_.                                                                            | FR-9.1.2               |
| UM-3  | Ganti nama plot; coba nama kosong.                                                                       | Nama berubah; nama kosong ditolak dengan pesan.                                                                                                  | FR-9.1.3               |
| UM-4  | Pilih 2–3 plot → Analisis → plugin M5 → orthomosaic.                                                     | Progres per plot/slice tampil; setiap plot punya nilai `coverage_percentage` **yang berbeda** sesuai isi plotnya.                                | FR-9.3.1, FR-9.3.5     |
| UM-5  | Buat plot di luar orthomosaic, lalu jalankan analisis bersama plot lain.                                 | Plot luar dilaporkan gagal ("di luar jangkauan citra"); plot lain tetap berhasil.                                                                | FR-9.3.3, NFR-9.3.1    |
| UM-6  | Buka dialog analisis.                                                                                    | Sumber session image nonaktif atau diberi peringatan.                                                                                            | FR-9.3.4               |
| UM-7  | Pilih indikator di perbandingan; ubah arah urutan.                                                       | Nilai, rata-rata, statistik, dan peringkat sesuai; plot tanpa nilai tidak ikut.                                                                  | FR-9.4, FR-9.5, FR-9.6 |
| UM-8  | Buka ringkasan plot yang sudah dan belum dianalisis.                                                     | Ringkasan lengkap per plugin; plot belum dianalisis menampilkan pesan kosong.                                                                    | FR-9.7                 |
| UM-9  | Publish layer plot → klik plot di peta.                                                                  | Layer tampil; popup memuat nama, luas, keliling (dan indikator setelah FR-9.8.3).                                                                | FR-9.8                 |
| UM-10 | Buka Modul 11 setelah UM-4.                                                                              | Dashboard dan laporan memuat plot dan indikator yang sama.                                                                                       | KD-3                   |
| UM-11 | Lihat sidebar aplikasi; di tab Plots klik Refresh, centang _Select all_, lalu klik hapus pada satu plot. | Menu "Area & Plot Analytics" tidak ada; Refresh memuat ulang plot dan AOI; semua plot tercentang; dialog konfirmasi muncul sebelum plot dihapus. | ISU-9.1, FR-9.1.4      |

### 7.4 Pemeriksaan Gaya Fungsional

Dijalankan dari root repo sebelum commit. Hasil yang diharapkan: **tidak ada baris yang muncul**, kecuali pengecualian yang sudah diberi komentar sesuai NFR-9.1.7.

```bash
# Rust — kode milik Modul 9
rg -n '\belse\b|\blet mut\b|^\s*for\s.*\bin\b|\.push\(' \
  src-tauri/src/modules/module_09 src-tauri/src/commands/plots.rs src-tauri/src/plot.rs

# TypeScript — kode milik Modul 9 (hanya file kode; REQUIREMENTS.md berisi contoh ❌/✅)
rg -n -g '*.ts' -g '*.tsx' '\belse\b|^\s*let\s|\bfor\s*\(|\.push\(' src/modules/module-09
```

> Pemeriksaan ini alat bantu, bukan pengganti code review. Bagian `#[cfg(test)]` boleh dikecualikan bila diperlukan. `Module09Page.tsx` dan `PlotMapEditor.tsx` sudah dihapus (ISU-9.1), sehingga tidak lagi ikut diperiksa.

**Baseline v1.0.0** (jumlah baris yang cocok, untuk mengukur kemajuan refactor). **Setelah refactor: 0 baris di semua file.**

| File                                                         | Baris cocok |
| ------------------------------------------------------------ | ----------- |
| `src-tauri/src/modules/module_09/ortho_analysis.rs`          | 37          |
| `src-tauri/src/modules/module_09/plots.rs`                   | 11          |
| `src-tauri/src/commands/plots.rs`                            | 8           |
| `src-tauri/src/modules/module_09/indicators.rs`              | 6           |
| `src-tauri/src/plot.rs`                                      | 4           |
| `src/modules/module-09/AnalysePlotsDialog.tsx`               | 9           |
| `src/modules/module-09/PlotAnalyticsPanel.tsx`               | 7           |
| `src/modules/module-09/PlotMapEditor.tsx` (dihapus, ISU-9.1) | 3           |
| `src/modules/module-09/Module09Page.tsx` (dihapus, ISU-9.1)  | 5           |

---

## 8. Isu Terbuka

### ISU-9.1 — Halaman `Module09Page` duplikat dengan tab Plots

- **Kondisi sebelumnya:** `src/shell/modules.ts` mendaftarkan menu "Area & Plot Analytics" yang membuka `Module09Page.tsx` (±789 baris). Di saat yang sama, `MapLeftPanel.tsx` Modul 3 memasang `PlotAnalyticsPanel.tsx` (±666 baris) sebagai tab Plots. Logika daftar plot, perbandingan, dan ranking ditulis dua kali.
- **Dampak:** perbaikan harus dikerjakan di dua tempat; pengguna bisa bingung memakai yang mana.
- **Pilihan:** (a) hapus halaman dan menunya; (b) ganti isinya dengan petunjuk ke Map Explorer → tab Plots; (c) biarkan apa adanya.
- **Keputusan kelompok:** **(a) hapus halaman dan menunya.** Sebelumnya (c) dibiarkan sementara. Semua fitur halaman lama sudah ada di tab Plots (KD-1); tiga yang belum ada dipindahkan lebih dulu: tombol **Refresh**, checkbox **Select all**, dan **dialog konfirmasi hapus** (FR-9.1.4). Fitur gambar plot (`PlotMapEditor`) tidak dipindah karena digantikan alur AOI → _From AOI_ (KD-5).
  - Dihapus: `src/modules/module-09/Module09Page.tsx`, `src/modules/module-09/PlotMapEditor.tsx`, dan entri `module-09` / slug `area-plot-analytics` di `src/shell/modules.ts`.
  - Tidak berubah: backend Modul 9, command Tauri, dan skema tabel `plot` / `plot_indicator` (KD-3), sehingga Modul 11 tidak terdampak.
  - Hash lama `#/area-plot-analytics` diarahkan ke modul default oleh `AppShell.tsx`.
- **Yang memutuskan:** Project Lead (menyentuh `shell/modules.ts`).
- **Status:** ✅ (a) selesai.

### ISU-9.2 — Kontrak lama `plot_api.rs` masih `todo!()`

- **Kondisi sekarang:** `src-tauri/src/plot_api.rs` berisi `define_plot`, `analyze_plot`, dan `get_plot_statistics`, ketiganya masih `todo!()`. Fungsinya sudah digantikan `modules/module_09/` dan `commands/plots.rs`, tetapi file masih didaftarkan di `lib.rs` (`pub mod plot_api`).
- **Dampak:** kontributor bisa mengira fungsi ini yang harus dipanggil; jika terpanggil aplikasi akan _panic_; dokumentasi kontrak tidak sesuai kode yang dipakai.
- **Pilihan:** (a) hapus file; (b) tandai `#[deprecated]` dan arahkan ke fungsi pengganti; (c) isi dengan memanggil implementasi baru.
- **Keputusan kelompok:** **(b) dulu, lalu (a)** setelah Lead setuju.

  | Fungsi lama           | Pengganti                                                                  |
  | --------------------- | -------------------------------------------------------------------------- |
  | `define_plot`         | `create_plot` / `modules::module_09::plots::insert_plot`                   |
  | `analyze_plot`        | `analyse_plots_from_ortho`                                                 |
  | `get_plot_statistics` | `list_plot_indicators` / `modules::module_09::indicators::list_indicators` |

- **Yang memutuskan:** kelompok Modul 9 bersama Lead.
- **Status:** 🔶 (b) selesai: ketiga fungsi bertanda `#[deprecated]` dan mengembalikan `Err` yang menyebut penggantinya (tidak lagi `todo!()`). (a) menunggu persetujuan Lead.

### ISU-9.3 — Komponen lama `PlotBoundaryManager`

- **Kondisi sekarang:**
  - `src/components/PlotBoundaryManager.tsx` dan `src/types/plot.ts` tidak di-import di mana pun, sehingga tidak muncul di aplikasi.
  - Command `import_plots` di `lib.rs` hanya dipakai komponen tersebut.
  - **Namun** `parse_plots()` di `src-tauri/src/plot.rs` (8 unit test) **masih menjadi inti impor GeoJSON** yang aktif (`import_plots_into_project` → `plot::parse_plots()`).
- **Dampak:** ada kode mati di repo; risiko terbesar adalah `plot.rs` ikut dihapus karena dikira kode lama.
- **Pilihan:** (a) hapus komponen UI, tipe, dan command lama, tetapi pertahankan `parse_plots`; (b) simpan semua dan beri label _legacy_.
- **Keputusan kelompok:** **(a)**.
  - Dihapus: `PlotBoundaryManager.tsx`, `src/types/plot.ts`, command `import_plots` (fungsi + pendaftaran di `lib.rs` + baris tabel rustdoc).
  - **Wajib dipertahankan:** `src-tauri/src/plot.rs` dan `parse_plots()` beserta unit test-nya.
- **Yang memutuskan:** kelompok Modul 9, dikonfirmasi ke Lead.
- **Status:** ✅ (a) selesai: `PlotBoundaryManager.tsx`, `src/types/plot.ts`, dan command `import_plots` dihapus; `plot.rs` / `parse_plots()` beserta 8 unit test-nya dipertahankan.

### ISU-9.4 — Analisis dari session image belum memotong per plot

- **Kondisi sekarang:**
  - **Sumber orthomosaic:** citra dipotong mengikuti batas plot (`clip_orthomosaic_for_plot`), digabung oleh `aggregate_slice_metrics`, dan plot di luar citra ditolak (`ClipperError::OutOfBounds`). ✅
  - **Sumber session image:** `AnalysePlotsDialog.tsx` mengirim **seluruh foto** ke plugin dengan `aoi = polygon plot`.
  - Plugin M4 (`resolve_bounds` di `plugins/rgb-vegetation-detection`), M5, dan M6 memakai geometri AOI sebagai **letak seluruh foto**, bukan sebagai batas potong.
  - Tidak ada pengecekan apakah plot berada di atas footprint foto.
- **Dampak:** semua plot bisa mendapat nilai yang hampir sama, sehingga perbandingan dan ranking tidak bermakna; overlay di peta ikut melar mengikuti bentuk plot.
- **Pilihan:** (a) Modul 9 mengecek irisan plot dengan footprint citra sebelum menjalankan plugin; (b) M4–M6 mengubah plugin agar AOI dipakai untuk memotong; (c) sementara analisis per plot hanya dari orthomosaic.
- **Keputusan kelompok:** **(c) + (a)**, sedangkan **(b) diajukan** lewat Discussion Board ke module lead M4–M6.
  - (c) → FR-9.3.4.
  - (a) → untuk orthomosaic sudah terpenuhi (FR-9.3.3). Jika sumber session image diaktifkan kembali, cek irisan dengan footprint foto wajib ditambahkan lebih dulu.
- **Yang memutuskan:** kelompok Modul 9 untuk (a) dan (c); module lead M4–M6 untuk (b).
- **Status:** 🔶 sebagian (jalur orthomosaic sudah benar; sumber session image dinonaktifkan lewat FR-9.3.4).
- _Catatan: kesimpulan tentang plugin M4–M6 berasal dari pembacaan kode dan belum diuji dengan menjalankan aplikasi._

### Ringkasan Isu

| ID      | Judul                         | Keputusan                        | Status         | Yang memutuskan    |
| ------- | ----------------------------- | -------------------------------- | -------------- | ------------------ |
| ISU-9.1 | `Module09Page` duplikat       | (a) halaman dan menu dihapus     | ✅             | Project Lead       |
| ISU-9.2 | `plot_api.rs` masih `todo!()` | (b) lalu (a)                     | 🔶 (b) selesai | Kelompok M9 + Lead |
| ISU-9.3 | `PlotBoundaryManager` lama    | (a), `parse_plots` dipertahankan | ✅             | Kelompok M9 + Lead |
| ISU-9.4 | Session image belum dipotong  | (c) + (a), (b) diajukan          | 🔶             | Kelompok M9; M4–M6 |

---

## 9. Ringkasan Status

| Fitur                        | Status | Pekerjaan tersisa                                                                                |
| ---------------------------- | ------ | ------------------------------------------------------------------------------------------------ |
| FR-9.1 Plot Definition       | ✅     | —                                                                                                |
| FR-9.2 Area Calculation      | ✅     | —                                                                                                |
| FR-9.3 Analysis by Plot      | ✅     | Aktifkan kembali session image setelah ada cek irisan footprint (ISU-9.4)                        |
| FR-9.4 Plot Statistics       | ✅     | —                                                                                                |
| FR-9.5 Multi-Plot Comparison | ✅     | —                                                                                                |
| FR-9.6 Plot Ranking          | ✅     | —                                                                                                |
| FR-9.7 Plot Summary          | ✅     | —                                                                                                |
| FR-9.8 Plot Layer di Peta    | ✅     | —                                                                                                |
| NFR-9.1 Functional Thinking  | ✅     | —                                                                                                |
| NFR-9.5 Kualitas Kode        | 🔶     | Hapus `plot_api.rs` setelah Lead setuju (ISU-9.2); sepakati test runner frontend untuk UT-1–UT-3 |
| Verifikasi manual (7.3)      | 🔲     | Jalankan UM-1 s.d. UM-11 di aplikasi (`npm run tauri dev`)                                       |
