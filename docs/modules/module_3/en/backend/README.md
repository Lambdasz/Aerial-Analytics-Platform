# Backend Architecture: Module 03 (Aerial Image Map Explorer)

This section details the backend architecture for **Module 03 (Aerial Image Map Explorer)** implemented in Rust and Tauri 2.

---

## 1. Architectural Philosophy & Layered Model

The backend is built upon strict separation of concerns, ensuring error isolation, atomic filesystem persistence, and zero-cost abstraction:

```text
+-------------------------------------------------------+
|                   Tauri IPC Layer                     |
|  commands.rs / storage_commands.rs (Thin Handlers)    |
+-------------------------------------------------------+
                           |
                           v
+-------------------------------------------------------+
|               Domain Service / Pipeline               |
|  image_pipeline.rs / geotiff_reader.rs / builder.rs   |
+-------------------------------------------------------+
                           |
                           v
+-------------------------------------------------------+
|               Storage & Cache Layer                   |
|  store.rs / reader.rs / writer.rs / state.rs          |
+-------------------------------------------------------+
                           |
                           v
+-------------------------------------------------------+
|             Filesystem Primitives (Disk)              |
|  map_storage/ (Atomic writes: temp_* -> rename)       |
+-------------------------------------------------------+
```

### Architectural Principles

1. **Thin Command Handlers**: Tauri IPC command functions validate incoming payloads, check process permissions (`require_primary_instance`), delegate execution to domain services or stores, and map internal errors to `CommandError`.
2. **Crash Resilience & Isolation**: Long-running or resource-intensive tasks (such as deep tiling of GeoTIFF orthomosaics) are executed in asynchronous background tasks via Tokio with active `CancellationToken` checkpoints.
3. **Atomic File Operations**: All write operations use temporary files (`temp_{uuid}.tmp`) that are flushed and atomically renamed to their final destination, preventing corrupted or half-written JSON files upon power disruption or application crashes.

---

## 2. Process Concurrency & Single-Instance Lock

To guarantee that multiple instances of the desktop application do not perform conflicting writes to `map_storage/`, Module 03 implements a single-instance filesystem lock in `instance_lock.rs`:

- **Lock File**: `map_storage/.app.lock` stores the operating system Process ID (PID) of the active primary instance.
- **Heartbeat & Liveness**: On startup, the application verifies whether the recorded PID corresponds to an actively running process. If the process has terminated (stale lock), the lock is automatically claimed.
- **Secondary Instance Enforcement**: Secondary windows or processes operate strictly in **read-only mode**. Mutating Tauri commands verify the lock via `require_primary_instance()?`. If called by a secondary instance, the command immediately returns `ACCESS_DENIED`.

---

## 3. Building Documentation (`cargo doc`)

To build the Rustdoc documentation suite with strict zero-warning enforcement (`-D warnings`), execute the following commands in your shell:

### PowerShell (Windows)

```powershell
# 1. Build public API documentation (Mode 1: without private items)
$env:RUSTDOCFLAGS="-D warnings"
cargo doc --manifest-path src-tauri/Cargo.toml --no-deps

# 2. Build complete private and internal documentation (Mode 2: with private items)
$env:RUSTDOCFLAGS="-D warnings"
cargo doc --manifest-path src-tauri/Cargo.toml --no-deps --document-private-items
```

### Bash (Linux / macOS)

```bash
# 1. Build public API documentation (Mode 1: without private items)
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path src-tauri/Cargo.toml --no-deps

# 2. Build complete private and internal documentation (Mode 2: with private items)
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path src-tauri/Cargo.toml --no-deps --document-private-items
```
