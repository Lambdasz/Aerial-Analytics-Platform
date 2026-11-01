# Architectural Decisions, Resource Limits & Audit: Module 03

This document records the foundational architectural decisions, resource limits, image display resolution mechanisms, the 25-finding audit matrix, and open technical issues for **Module 03 (Aerial Image Map Explorer)**.

---

## 1. Architectural Decisions: Pure Rust GeoTIFF Engine

A key design choice in Module 03 was replacing native C/C++ geospatial libraries (such as GDAL or libgeotiff) with a **100% pure Rust** pipeline based on the `tiff` and `image` crates:

| Criterion                           | Pure Rust Engine (`tiff` + `image`)                            | Native C/C++ Binding (`GDAL`)                                           | Rationale & Architectural Choice                                     |
| :---------------------------------- | :------------------------------------------------------------- | :---------------------------------------------------------------------- | :------------------------------------------------------------------- |
| **Portability & Cross-Compilation** | Compiles seamlessly across Windows, Linux, and macOS           | Requires complex C++ toolchains, shared libraries, and GDAL dev headers | Enables uniform CI builds without native platform dependencies       |
| **Memory Safety & Error Isolation** | Enforces Rust memory guarantees; zero risk of buffer overflows | Susceptible to memory corruption or segmentation faults in C libraries  | Prevents memory exploits when parsing untrusted aerial GeoTIFF files |
| **Binary Footprint**                | Adds minimal static footprint to the final desktop application | Substantially bloats binary size and requires bundling dynamic DLLs/.so | Keeps distributor installer sizes small and efficient                |
| **Feature Coverage**                | Baseline TIFF tags, tile pyramids, and UTM/WGS84 projection    | Full support for hundreds of exotic projections and formats             | Module 03 strictly focuses on drone RGB orthomosaics (WGS84 & UTM)   |

---

## 2. Resource Limits & Technical Constraints

To maintain responsive UI performance and protect system resources on standard workstations, Module 03 enforces the following hard constraints:

| Resource Constraint              | Configured Threshold     | Enforcement Mechanism                                  | Failure Response / Mitigation                                 |
| :------------------------------- | :----------------------- | :----------------------------------------------------- | :------------------------------------------------------------ |
| **JSON Payload Size Ceiling**    | 64 KB (65,536 bytes)     | Validated in `aoi/store.rs` and `annotations/store.rs` | Returns `InvalidGeometry` error if payload exceeds limit      |
| **Fast Import Dimension Limit**  | 4,096 x 4,096 pixels     | Validated in `geotiff_reader.rs`                       | Prompts user to select Mode B (Auto Tiling) for deep pyramids |
| **Synchronous Ingest Threshold** | 5 MB file size           | Checked in `ortho_source/commands.rs`                  | Larger files are routed through background async Tokio tasks  |
| **Tile Zoom Levels**             | Level 10 to Level 18     | Enforced in `tiler.rs` Web Mercator pyramid generator  | Tiles outside this range are clipped to avoid disk bloat      |
| **Single-Instance Lock**         | 1 Active Primary Process | Enforced via `map_storage/.app.lock`                   | Secondary processes run in read-only mode (`ACCESS_DENIED`)   |

---

## 3. Image Display URL & Storage Root Determination

The resolution of image assets (orthomosaic thumbnails, detection crops) differs fundamentally between development environments and packaged desktop releases:

| Execution Environment                 | Resolution Mechanism                                                    | Code Implementation Reference                                                       | Verification Status                                       |
| :------------------------------------ | :---------------------------------------------------------------------- | :---------------------------------------------------------------------------------- | :-------------------------------------------------------- |
| **Vite Development Server**           | Local filesystem middleware intercepts `/map_storage` requests          | Configured in `vite.config.ts` (line 24); served from local workspace root          | **Verified** in local browser and Tauri dev               |
| **Packaged Desktop App (Production)** | IPC fetches canonical storage root, then resolves custom asset protocol | `get_storage_root_path()` + `convertFileSrc("asset://...")` in `fieldResolution.ts` | **Unverified** (Production release bundle not yet tested) |

---

## 4. Audit Findings Matrix (25 Items from audit.md)

The following table records the status and verification evidence for all 25 architectural findings identified in `audit.md`:

| Audit ID  | Location & Scope                   | Description of Finding                                      | Resolution Status | Verification Evidence / Reference                                  |
| :-------- | :--------------------------------- | :---------------------------------------------------------- | :---------------- | :----------------------------------------------------------------- |
| **DC-01** | `hooks/useMeasurement.ts`          | Dead measurement hook not imported anywhere                 | **Completed**     | File removed from repository                                       |
| **DC-02** | `hooks/useAnnotations.ts`          | Hook not integrated into LayerManager                       | **Open**          | File retained; pending integration into LayerManager               |
| **DC-03** | `organisms/ManagedMapLayers.tsx`   | Mock condition `flight_session_01` never matches            | **Open**          | Mock branch retained in code; pending cleanup                      |
| **DC-04** | `organisms/MapSyncHandler.tsx`     | Invisible dead component searching empty markers            | **Completed**     | File removed from repository                                       |
| **DC-05** | `molecules/MapSearchBar.tsx`       | Search query state cosmetic; not filtered                   | **Open**          | Local search query remains unhooked to parent                      |
| **DC-06** | `services/api.ts`                  | Dead wrappers `reorderLayerStack`, `filterLayersByCategory` | **Completed**     | Dead functions removed from `api.ts`                               |
| **DC-07** | `src-tauri/.../type.rs`            | Empty 230-byte stub file with dead_code allow               | **Completed**     | File removed from `module_03/`                                     |
| **DC-08** | `src-tauri/.../aoi/pure.rs`        | 100% duplicate clone of `aoi/builder.rs`                    | **Completed**     | Duplicate `pure.rs` removed; `builder.rs` retained                 |
| **DC-09** | `src-tauri/.../layers/commands.rs` | IPC commands `reorder_layer_stack_cmd` dead                 | **Completed**     | Commands removed from `lib.rs` and `commands.rs`                   |
| **DC-10** | `src-tauri/.../logperson1.md`      | Personal log & outdated rules in Rust source                | **Completed**     | Files removed from `src-tauri/`                                    |
| **DC-11** | `config/mapConfig.ts`              | Dead `positron` basemap missing API keys                    | **Completed**     | Positron entry removed from `mapConfig.ts`                         |
| **DC-12** | `src/modules/module-03/index.ts`   | Unused re-exports (`AoiDemoToolbar`, aliases)               | **Completed**     | Aliases cleaned up in commits `f24e05c` & `bf5220d`                |
| **RV-01** | `geometry/validation.rs`           | Duplicate polygon validation between functions              | **Open**          | Both validation functions currently retained                       |
| **RV-02** | `aoi/commands.rs`                  | Double round-trip IPC (`create_aoi_cmd` then `save_aoi`)    | **Open**          | Both commands remain registered in IPC                             |
| **RV-03** | `leaflet/spatialLayerBuilder.ts`   | Redundant `isValidWgs84Bounds` calls on render              | **Open**          | Checks retained in layer builder                                   |
| **RV-04** | `molecules/OrthoCardItem.tsx`      | Fallback error image pointing to dummy drone photo          | **Completed**     | Replaced with clean `ResultPhoto` No Image component               |
| **DS-01** | `map_storage/photos/`              | Parallel 4x `useImageMarkers(500)` calls on empty storage   | **Completed**     | Polling removed from primary organisms                             |
| **DS-02** | `state.rs`                         | In-memory cache `Arc<RwLock>` duplicate vs direct disk      | **Open**          | In-memory `Module03State` cache maintained                         |
| **DS-03** | `map_storage/layers/`              | Empty directory vs dynamic layer generation                 | **Open**          | Disk layout retains `layers/` directory                            |
| **DS-04** | `store/ImageSyncContext.tsx`       | Cross-component context vs local URL state                  | **Open**          | Context provider maintained in tree                                |
| **FP-01** | `organisms/AoiToolbar.tsx`         | Monolithic 797-line component combining form & math         | **Completed**     | Decomposed to 330 lines; extracted form & detail modals            |
| **FP-02** | `organisms/LayerManager.tsx`       | Monolithic 790-line component with inline forms             | **Open**          | Partially cleaned (634 lines); pending modal extraction            |
| **FP-03** | `sp_result/store.rs`               | Monolithic 1,542-line file combining all operations         | **Completed**     | Decomposed into `reader.rs`, `writer.rs`, `patch.rs`, `cleanup.rs` |
| **FP-04** | `hooks/useSpatialResultLayers.ts`  | Imperative Leaflet mutations inside hook                    | **Open**          | Imperative Leaflet adapter pattern maintained                      |
| **FP-05** | `LayerManager.tsx` & `commands.rs` | IPC round-trip for boolean `is_visible` & opacity           | **Completed**     | Converted to functional immutable React client updates             |

---

## 5. Open Technical Issues ("Isu Terbuka")

1. **Path Traversal Risk in `delete_saved_image` (Legacy Command)**:
   - _Status_: Open.
   - _Analysis_: In `src-tauri/src/modules/module_03/ortho_source/store.rs` (line 597), `legacy_dir = get_images_dir().join(category).join(&safe_id)`. While `safe_id` is sanitized via `sanitize_filename`, the `category` string parameter is not checked for traversal sequences (`..`). An attacker could supply a malicious category string.
2. **Missing Instance Guard on `cancel_import_job`**:
   - _Status_: Open.
   - _Analysis_: In `src-tauri/src/modules/module_03/ortho_source/commands.rs` (line 178), `cancel_import_job` lacks `require_primary_instance()?`. A secondary read-only instance can mutate `.jobs/{job_id}.json` directly on disk, while the primary instance's in-memory `CancellationToken` remains uncancelled.
3. **Unrestricted File Path Inspection in `inspect_geotiff_header`**:
   - _Status_: Open.
   - _Analysis_: In `src-tauri/src/modules/module_03/ortho_source/commands.rs` (line 236), `inspect_geotiff_header` accepts any arbitrary file path string. While it checks `path.is_file()`, it enforces no directory boundary or file extension restrictions, allowing attempts to decode arbitrary system files as TIFF.
4. **Production Asset Protocol Verification**:
   - _Status_: Unverified.
   - _Analysis_: Loading tiled orthomosaics and thumbnails via `convertFileSrc` custom protocol has not yet been benchmarked or verified inside a production packaged release bundle (`--no-bundle` in CI).
5. **Other Known Repository Issues**:
   - _Cargo Audit Advisory_: Known vulnerability advisory in `zip` crate dependency.
   - _Projection Golden Tests_: UTM-to-WGS84 projection conversions lack automated golden dataset verification tests.
   - _CI Environment_: Ubuntu 24.04 compatibility in GitHub Actions workflow.
   - _Memory Usage Benchmarks_: Peak memory consumption tables during deep tiling are currently marked **Unverified** due to lack of raw runtime benchmark capture logs.
