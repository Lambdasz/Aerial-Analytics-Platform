# AUDIT & REFACTORING ARCHITECTURE REPORT: MODULE 03 (AERIAL MAP)

**Role**: Senior Code Auditor & Refactoring Specialist (Functional Programming & Atomic Architecture)  
**Target Modul**: Modul 03 — Aerial Image Map Explorer (Visualisasi Map & Orthomosaic, AOI, Spatial Results, Spatial Annotations)  
**Status**: DRAFT AUDIT REPORT — _Menunggu Konfirmasi Eksekusi_  
**Tanggal**: 24 Oktober 2026

---

## 1. Ringkasan Eksekutif

Audit komprehensif dilakukan terhadap seluruh berkas Modul 03, baik pada frontend React/TypeScript (`src/modules/module-03/`) maupun backend Rust Tauri (`src-tauri/src/modules/module_03/`). Pemindaian difokuskan pada pemenuhan prinsip **Functional Programming (FP)**, **Atomic Architecture**, efisiensi I/O Tauri IPC, integritas penyimpanan data `map_storage`, dan pembersihan residu kode/boilerplate yang terbengkalai.

### Matriks Kuantitatif Temuan

| Kategori Audit                             | Jumlah Temuan | Estimasi LOC Terdampak | Potensi Pengurangan Kompleksitas                                                     |
| :----------------------------------------- | :-----------: | :--------------------: | :----------------------------------------------------------------------------------- |
| **1. Dead Code & Unused Functions**        |   12 Temuan   |      ~1.150 baris      | Menghapus file orphan, hook tidak terpakai, dan command IPC mati.                    |
| **2. Useless / Redundant Validations**     |   4 Temuan    |       ~280 baris       | Mengeliminasi validasi ganda, double round-trip IPC, dan fallback dummy.             |
| **3. Template / Dummy `map_storage`**      |   4 Temuan    |       ~640 baris       | Menghentikan pemborosan 4x IPC fetch foto kosong & cache memory redundan.            |
| **4. FP & Atomic Architecture Violations** |   5 Temuan    |      ~3.100 baris      | Memecah 2 organisme monolitik raksasa (~800 baris) dan 1 file backend (1.542 baris). |
| **TOTAL**                                  | **25 Temuan** |    **~5.170 baris**    | **Dekomposisi modular, reduksi beban CPU/RAM, dan arsitektur murni.**                |

### Estimasi Dampak Positif

1. **Performa Startup & Runtime**: Menghilangkan 4 pemanggilan paralel `useImageMarkers(500)` dengan timeout 3.000 ms yang selama ini membidik direktori foto kosong.
2. **Efisiensi Tauri IPC**: Menghapus round-trip IPC yang hanya melakukan mutasi boolean/angka sederhana (`toggle_layer_visibility`, `set_layer_opacity`, `create_aoi` lalu `save_aoi`), beralih ke state immutable fungsional di client dan single-action persistence.
3. **Pemberantasan Residu Dummy**: Menghapus sisa fallback gambar hijau `"/sample_drone.jpg"` yang bertentangan dengan standar clean no-image.
4. **Pembersihan Backend Rust**: Menghapus file catatan personal mahasiswa (`logperson1.md`), panduan usang (`RULES_MAP_CONTROLLER.md`), modul kosong stub (`type.rs`), dan file duplikat identik 100% (`aoi/pure.rs`).

---

## 2. Tabel Matriks Temuan

| ID        | Lokasi (File & Line)                                                                                                          | Kategori             | Alasan Temuan                                                                                                                                                                                     | Rekomendasi (Hapus/Refaktor FP)                                                                | Risiko                   |
| :-------- | :---------------------------------------------------------------------------------------------------------------------------- | :------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :--------------------------------------------------------------------------------------------- | :----------------------- |
| **DC-01** | `src/modules/module-03/hooks/useMeasurement.ts` (L1-175)                                                                      | Dead Code            | Hook `useMeasurement` tidak diimpor atau dipanggil oleh komponen mana pun di seluruh project.                                                                                                     | Hapus berkas.                                                                                  | Isolated                 |
| **DC-02** | `src/modules/module-03/hooks/useAnnotations.ts` (L1-124)                                                                      | Dead Code            | Hook CRUD anotasi tidak pernah dipakai karena `LayerManager.tsx` menulis state inline sendiri.                                                                                                    | Integrasikan ke `LayerManager` atau hapus jika LayerManager dipecah.                           | Isolated                 |
| **DC-03** | `src/modules/module-03/organisms/ManagedMapLayers.tsx` (L32-52), `DroneImageMarker.tsx` (L1-44), `ImagePopupCard.tsx` (L1-94) | Dead Code            | Kondisi `layer.layer_id === "flight_session_01"` adalah hardcode dari mock fixture lama yang tidak pernah bernilai true di runtime. Komponen marker foto drone tidak pernah di-mount.             | Hapus komponen atau dekomposisi selektif jika fitur drone capture marker diaktifkan resmi.     | Isolated                 |
| **DC-04** | `src/modules/module-03/organisms/MapSyncHandler.tsx` (L1-26)                                                                  | Dead Code            | Komponen invisible yang mencari marker foto drone (selalu kosong) untuk `flyTo`, padahal kartu galeri sudah memanggil `map.flyTo` secara mandiri.                                                 | Hapus komponen `MapSyncHandler`.                                                               | Isolated                 |
| **DC-05** | `src/modules/module-03/molecules/MapSearchBar.tsx` (L5, L51-54)                                                               | Dead Code            | State `query` diinput user namun tidak dioper ke parent, tidak memfilter data apa pun (pure cosmetic dummy).                                                                                      | Hubungkan prop `onSearch` atau bersihkan local state mati.                                     | Isolated                 |
| **DC-06** | `src/modules/module-03/services/api.ts` (L132-152, L214-223)                                                                  | Dead Code            | `reorderLayerStack` tidak pernah dipanggil. `filterLayersByCategory` dipanggil di `LayerManager:156` tetapi hasil return-nya langsung dibuang/diabaikan. `getSavedLayers` tidak pernah dipanggil. | Hapus wrapper API mati dan hapus pemanggilan sia-sia di `LayerManager`.                        | Isolated                 |
| **DC-07** | `src-tauri/src/modules/module_03/type.rs` (L1-7)                                                                              | Dead Code            | File stub kosong berukuran 230 byte yang hanya berisi `#![allow(dead_code)]`.                                                                                                                     | Hapus berkas dan unregister dari `mod.rs`.                                                     | Isolated                 |
| **DC-08** | `src-tauri/src/modules/module_03/aoi/pure.rs` (L1-56)                                                                         | Dead Code            | File ini adalah 100% klon duplikat kata-per-kata dari `aoi/builder.rs`. Modul `aoi/mod.rs` me-reexport dari `builder.rs`.                                                                         | Hapus salah satu berkas (`pure.rs` atau `builder.rs`) dan rapikan ekspor.                      | Isolated                 |
| **DC-09** | `src-tauri/src/modules/module_03/layers/commands.rs` (L46-60) & `pure.rs` (L25-58)                                            | Dead Code            | IPC command `reorder_layer_stack_cmd` dan `filter_layers_by_category_cmd` beserta fungsi pure Rust-nya tidak memiliki konsumen fungsional di frontend.                                            | Hapus command dari IPC registration `lib.rs` & default.json.                                   | Breaking (IPC)           |
| **DC-10** | `src-tauri/src/modules/module_03/logperson1.md` (L1-159) & `RULES_MAP_CONTROLLER.md` (L1-132)                                 | Dead Code            | Berkas markdown log kerja personal dan panduan arsitektur usang tersimpan keliru di dalam folder source code Rust backend.                                                                        | Pindahkan ke `docs/` atau hapus dari backend `src-tauri/`.                                     | Isolated                 |
| **DC-11** | `src/modules/module-03/config/mapConfig.ts` (L75-84)                                                                          | Dead Code            | Definisi basemap `positron` membutuhkan API key yang tidak ada, dan UI toggle basemap di-hardcode hanya OSM ↔ Satellite.                                                                          | Hapus entry basemap `positron` yang mati.                                                      | Isolated                 |
| **DC-12** | `src/modules/module-03/index.ts` (L11, L17, L30)                                                                              | Dead Code            | Re-export `AoiDemoToolbar` (alias tak terpakai), `buildSpatialResultPopupHtml` (alias identik `buildPopupHtml`), dan `DroneImageMetadata` (tidak sinkron backend).                                | Bersihkan re-export tanpa merusak public signature modul lain.                                 | Isolated                 |
| **RV-01** | `src-tauri/src/modules/module_03/geometry/validation.rs` (L62-88 vs L102-136)                                                 | Redundant Validation | `validate_polygon` dan `SpatialGeometry::validate` menduplikasi pengecekan ring (<4, unclosed), namun `SpatialGeometry` tidak memvalidasi range WGS84 koordinatnya.                               | Satukan ke satu fungsi validasi geometri murni yang deterministik.                             | Isolated                 |
| **RV-02** | `src-tauri/src/modules/module_03/aoi/commands.rs` (L10-31, L35-37) & `useAOI.ts` (L199-207)                                   | Redundant Validation | Alur simpan AOI melakukan 2x IPC round-trip: `create_aoi_cmd` (validasi & hitung metrik) lalu `save_aoi` (validasi ulang & tulis disk).                                                           | Gabungkan ke dalam satu command atomic `save_aoi` yang menerima draft polygon.                 | Breaking (Frontend Hook) |
| **RV-03** | `src/modules/module-03/lib/leaflet/spatialLayerBuilder.ts` (L30-61, L73-77, L178-201)                                         | Redundant Validation | `isValidWgs84Bounds` dipanggil berulang-ulang pada setiap render overlay, padahal bounds sudah divalidasi ketat saat proses impor citra.                                                          | Jadikan validasi guard saat import/ingest, gunakan tipe terpercaya di layer builder.           | Isolated                 |
| **RV-04** | `src/modules/module-03/molecules/OrthoCardItem.tsx` (L93, L97-98) & `ImageGallerySidebar.tsx` (L416-418)                      | Redundant Validation | Fallback error `onError` menimpa gambar ke `"/sample_drone.jpg"` (gambar hijau dummy yang sebelumnya diminta dihapus).                                                                            | Ganti fallback ke komponen `ResultPhoto` (No Image state).                                     | Isolated                 |
| **DS-01** | `map_storage/photos/` & `src-tauri/src/modules/module_03/photos/store.rs` & `useImageMarkers.ts`                              | Dummy Storage        | Direktori `map_storage/photos/` kosong. 4 komponen memanggil `useImageMarkers(500)` dengan timeout 3 detik secara paralel, menghasilkan 4 IPC sia-sia.                                            | Satukan ke single hook terpusat atau nonaktifkan polling jika foto tidak di-load.              | Isolated                 |
| **DS-02** | `src-tauri/src/modules/module_03/state.rs` (L23-145)                                                                          | Dummy Storage        | `Module03State` membuat in-memory cache `Arc<RwLock<HashMap<...>>>` untuk 6 entitas, tetapi submodul `photos` & `layers` langsung membaca `fs::read_dir`, mengabaikan state ini.                  | Simplifikasi state in-memory atau jadikan penyimpanan disk-first murni tanpa cache ganda.      | Isolated                 |
| **DS-03** | `map_storage/layers/` & `src-tauri/src/modules/module_03/layers/store.rs`                                                     | Dummy Storage        | Direktori `map_storage/layers/` kosong. UI menyimpan citra di `images/`, AOI di `aois/`, dan hasil di `spatial_results/`. File layer JSON individual diabaikan.                                   | Evaluasi kebutuhan entitas layer persisten vs layer dinamis yang di-generate dari data sumber. | Isolated                 |
| **DS-04** | `src/modules/module-03/store/ImageSyncContext.tsx` (L1-37)                                                                    | Dummy Storage        | Konteks state `activeImageId` dan `interactionSource` membungkus seluruh tree, tetapi aksi sinkronisasi peta sudah di-handle direct callback di UI.                                               | Ringkaskan state menjadi state lokal atau URL query ephemeral.                                 | Isolated                 |
| **FP-01** | `src/modules/module-03/organisms/AoiToolbar.tsx` (L1-797)                                                                     | FP/Atomic Violation  | Komponen monolitik 797 baris menggabungkan UI Blueprint, state form, event imperatif Geoman, kalkulasi metrik, dan IPC Tauri.                                                                     | Pecah menjadi: `AoiFormMolecule`, `AoiListMolecule`, dan `useAoiLifecycle` hook.               | Isolated                 |
| **FP-02** | `src/modules/module-03/organisms/LayerManager.tsx` (L1-790)                                                                   | FP/Atomic Violation  | Komponen monolitik 790 baris memuat form anotasi titik/garis/poligon, kalkulasi area Shoelace manual di client, dan rendering list.                                                               | Ekstrak form anotasi menjadi `AnnotationFormModal` dan gunakan hook `useAnnotations`.          | Isolated                 |
| **FP-03** | `src-tauri/src/modules/module_03/sp_result/store.rs` (L1-1542)                                                                | FP/Atomic Violation  | File monolitik 1.542 baris menggabungkan parsing file, patching JSON, karantina folder `.trash`, rekursi cascade delete, dan resolusi foto.                                                       | Dekomposisi fungsional: pisahkan `reader.rs`, `writer.rs`, `patch.rs`, dan `cleanup.rs`.       | Isolated                 |
| **FP-04** | `src/modules/module-03/hooks/useSpatialResultLayers.ts` (L40, L54-68, L74-140)                                                | FP/Atomic Violation  | Hook memutasi Leaflet `map.addLayer`/`map.removeLayer` dan objek Leaflet mutable di dalam `useRef`, alih-alih declarative rendering.                                                              | Pisahkan kalkulasi layer payload (pure) dari adapter efek Leaflet imperative.                  | Isolated                 |
| **FP-05** | `src/modules/module-03/organisms/LayerManager.tsx` (L123-147) & `layers/commands.rs` (L32-44)                                 | FP/Atomic Violation  | Frontend mengirim seluruh objek `LayerState` ke backend Rust via IPC hanya untuk mengubah `is_visible` dan `opacity`.                                                                             | Lakukan update fungsional immutable langsung di React client (`map` array).                    | Breaking (IPC Layer)     |

---

## 3. Detail Analisis Temuan

### A. Dead Code & Unused Hooks

#### 1. Kasus `useMeasurement.ts` (Dead Hook)

- **Lokasi**: [src/modules/module-03/hooks/useMeasurement.ts](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/hooks/useMeasurement.ts#L1-L175)
- **Analisis**: Berkas ini mendefinisikan hook sepanjang 175 baris dengan API lengkap: `startMeasureDistance`, `startMeasureArea`, `startMeasurePerimeter`, dsb. Namun, tidak ada satu pun komponen React (`AoiToolbar`, `MapViewControlBar`, `MapLeftPanel`) yang mengimpor atau menggunakan hook ini. Pengukuran geometri AOI dan layer saat ini dilakukan di dalam `useAOI.ts` dan fungsi pure `geoJson.ts`.
- **Rekomendasi**: Hapus file `useMeasurement.ts` untuk menghemat bundle dan mengurangi beban maintenance.

#### 2. Kasus `ManagedMapLayers.tsx` & `DroneImageMarker.tsx` (Dead Render Tree)

- **Lokasi**: [src/modules/module-03/organisms/ManagedMapLayers.tsx#L32-L49](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/ManagedMapLayers.tsx#L32-L49)
- **Snippet Lama**:

```tsx
// ManagedMapLayers.tsx:32-49
if (layer.layer_id === "flight_session_01") {
  if (currentZoom < minZoom) return null;
  return (
    <React.Fragment key={layer.layer_id}>
      {markers.map((marker) => (
        <DroneImageMarker
          key={`${layer.layer_id}-${marker.image_id}`}
          image={marker}
          opacity={layer.opacity}
        />
      ))}
    </React.Fragment>
  );
}
return null;
```

- **Masalah**: Layer ID `"flight_session_01"` adalah artefak peninggalan mock sprint awal yang hanya ada di `tests/fixtures/map-fixtures.json`. Di database nyata (`map_storage/layers/` dan `map_storage/photos/`), layer ini tidak pernah ada. Akibatnya, `ManagedMapLayers` selalu me-return `null`, dan komponen `DroneImageMarker` serta `ImagePopupCard` tidak pernah dieksekusi di runtime.
- **Rancangan Fungsional Bersih**:
  Jika penanda foto drone tidak menjadi kebutuhan utama saat ini, hapus `ManagedMapLayers`, `DroneImageMarker`, dan `ImagePopupCard`. Jika di kemudian hari foto drone ingin ditampilkan, buat deklarasi layer fungsional berbasis tipe data nyata:

```tsx
export const DronePhotoLayer: React.FC<{
  photos: ReadonlyArray<PhotoMetadata>;
  isVisible: boolean;
}> = ({ photos, isVisible }) => {
  const currentZoom = useMapZoom();
  if (!isVisible || currentZoom < MIN_POINT_FEATURE_ZOOM) return null;
  return (
    <>
      {photos.map((p) => (
        <CircleMarker key={p.id} center={[p.latitude, p.longitude]} radius={5} />
      ))}
    </>
  );
};
```

---

### B. Useless & Redundant Validations

#### 1. Validasi Ganda & Double IPC Round-Trip pada Pembuatan AOI

- **Lokasi**: [src/modules/module-03/hooks/useAOI.ts#L199-L208](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/hooks/useAOI.ts#L199-L208) vs [src-tauri/src/modules/module_03/aoi/commands.rs#L10-L37](file:///d:/project/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/aoi/commands.rs#L10-L37)
- **Snippet Lama (2x IPC Call)**:

```typescript
// useAOI.ts
const feature = await createAoi({
  aoiId,
  name,
  aoiType,
  polygon: currentPolygon,
  createdAt: new Date().toISOString(),
}); // IPC CALL 1 -> create_aoi_cmd (validate_polygon + calculate_area)

await saveAoiToStorage(feature); // IPC CALL 2 -> save_aoi (validate_polygon lagi + write_atomic)
```

- **Masalah**: Frontend memanggil `create_aoi_cmd` hanya untuk menghitung metrik geometri dan membungkus properti GeoJSON, lalu langsung memanggil `save_aoi` yang menjalankan validasi dan menyimpan file. Ini menimbulkan overhead ganda serialisasi JSON dan 2x latensi bridge Tauri IPC untuk satu aksi simpan pengguna.
- **Rancangan Fungsional Bersih (1 Atomic IPC Call)**:
  Satukan pembuatan dan penyimpanan ke dalam command atomic fungsional di Rust:

```rust
// src-tauri/src/modules/module_03/aoi/commands.rs
#[tauri::command]
pub fn save_new_aoi_cmd(
    aoi_id: String,
    name: String,
    aoi_type: Option<String>,
    polygon: Polygon,
) -> Result<AoiFeature, CommandError> {
    geometry::validate_polygon(&polygon)?;
    let area_sq_m = geometry::calculate_area(&polygon);
    let perimeter_m = geometry::calculate_perimeter(&polygon);
    let feature = aoi::create_aoi(
        aoi_id,
        name,
        aoi_type,
        polygon,
        chrono::Utc::now().to_rfc3339(),
        area_sq_m,
        perimeter_m,
    );
    Ok(aoi::store::save_aoi(feature)?)
}
```

#### 2. Inkoherensi Validasi Geometri (`validation.rs` vs `sp_result_type.rs`)

- **Lokasi**: [src-tauri/src/modules/module_03/geometry/validation.rs#L62-L88](file:///d:/project/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/validation.rs#L62-L88) dan [validation.rs#L102-L136](file:///d:/project/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/geometry/validation.rs#L102-L136)
- **Masalah**: Terdapat dua parser validasi poligon:
  1. `validate_polygon(&Polygon)`: memeriksa cincin kosong, panjang < 4, `ring.first() == ring.last()`, dan memanggil `validate_coordinate(lon, lat)` (-180..180, -90..90).
  2. `SpatialGeometry::validate(&self)`: memeriksa panjang ring, titik pertama sama dengan terakhir, tetapi **TIDAK** memvalidasi rentang koordinat WGS84 numerik.
- **Rekomendasi**: Delegasikan validasi `SpatialGeometry::Polygon` langsung ke `validate_polygon` murni agar terhindar dari _logic drift_.

---

### C. Analisis Template / Dummy `map_storage`

#### 1. Direktori `photos/` Kosong & Pemborosan 4x IPC `get_photos`

- **Lokasi**:
  - `map_storage/photos/raw/` (Kosong)
  - `map_storage/photos/thumbnails/` (Kosong)
  - `src/modules/module-03/hooks/useImageMarkers.ts:9`
  - Dipanggil paralel oleh: `MapViewControlBar.tsx:13`, `MapSyncHandler.tsx:9`, `MapCanvas.tsx:40`, `ImageGallerySidebar.tsx:30`.
- **Snippet Lama**:

```typescript
// useImageMarkers.ts
export const useImageMarkers = (count: number = 200) => {
  const [markers, setMarkers] = useState<ReadonlyArray<DroneImageMetadata>>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const timer = setTimeout(() => setIsLoading(false), 3000); // 3-second safety timer
    getProjectImageMarkers() // -> invokeMap("get_photos")
      .then((data) => {
        setMarkers(data.slice(0, count));
        setIsLoading(false);
      });
    return () => clearTimeout(timer);
  }, [count]);
  return { markers, isLoading };
};
```

- **Masalah**:
  1. Frontend mengharapkan tipe `DroneImageMetadata` (`image_id`, `thumbnail_url`), sedangkan backend Rust `PhotoMetadata` mengembalikan (`id`, `thumbnail_path`). Keduanya tidak pernah cocok (type mismatch).
  2. Karena folder `photos/` kosong, `get_photos` selalu mereturn `[]`.
  3. Setiap kali pengguna membuka halaman Modul 03, 4 komponen secara mandiri mengeksekusi IPC `get_photos`, menyalakan timer 3 detik, dan mengalokasikan memori untuk data yang tidak ada.
- **Rancangan Fungsional Bersih**:
  Jika foto EXIF drone belum diintegrasikan, hentikan pemanggilan `useImageMarkers(500)` di `MapViewControlBar`, `MapSyncHandler`, `MapCanvas`, dan `ImageGallerySidebar`. Ganti dengan state statis kosong atau jadikan parameter opsional di parent root (`MapExplorer`).

#### 2. Redundansi In-Memory State Cache di `src-tauri/state.rs`

- **Lokasi**: [src-tauri/src/modules/module_03/state.rs#L23-L38](file:///d:/project/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/state.rs#L23-L38)
- **Masalah**:
  - `state.rs` memiliki `Module03State` dengan 6 `Arc<RwLock<HashMap<String, ...>>>` yang dibaca ulang saat inisialisasi (`refresh_from_disk`).
  - Namun pada kenyataannya, modul `photos/store.rs` dan `layers/store.rs` melakukan direct filesystem read (`fs::read_dir`) dan tidak membaca dari `state.rs`.
  - Sementara untuk `sp_result/store.rs`, pembacaan file dilakukan dengan arsitektur dual-reader disk-first.
  - Memelihara in-memory cache dengan kunci string terpisah untuk dataset desktop lokal yang ukurannya hanya hitungan kilobyte menambah kompleksitas concurrency (`RwLock`) tanpa memberikan manfaat performa yang nyata.

---

### D. Pelanggaran Functional Programming & Atomic Architecture

#### 1. Organisme Monolitik Raksasa: `AoiToolbar.tsx` (797 Baris)

- **Lokasi**: [src/modules/module-03/organisms/AoiToolbar.tsx](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/AoiToolbar.tsx)
- **Pelanggaran**:
  - Menggabungkan 5 tanggung jawab: (1) Form pembuatan AOI, (2) List AOI dengan status filter, (3) Inline editing metadata AOI, (4) Event binding imperatif Leaflet Geoman, (5) Integrasi Tauri API CRUD.
- **Rancangan Dekomposisi Atomik**:
  - `atoms/AoiStatusTag.tsx`: Tag penanda tipe AOI.
  - `molecules/AoiCardItem.tsx`: Kartu render tiap item AOI dengan tombol aksi (fokus, edit, hapus).
  - `molecules/AoiCreateForm.tsx`: Formulir input nama dan tipe AOI.
  - `hooks/useAoiDrawing.ts`: Hook murni penanganan interaksi Geoman pada peta.
  - `organisms/AoiToolbar.tsx`: Organisme murni yang merekatkan molekul-molekul di atas secara deklaratif (<150 baris).

#### 2. Organisme Monolitik Raksasa: `LayerManager.tsx` (790 Baris)

- **Lokasi**: [src/modules/module-03/organisms/LayerManager.tsx](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/LayerManager.tsx)
- **Pelanggaran**:
  - File ini memuat seluruh form anotasi (titik, garis, poligon), parsing string koordinat textarea, kalkulasi geodesik di browser, listener Geoman, serta daftar kontrol opacity/visibility layer.
  - Terdapat pemanggilan IPC yang tidak perlu:

```typescript
// LayerManager.tsx:125
const updatedLayer = await toggleLayerVisibility(layer, !layer.is_visible);
```

- **Rancangan Fungsional Bersih (Immutable React Update)**:
  Status visibility layer tampilan tidak perlu dikirim ke Rust:

```typescript
// Pure immutable update di React
const handleToggleVisibility = (layerId: string) => {
  setLayers((prev) =>
    prev.map((l) => (l.layer_id === layerId ? { ...l, is_visible: !l.is_visible } : l)),
  );
};
```

#### 3. Backend Monolitik: `sp_result/store.rs` (1.542 Baris, 62 KB)

- **Lokasi**: [src-tauri/src/modules/module_03/sp_result/store.rs](file:///d:/project/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/sp_result/store.rs)
- **Pelanggaran**:
  - Satu file Rust memuat lebih dari 1.500 baris kode yang mencakup logika dual-reader (flat vs decomposed folder), resolusi URL foto, quarantine cleanup `.trash`, penghitungan cascade dependents, patching JSON properti fitur, dan unit test.
- **Rancangan Fungsional Bersih**:
  - Pecah submodul `sp_result/` menjadi:
    - `reader.rs`: Pembacaan flat file & folder decomposed (pure read).
    - `writer.rs`: Penulisan atomic file dan folder structure.
    - `patch.rs`: Logika pure transformation untuk update field dan properti fitur.
    - `quarantine.rs`: Penanganan pemulihan dan pembersihan crash `.trash`.

---

## 4. Action Plan & Safe-Deletion Checklist

Berikut adalah daftar checklist terurut langkah demi langkah yang dirancang untuk dieksekusi secara aman tanpa menyebabkan efek samping (_breaking changes_) pada modul lain (termasuk Modul 10):

### Tahap 1: Pembersihan Dead Code & File Orphan (Zero Risk)

- [x] **1.1** Hapus file stub kosong [src-tauri/src/modules/module_03/type.rs](file:///d:/project/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/type.rs) dan hapus baris `pub mod r#type;` di [src-tauri/src/modules/module_03/mod.rs](file:///d:/project/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/mod.rs#L19). _(SELESAI)_
- [x] **1.2** Hapus file catatan personal [src-tauri/src/modules/module_03/logperson1.md](file:///d:/project/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/logperson1.md) dan panduan usang [RULES_MAP_CONTROLLER.md](file:///d:/project/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/RULES_MAP_CONTROLLER.md). _(SELESAI)_
- [x] **1.3** Hapus duplikasi file identik [src-tauri/src/modules/module_03/aoi/pure.rs](file:///d:/project/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/aoi/pure.rs) dan satukan ekspor di `aoi/mod.rs` ke `builder.rs`. _(SELESAI)_
- [x] **1.4** Hapus hook tidak terpakai [src/modules/module-03/hooks/useMeasurement.ts](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/hooks/useMeasurement.ts). _(SELESAI)_
- [x] **1.5** Hapus komponen mati [src/modules/module-03/organisms/MapSyncHandler.tsx](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/MapSyncHandler.tsx) dan lepas pemanggilannya dari [MapCanvas.tsx](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/MapCanvas.tsx#L53). _(SELESAI)_
- [x] **1.6** Bersihkan basemap mati `positron` dari [src/modules/module-03/config/mapConfig.ts](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/config/mapConfig.ts#L75-L84) serta referensinya di [MapTypeThumbnail.tsx](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/atoms/MapTypeThumbnail.tsx). _(SELESAI)_
- [x] **1.7** Bersihkan re-export tak terpakai di [src/modules/module-03/index.ts](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/index.ts) (`AoiDemoToolbar`, `buildSpatialResultPopupHtml`). _(SELESAI)_

### Tahap 2: Eliminasi Dummy Polling & Fallback Hijau (Safe Polish)

- [x] **2.1** Hapus fallback error ke gambar hijau `"/sample_drone.jpg"` di [OrthoCardItem.tsx:93, 97](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/molecules/OrthoCardItem.tsx#L93-L98) dan [ImageGallerySidebar.tsx:416-418](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/ImageGallerySidebar.tsx#L416-L418), ganti dengan placeholder `No Image` / Blueprint Icon bersih. _(SELESAI)_
- [x] **2.2** Hentikan pemanggilan independen `useImageMarkers(500)` pada [MapViewControlBar.tsx:13](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/MapViewControlBar.tsx#L13), [MapCanvas.tsx:40](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/MapCanvas.tsx#L40), dan [ImageGallerySidebar.tsx:30](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/ImageGallerySidebar.tsx#L30) serta menggantinya dengan array statis aman `[]`. _(SELESAI)_
- [x] **2.3** Perbaiki tombol "Bidik Keseluruhan Proyek" (FitBounds) di [MapViewControlBar.tsx:19-23](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/MapViewControlBar.tsx#L19-L23) agar membidik bounds nyata layer proyek/citra orthomosaic aktif dengan fallback ke default view, bukan mengandalkan array markers foto yang kosong. _(SELESAI)_

### Tahap 3: Optimasi Tauri IPC & Konsolidasi Validasi (FP Transition)

- [x] **3.1** Hapus IPC round-trip mutasi layer (`toggle_layer_visibility_cmd`, `set_layer_opacity_cmd`, `filter_layers_by_category_cmd`, `reorder_layer_stack_cmd`) di [src-tauri/src/modules/module_03/layers/commands.rs](file:///d:/project/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/layers/commands.rs). Ganti dengan update immutable murni di state React. _(SELESAI)_
- [x] **3.2** Satukan pembuatan dan penyimpanan AOI menjadi single command `save_new_aoi_cmd` untuk mengeliminasi 2x IPC call per aksi simpan. _(SELESAI)_
- [x] **3.3** Satukan validasi ring poligon antara `geometry/validation.rs` dan `sp_result_type.rs` ke fungsi validasi tunggal yang deterministik. _(SELESAI)_

### Tahap 4: Dekomposisi Atomic Komponen Monolitik (Refactoring Architecture)

- [x] **4.1** Pecah [AoiToolbar.tsx](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/AoiToolbar.tsx) (dari 797 menjadi 239 baris) menjadi sub-komponen molekul (`AoiCardItem.tsx`, `AoiCreateForm.tsx`) dan integrasikan `useAOI`. _(SELESAI)_
- [x] **4.2** Pecah [LayerManager.tsx](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/organisms/LayerManager.tsx) (dari 790 menjadi 289 baris) dengan memindahkan formulir anotasi ke modal terpisah (`AnnotationModal.tsx`) dan memanfaatkan hook `useAnnotations`. _(SELESAI)_
- [x] **4.3** Dekomposisi backend [src-tauri/src/modules/module_03/sp_result/store.rs](file:///d:/project/Aerial-Analytics-Platform/src-tauri/src/modules/module_03/sp_result/store.rs) (dari 1.542 baris monolitik menjadi facade bersih 18 baris) ke dalam unit fungsional modular (`reader.rs`, `writer.rs`, `patch.rs`, `quarantine.rs`). _(SELESAI)_

---

> [!NOTE]
> **Status Audit & Refactoring Keseluruhan**:  
> Seluruh Tahap (Tahap 1, 2, 3, dan 4) pada Action Plan telah selesai dieksekusi 100%.  
> Seluruh pre-commit gates (`npm run lint`, `npm run lint:rust`, `npm run format:check`, `npm run format:rust:check`), test suites (`vitest` 34 tests dan `cargo test` 87 tests), serta full build (`npm run build`) berhasil lolos dengan status GREEN tanpa error atau warning.
