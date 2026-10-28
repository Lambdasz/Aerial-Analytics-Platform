# Rincian Komponen Frontend: Modul 03

Dokumen ini merinci spesifikasi fungsional, interaksi komponen, dan tanggung jawab state dari komponen-komponen React utama yang menyusun **Modul 03 (Aerial Image Map Explorer)**.

---

## 1. Page Container & Layout (`MapPage.tsx`)

Komponen halaman tingkat akar untuk Modul 03 adalah [`MapPage.tsx`](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/pages/MapPage.tsx).

- **Peran**: Mengoordinasikan tata letak tingkat tinggi antara kanvas peta Leaflet interaktif dan panel samping kontrol yang dapat diciutkan.
- **Organisme Anak**:
  - `MapCanvas`: Kontainer Leaflet layar penuh.
  - `MapLeftPanel`: Laci yang dapat diciutkan yang memuat tab operasional utama (AOI, Layer, Hasil, Orto).
  - `MapViewControlBar`: Lapisan kontrol melayang untuk manipulasi peta (zoom, pusatkan, alat ukur).
  - `ImageGallerySidebar`: Laci sekunder untuk menginspeksi foto tangkapan drone ber-geotag.
- **Koordinasi State**: Mempertahankan indeks tab aktif dan status buka/tutup laci tanpa memicu render ulang kanvas peta yang tidak perlu.

---

## 2. Interactive Map Canvas & Layer Rendering

### `MapCanvas.tsx`

- **Peran**: Menginstansiasi `MapContainer` Leaflet, mengatur pusat bawaan (`[-7.2575, 112.7521]` Surabaya) dan zoom (16), serta menginisialisasi layer tile.
- **Komponen Anak**:
  - `ManagedMapLayers`: Merender seluruh lapisan vektor dan raster aktif.
  - `DrawingControlBar`: Menyuntikkan kontrol Geoman ke dalam kontainer peta.
  - `MeasureResultCard`: Tampilan pengukuran melayang saat mengukur jarak atau luas.

### `ManagedMapLayers.tsx`

- **Peran**: Koordinator deklaratif untuk seluruh layer peta Leaflet yang sedang aktif.
- **Tipe Layer yang Ditangani**:
  - **TileLayer**: Tile slippy lokal atau offline yang dihasilkan dari ortomosaik (`map_storage/images/{id}/tiles/{z}/{x}/{y}.png`).
  - **GeoJSON**: Layer vektor untuk hasil klasifikasi spasial (klaster vegetasi, batas tutupan lahan).
  - **Polygon**: Lapisan batas Area of Interest dengan interaksi pemilihan dan penyorotan aktif.
  - **Markers**: Lokasi foto drone ber-geotag dan anotasi spasial buatan pengguna.
- **Optimasi Performa**: Menggunakan rendering deklaratif alih-alih mutasi DOM imperatif langsung untuk mencegah kebocoran memori saat pengalihan layer cepat.

---

## 3. Left Panel Navigation & Toolbars

### `MapLeftPanel.tsx`

- **Peran**: Menyediakan antarmuka bertab yang rapi menggunakan Blueprint.js `Tabs` dan `Tab`:
  - **Tab 1: Area of Interest (AOI)**: Memuat `AoiToolbar` untuk menggambar, mendaftar, dan menghitung metrik poligon.
  - **Tab 2: Layers**: Memuat `LayerManager` untuk mengatur visibilitas, slider opasitas, dan mengelola anotasi.
  - **Tab 3: Spatial Results**: Memuat `SpatialResultTree` untuk menelusuri hasil deteksi dan klasifikasi.
  - **Tab 4: Orthomosaics**: Memuat `OrthoImportPanel` untuk mengunggah, menginspeksi, dan mengelola raster GeoTIFF.

### `AoiToolbar.tsx`

- **Peran**: Manajemen siklus hidup poligon AOI yang digambar pengguna.
- **Fitur**:
  - Terintegrasi dengan Geoman melalui `useLeafletGeoman` untuk memicu mode gambar (`polygon`, `rectangle`).
  - Menghitung luas waktu nyata (hektar, meter persegi) dan keliling menggunakan rumus Shoelace dan algoritma geodesik di `lib/aoiCalculations.ts`.
  - Molekul Anak:
    - `AoiCreateForm`: Formulir inline untuk memberi nama dan gaya draf poligon.
    - `AoiCardItem`: Kartu sidebar yang menampilkan nama poligon, warna, luas, dan tombol aksi.
    - `AoiDetailModal`: Modal teknis yang menampilkan koordinat simpul dan bounds.
    - `DeleteAoiDialog`: Dialog konfirmasi penghapusan.

### `LayerManager.tsx`

- **Peran**: Kontrol terpusat untuk opasitas layer peta dan hierarki tampilan.
- **Fitur**:
  - Slider opasitas (0.0 hingga 1.0) dengan pembaruan opasitas layer Leaflet waktu nyata.
  - Sakelar visibilitas (ikon mata) dengan pembaruan state immutable yang fungsional.
  - Manajemen anotasi: Anotasi vektor Titik, Garis, dan Poligon dengan tag label dan catatan metadata.

---

## 4. Spatial Analysis Results & Orthomosaics

### `SpatialResultTree.tsx` & `SpatialResultDetail.tsx`

- **Peran**: Menelusuri dan menginspeksi hasil klasifikasi spasial yang dihasilkan oleh plugin AI (Modul 04, Modul 05).
- **Fitur**:
  - Struktur pohon hierarkis yang mengelompokkan hasil berdasarkan layer, kategori, atau ambang keyakinan.
  - Pemilihan fitur individual memicu pergeseran kamera peta (`flyTo`) dan membuka inspektor properti.
  - Menampilkan metrik klasifikasi (jumlah pohon, diameter tajuk, indeks kesehatan vegetasi NDVI).

### `EditSpResultModal.tsx`

- **Peran**: Modal komprehensif untuk menyunting metadata hasil spasial, warna tampilan, dan properti fitur.
- **Kapabilitas Kunci (Sinkronisasi Opsi 2)**:
  - Mendukung unggah atau hapus foto representatif tingkat layer.
  - Menyinkronkan pembaruan gambar layer secara otomatis ke seluruh fitur anak di dalam `features/*.json`.
  - Menyediakan kartu pratinjau langsung waktu nyata yang menunjukkan bagaimana kartu popup fitur akan terlihat di peta.

### `OrthoUploadModal.tsx` & `OrthoImportPanel.tsx`

- **Peran**: Alur kerja ingesti dual-mode untuk citra ortomosaik aerial GeoTIFF berukuran besar.
- **Dual Mode**:
  - **Mode A (Fast Copy)**: Ingesti cepat untuk raster ukuran kecil hingga menengah tanpa pra-pemotongan tile.
  - **Mode B (Auto Tiling)**: Pembuatan tile piramida asinkron (tingkat zoom 10 hingga 18) yang dikelola pekerja latar belakang.
- **Pemantauan**: Bilah progres waktu nyata yang melakukan polling ke `.jobs/{job_id}.json` dengan dukungan pembatalan.
