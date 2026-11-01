# Alur Kerja & Siklus Hidup Frontend: Modul 03

Dokumen ini merinci pipeline reaktif, siklus hidup aliran data, dan urutan resolusi layer di dalam frontend **Modul 03 (Aerial Image Map Explorer)**.

---

## 1. User Interaction to Backend IPC Pipeline

Setiap mutasi yang dipicu pengguna mengikuti aliran data searah yang ketat untuk menjaga konsistensi state dan isolasi error:

```text
+-------------------+       Aksi User      +--------------------+
|   Molekul UI /    | -------------------> |  Organisme React / |
|  Toolbar Kontrol  |                      |  State Hook Kustom |
+-------------------+                      +--------------------+
                                                     |
                                                     | Memvalidasi Draf &
                                                     | Mengatur Status Pemuatan
                                                     v
+-------------------+     invokeMap()      +--------------------+
|    Inti Tauri 2   | <------------------- |  services/api.ts   |
|    Backend Rust   |                      | (Panggilan Bertipe)|
+-------------------+                      +--------------------+
         |
         | Hasil / CommandError
         v
+-------------------+    Komitmen State /  +--------------------+
|   Cache Lokal /   | -------------------> | Notifikasi Toast / |
| Sinkronisasi Hook |    Pemulihan Error   | Render Ulang Peta  |
+-------------------+                      +--------------------+
```

### Flow Stages

1. **Interaksi Pengguna**: Tindakan dipicu melalui molekul UI (misalnya menekan "Simpan AOI" di `AoiCreateForm` atau menggeser opasitas di `LayerManager`).
2. **Validasi Hook**: Organisme pengelola atau hook kustom (`useAOI`, `useSpatialResultLayers`) memvalidasi parameter dan mengatur status optimis atau pemuatan.
3. **Lapisan Transportasi**: Fungsi bertipe di `services/api.ts` memanggil Tauri IPC melalui `invokeMap`.
4. **Komitmen Hasil**: Jika berhasil, state internal diperbarui dan komponen anak dirender ulang. Jika gagal, error dinormalisasi menjadi notifikasi toast Blueprint yang ramah pengguna.

---

## 2. Spatial Result Layer Rendering Pipeline

Hasil analisis spasial (klaster vegetasi, segmentasi tutupan lahan) diingesti, diformat, dan dikonversi menjadi lapisan overlay Leaflet:

```text
[ Penyimpanan Disk: map_storage/spatial_results/ ]
                       |
                       | IPC: get_saved_spatial_results()
                       v
        [ Hook: useSpatialResultLayers ]
                       |
                       | Transformasi & Normalisasi
                       v
        [ Pure: spatialLayerBuilder.ts ]
          - Parsing Poligon / Titik GeoJSON
          - Injeksi Gaya Dinamis & Warna Isian
          - Pembuatan Template HTML Tooltip & Popup
                       |
                       v
        [ Organism: ManagedMapLayers.tsx ]
          - <GeoJSON> & <Polygon> Deklaratif
          - Injeksi Kanvas Peta Leaflet
```

---

## 3. Image URL Resolution Pipeline (`resolveImageSrc`)

Meresolusi URL gambar (thumbnail ortomosaik, potongan foto deteksi) memerlukan penanganan untuk tiga lingkungan berbeda: protokol web eksternal, lingkungan pengembangan lokal Vite, dan paket biner aplikasi desktop Tauri.

```text
String URL Gambar Mentah
         |
         +---> 1. Dimulai dengan "http://", "https://", "data:", atau "asset:"?
         |        YA -> Kembalikan URL yang telah dipangkas spasinya langsung.
         |
         +---> 2. Mengandung path traversal ("..")?
         |        YA -> Catat peringatan keamanan & Kembalikan null.
         |
         +---> 3. Mengandung "map_storage/"?
         |        |
         |        +---> Lingkungan: Runtime Desktop Tauri (isTauri() == true)?
         |        |     YA -> Ambil storageRoot yang di-cache (get_storage_root_path)
         |        |            Gabungkan path kanonikal + path aset relatif
         |        |            Teruskan ke convertFileSrc() -> "asset://localhost/..."
         |        |
         |        +---> Lingkungan: Server Dev Web Browser (isTauri() == false)?
         |              YA -> Layani via middleware Vite -> "/map_storage/..."
         |
         +---> 4. Aset Publik Statis (misalnya "/sample_drone.jpg")?
                  Kembalikan path dengan garis miring depan.
```

---

## 4. AOI Drawing & Calculation Pipeline

```text
[ Pengguna Memilih Alat Poligon pada DrawingControlBar ]
                       |
                       v
[ Event Leaflet-Geoman: pm:create ]
                       |
                       v
[ Hook: useLeafletGeoman ]
  - Mencegat Pembuatan Lapisan
  - Mengekstrak Cincin Koordinat: [[lat, lng], ...]
                       |
                       v
[ Pure: aoiCalculations.ts ]
  - Menghitung Luas Shoelace (m² dan Hektar)
  - Menghitung Keliling Geodesik Haversine (m)
  - Menghitung Kotak Pembatas (SouthWest, NorthEast)
                       |
                       v
[ Molecule: AoiCreateForm.tsx ]
  - Pengguna Memberi Nama AOI dan Memilih Warna Tampilan
                       |
                       v
[ API: saveAoi() via invokeMap ]
  - Backend menulis secara atomik ke map_storage/aois/{id}.json
```
