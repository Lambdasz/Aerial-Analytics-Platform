# Backend Pipelines & Concurrency: Module 03

This document details the operational pipelines, background concurrency supervisors, and filesystem reconciliation mechanisms implemented in the **Module 03 (Aerial Image Map Explorer)** backend.

---

## 1. Storage to Frontend Pipeline (Read Direction)

The read pipeline safely ingests persisted disk entities into memory and passes them across Tauri IPC boundaries without risking application panics on malformed data:

```text
[ Disk Storage: map_storage/ ]
           |
           | 1. fs::read_dir() / fs::read_to_string()
           v
[ Module Submodule Reader (reader.rs / store.rs) ]
           |
           +---> Valid JSON?
           |     YES -> Deserialize into domain struct (e.g. SpatialResultItem)
           |     NO  -> Catch error, flag is_corrupted = true, isolate error
           |
           v
[ Memory Cache Sync (state.rs: Module03State) ]
           |
           | 2. RwLock::write() -> Updates in-memory lookup cache
           v
[ Tauri Command Handler (commands.rs) ]
           |
           | 3. Serialize to serde_json::Value
           v
[ Frontend IPC Receiver (invokeMap) ]
```

---

## 2. Frontend to Storage Pipeline (Write & Mutate Direction)

Every state mutation enforces a strict Write-Through policy with atomic filesystem replacement:

```text
[ Frontend Mutation Payload (IPC Request) ]
                       |
                       | 1. IPC Handler in commands.rs
                       v
[ Primary Instance Lock Verification ]
                       |
                       +---> Is Primary Process? (instance_lock::require_primary_instance)
                       |     NO  -> Return Err(CommandError: ACCESS_DENIED)
                       |     YES -> Proceed
                       v
[ Parameter & Geometry Validation ]
  - Verify WGS84 coordinate range (-90..90, -180..180)
  - Verify JSON payload size ceiling (<= 64 KB for annotations/AOI)
                       |
                       v
[ Atomic Disk Persistence (write_atomic) ]
  - Write payload into ephemeral buffer: temp_{uuid}.tmp
  - Flush OS file buffers (sync_all)
  - Atomic rename: temp_{uuid}.tmp -> target.json
                       |
                       v
[ Memory Cache Update (state.rs) ]
  - Acquire write lock on Arc<RwLock<HashMap<...>>>
  - Insert committed entity
                       |
                       v
[ Return Success Payload across IPC ]
```

---

## 3. Dual-Mode Orthomosaic Ingestion Pipeline

Importing aerial orthomosaics is split into two modes depending on user requirements:

```text
[ User Selects GeoTIFF File in UI ]
                       |
                       v
[ inspect_geotiff_header() ]
  - Fast tag validation (width, height, bit depth, CRS code)
  - Bounds conversion from UTM / projected CRS to WGS84
                       |
                       v
[ User Selects Mode A or Mode B in OrthoUploadModal ]
                       |
       +---------------+---------------+
       |                               |
       v                               v
[ Mode A: Fast Copy ]       [ Mode B: Auto Tiling ]
  - Copy raw.tif to images/   - Allocate job_id & CancellationToken
  - Generate meta.json        - Record initial status in .jobs/{job_id}.json
  - Register default layer    - Spawn async background task on thread pool
  - Complete in < 500 ms      - Generate Web Mercator tiles (zoom 10-18)
                              - Stream progress % to .jobs/{job_id}.json
                              - Clean up job token on completion
```

---

## 4. Startup Storage Reconciliation & Auto-Heal

When the backend initializes, `ortho_source::store::reconcile_storage_consistency()` audits `map_storage/`:

1. **Quarantine Resolution (`.trash_*`)**:
   - Scans for leftover `.trash_{timestamp}_{id}` directories.
   - If the corresponding layer (`layers/{id}.json`) still exists, restores the quarantined folder back to `images/{id}/`.
   - If the layer is gone, permanently purges the quarantined folder.
2. **Auto-Heal Missing Layers (Case A)**:
   - If `images/{id}/meta.json` exists but `layers/{id}.json` is missing, regenerates the default raster layer record.
3. **Orphan Layer Pruning (Case B)**:
   - If `layers/{id}.json` of type raster exists but its backing directory `images/{id}/` does not exist, purges the orphaned layer file to prevent phantom Leaflet layers.

---

## 5. Process Concurrency & Instance Locking Pipeline

```text
App Launch (run in lib.rs)
         |
         v
Check map_storage/.app.lock
         |
         +---> Lock file does NOT exist?
         |     Claim lock -> Write current OS PID -> Become PRIMARY.
         |
         +---> Lock file exists?
               |
               +---> Is PID process actively running?
                     NO (Stale Lock) -> Overwrite with current PID -> Become PRIMARY.
                     YES -> Mark instance as SECONDARY (Read-Only Mode).
```
