# Module 03 — Aerial Image Map Explorer

Modul visualisasi peta interaktif untuk menampilkan citra ortomosaik, sebaran foto drone (telemetri & heading), pembuatan Area of Interest (AOI), manajemen layer, serta visualisasi hasil analisis spasial.

---

## 1. Arsitektur Atomic Design

Modul ini disusun mengikuti hierarki Atomic Design dan prinsip pemrograman fungsional:

```text
src/modules/module-03/
├── index.ts                         # SATU-SATUNYA pintu masuk publik modul
├── MapExplorer.tsx                  # Composition Root mandiri (Provider + Styles + Icon fix)
├── README.md                        # Dokumentasi modul dan panduan pengembang
├── pages/
│   └── Module03Page.tsx             # Halaman utuh modul (merender <MapExplorer />)
├── templates/
│   └── MapExplorerLayout.tsx        # Layout murni: kanvas peta + slot panel kiri + slot kontrol
├── organisms/                       # Terikat state Leaflet MapContext / Context / IPC
│   ├── MapCanvas.tsx                # Inisialisasi MapContainer, TileLayer, ScaleControl
│   ├── ManagedMapLayers.tsx         # Layer ortomosaik, marker drone, dan poligon analitik
│   ├── SpatialResultLayer.tsx       # Layer hasil analitik dinamis
│   ├── MapLeftPanel.tsx             # Panel samping kiri (tab Layers, AOI, Gallery)
│   ├── LayerManager.tsx             # Orkestrator layer & anotasi spasial
│   ├── AoiToolbar.tsx               # Toolbar gambar & simpan AOI (alias AoiDemoToolbar)
│   ├── ImageGallerySidebar.tsx      # Galeri foto drone dengan sinkronisasi scroll
│   ├── DroneImageMarker.tsx         # Marker foto drone dengan rotasi heading SVG
│   ├── ImagePopupCard.tsx           # Kartu preview foto drone (useMap & useImageSync)
│   ├── MapViewControlBar.tsx        # Kontrol navigasi peta (zoom, fit bounds, reset north)
│   └── MapSyncHandler.tsx           # Headless synchronization handler (map.flyTo)
├── molecules/                       # Presentasional murni (props & callbacks)
│   ├── MapSearchBar.tsx             # Input pencarian dengan tombol menu
│   ├── MapLegend.tsx                # Legenda sektor hasil spasial
│   ├── AnnotationMarkers.tsx        # Marker & popup Leaflet untuk anotasi titik
│   ├── LayerItem.tsx                # Baris layer: nama, badge, toggle, slider
│   ├── LayerCategoryFilter.tsx      # Filter kategori layer
│   └── AnnotationForm.tsx           # Form pembuatan catatan anotasi spasial
├── atoms/                           # Elemen visual terkecil
│   └── MapTypeThumbnail.tsx         # Tombol thumbnail beralih basemap OSM/Satelit
├── hooks/                           # Custom React Hooks
│   ├── useAOI.ts                    # Hook lifecycle Leaflet-Geoman
│   ├── useMeasurement.ts            # Hook pengukuran jarak, luas, keliling
│   ├── useImageMarkers.ts           # Hook pengambil marker foto dari backend
│   └── useSpatialResultLayers.ts    # Hook pengambil dummy spatial result layers
├── store/                           # Context API
│   └── ImageSyncContext.tsx         # Sinkronisasi dua arah foto galeri ↔ marker peta
├── services/                        # Komunikasi IPC Tauri
│   ├── api.ts                       # Typed Tauri command wrappers
│   └── error.ts                     # invokeMap centralized error logging
├── lib/                             # FUNGSI MURNI (Zero React, Zero Leaflet Object)
│   ├── geoJson.ts                   # Konversi dan validasi GeoJSON RFC 7946
│   ├── markerSvg.ts                 # Pembuat string markup SVG drone murni
│   ├── popupHtml.ts                 # Pembuat string HTML popup tabel properti
│   └── legendUtils.ts               # Pengelompokan layer per sektor
├── lib/leaflet/                     # BUILDER OBJEK LEAFLET (Zero React)
│   ├── markerIconBuilder.ts         # Pembuat instance L.DivIcon dari SVG
│   └── spatialLayerBuilder.ts       # Pembuat instance L.Layer dari payload spasial
├── config/                          # Konstanta dan konfigurasi koordinat
│   └── mapConfig.ts                 # KARIANGAU_BOUNDS, DEFAULT_MAP_VIEW, BASEMAPS
├── styles/                          # Lembar gaya CSS komponen
└── types/                           # Kontrak TypeScript (aoi, layer, map, marker)
```

---

## 2. Cara Penggunaan Publik

### 2.1 Komponen Mandiri `<MapExplorer />`

Komponen `<MapExplorer />` adalah _composition root_ mandiri. Komponen ini sudah otomatis menginisialisasi:

- `<ImageSyncProvider>`
- Stylesheet Leaflet & Geoman
- Stylesheet internal modul
- Leaflet default marker icon fix untuk Vite bundler

```tsx
import React from "react";
import { MapExplorer } from "../modules/module-03";

export const MyCustomScreen: React.FC = () => {
  return (
    <div style={{ width: "100%", height: "100vh" }}>
      <MapExplorer
        onLayersChange={(layers) => console.log("Layers:", layers)}
        onAnnotationsChange={(annos) => console.log("Annotations:", annos)}
      />
    </div>
  );
};
```

> [!IMPORTANT]
> **Kontainer Induk Wajib Memiliki Tinggi (Height) Terdefinisi**:
> Leaflet membutuhkan dimensi kontainer yang terdefinisi (misalnya `height: 100vh`, `height: 100%` di dalam flex container, atau nilai piksel tetap). Jika tinggi bernilai 0, tiles peta tidak dapat dirender.

### 2.2 Props `<MapExplorer />`

| Prop                  | Tipe                                         | Default     | Deskripsi                                                                                 |
| :-------------------- | :------------------------------------------- | :---------- | :---------------------------------------------------------------------------------------- |
| `className`           | `string`                                     | `undefined` | Kelas CSS tambahan untuk kontainer pembungkus terluar.                                    |
| `style`               | `React.CSSProperties`                        | `{}`        | Digabung secara shallow dengan `{ width: "100%", height: "100%", position: "relative" }`. |
| `onLayersChange`      | `(layers: LayerState[]) => void`             | `undefined` | Callback saat daftar layer aktif mengalami perubahan.                                     |
| `onAnnotationsChange` | `(annotations: AnnotationFeature[]) => void` | `undefined` | Callback saat daftar anotasi spasial berubah.                                             |

### 2.3 Halaman Penuh `<Module03Page />`

Digunakan oleh app shell / router aplikasi:

```tsx
import { Module03Page } from "../modules/module-03";
```

---

## 3. Aturan Dependensi Antar Level (Enforced by ESLint)

1. **Pages** $\to$ **Composition Root (MapExplorer)** $\to$ **Templates** $\to$ **Organisms** $\to$ **Molecules** $\to$ **Atoms**.
2. **Atoms** dan **Molecules** bersifat **murni presentasional**:
   - Dilarang mengimpor `useMap`, `useMapEvent`, atau `useMapEvents` dari `react-leaflet`.
   - Dilarang mengimpor dari `organisms/`, `templates/`, `hooks/`, `store/`, `services/`, atau `lib/leaflet/`.
   - `Atoms` dilarang mengimpor `Molecules`.
3. **`lib/`**:
   - Berisi fungsi murni (deterministik, zero side-effects).
   - Dilarang mengimpor `react` maupun `leaflet`.
4. **`lib/leaflet/`**:
   - Berisi pembuat objek Leaflet (imperatif/mutable builder).
   - Boleh mengimpor `leaflet`, tetapi dilarang mengimpor `react`.
5. **Path Impor**:
   - Gunakan path relatif bersih (`../modules/module-03`).
   - Jangan menggunakan alias path `@/*`.
