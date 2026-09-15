# Aturan Pengembangan: Frontend Modul Map (`src/map/`)

Modul ini memuat kode frontend untuk **Module 3: Aerial Image Map Explorer**.
Anda diberikan **kebebasan penuh** dalam mengatur struktur folder dan file di dalam modul ini sesuai dengan gaya dan kenyamanan Anda, selama mengikuti kontrak data dan pola interop di bawah ini — karena modul ini menjadi fondasi input spasial untuk Module 4–10.

## Struktur Awal

Meskipun Anda bebas membuat struktur sendiri, kami telah menyediakan folder standar sebagai referensi:

- `components/`: Disarankan untuk meletakkan komponen-komponen UI React Anda (misalnya `MapContainer.tsx`, `ImagePopup.tsx`).
- `types/`: Disarankan untuk meletakkan file TypeScript definition (`.ts` / `.d.ts`) agar kontrak data spasial terpusat.
- `error.ts`: Titik tunggal pemanggilan command Tauri dan penanganan error dari backend (lihat bagian Error Handling).

## ✅ Yang Boleh (dan Disarankan) Dilakukan (Do's)

- **Gunakan `@geoman-io/leaflet-geoman-free`**: Untuk fitur drawing polygon (Area of Interest), editing, dan spasial lainnya. Library ini sangat aktif di-maintain dan kompatibel.
- **Ekstrak logika state**: Pisahkan logika manajemen layer atau Area of Interest (AOI) ke dalam _custom hooks_ agar komponen React tetap bersih dan mudah di-test.
- **Manfaatkan Leaflet native**: Untuk data GeoJSON yang sangat besar, gunakan kapabilitas rendering Leaflet (`L.geoJSON` + `renderer: L.canvas()`) daripada menaruh semuanya ke dalam React state yang bisa menurunkan performa.
- **Pisahkan komunikasi API**: Semua pemanggilan command Tauri ke backend Rust **wajib** lewat wrapper `invokeMap()` (lihat bagian Error Handling), jangan panggil `invoke()` langsung dari komponen.

## ❌ Yang Tidak Boleh Dilakukan (Don'ts)

- **DILARANG menggunakan `react-leaflet-draw`**: Library ini sudah lama _deprecated_ dan tidak direkomendasikan untuk React 19.
- **Jangan memanggil backend secara langsung di dalam komponen render**: Pisahkan logika _fetching_ dari komponen visual.
- **Hindari re-render peta secara keseluruhan**: Peta Leaflet cukup berat, pastikan manajemen state tidak membuat `MapContainer` ter-mount dan unmount berulang kali.
- **Jangan invoke command Tauri di setiap event drag/edit mentah**: Debounce atau tunggu event `pm:create` / `pm:edit` selesai (bukan tiap frame drag) sebelum mengirim data ke backend.
- **Jangan `console.error` manual tersebar di tiap komponen**: logging error dari pemanggilan backend harus lewat `invokeMap()`, bukan try/catch berulang di tiap komponen.

## Pola Interop React-Leaflet ⟷ Geoman

`react-leaflet` bersifat deklaratif, sedangkan `leaflet-geoman-free` bersifat imperatif (memanipulasi instance Leaflet secara langsung). Tanpa aturan yang jelas, keduanya mudah bentrok (map ter-init ulang, listener terpasang dobel). Ikuti pola berikut:

- Ambil instance map lewat hook `useMap()` di komponen anak `<MapContainer>`. **Jangan** simpan instance map di React state (`useState`) — cukup simpan di `useRef` jika perlu diakses lintas efek.
- Pasang dan lepas kontrol Geoman (`map.pm.addControls(...)`) di dalam `useEffect`, dengan _cleanup function_ yang memanggil `map.pm.removeControls()` dan melepas semua event listener saat komponen unmount.
- Event dari Geoman (`pm:create`, `pm:edit`, `pm:remove`) harus di-_bridge_ ke custom hook (misalnya `useAOI()`), bukan langsung memanggil `setState` di dalam komponen map itu sendiri. Ini menjaga logika AOI tetap testable terpisah dari rendering peta.

## Kontrak Data Spasial (AOI / GeoJSON)

Karena Area of Interest (AOI) akan dikonsumsi oleh Module 4–10 (plugin vegetasi, tree counting, land cover, temporal, dsb), kontrak berikut **wajib** dipatuhi sejak awal, bukan menyusul:

- AOI harus direpresentasikan sebagai `GeoJSON.Feature<Polygon>` sesuai standar **RFC 7946**, bukan format koordinat custom.
- Sistem koordinat default adalah **WGS84 (EPSG:4326)** — konsisten dengan GPS EXIF pada citra drone. Jika suatu saat dibutuhkan proyeksi lain (misalnya UTM untuk perhitungan luas yang lebih akurat), itu menjadi tanggung jawab backend (Rust), bukan frontend.
- Definisikan `interface` TypeScript untuk AOI, marker lokasi gambar, dan struktur layer di `types/`, agar tim lain (dan dokumen desain plugin) bisa merujuk ke satu sumber kebenaran.

## Error Handling: `invokeMap()`

### Kegunaan

Semua command Tauri di modul map dipanggil lewat satu wrapper, bukan `invoke()` langsung di tiap komponen. Ini punya tiga manfaat:

1. **Logging konsisten** — setiap kegagalan command otomatis tercatat ke `console.error` dengan format yang sama (`[map] <nama command> failed: ...`), tanpa perlu menulis try/catch berulang di tiap komponen yang memanggil backend.
2. **Kontrak error yang bisa di-`switch`** — backend mengembalikan `CommandError { code, message }` (lihat `RULES_MAP_CONTROLLER.md`), jadi frontend bisa membedakan jenis error secara terprogram, bukan mem-parsing string bebas.
3. **Satu titik untuk berkembang nanti** — kalau nanti dibutuhkan hal seperti retry, toast/notifikasi global, atau redaksi pesan error, cukup diubah di satu fungsi ini, tidak perlu menyentuh tiap komponen yang memanggil backend.

### Implementasi

```ts
// src/map/error.ts
import { invoke } from "@tauri-apps/api/core";

export interface CommandError {
  code: string;
  message: string;
}

export async function invokeMap<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    // titik logging tunggal — semua kegagalan command map lewat sini
    console.error(`[map] ${cmd} failed:`, err);
    throw err as CommandError;
  }
}
```

### Cara Pemakaian

Panggil `invokeMap()` dari custom hook (bukan langsung dari komponen visual), lalu tangani error di level yang memang perlu menampilkan pesan ke user:

```ts
// src/map/components/useAOI.ts (custom hook)
import { invokeMap, type CommandError } from "../error";
import type { AoiFeature } from "../types";

export function useAOI() {
  async function saveAoi(aoi: AoiFeature) {
    try {
      return await invokeMap<AoiFeature>("save_aoi", { aoi });
    } catch (err) {
      const error = err as CommandError;
      if (error.code === "INVALID_GEOMETRY") {
        // tampilkan pesan spesifik ke user, misal lewat Blueprint.js Toaster
      }
      throw error;
    }
  }

  return { saveAoi };
}
```

Komponen visual tidak pernah memanggil `invoke()` atau menangani logging secara langsung — cukup memanggil `saveAoi()` dari hook dan menampilkan hasil/error yang sudah terstruktur.

## Rekomendasi

- Untuk tile peta, Anda bisa mulai dengan OpenStreetMap (OSM) sebagai _default_.
- Buat interface data spasial yang jelas, seperti definisi struktur koordinat, definisi marker, dan struktur AOI, agar mudah dioper ke modul analitik lainnya (Modul 4–10).
- Jika memungkinkan, tulis unit test untuk custom hooks (`useAOI`, `useLayerManager`, dst.) menggunakan Vitest + React Testing Library, mengingat repo belum punya test runner terkonfigurasi sama sekali.
