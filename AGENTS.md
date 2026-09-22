# AGENTS.md

Instructions for AI coding agents working in this repository. Every line answers: "Would an agent likely miss this without help?"

## Project

Aerial Analytics Platform — a Tauri 2 + React desktop app for processing RGB aerial imagery via a plugin-based architecture. University PBL project (2026). 11 planned modules; several are partially implemented. Favors functional programming principles.

## Commands

```bash
npm ci                  # ALWAYS use this over npm install (lockfile-locked, avoids lockfile conflicts)

npm run dev              # Vite dev server only (web, no Tauri)
npm run tauri dev        # Full desktop dev (Vite + Tauri/Rust) — first run is slow (builds Rust deps)
npm run build             # tsc typecheck + vite build
npm run tauri build       # Build distributable desktop app (no bundle in CI: --no-bundle)

npm run lint              # ESLint (TS/TSX)
npm run lint:fix          # ESLint --fix
npm run lint:rust         # cargo clippy --all-targets -D warnings (manifest: src-tauri/Cargo.toml)
npm run format:check      # Prettier check
npm run format            # Prettier write
npm run format:rust:check # cargo fmt --check (manifest: src-tauri/Cargo.toml)
npm run format:rust       # cargo fmt

cargo doc --manifest-path src-tauri/Cargo.toml --document-private-items --open  # generate rustdoc
```

**There is no test runner configured yet.** Do not reference one in changes or PR descriptions.

### Pre-commit gates (all four MUST pass)

`.husky/pre-commit` runs in order: `lint`, `lint:rust`, `format:check`, `format:rust:check`. Quick fix before committing:

```bash
npm run lint:fix && npm run format && npm run format:rust
```

Commit messages are validated by commitlint (Conventional Commits). CI (`.github/workflows/ci.yml`) runs the same checks plus a cross-platform build (macOS, Ubuntu, Windows).

## Git Conventions

- Atomic commits/PRs — one logical change, nothing unrelated bundled in.
- Branch naming: `<type>/<short-kebab-description>` (e.g. `feat/plugin-loader`), where `<type>` matches Conventional Commits types.
- Commit message format:
  ```
  <type>[optional scope]: <description>

  [optional body]

  [optional footer(s)]
  ```
  - Blank line REQUIRED before body (if present) and before footer(s) (if present).
  - `<type>` MUST be one of: `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`, `revert`, `style`, `test`.
  - `body-max-line-length` is disabled (per `commitlint.config.js`).

## Architecture

### Tech stack

- Frontend: TypeScript, React 19, Blueprint.js, Leaflet/react-leaflet, Vite 8
- Backend: Rust, Tauri 2
- Tooling: ESLint 9 (flat config) + typescript-eslint, Prettier 3, Clippy, rustfmt, Husky, commitlint

### Directory layout

- `src/` — React frontend. Entry: `src/main.tsx`, root: `src/App.tsx` → `src/shell/AppShell.tsx`.
  - `src/shell/` — App shell, sidebar, module routing.
  - `src/map/` — Leaflet map explorer, AOI drawing, layer management.
  - `src/features/` — Feature modules (mostly scaffolding).
  - `src/components/`, `src/hooks/`, `src/lib/`, `src/types/` — shared code.
- `src-tauri/src/` — Rust backend. Entry: `lib.rs` → `run()`.
  - `plugin_manager/` — Plugin discovery, lifecycle, async execution, error isolation.
  - `commands/` — Tauri IPC commands (thin handlers).
  - `models/` — Shared Rust types (must derive `Serialize`/`Deserialize` if crossing IPC boundary).
  - `services/` — Business logic (commands delegate here).
  - `modules/` — Module-specific logic (module_01 metadata, module_10 temporal).
  - `map_controller/` — Map data provider and AOI storage.
  - `reporting/` — Reporting module.
- `plugins/` — Plugin implementations (mock, mock_rust, rgb-vegetation-detection, rgb-landcover-classification, template).
- `schemas/` — JSON Schema definitions for plugin manifests, execution payloads, and results.

### Adding new Tauri commands (3 required steps)

1. Implement the `#[tauri::command]` function (thin — parse/validate, delegate to `services/`, map errors).
2. Register it in `invoke_handler(tauri::generate_handler![...])` in `src-tauri/src/lib.rs`.
3. Add the permission to `src-tauri/capabilities/default.json` for the `main` window.

### Plugin contract

Plugins declare their interface via `manifest.json` (validated against `schemas/plugin.schema.json`). Each plugin directory contains: `manifest.json`, `parameters.json`, `inputs.json`, `outputs.json`, and an entrypoint (`main.py` for Python plugins). See `plugins/template/` for the canonical starter.

Runtime types: `python`, `binary`, `wasm`. Plugins communicate results via `payload.json` / `result.json` files. The executor spawns plugins as async subprocesses with timeout enforcement and stdout progress streaming (`PROGRESS: {...}` protocol).

### Key constraints

- **Failing plugins MUST NOT crash the core app.** All plugin execution is isolated via async subprocesses (Tokio). Design plugin boundaries with this in mind.
- **Vite port is fixed at 1420** (`strictPort: true` in `vite.config.ts`) because Tauri's `devUrl` expects it (`src-tauri/tauri.conf.json`). `src-tauri/**` is excluded from Vite's file watcher.
- **TypeScript strict mode** is enabled (`tsconfig.json`): `noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch`.
- **Prettier**: semicolons on, double quotes, trailing commas, 100 char print width.

### Where new code goes

- New Tauri command → `src-tauri/src/commands/` + register in `lib.rs` + add capability in `default.json`.
- Command bodies MUST stay thin — logic belongs in `services/`.
- New Rust data model → `src-tauri/src/models/`.
- New TypeScript helper → `src/lib/`.
- New UI → `src/features/<feature>/`. Promote to shared `src/components/` only when 2+ features need it.
- New plugin → its own directory under `plugins/` following the template structure.

## Reference files

- `docs/Proyek.md` — full project brief (11 modules, feature lists).
- `CONTRIBUTING.md` — full development workflow, branching, PR process.
- `README.md` — project overview and getting started.
- `src-tauri/src/plugin_manager/DOCS/` — detailed plugin system design docs (6 files).
- `.github/workflows/ci.yml` — CI pipeline definition.
- `.github/pull_request_template.md` — PR checklist.
