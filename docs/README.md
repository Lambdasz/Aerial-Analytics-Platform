# Aerial Analytics Platform Documentation

Welcome to the documentation repository for the **Aerial Analytics Platform** (Tauri 2 + React 19 desktop application for aerial image analysis).

## Repository Documentation Index

| Section                                   | Description                                                              | Target Path                                                |
| :---------------------------------------- | :----------------------------------------------------------------------- | :--------------------------------------------------------- |
| **System Overview & Getting Started**     | General architecture, setup guide, and PBL 2026 requirements             | [`README.md`](../README.md)                                |
| **Contributing Guidelines**               | Development workflow, branching, commit convention, and gates            | [`CONTRIBUTING.md`](../CONTRIBUTING.md)                    |
| **Agent Instructions**                    | Rules and boundaries for AI coding assistants                            | [`AGENTS.md`](../AGENTS.md)                                |
| **Security Policy**                       | Reporting vulnerabilities and isolation model                            | [`SECURITY.md`](../SECURITY.md)                            |
| **Spatial Results Specification**         | Data contract, schema, and storage mechanics (Bilingual: EN / ID)        | [`spatial_result.md`](spatial_result.md)                   |
| **Module 03 — Aerial Image Map Explorer** | Full architectural and implementation documentation (Bilingual: EN / ID) | [`modules/module_3/README.md`](modules/module_3/README.md) |

---

## Module Index

The Aerial Analytics Platform is divided into modular domains. Module 03 serves as the primary spatial visualization and persistence engine:

- [Module 03 — Aerial Image Map Explorer](modules/module_3/README.md)
  - Interactive Leaflet explorer
  - Area of Interest (AOI) polygon management
  - Spatial detection & classification result inspection
  - Orthomosaic GeoTIFF & raster tiling engine
  - Disk-backed single-instance storage subsystem (`map_storage/`)

---

## Repository Markdown Files Inventory & Fate

To maintain documentation integrity across the codebase, the following table lists all Markdown files in the repository, their purpose, and their designated fate:

| File Path                                     | Purpose                                                                       | Status / Fate                                    |
| :-------------------------------------------- | :---------------------------------------------------------------------------- | :----------------------------------------------- |
| `README.md`                                   | Primary project repository overview and quickstart                            | Retained at root                                 |
| `CONTRIBUTING.md`                             | Development workflow, branching conventions, pre-commit gates                 | Retained at root                                 |
| `AGENTS.md`                                   | Instructions for automated AI coding agents                                   | Retained at root                                 |
| `SECURITY.md`                                 | Security policy and vulnerability disclosure protocol                         | Retained at root                                 |
| `LICENSE.md`                                  | Apache License 2.0 terms                                                      | Retained at root                                 |
| `CLAUDE.md`                                   | Assistant guidance file                                                       | Retained at root                                 |
| `.github/pull_request_template.md`            | Standard pull request checklist                                               | Retained in `.github/`                           |
| `docs/README.md`                              | Central documentation index (this file)                                       | Active documentation index                       |
| `docs/spatial_result.md`                      | Full specification & storage architecture for spatial results (Bilingual)     | Active specification document                    |
| `docs/modules/module_3/README.md`             | Module 3 landing page and language switcher                                   | Active documentation index                       |
| `plugins/*/README.md`                         | Documentation for individual plugins (`template`, `mock_rust`, etc.)          | Retained in respective plugin directories        |
| `src-tauri/src/plugin_manager/DOCS/*.md`      | Deep architectural documentation for the plugin supervisor                    | Retained in plugin manager directory             |
| `src-tauri/src/reporting/RULES_REPORTING.md`  | Engineering rules for the reporting module                                    | Retained in reporting directory                  |
| `src/modules/module-03/RULES_MAP_FR.md`       | Local engineering rules for Frontend Module 3 (Leaflet, state, atomic design) | Retained as local frontend contributor guide     |
| `src/modules/module-03/README.md`             | Quickstart guide for frontend map module                                      | Retained in module folder                        |
| `src/modules/module-03/REQUIREMENTS.md`       | Feature specifications and acceptance criteria                                | Retained in module folder                        |
| `src/modules/module-03/docs/Proyek.md`        | Local copy of course PBL project specification                                | Retained in module docs                          |
| `src/modules/module-03/docs/README_UMUM.md`   | General overview of module components                                         | Retained in module docs                          |
| `src/modules/module-03/docs/README_modul3.md` | Detailed module 3 notes and specifications                                    | Retained in module docs                          |
| `implementasi.md`                             | Early implementation notes for Module 3 backend                               | Migrated to `docs/modules/module_3/` and removed |
| `implementasi-frontend.md`                    | Early implementation notes for Module 3 frontend                              | Migrated to `docs/modules/module_3/` and removed |
| `audit.md`                                    | Comprehensive architectural audit report (25 findings)                        | Migrated to `docs/modules/module_3/` and removed |
| `CLEANUP_LOG.md`                              | Execution log of dead code cleanup phases                                     | Migrated to `docs/modules/module_3/` and removed |

---

## Language Policy

All architecture documents within `docs/modules/module_3/` are strictly maintained with identical structure and parity across two languages:

- **English**: [`docs/modules/module_3/en/`](modules/module_3/en/project-structure.md)
- **Bahasa Indonesia**: [`docs/modules/module_3/id/`](modules/module_3/id/project-structure.md)
