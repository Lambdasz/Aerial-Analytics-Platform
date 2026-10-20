# Rencana Implementasi: Penyatuan Data Template & Refaktor Fungsional Module 3 (Aerial Image Map Explorer)

Dokumen ini disusun sebagai panduan arsitektur dan eksekusi sebelum mengubah kode (**Tahap 0**), sesuai aturan keras proyek PBL Aerial Analytics Platform.

---

## 1. Peta File Module 3

### 1.1 Struktur Frontend (`src/modules/module-03/`)

- **`Module03Page.tsx`**  
  _Peran_: Halaman utama Modul 3. Membungkus `MapCanvas` dengan `ImageSyncProvider` agar state sinkronisasi foto dan peta aktif.
- **`components/MapCanvas.tsx`**  
  _Peran_: Kanvas peta Leaflet utama. Menginisialisasi `MapContainer`, me-render `TileLayer` (OSM / Citra Satelit), `ManagedMapLayers` (orthomosaic, flight session, vegetation mask, tree detection), `AnnotationMarkers`, serta mengintegrasikan panel kontrol, pencarian, dan legenda.
- **`components/MapLeftPanel.tsx`**  
  _Peran_: Panel samping kiri yang dapat diciutkan (_collapsible_). Memiliki 3 tab: `Layers` (`LayerManager`), `AOI Tools` (`AoiDemoToolbar`), dan `Gallery` (`ImageGallerySidebar`).
- **`components/LayerManager.tsx`**  
  _Peran_: Pengelola layer peta (visibilitas, slider opasitas, filter kategori) dan form input pembuatan anotasi spasial baru.
- **`components/AoiDemoToolbar.tsx`**  
  _Peran_: Toolbar mengambang untuk pembuatan poligon Area of Interest (AOI), memicu drawing mode via Leaflet-Geoman, validasi, dan penyimpanan AOI ke backend.
- **`components/ImageGallerySidebar.tsx`**  
  _Peran_: Galeri daftar thumbnail foto drone hasil sesi terbang (`flight_session_01`). Mendukung pencarian, filter, dan sinkronisasi seleksi dengan marker pada peta.
- **`components/DroneImageMarker.tsx`**  
  _Peran_: Marker posisi foto udara pada peta dengan ikon drone yang dirotasi sesuai sudut kamera (`heading_deg`). Membuka `ImagePopupCard` saat diklik.
- **`components/ImagePopupCard.tsx`**  
  _Peran_: Kartu preview foto yang muncul saat marker diklik. Menampilkan thumbnail, nama file, koordinat, ketinggian, dan tombol "Pusatkan & Zoom".
- **`components/MapSearchBar.tsx`**  
  _Peran_: Komponen bilah pencarian spasial dengan filter kategori layer/fitur.
- **`components/MapTypeThumbnail.tsx`**  
  _Peran_: Tombol pratinjau kecil di sudut kiri bawah untuk berpindah antara peta jalan (OpenStreetMap) dan citra satelit (Esri World Imagery).
- **`components/MapViewControlBar.tsx`**  
  _Peran_: Bar kontrol navigasi peta: tombol Reset View (kembali ke tengah orthomosaic), Zoom In, Zoom Out, dan Zoom to Extent (Fit Bounds).
- **`components/MapSyncHandler.tsx`**  
  _Peran_: Komponen headless yang menyimak perubahan `activeImageId` dari galeri lalu menggeser kanvas peta (`map.flyTo`) ke koordinat foto tersebut.
- **`components/useImageMarkers.ts`**  
  _Peran_: Hook penyedia daftar marker foto drone. **Saat ini masih menghasilkan data acak lokal (`Math.random`)**.
- **`components/useSpatialResultLayers.ts`**  
  _Peran_: Hook yang memanggil Tauri command `get_dummy_spatial_layers` untuk mengambil data layer hasil analitik dan membangun `L.LayerGroup` di Leaflet.
- **`hooks/useAOI.ts`**  
  _Peran_: Hook pengelola siklus hidup poligon AOI (mulai menggambar, batal, hapus, dan simpan AOI via `createAoi`).
- **`hooks/useMeasurement.ts`**  
  _Peran_: Hook untuk pengukuran spasial (jarak, luas poligon, keliling poligon).
- **`store/ImageSyncContext.tsx`**  
  _Peran_: React Context untuk sinkronisasi dua arah (_bi-directional_) antara kartu galeri dan marker peta (`activeImageId`, `interactionSource`, `setActiveImage`).
- **`api.ts`** & **`error.ts`**  
  _Peran_: Lapisan pemanggilan IPC Tauri terisolasi (`invokeMap`) dengan penanganan error dan logging tersentralisasi.
- **`types/`** (`aoi.ts`, `layer.ts`, `map.ts`, `marker.ts`)  
  _Peran_: Definisi interface TypeScript yang menjadi kontrak data frontend.
- **`fixtures/map-fixtures.json`** & **`docs/examples/`**  
  _Peran_: Kumpulan data tiruan per fitur dan contoh kontrak lintas modul (tetap berada di folder docs/fixtures untuk pengujian & developer luar, tidak dijadikan runtime data).
- **`example_photo/formated/`**  
  _Peran_: Aset raster lokal (`bounds.json`, `drone_ortho.png` 256 MB).

---

### 1.2 Struktur Backend Baru Module 3 `[REVISI]`

Struktur lama `sp_measurement` yang mencampuradukkan geometri, AOI, dan hasil analitik dipecah menjadi submodul dengan tanggung jawab tunggal di bawah `src-tauri/src/modules/module_03/`:

```text
src-tauri/src/modules/module_03/
├── geometry/                # Logika murni matematika & validasi dasar GeoJSON (dipakai bersama)
│   ├── mod.rs
│   ├── haversine.rs         # calculate_distance
│   ├── spherical_excess.rs  # calculate_area, calculate_perimeter
│   └── validation.rs        # validate_coordinate, validate_polygon, validasi Point/Polygon/LineString
├── aoi/                     # Pengelolaan Area of Interest
│   ├── mod.rs
│   └── builder.rs           # create_aoi, create_aoi_cmd, get_sample_aoi
├── sp_result/               # Penanganan hasil spasial analitik & dispatcher event
│   ├── mod.rs
│   ├── dispatcher.rs        # dispatch_spatial_result, SPATIAL_RESULT_EVENT
│   └── reader.rs            # get_dummy_spatial_layers, pembaca spatial_results.json
├── layer.rs                 # Fungsi murni manipulasi state layer & anotasi
├── layer_commands.rs        # Tauri commands tipis untuk layer & anotasi
├── sample_data_commands.rs  # Tauri commands pembaca resource data terpusat (include_str!)
├── commands.rs              # Re-export & commands pengukuran spasial
├── types/                   # Kontrak data terpisah (cermin dari types frontend)
│   ├── mod.rs
│   ├── aoi_type.rs          # AoiFeature, AoiProperties, Coordinate, Polygon (cermin types/aoi.ts)
│   ├── sp_result_type.rs    # MapLayerCommand, LayerPayload, SpatialResult, SpatialProperties (cermin types/map.ts)
│   └── layer_type.rs        # LayerState, AnnotationFeature (cermin types/layer.ts)
├── error.rs                 # MapControllerError & pemetaan ke CommandError
└── mod.rs                   # Entry point modul & facade re-export
```

#### Aturan Ketergantungan Satu Arah `[REVISI]`

- `aoi` → bergantung pada `geometry` dan `types/aoi_type`.
- `sp_result` → bergantung pada `geometry` dan `types/sp_result_type`.
- `geometry` bersifat independen, **TIDAK BOLEH** bergantung pada `aoi` maupun `sp_result`.

---

### 1.3 Audit Ketergantungan & Re-Export Kompatibilitas `[REVISI]`

Hasil audit `grep -rn "map_controller\|sp_measurement" src-tauri/src`:

- **`src-tauri/src/lib.rs`**:
  - Baris 367: `mod map_controller;`
  - Baris 372: `use crate::map_controller::sp_measurement::spatial_rs::dummy::get_dummy_spatial_layers;`
  - Baris 427-436: Pendaftaran handler IPC:
    `map_controller::commands::measure_distance`, `measure_area`, `measure_perimeter`, `create_aoi_cmd`,
    `map_controller::layer_commands::toggle_layer_visibility_cmd`, `set_layer_opacity_cmd`, `reorder_layer_stack_cmd`, `filter_layers_by_category_cmd`, `create_annotation_cmd`.
- **`src-tauri/src/reporting/`**:
  - `RULES_REPORTING.md` dan `reporting/error.rs` hanya merujuk nama aturan `map_controller/RULES_MAP_CONTROLLER.md` dalam komentar dokumentasi; **tidak ada kode Rust yang mengimpor langsung**.
- **`src-tauri/src/modules/module_10/functions.rs`**:
  - Baris 17, 59-77: Mendefinisikan DTO `SpatialResultPayload` secara mandiri tanpa mengimpor `map_controller`. Struktur field JSON-nya kompatibel penuh dengan `LayerPayload`.

#### Jalur Re-Export Kompatibilitas (`pub use`)

Untuk menjamin tidak ada kode lama atau modul lain yang rusak, file `src-tauri/src/map_controller.rs` dipertahankan sebagai **facade re-export**:

```rust
//! Facade re-export untuk kompatibilitas penuh kode yang memanggil map_controller.
pub use crate::modules::module_03::*;

pub mod sp_measurement {
    pub use crate::modules::module_03::geometry;
    pub use crate::modules::module_03::aoi;
    pub use crate::modules::module_03::sp_result as spatial_rs;
    pub mod validation_input {
        pub use crate::modules::module_03::geometry::validation::*;
        pub use crate::modules::module_03::error::MapControllerError;
    }
}
```

---

## 2. Inventaris Data Template yang Tersebar

| ID      | Path:Baris                                                                           | Jenis Data                            | Bentuk Data                                      | Pemakai                                                                                 | Ada Data Acak?                                                            | Duplikasi & Inkonsistensi yang Ditemukan                                                                                                                                                               |
| :------ | :----------------------------------------------------------------------------------- | :------------------------------------ | :----------------------------------------------- | :-------------------------------------------------------------------------------------- | :------------------------------------------------------------------------ | :----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **D1**  | `src/modules/module-03/components/useImageMarkers.ts:5-18`                           | Marker Foto Drone                     | Array 200 item `DroneImageMetadata`              | `useImageMarkers` (`MapCanvas`, `ImageGallerySidebar`, `MapViewControlBar`)             | **YA** (`Math.random` untuk lat, lng, altitude, heading, dan seed picsum) | Dihasilkan secara dinamis di memori frontend setiap mount. Menempatkan marker di Balikpapan Selatan (`-1.247, 116.893`), tidak sesuai dengan lokasi orthomosaic asli di Kariangau (`-1.221, 116.859`). |
| **D2**  | `src/modules/module-03/components/LayerManager.tsx:13-54`                            | Daftar Layer Awal                     | Array 5 item `LayerState`                        | State awal `LayerManager.tsx`, diteruskan ke `MapCanvas.tsx`                            | Tidak (statis)                                                            | Duplikasi konsep dengan grup layer pada `map-fixtures.json:170-224`. Urutan z-index dan visibilitas berbeda.                                                                                           |
| **D3**  | `src/modules/module-03/components/MapCanvas.tsx:172-175, 194-197`                    | Koordinat Poligon Layer               | Koordinat cincin `[lat, lng][]`                  | Komponen `ManagedMapLayers` di `MapCanvas.tsx`                                          | Tidak (hardcoded)                                                         | Koordinat poligon untuk `plugin_veg_mask_01` dan `plugin_tree_detect_01` di-hardcode langsung di JSX, tidak sinkron dengan geometri yang ada di `dummy_sp_result.json`.                                |
| **D4**  | `src/modules/module-03/example_photo/formated/bounds.json:1-4` & `MapCanvas.tsx:138` | Batas Spasial Citra Orthomosaic       | `[[lat, lng], [lat, lng]]`                       | `ImageOverlay` di `MapCanvas.tsx`                                                       | Tidak (statis)                                                            | Koordinat bounds berada di Kariangau (`lat: -1.224, lng: 116.855`), yang merupakan **georeferensi asli orthomosaic**.                                                                                  |
| **D5**  | `src/modules/module-03/example_photo/formated/drone_ortho.png`                       | Citra Orthomosaic Asli                | File PNG biner (256 MB)                          | Asset mentah                                                                            | Tidak                                                                     | Ukuran file 256 MB tidak efisien jika dimasukkan ke bundle desktop.                                                                                                                                    |
| **D6**  | `src-tauri/src/modules/module_03/example_data/dummy_sp_result.json:1-179`            | Hasil Spasial Analitik Modul 4/7/8    | Array 4 `MapLayerCommand`                        | `spatial_rs.rs:68` (`get_dummy_spatial_layers`), dikonsumsi `useSpatialResultLayers.ts` | Tidak (statis)                                                            | Pada layer `result_drone_ortho_001` (baris 123), `image_url` mengarah ke URL usang `"http://localhost:1420/src/map/dummy_image/drone_ortho.png"` yang menghasilkan 404.                                |
| **D7**  | `src/modules/module-03/fixtures/map-fixtures.json:1-348`                             | Fixture Lengkap Fitur Modul 3         | JSON dictionary 9 fitur modul                    | Dokumentasi & pengujian frontend                                                        | Tidak (statis)                                                            | Koordinat di fixture ini berada di `lat: -1.268, lng: 116.831`. Tetap berada di folder fixtures untuk keperluan test.                                                                                  |
| **D8**  | `src/modules/module-03/docs/examples/aoi.example.geojson:1-23`                       | Contoh AOI Feature                    | GeoJSON `Feature<Polygon>`                       | Dokumentasi contoh lintas modul                                                         | Tidak (statis)                                                            | Contoh kontrak poligon valid untuk tim analitik.                                                                                                                                                       |
| **D9**  | `src/modules/module-03/docs/examples/aoi.invalid.example.geojson:1-348`              | Contoh AOI Tidak Valid                | JSON dictionary 348 baris                        | Dokumentasi contoh pengujian negatif                                                    | Tidak (statis)                                                            | **Inkonsistensi Kritis**: File ini secara keliru berisi salinan identik dari seluruh isi `map-fixtures.json`.                                                                                          |
| **D10** | `src/modules/module-03/docs/examples/spatial-result.example.json:1-26`               | Contoh Kontrak Hasil Spasial          | JSON `MapLayerCommand`                           | Dokumentasi kontrak untuk Modul 4/7/8                                                   | Tidak (statis)                                                            | Contoh payload hasil spasial yang diharapkan Module 3.                                                                                                                                                 |
| **D11** | `src/modules/module-03/docs/examples/plugin-execution-payload.example.json:1-26`     | Contoh Kontrak Payload Eksekusi       | JSON `MapLayerCommand`                           | Dokumentasi kontrak plugin Modul 2                                                      | Tidak (statis)                                                            | **Duplikasi 100%** dari `spatial-result.example.json`. Seharusnya berisi skema `ExecutionPayload` plugin.                                                                                              |
| **D12** | `src/modules/module-03/components/MapCanvas.tsx:323` & `MapViewControlBar.tsx:34`    | Titik Pusat Peta & Default Reset View | Koordinat `[-1.247, 116.893]` (zoom 16)          | Viewport awal dan handler tombol Reset View                                             | Tidak (statis)                                                            | Hardcoded di dua file terpisah pada koordinat Balikpapan Selatan.                                                                                                                                      |
| **D13** | `src/modules/module-03/components/AoiDemoToolbar.tsx:14-16`                          | ID Generator AOI Saat Runtime         | Fungsi `generateAoiId()` (`"aoi-" + Date.now()`) | Form input AOI ID baru                                                                  | Menggunakan `Date.now()` (efek samping waktu runtime)                     | Dipakai saat user membuat AOI baru secara interaktif.                                                                                                                                                  |
| **D14** | `src/modules/module-03/components/LayerManager.tsx:182, 188`                         | ID & Timestamp Anotasi Runtime        | String `anno_${Date.now()}` dan ISO string waktu | Handler form pembuatan anotasi baru                                                     | Menggunakan `Date.now()` (efek samping waktu runtime)                     | Dipakai saat user menambahkan catatan anotasi baru.                                                                                                                                                    |

---

## 3. Inventaris Kontrak I/O yang Harus Dipertahankan

Seluruh kontrak di bawah ini **TIDAK BOLEH BERUBAH** nama field, tipe, urutan properti, dan perilakunya:

### 3.1 Kontrak TypeScript (`src/modules/module-03/types/`)

#### A. Marker Foto Drone (`types/marker.ts`)

```typescript
export interface DroneImageMetadata {
  image_id: string;
  file_name: string;
  latitude: number;
  longitude: number;
  altitude_agl: number;
  heading_deg: number;
  thumbnail_url: string;
}
```

#### B. State Layer & Anotasi (`types/layer.ts`)

```typescript
export type LayerState = {
  layer_id: string;
  layer_name: string;
  category: string;
  is_visible: boolean;
  opacity: number;
  z_index: number;
};

export type AnnotationFeature = {
  annotation_id: string;
  text: string;
  geometry: SpatialGeometry;
  created_at: string;
};

export type SpatialGeometry =
  | { type: "Point"; coordinates: [number, number] }
  | { type: "Polygon"; coordinates: number[][][] }
  | { type: "LineString"; coordinates: [number, number][] };
```

#### C. Geometri & Pengukuran AOI (`types/aoi.ts`)

```typescript
export type Coordinate = [number, number]; // [longitude, latitude] WGS84
export type Polygon = Coordinate[][];

export interface AoiProperties {
  aoi_id: string;
  name: string;
  calculated_area_sq_m: number;
  perimeter_m: number;
  created_at: string;
}

export interface AoiGeometry {
  type: "Polygon";
  coordinates: Polygon;
}

export interface AoiFeature {
  type: "Feature";
  properties: AoiProperties;
  geometry: AoiGeometry;
}

export interface MeasurementResult {
  mode: "distance" | "area" | "perimeter";
  value: number;
  timestamp: string;
}
```

#### D. Kontrak Hasil Analitik Spasial (`types/map.ts`)

```typescript
export interface MapLayerCommand {
  action: string; // "load_spatial_result"
  payload: LayerPayload;
}

export interface LayerPayload {
  layer_id: string;
  layer_name: string;
  display_preference: LayerDisplayMode;
  spatial_results: SpatialResult[];
}

export type LayerDisplayMode =
  | { type: "polygon"; color: string; opacity: number }
  | { type: "point"; color: string; icon: string }
  | {
      type: "image_overlay";
      image_url: string;
      opacity: number;
      bounds: [[number, number], [number, number]];
    };

export interface SpatialResult {
  source: string;
  geometry: SpatialGeometry;
  properties: SpatialProperties;
}
```

### 3.2 Props Komponen Visual (UI Contract)

- **`MapCanvas`**: Tidak menerima props.
- **`MapLeftPanel`**: `{ activeTab: "layers" | "aoi" | "gallery", onTabChange: (tab) => void, onAnnotationsChange?: (annotations: AnnotationFeature[]) => void, onLayersChange?: (layers: LayerState[]) => void, map: L.Map | null }`
- **`LayerManager`**: `{ onAnnotationsChange?: (annotations: AnnotationFeature[]) => void, onLayersChange?: (layers: LayerState[]) => void }`
- **`AoiDemoToolbar`**: `{ map: L.Map | null }`
- **`DroneImageMarker`**: `{ image: DroneImageMetadata, opacity?: number }`
- **`ImagePopupCard`**: `{ image: DroneImageMetadata }`
- **`ImageGallerySidebar`**: Tidak menerima props.
- **`MapLegend`**: `{ layers: LayerPayload[] }`
- **`MapSearchBar`**: `{ onSearch?: (query: string, filter: string) => void }`
- **`MapTypeThumbnail`**: `{ currentType: "osm" | "satellite", onToggle: () => void }`
- **`MapViewControlBar`**: Tidak menerima props.

### 3.3 Signature Tauri IPC Commands

- `measure_distance(point_a: Coordinate, point_b: Coordinate) -> Result<f64, CommandError>`
- `measure_area(polygon: Polygon) -> Result<f64, CommandError>`
- `measure_perimeter(polygon: Polygon) -> Result<f64, CommandError>`
- `create_aoi_cmd(aoi_id: String, name: String, polygon: Polygon, created_at: String) -> Result<AoiFeature, CommandError>`
- `toggle_layer_visibility_cmd(layer: LayerState, is_visible: bool) -> Result<LayerState, CommandError>`
- `set_layer_opacity_cmd(layer: LayerState, opacity: f64) -> Result<LayerState, CommandError>`
- `reorder_layer_stack_cmd(layers: Vec<LayerState>, from_index: usize, to_index: usize) -> Result<Vec<LayerState>, CommandError>`
- `filter_layers_by_category_cmd(layers: Vec<LayerState>, category: String) -> Vec<LayerState>`
- `create_annotation_cmd(annotation_id: String, text: String, geometry: SpatialGeometry, created_at: String) -> AnnotationFeature`
- `get_dummy_spatial_layers() -> Result<Vec<MapLayerCommand>, String>`

---

## 4. Desain Penyatuan Data Template `[REVISI]`

### 4.1 Struktur Direktori Target di Backend

Seluruh data runtime Module 3 disatukan di:

```text
src-tauri/resources/sample-data/module3/
├── images/
│   └── drone_ortho.jpg          # Citra orthomosaic kompresi teroptimasi (1.2 MB, no-alpha)
├── markers.json                 # 200 data drone image marker deterministik hasil skrip ber-seed
├── layers.json                  # Konfigurasi 5 layer awal
├── aoi.json                     # Koleksi contoh poligon AOI valid di dalam bounds Kariangau
├── annotations.json             # Catatan inspeksi spasial awal di dalam bounds Kariangau
├── spatial_results.json         # Hasil analisis spasial dummy (Modul 4/7/8) di dalam bounds Kariangau
├── manifest.json                # Daftar berkas + hash SHA-256 + metadata tanggal & ukuran
└── README.md                    # Provenance data nyata OAM, spesifikasi data sintetis, lisensi
```

_Catatan Lokasi Fixture & Dokumen_:

- File `fixtures/map-fixtures.json` dipindahkan ke `tests/fixtures/map-fixtures.json` sebagai data uji unit test frontend, bukan data runtime.
- File `docs/examples/` tetap berada di folder dokumentasi sebagai acuan tim pengembang lintas modul.

### 4.2 Satu Sumber Kebenaran Pembacaan Data (`include_str!`) `[REVISI]`

Pendekatan hybrid dihapus. Seluruh data JSON dibaca secara **deterministik dan tanpa fallback** menggunakan `include_str!` saat kompilasi. Ini menjamin:

- 0% risiko kegagalan IO atau salah path runtime pada desktop release maupun mode web development.
- Data yang terbaca selalu identik dengan isi repository.

### 4.3 Pemuatan Gambar dari Resources & Asset Protocol `[REVISI]`

Webview Tauri memblokir akses file lokal langsung (`file://`). Untuk memuat `drone_ortho.jpg`:

- **Pilihan Arsitektur: Asset Protocol (`convertFileSrc`)**.
  - _Alasan_: Menghindari transfer buffer byte masif (~1.2 MB) lewat IPC IPC command yang membebani memori JavaScript. Asset protocol streaming langsung melalui HTTP engine internal Tauri dengan caching browser yang efisien.
  - _Konfigurasi_:
    - Di `src-tauri/tauri.conf.json`:
      ```json
      {
        "app": {
          "security": {
            "assetProtocol": {
              "enable": true,
              "scope": ["$RESOURCE/sample-data/module3/images/*"]
            }
          }
        },
        "bundle": {
          "resources": ["resources/sample-data/module3/images/*"]
        }
      }
      ```
  - _Perbaikan URL Usang_:
    - URL `http://localhost:1420/src/map/dummy_image/drone_ortho.png` pada `spatial_results.json` diubah menjadi path relatif aset terstandar: `"/drone_ortho.jpg"` (untuk mode Vite dev) atau diselesaikan via `convertFileSrc` saat runtime Tauri.

### 4.4 Command Tauri Pembaca Data Terpusat

1. `get_project_image_markers(count?: Option<usize>) -> Result<Vec<DroneImageMetadata>, CommandError>`
2. `get_initial_map_layers() -> Result<Vec<LayerState>, CommandError>`
3. `get_dummy_spatial_layers() -> Result<Vec<MapLayerCommand>, String>`
4. `get_sample_aoi() -> Result<Vec<AoiFeature>, CommandError>`
5. `get_sample_annotations() -> Result<Vec<AnnotationFeature>, CommandError>`

---

## 5. Fakta Provenance Citra Orthomosaic & Rencana Data `[REVISI-2]`

### 5.1 Verifikasi Bounds & Ekstrak Metadata GeoTIFF

Pemeriksaan langsung terhadap GeoTIFF asli `public/600fcc9055481e0005e312a1.tif` menggunakan `rasterio` menghasilkan:

- **CRS**: `EPSG:4326` (WGS84)
- **BoundingBox**:
  - `left (lng_min)`: `116.855973140003`
  - `bottom (lat_min)`: `-1.2243110280554836`
  - `right (lng_max)`: `116.863202751651`
  - `top (lat_max)`: `-1.2186297002884836`
- **Dimensi**: 16.352 × 12.767 pixel, 3 channels (RGB)
- **Nodata**: `None` (Area piksel penuh, tidak memiliki transparansi nodata).

#### Titik Acuan Tunggal Koordinat (Single Source of Truth)

Berdasarkan data georeferensi asli:

- **South-West**: `[-1.2243110280554836, 116.855973140003]` (lat, lng)
- **North-East**: `[-1.2186297002884836, 116.863202751651]` (lat, lng)
- **Center**:
  - `lat_center = (-1.2243110280554836 + -1.2186297002884836) / 2 = -1.221470364`
  - `lng_center = (116.855973140003 + 116.863202751651) / 2 = 116.859587946`
  - **Koordinat Tengah**: `[-1.221470, 116.859588]` pada Zoom 16.

**Keputusan Konsistensi**:

- Nilai pusat ini diekstrak menjadi satu konstanta tunggal di frontend: `DEFAULT_MAP_VIEW = { center: [-1.221470, 116.859588], zoom: 16, bounds: ... }`.
- `MapCanvas.tsx` dan tombol Reset View di `MapViewControlBar.tsx` wajib mengambil dari konstanta tunggal ini. **Tidak boleh ada dua koordinat terpisah**.
- Seluruh data marker, poligon layer hasil analitik, sample AOI, dan anotasi diselaraskan berada di dalam bounding box Kariangau ini.

### 5.2 Provenance Citra (`README.md` Data Sampel)

Dokumentasi pada `src-tauri/resources/sample-data/module3/README.md` memuat fakta jujur:

- **Sumber Data**: OpenAerialMap (OAM)
- **OAM Image ID**: `600fcc9055481e0005e312a1`
- **Tautan**: `https://map.openaerialmap.org/#/600fcc9055481e0005e312a1`
- **Lokasi Geografis**: Kariangau, Balikpapan, Kalimantan Timur, Indonesia
- **CRS Asli**: EPSG:4326 (WGS84)
- **Lisensi**: Creative Commons Attribution 4.0 International (CC-BY 4.0)
- **Status Transparansi**: Tidak ada nodata (`nodata: None`). Format JPEG 1.2 MB digunakan untuk efisiensi bundle tanpa mengorbankan kualitas visual.
- **Status Data**:
  - **Citra Orthomosaic**: Data spasial nyata (fotogrametri drone OpenAerialMap).
  - **Marker, AOI, Anotasi, Spatial Result**: Data sintetis deterministik yang dibangkitkan oleh skrip ber-seed di `scripts/`.

### 5.3 Skrip Pembangkit Data Marker Ber-seed (`scripts/generate_sample_markers.py`)

Skrip Python ber-seed (`seed = 42`) yang di-commit di `scripts/` untuk mereproduksi `markers.json`:

- Menghasilkan 200 titik dengan lintasan kisi terbang (_grid flight mission_) di dalam bounds Kariangau:
  - Latitud: rentang `-1.2240` s/d `-1.2190`
  - Longitud: rentang `116.8565` s/d `116.8625`
  - Altitude: 120.0 m tetap
  - Heading: 0° (lintasan ke utara) dan 180° (lintasan balik ke selatan)
  - Thumbnail: path aset lokal `/assets/thumbnails/drone_thumb.jpg` (bukan external picsum).
- Output: `src-tauri/resources/sample-data/module3/markers.json`.

### 5.4 Penanganan File Besar (GeoTIFF 40 MB & PNG 256 MB)

- Status Git saat ini:
  - `public/600fcc9055481e0005e312a1.tif` (40 MB, blob SHA `a65c293449...`) sudah ter-commit di repository.
  - `src/map/dummy_image/example.tif` (40 MB, identik) sudah ter-commit di repository.
  - `drone_ortho.png` (256 MB) berstatus _untracked_ (belum di-commit).
- **Usulan Pengelolaan**:
  1. _Jangka Pendek_: File 256 MB tetap dibiarkan di lokal dan diabaikan dari git (`.gitignore`). `drone_ortho.jpg` (1.2 MB) menjadi aset resmi di bundle resources.
  2. _Jangka Panjang_: File TIFF 40 MB dikeluarkan dari tracking git pada maintenance mendatang, digantikan tautan unduhan resmi OpenAerialMap + checksum SHA-256 dan skrip unduh di `scripts/download_sample_tif.py`.

### 5.5 Usulan Opsional: Ekstraksi Thumbnail dari Orthomosaic

- _Usulan_: Skrip `scripts/extract_marker_thumbnails.py` yang memotong crop 150×150 px dari citra orthomosaic pada masing-masing titik koordinat marker.
- _Estimasi Ukuran_: 200 thumbnail × ~12 KB = ~2.4 MB.
- _Risiko_: Menambah waktu proses kompilasi jika dijalankan otomatis.
- _Rekomendasi_: Bersifat opsional untuk rilis berikutnya. Untuk saat ini, galeri menggunakan thumbnail representatif statis lokal.

---

## 6. Audit Kemurnian Fungsi & Arsitektur "Functional Core, Imperative Shell" `[REVISI]`

### 6.1 Functional Core (Komputasi Murni — Tanpa Efek Samping, Input Sama → Output Sama)

- **Geometri (`modules/module_03/geometry/`)**:
  - `haversine::calculate_distance(p1, p2) -> f64`
  - `spherical_excess::calculate_area(poly) -> f64`
  - `spherical_excess::calculate_perimeter(poly) -> f64`
  - `validation::validate_coordinate(lon, lat) -> Result<(), CommandError>`
  - `validation::validate_polygon(poly) -> Result<(), CommandError>`
- **AOI (`modules/module_03/aoi/`)**:
  - `builder::create_aoi(id, name, polygon, created_at, area, perimeter) -> AoiFeature`
- **Layer & Anotasi (`modules/module_03/layer.rs`)**:
  - Seluruh fungsi diubah menjadi ekspresi murni tanpa mutasi argumen `mut layer`:
    ```rust
    pub fn toggle_layer_visibility(layer: LayerState, is_visible: bool) -> Result<LayerState, LayerError> {
        Ok(LayerState { is_visible, ..layer })
    }
    pub fn set_layer_opacity(layer: LayerState, opacity: f64) -> Result<LayerState, LayerError> {
        if !(0.0..=1.0).contains(&opacity) {
            return Err(LayerError::InvalidOpacity);
        }
        Ok(LayerState { opacity, ..layer })
    }
    ```
  - `reorder_layer_stack`: Menghasilkan `Vec<LayerState>` baru dengan pemetaan murni dan perhitungan ulang `z_index` tanpa mutasi in-place yang rawan bugs.
  - `create_annotation(id, text, geometry, created_at) -> AnnotationFeature`: Fungsi murni yang menerima ID dan timestamp **sebagai parameter**.

### 6.2 Imperative Shell (Lapisan Tepi)

- **Generator ID & Waktu Runtime**:
  - Generator `Date.now()` dan `new Date().toISOString()` berada secara ketat di event handler UI (`handleSave` di `AoiDemoToolbar.tsx` dan `handleCreateAnnotation` di `LayerManager.tsx`), lalu diserahkan sebagai argumen ke fungsi API. Fungsi inti tidak pernah memanggil `Date.now()`.
- **Tauri IPC Handlers**: Menangani serialisasi/deserialisasi, memanggil fungsi murni, dan mengembalikan `Result`.
- **React Hooks**: Mengelola state efek samping Leaflet dan asynchronous fetch.

---

## 7. Rencana Verifikasi Komprehensif `[REVISI]`

### 7.1 Karakterisasi Sebelum Refaktor

- Jalankan test Rust eksisting: `cargo test --manifest-path src-tauri/Cargo.toml`.
- Ambil dump JSON data acuan lama dari `INITIAL_LAYERS`, `dummy_sp_result.json`, dan koordinat bounds.

### 7.2 Verifikasi Pasca Refaktor (D.1 s/d D.7)

1. **D.1 - Deserialisasi Seluruh JSON Resource**:
   - Unit test Rust di `tests/sample_data_test.rs` yang membaca setiap file di `src-tauri/resources/sample-data/module3/` dan men-deserialisasikannya ke tipe struct masing-masing:
     - `markers.json` → `Vec<DroneImageMetadata>`
     - `layers.json` → `Vec<LayerState>`
     - `aoi.json` → `Vec<AoiFeature>`
     - `annotations.json` → `Vec<AnnotationFeature>`
     - `spatial_results.json` → `Vec<MapLayerCommand>`
2. **D.2 - Integritas Checksum SHA-256**:
   - Unit test Rust yang menghitung hash SHA-256 dari setiap berkas di `resources/sample-data/module3/` dan memvalidasi kecocokannya 100% dengan entri di `manifest.json`.
3. **D.3 - Validasi Konsistensi Geospasial Spasial**:
   - Unit test yang memvalidasi bahwa seluruh koordinat marker, poligon AOI, anotasi, dan layer analitik berada di dalam batas bounding box Kariangau (`lat: -1.224311..-1.218630`, `lng: 116.855973..116.863203`).
4. **D.4 - Uji Kompatibilitas Kontrak Module 10**:
   - Unit test di Module 3 yang menggunakan literal JSON contoh keluaran `get_change_area_result` (Modul 10.7) dan membuktikan bahwa payload tersebut dapat di-parse secara sempurna oleh struct `LayerPayload` dan `SpatialResult` tanpa error.
5. **D.5 - Uji Determinisme**:
   - Unit test yang memanggil command pembaca sample data dua kali berturut-turut dan memverifikasi bahwa balikan kedua panggilan tersebut identik bit-per-bit (tidak ada perbedaan urutan maupun data acak).
6. **D.6 - Validasi Schema File `docs/examples/`**:
   - Unit test / script yang memeriksa keabsahan `aoi.example.geojson`, `aoi.invalid.example.geojson`, `spatial-result.example.json`, dan `plugin-execution-payload.example.json` terhadap tipe TypeScript / Rust-nya.
7. **D.7 - Audit Penanganan ID & Zero Random Check**:
   - Audit `grep`:
     ```bash
     grep -rniE "Math\.random|rand::|random\.|faker" src/modules/module-03 src-tauri/src/modules/module_03 src-tauri/resources/sample-data/module3
     ```
     Wajib menghasilkan **0 match**.

---

## 8. Urutan Tahap & Rencana Commit `[REVISI]`

Pekerjaan dieksekusi secara berurutan dengan prinsip **satu commit per tahap** agar riwayat commit bersih, mudah ditinjau, dan aman di-revert:

1. **Commit 1: Karakterisasi Awal**
   - _Pesan_: `test(module-03): add characterization tests for geometry, layer, and data contracts`
   - Menambahkan test karakterisasi sebelum refaktor dan memastikan seluruh test hijau pada kode lama.
2. **Commit 2: Pemecahan Modul Backend `sp_measurement`**
   - _Pesan_: `refactor(module-03): split sp_measurement into geometry, aoi, and sp_result modules`
   - Memecah folder `sp_measurement` menjadi `geometry/`, `aoi/`, `sp_result/`, dan `types/`. Menambahkan re-export facade di `map_controller.rs`. Memastikan `cargo test` tetap hijau.
3. **Commit 3: Penyatuan Data Template & Command Pembaca**
   - _Pesan_: `feat(module-03): centralize sample data in backend and add reader commands`
   - Membuat `resources/sample-data/module3/` (`layers.json`, `aoi.json`, `annotations.json`, `spatial_results.json`, `manifest.json`, `README.md`). Menambahkan command pembaca via `include_str!`.
4. **Commit 4: Pembersihan Data Acak & Penyelarasan Koordinat Orthomosaic**
   - _Pesan_: `fix(module-03): replace random marker generator with deterministic sample data`
   - Menambahkan skrip pembangkit ber-seed `scripts/generate_sample_markers.py`, menghasilkan `markers.json`. Menghubungkan `useImageMarkers.ts` ke Tauri command. Menetapkan satu konstanta titik pusat peta Kariangau di frontend.
5. **Commit 5: Refaktor Fungsional Murni**
   - _Pesan_: `refactor(module-03): apply functional core imperative shell architecture`
   - Menghilangkan mutasi di `layer.rs`, merapikan helper transformasi koordinat di frontend, mengisolasi ID generator runtime di event handler.
6. **Commit 6: Verifikasi Akhir & Dokumentasi**
   - _Pesan_: `docs(module-03): update implementation record and verify all gates`
   - Memperbaiki `aoi.invalid.example.geojson` dan `plugin-execution-payload.example.json`, menjalankan seluruh gate pre-commit (lint, clippy, fmt, tsc, cargo test).

---

## 9. Daftar File yang Diubah / Dibuat / Dipindah / Dihapus

### 9.1 File yang Dibuat Baru

- `scripts/generate_sample_markers.py`
- `src-tauri/resources/sample-data/module3/manifest.json`
- `src-tauri/resources/sample-data/module3/README.md`
- `src-tauri/resources/sample-data/module3/images/drone_ortho.jpg`
- `src-tauri/resources/sample-data/module3/markers.json`
- `src-tauri/resources/sample-data/module3/layers.json`
- `src-tauri/resources/sample-data/module3/aoi.json`
- `src-tauri/resources/sample-data/module3/annotations.json`
- `src-tauri/resources/sample-data/module3/spatial_results.json`
- `src-tauri/src/modules/module_03/geometry/mod.rs`
- `src-tauri/src/modules/module_03/geometry/haversine.rs`
- `src-tauri/src/modules/module_03/geometry/spherical_excess.rs`
- `src-tauri/src/modules/module_03/geometry/validation.rs`
- `src-tauri/src/modules/module_03/aoi/mod.rs`
- `src-tauri/src/modules/module_03/aoi/builder.rs`
- `src-tauri/src/modules/module_03/sp_result/mod.rs`
- `src-tauri/src/modules/module_03/sp_result/dispatcher.rs`
- `src-tauri/src/modules/module_03/sp_result/reader.rs`
- `src-tauri/src/modules/module_03/types/aoi_type.rs`
- `src-tauri/src/modules/module_03/types/sp_result_type.rs`
- `src-tauri/src/modules/module_03/types/layer_type.rs`
- `src-tauri/src/modules/module_03/sample_data_commands.rs`
- `src-tauri/tests/sample_data_test.rs`
- `src/modules/module-03/config/mapConfig.ts` (Titik pusat tunggal koordinat Kariangau)

### 9.2 File yang Diubah

- `src-tauri/tauri.conf.json`
- `src-tauri/src/lib.rs`
- `src-tauri/src/map_controller.rs` (Facade re-export)
- `src-tauri/src/modules/mod.rs`
- `src-tauri/src/modules/module_03/layer.rs`
- `src-tauri/src/modules/module_03/layer_commands.rs`
- `src-tauri/capabilities/default.json`
- `src/modules/module-03/components/useImageMarkers.ts`
- `src/modules/module-03/components/useSpatialResultLayers.ts`
- `src/modules/module-03/components/LayerManager.tsx`
- `src/modules/module-03/components/MapCanvas.tsx`
- `src/modules/module-03/components/MapViewControlBar.tsx`
- `src/modules/module-03/api.ts`
- `src/modules/module-03/docs/examples/aoi.invalid.example.geojson` (Perbaikan isi poligon tidak valid)
- `src/modules/module-03/docs/examples/plugin-execution-payload.example.json` (Diferensiasi kontrak payload plugin)

### 9.3 File yang Dipindah / Dihapus

- `src-tauri/src/modules/module_03/sp_measurement/` (Dihapus setelah dipindahkan secara modular ke `geometry/`, `aoi/`, `sp_result/`)
- `src-tauri/src/modules/module_03/types/sp_measurement_type.rs` (Dihapus setelah dipecah ke `aoi_type.rs`, `sp_result_type.rs`, `layer_type.rs`)
- `src-tauri/src/modules/module_03/example_data/dummy_sp_result.json` (Dilebur ke `spatial_results.json`)
- `src/modules/module-03/fixtures/map-fixtures.json` (Dipindahkan ke `tests/fixtures/map-fixtures.json` untuk data uji unit test)

---

## 10. Catatan Hasil Eksekusi & Status Akhir

1. **Tahap 1 (Karakterisasi Awal)**:
   - Commit `2d90b5d`: `test(module-03): add characterization tests for geometry, layer, and data contracts`
   - Berhasil mengunci baseline perilaku via 6 characterization test.
2. **Tahap 2 (Pemecahan Modul Backend `sp_measurement`)**:
   - Commit `8282fb3`: `refactor(module-03): split sp_measurement into geometry, aoi, and sp_result modules`
   - Berhasil memecah ke `geometry/`, `aoi/`, `sp_result/`, `types/` dengan facade kompatibilitas re-export penuh.
3. **Tahap 3 (Penyatuan Data Template & Command Pembaca)**:
   - Commit `24aaa7f`: `feat(module-03): centralize sample data in backend and add reader commands`
   - Berhasil memusatkan data sampel di `src-tauri/resources/sample-data/module3/` dan menambahkan reader commands `include_str!`.
4. **Tahap 4 (Pembersihan Data Acak & Koordinat Orthomosaic Kariangau)**:
   - Commit `76f5b87`: `fix(module-03): replace random marker generator with deterministic sample data`
   - Skrip deterministik ber-seed `scripts/generate_sample_markers.py`, konstanta tunggal `DEFAULT_MAP_VIEW`, eliminasi penuh `Math.random`.
5. **Tahap 5 (Refaktor Fungsional Murni)**:
   - Commit `193ab5e`: `refactor(module-03): apply functional core imperative shell architecture`
   - Pure record updates pada operasi layer, isolasi efek samping waktu/ID di imperative shell.
6. **Tahap 6 (Verifikasi Akhir & Dokumentasi)**:
   - 52/52 test Rust lulus (35 unit + 6 characterization + 5 sample data/contract + 6 doctests).
   - Validasi D.1 s/d D.7 seluruhnya terverifikasi lulus 100%.
   - Audit `grep` data acak menghasilkan **0 match**.

**STATUS**: SELESAI PENUH (Semua tahap berhasil dieksekusi, atomic commit tersusun rapi, dan seluruh pre-commit gates hijau).
