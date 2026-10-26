# Module 03 — Aerial Image Map Explorer

Module 03 provides the interactive geospatial map viewer, orthomosaic raster tile rendering, Area of Interest (AOI) polygon management, spatial analysis inspection (vegetation detection, landcover classification), and disk-backed single-instance storage subsystem.

---

## Language Selection / Pilihan Bahasa

Documentation is available in two identical editions with 100% content parity:

- 🇬🇧 **[English Documentation](en/project-structure.md)**
- 🇮🇩 **[Dokumentasi Bahasa Indonesia](id/project-structure.md)**

---

## Documentation Tree / Struktur Dokumen

```text
docs/modules/module_3/
├── README.md                           # This index file / Berkas indeks utama ini
├── en/                                 # English documentation suite
│   ├── project-structure.md           # Full source tree (Rust, React, storage)
│   ├── frontend/
│   │   ├── README.md                  # Frontend architecture & atomic design overview
│   │   ├── module-03.md               # Detailed React component breakdown & responsibilities
│   │   ├── mapping.md                 # Complete 38 api.ts functions to components & commands
│   │   └── pipeline.md                # Frontend data pipelines & Leaflet layer lifecycle
│   └── backend/
│       ├── README.md                  # Rust backend architecture & cargo doc build commands
│       ├── mapping.md                 # Complete 39 Tauri IPC commands mapping & permissions
│       ├── pipeline.md                # Storage<->Backend<->Frontend bidirectional pipelines
│       └── decisions-and-limitations.md # Architectural decisions, resource limits, 25 audit findings
└── id/                                 # Dokumentasi lengkap Bahasa Indonesia
    ├── project-structure.md           # Pohon direktori lengkap (Rust, React, storage)
    ├── frontend/
    │   ├── README.md                  # Gambaran arsitektur frontend & desain atomik
    │   ├── module-03.md               # Rincian komponen React & tanggung jawabnya
    │   ├── mapping.md                 # Pemetaan lengkap 38 fungsi api.ts ke komponen & command
    │   └── pipeline.md                # Alur data frontend & siklus hidup layer Leaflet
    └── backend/
        ├── README.md                  # Arsitektur backend Rust & perintah build cargo doc
        ├── mapping.md                 # Pemetaan lengkap 39 Tauri IPC command & perizinan
        ├── pipeline.md                # Alur data storage<->backend<->frontend dua arah
        └── decisions-and-limitations.md # Keputusan arsitektur, batasan sumber daya, 25 temuan audit
```

---

## Key Architectural Principles

1. **Isolation & Concurrency**:
   - Single-instance lock via `map_storage/.app.lock`. Secondary app windows/processes run in read-only mode.
   - Long-running orthomosaic tiling processes execute asynchronously on background thread pools with cancellation token support.
2. **Atomic Write-Through Storage**:
   - Every state mutation writes to a temporary file (`temp_*`) and performs an atomic rename to prevent corruption on crash or power loss.
   - Quarantine mechanism via `.trash_*` folders with automatic recovery during startup reconciliation.
3. **Pure Rust GeoTIFF Engine**:
   - Decoding, pyramid tiling, and projection transformations run purely in Rust without external C++ GDAL dependencies.
4. **Typed IPC & Functional Boundaries**:
   - All frontend IPC requests go through `invokeMap` with standardized error wrapping (`CommandError`).
