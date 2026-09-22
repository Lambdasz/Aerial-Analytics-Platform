# Module 2: Plugin Architecture & Extension Manager

> **Master Overview & System Architecture**  
> **Project**: Aerial Analytics Platform (PBL 2026 - Pemrograman Fungsional)  
> **Scope**: Module 2 (Extension System & Plugin Manager)

---

## 1. Executive Summary & Objective

**Module 2 (Plugin Architecture & Extension Manager)** provides the extensibility engine for the Aerial Analytics Platform. The platform ingests, processes, and analyzes RGB aerial drone imagery. Analytical capabilities (vegetation detection, tree canopy density, land-cover classification) operate as autonomous, decoupled plugins rather than hardcoded core features.

### Core Architectural Mandates

1. **Dynamic Extensibility**: Analytical plugins can be discovered, installed, configured, enabled, disabled, and executed at runtime without modifying or recompiling the core desktop application.
2. **Platform Stability & Fault Isolation**: An analytical plugin (Python script or native binary) **MUST NOT** crash, freeze, or destabilize the host application under any circumstances (including unhandled exceptions, memory exhaustion, segmentation faults, invalid output formats, or infinite loops).
3. **Strict Contract-Driven Communication**: All host-plugin communication is strictly mediated by validated JSON schemas located in [`schemas/`](file:///home/perhanjay/Documents/Programming/Aerial-Analytics-Platform/schemas).
4. **Functional Programming (FP) Rigor**: Pure domain calculations (parsers, validators, payload assemblers) are strictly separated from impure side effects (filesystem I/O, process spawning, OS signals).

---

## 2. System Architecture Diagram

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                CORE TAURI HOST PROCESS                                 │
│                                                                                        │
│   ┌────────────────────────┐                   ┌───────────────────────────────────┐   │
│   │     React Frontend     │◄──[Tauri IPC]────►│      Rust Supervisor & Core       │   │
│   │ (Blueprint.js 6 UI /   │                   │ (Active Job Registry, State Store,│   │
│   │  Dynamic Parameter Form)                   │  Security & Healthcheck Guard)    │   │
│   └────────────────────────┘                   └─────────────────┬─────────────────┘   │
└──────────────────────────────────────────────────────────────────┼─────────────────────┘
                                                                   │
                         ┌─────────────────────────────────────────┴─────────────────────────────────────────┐
                         │ Subprocess Isolation Boundary: CLI --input <payload> --output <result>            │
                         ▼                                                                                   ▼
          ┌──────────────────────────────┐                                                    ┌──────────────────────────────┐
          │   Python Plugin Subprocess   │                                                    │   Native Binary Subprocess   │
          │   (e.g., rgb-vegetation-exg) │                                                    │   (e.g., tree-canopy-density)│
          │   - Isolated memory space    │                                                    │   - High performance C++/Rust│
          │   - Sandboxed file outputs   │                                                    │   - Strict signal handling   │
          └──────────────────────────────┘                                                    └──────────────────────────────┘
```

---

## 3. Team Division of Labor (4 Engineering Roles)

| Role / Engineer                                            | Primary Responsibilities                                                                                                      | Target Features                                                                                         | Primary Deliverables                                                                                                                             | Documentation Guide                                                                                                                        |
| :--------------------------------------------------------- | :---------------------------------------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------ | :----------------------------------------------------------------------------------------------------------------------------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------- |
| **1. Subprocess Supervisor & Isolation Engineer** _(Rust)_ | Execution engine, timeout enforcement, process tree killing, healthcheck runner                                               | **M2.7** (Execution)<br>**M2.9** (Error Isolation)                                                      | `src-tauri/src/plugins/executor.rs`                                                                                                              | [03_DOMAIN_3_EXECUTION_SUPERVISOR.md](03_DOMAIN_3_EXECUTION_SUPERVISOR.md)                                                                 |
| **2. Plugin Lifecycle & Package Manager** _(Rust)_         | Directory scanner, manifest loader, state persistence (`plugin_state.json`), safe ZIP archive installer with Zip-Slip defense | **M2.2** (Registration)<br>**M2.3** (Discovery)<br>**M2.4** (Installation)<br>**M2.5** (Enable/Disable) | `src-tauri/src/plugins/registry.rs`<br>`src-tauri/src/plugins/installer.rs`                                                                      | [01_DOMAIN_1_DISCOVERY_REGISTRY.md](01_DOMAIN_1_DISCOVERY_REGISTRY.md)<br>[02_DOMAIN_2_LIFECYCLE_STATE.md](02_DOMAIN_2_LIFECYCLE_STATE.md) |
| **3. Data Bridge & Result Ingestor** _(Rust/Data)_         | Payload assembler, pre-flight input validator (image path, CRS, AOI), execution result parser, artifact validator             | **M2.6** (Config Backend)<br>**M2.8** (Result Handling)<br>Pre-flight Check                             | `src-tauri/src/plugins/payload.rs`<br>`src-tauri/src/plugins/result_handler.rs`<br>`src-tauri/src/plugins/models.rs`                             | [03_DOMAIN_3_EXECUTION_SUPERVISOR.md](03_DOMAIN_3_EXECUTION_SUPERVISOR.md)                                                                 |
| **4. Frontend UI & Extension UX Engineer** _(React/TS)_    | Extension manager UI, Blueprint.js cards, status badges, dynamic parameter forms, execution launcher modal                    | **M2.10** (Info Viewer)<br>**M2.6** (Dynamic Form UI)                                                   | `src/components/plugins/PluginManager.tsx`<br>`src/components/plugins/DynamicParamForm.tsx`<br>`src/components/plugins/PluginExecutionModal.tsx` | [04_DOMAIN_4_FRONTEND_COMPONENTS.md](04_DOMAIN_4_FRONTEND_COMPONENTS.md)                                                                   |

---

## 4. Global Inter-Module Connector Matrix

Module 2 provides standard services to all surrounding platform modules:

| Connector Function (Module 2)            | Provider / Caller     | Consumer Modules                 | Exchanged Data Objects                                                                                                          |
| :--------------------------------------- | :-------------------- | :------------------------------- | :------------------------------------------------------------------------------------------------------------------------------ |
| **`get_installed_plugins`**              | Module 2 Registry     | **Module 1, 3, 11**              | `Vec<PluginSummaryDto>` (Status, Enabled, Metadata)                                                                             |
| **`query_compatible_plugins`**           | Module 1, 3, 10       | **Module 1, 3, 10**              | **Input**: `TargetDescriptorDto`<br>**Output**: Compatible `Vec<PluginSummaryDto>`                                              |
| **`get_plugin_details`**                 | Module 2 Registry     | **Module 1, 3, 11**              | Full resolved contract: `Manifest`, `Parameters`, `Inputs`, `Outputs`                                                           |
| **`start_plugin_job`**                   | Module 1, 3, 10       | **Module 4, 7, 8** (Plugins)     | **Input**: `StartJobRequestDto`<br>**Output**: `JobHandleDto` (`job_id`)                                                        |
| **`abort_plugin_job`**                   | Module 3, 11 (UI)     | OS Child Subprocess              | **Input**: `job_id: String`<br>**Output**: `Result<(), String>`                                                                 |
| **`get_job_status`**                     | Module 2 Supervisor   | **Module 1, 3, 11**              | `JobStatusDto` (`Queued`, `Running`, `Completed`, `Failed`, `Aborted`)                                                          |
| **`get_job_result`**                     | Module 2 Result Store | **Module 3, 5, 6, 7, 9, 10, 11** | Full `StandardJobResultDto`:<br>• **Scalar Metrics** (coverage %, tree count)<br>• **Artifact Descriptors** (PNG mask, GeoJSON) |
| **`emit_job_progress`** _(Event Stream)_ | Subprocess (Stdout)   | **Module 3, 11** (Task Monitor)  | Tauri Event: `app.emit("plugin://progress", JobProgress)`                                                                       |

---

## 5. Documentation Suite Index

To avoid context overload, specific technical designs and code specifications are modularized into domain guides:

1. **[01_DOMAIN_1_DISCOVERY_REGISTRY.md](01_DOMAIN_1_DISCOVERY_REGISTRY.md)**:
   Plugin scanning, JSON schema validation, sub-contract bundle resolution, compatibility querying, and health checking.
2. **[02_DOMAIN_2_LIFECYCLE_STATE.md](02_DOMAIN_2_LIFECYCLE_STATE.md)**:
   ZIP archive installation with Zip-Slip path traversal defense, staging rollback, and atomic state persistence (`plugin_state.json`).
3. **[03_DOMAIN_3_EXECUTION_SUPERVISOR.md](03_DOMAIN_3_EXECUTION_SUPERVISOR.md)**:
   CLI invocation (`--input` and `--output`), subprocess lifecycle supervision, progress streaming, timeout enforcement, and artifact/metric validation.
4. **[04_DOMAIN_4_FRONTEND_COMPONENTS.md](04_DOMAIN_4_FRONTEND_COMPONENTS.md)**:
   React 19 + Blueprint.js 6 UI suite, dynamic parameter form generation, client-side validation, and the self-contained `<PluginExecutionModal />`.
5. **[05_SYSTEM_INTEGRATION_AND_CONFIG.md](05_SYSTEM_INTEGRATION_AND_CONFIG.md)**:
   Cargo crate dependencies, Tauri 2 ACL permissions (`capabilities/default.json`), and CI pre-commit verification gates.
