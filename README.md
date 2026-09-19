# Module 3: Aerial Image Map Explorer

Modul ini mengembangkan lingkungan geospasial interaktif untuk menjelajahi citra udara (aerial imagery) beserta lokasinya. Modul ini menjadi **fondasi input spasial** bagi seluruh modul analitik lain (Module 4–10): Area of Interest (AOI) yang dibuat di sini adalah kontrak data yang dikonsumsi oleh plugin vegetasi, tree counting, land cover, plot analytics, hingga temporal change analysis.

> Aturan pengembangan detail (do's/don'ts) ada di dua dokumen terpisah:
>
> - [`RULES_MAP_FR.md`](./src/map/RULES_MAP_FR.md) — untuk frontend (`src/map/`)
> - [`RULES_MAP_CONTROLLER.md`](./src-tauri/src/map_controller/RULES_MAP_CONTROLLER.md) — untuk backend (`src-tauri/src/map_controller/`)
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

```text
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

| Layer               | Teknologi                                                                       |
| ------------------- | ------------------------------------------------------------------------------- |
| Peta                | Leaflet + `react-leaflet`                                                       |
| Drawing/Editing AOI | `@geoman-io/leaflet-geoman-free` (bukan `react-leaflet-draw`, sudah deprecated) |
| Tile default        | OpenStreetMap (OSM)                                                             |
| Backend command     | Rust + Tauri 2, Serde untuk serialisasi                                         |
| Validasi geometri   | crate `geo`                                                                     |

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

## Available Scripts

| Task                            | Command                     |
| ------------------------------- | --------------------------- |
| Start dev server                | `npm run dev`               |
| Run Tauri desktop app           | `npm run tauri dev`         |
| Build the Vite app              | `npm run build`             |
| Build the Tauri app             | `npm run tauri build`       |
| Run ESLint                      | `npm run lint`              |
| Run Rust linting (clippy)       | `npm run lint:rust`         |
| Fix ESLint issues               | `npm run lint:fix`          |
| Check Prettier formatting       | `npm run format:check`      |
| Check Rust formatting (rustfmt) | `npm run format:rust:check` |
| Fix Prettier formatting issues  | `npm run format`            |
| Fix Rust formatting issues      | `npm run format:rust`       |

## Continuous Integration

Every push and pull request targeting `main` runs the [CI workflow](.github/workflows/ci.yml) via GitHub Actions:

- **Lint & format (JS/TS)**: ESLint (`npm run lint`) and Prettier (`npm run format:check`).
- **Lint & format (Rust)**: rustfmt (`npm run format:rust:check`) and Clippy (`npm run lint:rust`).
- **Build**: verifies the Vite build (`npm run build`) and a Tauri build without bundling
  (`npm run tauri build -- --no-bundle`) succeed on `macos-latest`, `ubuntu-24.04`, and `windows-latest`.

The build matrix only runs once both lint jobs pass. The workflow does not publish or deploy anything.

## Project Structure

```bash
.
├── .github/                      # GitHub community health files
│   ├── ISSUE_TEMPLATE/
│   │   ├── bug_report.yml
│   │   ├── config.yml
│   │   └── feature_request.yml
│   └── pull_request_template.md
├── CONTRIBUTING.md               # Contribution guidelines
├── eslint.config.js              # JavaScript/TypeScript linting rules
├── index.html                    # HTML entry point for the web app
├── LICENSE                       # MIT License
├── package.json                  # Node.js dependencies and scripts
├── package-lock.json             # Locked versions of npm dependencies
├── plugins/                      # Analytical plugins (Module 2: Extension System)
│   ├── mock/                     # Python mock plugin (rgb-vegetation-exg)
│   ├── mock_rust/                # Native binary mock plugin (tree-canopy-density)
│   └── template/                 # Starter template and specification guide
├── public/                       # Static assets served directly
│   ├── tauri.svg
│   └── vite.svg
├── README.md                     # Project documentation
├── schemas/                      # Canonical JSON Schemas for plugin contracts
├── SECURITY.md                   # Vulnerability reporting policy
├── src/                          # React frontend application source
│   ├── App.css                   # Main component styling
│   ├── App.tsx                   # Main React component
│   ├── assets/                   # Application assets
│   │   └── react.svg
│   ├── main.tsx                  # Application entry point
│   └── vite-env.d.ts             # Vite type definitions
├── src-tauri/                    # Rust backend and Tauri desktop configuration
│   ├── build.rs                  # Rust build script
│   ├── capabilities/             # ACL capability definitions
│   │   └── default.json
│   ├── Cargo.lock                # Locked versions of Rust dependencies
│   ├── Cargo.toml                # Rust project manifest
│   ├── gen/                      # Generated JSON schemas
│   │   └── schemas/
│   │       ├── acl-manifests.json
│   │       ├── capabilities.json
│   │       ├── desktop-schema.json
│   │       └── linux-schema.json
│   ├── icons/                    # App icons for different platforms
│   │   ├── icon.icns             # macOS icon
│   │   ├── icon.ico              # Windows icon
│   │   └── icon.png              # Linux icon
│   ├── rustfmt.toml              # Rust code formatting rules
│   ├── src/                      # Rust application source code
│   │   ├── lib.rs
│   │   └── main.rs
│   └── tauri.conf.json           # Tauri app configuration
├── tsconfig.json                 # TypeScript configuration
├── tsconfig.node.json            # TypeScript configuration for Vite
└── vite.config.ts                # Vite bundler and dev server configuration
```

## `map/map-fixtures.json`

Dummy data **internal** Module 3 — satu file berisi contoh input/output untuk semua fitur UI (Geo-Referenced Image Explorer, Image Marker, Popup, AOI Selection, Layer Management, Spatial Measurement, Spatial Annotation, Map↔Image Interaction, Map View Control). Dipakai untuk:

- Membangun komponen React tanpa perlu backend Tauri hidup (mock `invokeMap()` bisa return dari file ini saat `import.meta.env.DEV`).
- Referensi cepat tim internal Module 3 saat development, tanpa scroll dokumen desain panjang.

`aoi_selection.output_invalid` sengaja disiapkan sebagai test case negatif — dipakai untuk memastikan validasi geometri di backend (`RULES_MAP_CONTROLLER.md`) benar-benar menolak poligon yang tidak valid.

## `shared/` — kontrak lintas modul

File-file ini **canonical**, artinya jadi acuan bersama supaya Module 2 dan Module 4–10 bisa mulai develop & test tanpa menunggu implementasi Module 3 selesai:

- **`aoi.example.geojson`** — bentuk AOI valid yang dikirim Module 3 ke plugin manapun. Format `GeoJSON.Feature<Polygon>`, WGS84.
- **`aoi.invalid.example.geojson`** — AOI tidak valid, untuk test negatif di sisi manapun yang mengonsumsi AOI.
- **`spatial-result.example.json`** — kontrak arah sebaliknya: hasil dari plugin (Module 4/7/8) yang dikonsumsi Module 3 untuk fitur _Spatial Result Visualization_. Berguna buat tim Module 4/7/8 supaya tahu bentuk output yang Module 3 harapkan, tanpa perlu nunggu UI-nya jadi.
- **`plugin-execution-payload.example.json`** — salinan lokal contoh dari template Module 2 (`plugins/template`), disertakan supaya jelas bagaimana `aoi.geometry` dari Module 3 dipakai sebagai parameter eksekusi plugin. **Bukan sumber kebenaran** — kalau template Module 2 berubah, update fixture ini menyusul, jangan sebaliknya.

## Catatan

Semua fixture ini murni untuk development/testing (mock data & regression check), bukan data produksi. Kalau kontrak berubah (nama field, struktur AOI, dst.), fixture ini **wajib** diupdate di commit yang sama — sama seperti aturan dokumentasi di `AGENTS.md`.

## Testing

Belum ada test runner terkonfigurasi di repo. Untuk modul ini:

- Backend: fungsi komputasi spasial murni (luas, perimeter, validasi polygon) wajib punya unit test (`cargo test`), dipisah dari `#[tauri::command]`.
- Frontend: custom hooks (`useAOI`, `useLayerManager`, dst.) disarankan diuji dengan Vitest + React Testing Library.

## Referensi

- [`AGENTS.md`](./AGENTS.md) — konvensi proyek secara keseluruhan.
- [`Proyek.md`](./Proyek.md) — deskripsi lengkap 11 modul Aerial Analytics Platform.
- [`RULES_MAP_FR.md`](./src/map/RULES_MAP_FR.md) / [`RULES_MAP_CONTROLLER.md`](./src-tauri/src/map_controller/RULES_MAP_CONTROLLER.md) — aturan detail per sisi.

## APL Reference Dev Singkat

- Website : http://prototype-lab-bice.vercel.app/
