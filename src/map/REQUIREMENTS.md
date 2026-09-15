# Requirements — Module 3: Aerial Image Map Explorer

> File ini **bukan** untuk pip / package manager mana pun — sifatnya informatif, supaya tim (terutama **Module 2: Plugin System & Extension Manager**) tahu dependensi apa saja yang dipakai Module 3 tanpa perlu membuka `package.json` / `Cargo.toml` langsung. Sumber kebenaran instalasi tetap `package.json` (`npm ci`) dan `Cargo.toml` (`cargo build`).
>
> Status: Module 3 masih tahap desain/rules, belum implementasi (2026).

## Frontend (npm — `package.json`, root repo)

| Package                          | Versi     | Fungsi                                                           |
| -------------------------------- | --------- | ---------------------------------------------------------------- |
| `react`                          | `^19.1.0` | UI framework                                                     |
| `react-dom`                      | `^19.1.0` | React renderer                                                   |
| `react-leaflet`                  | `^5.0.0`  | Binding React untuk Leaflet                                      |
| `leaflet`                        | `^1.9.4`  | Engine peta                                                      |
| `@geoman-io/leaflet-geoman-free` | `^2.20.1` | Drawing/editing AOI (polygon, dsb)                               |
| `@blueprintjs/core`              | `^6.18.0` | Komponen UI (panel, toolbar map, dll)                            |
| `@tauri-apps/api`                | `^2`      | `invoke()` ke backend Rust (dipakai lewat wrapper `invokeMap()`) |
| `@tauri-apps/plugin-opener`      | `^2`      | Plugin bawaan scaffold Tauri (bukan spesifik map)                |

Dev-only:

| Package          | Versi     | Fungsi                  |
| ---------------- | --------- | ----------------------- |
| `@types/leaflet` | `^1.9.22` | Type definition Leaflet |

> `react-leaflet-draw` **tidak** dipakai (deprecated, dilarang di `RULES_MAP_FR.md`).

## Backend (Cargo — `src-tauri/Cargo.toml`)

Sudah dipakai (scaffold Tauri default):

| Crate        | Fungsi                                                                              |
| ------------ | ----------------------------------------------------------------------------------- |
| `serde`      | Serialisasi struct (AOI, ImageMarker, LayerConfig, `CommandError`) ke/dari frontend |
| `serde_json` | Pendukung serde untuk payload JSON/GeoJSON                                          |

Direncanakan / perlu ditambahkan seiring implementasi `map_controller` (**belum dikonfirmasi ada di `Cargo.toml` saat ini** — cek dan tambahkan saat mulai coding):

| Crate       | Fungsi                                                                                |
| ----------- | ------------------------------------------------------------------------------------- |
| `geo`       | Validasi & operasi geometri (polygon closed-check, self-intersection, luas/perimeter) |
| `thiserror` | Definisi `MapControllerError` (enum error internal)                                   |
| `log`       | Logging terpusat di titik konversi `MapControllerError` → `CommandError`              |

## Di luar scope Module 3

**Python** — direncanakan di `AGENTS.md` untuk modul AI/komputasi berat (Module 4–10), **bukan** dipakai di Module 3. Tidak ada dependensi Python untuk map explorer.

## Kontrak yang perlu diketahui Module 2

- AOI yang dihasilkan Module 3 berformat `GeoJSON.Feature<Polygon>` (RFC 7946), CRS **WGS84 (EPSG:4326)**.
- Error dari command `map_controller` berbentuk `CommandError { code, message }` — pola ini dirancang supaya bisa dipakai ulang untuk kebutuhan _"Plugin Error Isolation"_ di Module 2. Lihat `RULES_MAP_CONTROLLER.md` bagian "Error Handling" untuk detail struct-nya.
