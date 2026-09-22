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

cargo test --manifest-path src-tauri/Cargo.toml                               # Rust unit + doc tests
cargo doc --manifest-path src-tauri/Cargo.toml --document-private-items --open  # generate rustdoc
```

**No JS/TS test runner** (no `test` script in `package.json`) — don't mention one in changes or PRs. Rust tests exist (`cargo test`, see above). Plugin tests: `python -m unittest discover -s tests -v` from inside the plugin dir (e.g. `plugins/rgb-landcover-classification/`; needs the plugin's Python deps).

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
  - `src/modules/module-NN/` — module pages (placeholder shells for most).
  - `src/features/`, `src/component_plugin_manager/` — feature code (mostly scaffolding).
  - `src/components/`, `src/hooks/`, `src/lib/`, `src/types/` — shared code.
- `src-tauri/src/` — Rust backend. Entry: `lib.rs` → `run()`; crate docs there are the module index (see rustdoc convention below).
  - `plugin_manager.rs` + `plugin_manager/` — plugin discovery, lifecycle, async execution, error isolation. The entry is the file `plugin_manager.rs`; never reintroduce `plugin_manager/mod.rs` alongside it (Rust rejects both existing).
  - `commands/` — Tauri IPC commands (thin handlers).
  - `models/` — Shared Rust types (must derive `Serialize`/`Deserialize` if crossing IPC boundary).
  - `services/` — Business logic (commands delegate here).
  - `modules/` — Module-specific logic (module_01 metadata, module_10 temporal).
  - `map_controller/` — map backend (geometry, AOI, layers, spatial results); deep docs live in `map_controller.rs`.
  - `reporting/` — Reporting module.
- `plugins/` — Plugin implementations (mock, mock_rust, rgb-vegetation-detection, rgb-landcover-classification, template).
- `schemas/` — JSON Schema definitions for plugin manifests, execution payloads, and results.

### Adding new Tauri commands (2 required steps)

1. Implement the `#[tauri::command]` function (thin — parse/validate, delegate to `services/`, map errors).
2. Register it in `invoke_handler(tauri::generate_handler![...])` in `src-tauri/src/lib.rs`.

**Do NOT add capability entries for custom commands.** They are auto-allowed; an unknown `allow-*` entry in `src-tauri/capabilities/default.json` breaks the Tauri build script ("Permission ... not found"). That file is only for plugin/core permissions (e.g. `opener:default`, `dialog:default`). (CONTRIBUTING.md still claims otherwise — the build script is the source of truth.)

### Rustdoc: module entries in `lib.rs`

Each module gets a `## Module N — Name` section in the crate docs at the top of `src-tauri/src/lib.rs`, separated by `//! ---`. Follow the Module 1/2/3 shape:

1. One-paragraph description.
2. Component bullets — backticked module paths. If the module has deep docs, put them in the submodule's own rustdoc and link to it (e.g. ``[`map_controller`]``) instead of inlining detail; keep the `lib.rs` entry short. The linked module must be `pub` or the intra-doc link warns.
3. `### Tauri Commands` table (`| Command | Description |`) listing that module's registered commands. Plugin modules (4/7/8) instead use a `### Plugin Contract` table (Architecture/Interface/Entrypoint/CLI/Execution/Progress) plus a `main()` code block — `python` fences for `main.py`, `rust,ignore` for binary entrypoints (a bare `rust` `fn main` doctest fails clippy's `needless_doctest_main`). Escape `|` inside table cells as `\|`.
4. A `**Status**:` paragraph — what's implemented vs `todo!()` stubs.

There is also a `## Development Priorities` table right after the intro; keep it in sync when module scope changes.

### Plugin contract

Plugins declare their interface via `manifest.json` (validated against `schemas/plugin.schema.json`). Each plugin directory contains: `manifest.json`, `parameters.json`, `inputs.json`, `outputs.json`, and an entrypoint (`main.py` for Python plugins). See `plugins/template/` for the canonical starter.

Runtime types: `python`, `binary`, `wasm`. Plugins communicate results via `payload.json` / `result.json` files. The executor spawns plugins as async subprocesses with timeout enforcement and stdout progress streaming (`PROGRESS: {...}` protocol).

### Key constraints

- **Failing plugins MUST NOT crash the core app.** All plugin execution is isolated via async subprocesses (Tokio). Design plugin boundaries with this in mind.
- **Vite port is fixed at 1420** (`strictPort: true` in `vite.config.ts`) because Tauri's `devUrl` expects it (`src-tauri/tauri.conf.json`). `src-tauri/**` is excluded from Vite's file watcher.
- **TypeScript strict mode** is enabled (`tsconfig.json`): `noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch`.
- **Prettier**: semicolons on, double quotes, trailing commas, 100 char print width.

### Where new code goes

- New Tauri command → `src-tauri/src/commands/` + register in `lib.rs` (no capability entry — see above).
- Command bodies MUST stay thin — logic belongs in `services/`.
- New Rust data model → `src-tauri/src/models/`.
- New TypeScript helper → `src/lib/`.
- New UI → `src/features/<feature>/` (note: pages currently live in `src/modules/module-NN/`). Promote to shared `src/components/` only when 2+ features need it.
- New plugin → its own directory under `plugins/` following the template structure.
- Module docs → update that module's `## Module N` entry in `lib.rs` (rustdoc convention above) in the same change.

## Reference files

- `src-tauri/src/lib.rs` crate docs — full project brief (11 modules, possible features) + module index/status (see rustdoc convention above).
- `CONTRIBUTING.md` — full development workflow, branching, PR process.
- `README.md` — project overview and getting started.
- `src-tauri/src/plugin_manager/DOCS/` — detailed plugin system design docs (6 files).
- `.github/workflows/ci.yml` — CI pipeline definition.
- `.github/pull_request_template.md` — PR checklist.
