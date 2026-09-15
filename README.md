# Module 3: Aerial Image Map Explorer

Modul ini mengembangkan lingkungan geospasial interaktif untuk menjelajahi citra udara (aerial imagery) beserta lokasinya. Modul ini menjadi **fondasi input spasial** bagi seluruh modul analitik lain (Module 4–10): Area of Interest (AOI) yang dibuat di sini adalah kontrak data yang dikonsumsi oleh plugin vegetasi, tree counting, land cover, plot analytics, hingga temporal change analysis.

> Aturan pengembangan detail (do's/don'ts) ada di dua dokumen terpisah:
> - [`RULES_MAP_FR.md`](./RULES_MAP_FR.md) — untuk frontend (`src/map/`)
> - [`RULES_MAP_CONTROLLER.md`](./RULES_MAP_CONTROLLER.md) — untuk backend (`src-tauri/src/map_controller/`)
>
> README ini merangkum gambaran besar; untuk detail implementasi rujuk kedua dokumen di atas.

## Fitur (sesuai brief proyek)

- **Geo-Referenced Image Explorer** — menampilkan citra udara berdasarkan lokasi GPS-nya.
- **Image Location Marker** — menandai titik pengambilan tiap citra.
- **Area of Interest Selection** — menggambar dan mengedit area untuk dianalisis.
- **Layer Management** — menampilkan imagery, boundary, anotasi, dan hasil analitik.
- **Spatial Measurement** — mengukur jarak, luas, dan perimeter.
- **Spatial Annotation** — menandai dan memberi anotasi pada lokasi/area tertentu.

## Status

Belum diimplementasikan — repo masih scaffold default Tauri + React. Dokumen ini dan dua `RULES_MAP_*.md` disiapkan sebagai pedoman sebelum implementasi dimulai.

## Arsitektur

```
src/map/                          # Frontend (React + Leaflet)
├── components/                   # MapContainer, ImagePopup, dll
├── types/                        # AOI, ImageMarker, LayerConfig (kontrak GeoJSON)
└── error.ts                      # invokeMap() — wrapper invoke() + error handling

src-tauri/src/map_controller/     # Backend (Rust, Tauri command)
├── commands.rs                   # #[tauri::command] — didaftarkan di lib.rs invoke_handler
├── types.rs                      # Struct Serde yang sepadan dengan types/ di frontend
└── error.rs                      # MapControllerError + CommandError
```

Komunikasi frontend ⟷ backend selalu lewat Tauri `invoke`, dibungkus `invokeMap()` di sisi frontend dan dikembalikan sebagai `Result<T, CommandError>` di sisi backend.

## Tech Stack Modul

| Layer | Teknologi |
|---|---|
| Peta | Leaflet + `react-leaflet` |
| Drawing/Editing AOI | `@geoman-io/leaflet-geoman-free` (bukan `react-leaflet-draw`, sudah deprecated) |
| Tile default | OpenStreetMap (OSM) |
| Backend command | Rust + Tauri 2, Serde untuk serialisasi |
| Validasi geometri | crate `geo` |

## Kontrak Data Spasial (AOI / GeoJSON)

Karena AOI dipakai lintas modul, kontraknya dikunci sejak awal:

- AOI direpresentasikan sebagai `GeoJSON.Feature<Polygon>` sesuai **RFC 7946** — bukan format koordinat custom.
- Sistem koordinat default **WGS84 (EPSG:4326)**, konsisten dengan GPS EXIF citra drone. Reproyeksi (mis. ke UTM untuk hitung luas) menjadi tanggung jawab backend, bukan frontend.
- Validasi geometri (poligon tertutup, tidak self-intersecting, minimal 3 titik) dilakukan di backend sebelum AOI disimpan/diteruskan — frontend tidak boleh dianggap sebagai sumber data tepercaya.

## Pola Error Handling

Modul ini memakai kontrak error terstruktur di kedua sisi, dengan satu titik logging masing-masing (bukan tersebar di tiap command/komponen):

- **Backend**: setiap command mengembalikan `Result<T, CommandError>`, dikonversi otomatis dari `MapControllerError` (enum internal) lewat `impl From`. `CommandError { code, message }` inilah yang diserialisasi ke frontend.
- **Frontend**: semua pemanggilan command lewat `invokeMap()` di `src/map/error.ts` — logging dan pelemparan error terjadi di satu tempat, komponen visual tidak pernah memanggil `invoke()` langsung.
- `code` di `CommandError` (mis. `INVALID_GEOMETRY`, `AOI_NOT_FOUND`) adalah kontrak bersama yang di-`match` di kedua sisi, sehingga UI bisa menampilkan pesan yang sesuai jenis error, bukan hanya string generik.

Implementasi lengkap kedua sisi ada di `RULES_MAP_CONTROLLER.md` (bagian "Error Handling") dan `RULES_MAP_FR.md` (bagian "Error Handling").

## Pola Interop React-Leaflet ⟷ Geoman

`react-leaflet` deklaratif, `leaflet-geoman-free` imperatif — keduanya dijembatani lewat `useMap()` untuk mengambil instance map, dipasang/dilepas di `useEffect` dengan cleanup, dan event Geoman (`pm:create`, `pm:edit`, `pm:remove`) di-_bridge_ ke custom hook (`useAOI()`), bukan langsung `setState` di komponen map. Detail lengkap di `RULES_MAP_FR.md`.

## Registrasi Command & Capabilities

Setiap command Tauri baru di `map_controller` **wajib**:
1. Didaftarkan di `invoke_handler` pada `src-tauri/src/lib.rs`.
2. Diberi izin eksplisit di `src-tauri/capabilities/default.json`.

## Testing

Belum ada test runner terkonfigurasi di repo. Untuk modul ini:
- Backend: fungsi komputasi spasial murni (luas, perimeter, validasi polygon) wajib punya unit test (`cargo test`), dipisah dari `#[tauri::command]`.
- Frontend: custom hooks (`useAOI`, `useLayerManager`, dst.) disarankan diuji dengan Vitest + React Testing Library.

## Referensi

- [`AGENTS.md`](../../AGENTS.md) — konvensi proyek secara keseluruhan.
- [`Proyek.md`](../../Proyek.md) — deskripsi lengkap 11 modul Aerial Analytics Platform.
- [`RULES_MAP_FR.md`](./RULES_MAP_FR.md) / [`RULES_MAP_CONTROLLER.md`](./RULES_MAP_CONTROLLER.md) — aturan detail per sisi.