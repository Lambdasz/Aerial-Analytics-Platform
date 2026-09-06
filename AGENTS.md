# AGENTS.md

This file provides guidance to AI coding agents (Claude Code, and other agents that read AGENTS.md) when working with code in this repository.

## Conventions

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in [BCP 14](https://www.rfc-editor.org/bcp/bcp14) ([RFC2119](https://www.rfc-editor.org/bcp/bcp14)) ([RFC8174](https://www.rfc-editor.org/info/rfc8174/)) when, and only when, they appear in all capitals, as shown here.

All documentation in this repository (this file, README.md, and any future docs) MUST be kept up to date as the code changes — do not let documented commands, architecture, or project scope drift out of sync with reality. When you change something a doc describes, update that doc in the same change.

## Project

Aerial Analytics Platform: a desktop app to process and analyze RGB aerial imagery captured by standard drones, using a plugin-based architecture so analytical capabilities can be added independently without modifying the core system. This is a university PBL final project (2026); the lecturer's stated goals are verifiable outcomes, adherence to functional programming principles, and preparing the project for open-source release. The repo is currently a fresh Tauri + React scaffold — the plugin architecture and analysis features are not yet implemented (`src-tauri/src/lib.rs` only has the default Tauri `greet` command).

### Planned modules (per project brief, not yet implemented)

- **Foundation**: image management, project organization, plugin architecture
- **Analysis**: vegetation detection, land cover analysis, tree counting, coverage measurement, vegetation condition assessment, temporal (multi-date) change analysis
- **Visualization & reporting**: interactive maps/dashboards, structured reports with GIS and statistical exports

Cross-cutting requirements: geospatial exploration, standardized plugin input/output contracts, plot-based analytics, multi-date comparison, and platform stability even when a plugin fails (a failing plugin MUST NOT crash the core app).

## Tech Stack

- Frontend: TypeScript, React 19, Blueprint.js (UI components), Vite
- Backend/desktop shell: Rust, Tauri 2
- Analytical/computational: Python, planned for AI and heavier computation that plugins need (not yet integrated into the repo/build)
- Tooling: ESLint 9 + typescript-eslint (flat config), Prettier 3, Clippy, rustfmt, Husky, commitlint (Conventional Commits)

## Commands

```bash
npm ci                  # ALWAYS use this over `npm install` (lockfile-locked, avoids lockfile merge conflicts)

npm run dev              # Vite dev server only (web, no Tauri)
npm run tauri dev        # Full desktop dev (Vite + Tauri/Rust)
npm run build             # tsc typecheck + vite build
npm run tauri build       # Build distributable desktop app

npm run lint              # ESLint
npm run lint:fix          # ESLint --fix
npm run lint:rust         # cargo clippy --all-targets -D warnings (manifest: src-tauri/Cargo.toml)
npm run format:check      # Prettier check
npm run format             # Prettier write
npm run format:rust:check # cargo fmt --check (manifest: src-tauri/Cargo.toml)
npm run format:rust       # cargo fmt
```

There is no test runner configured yet.

Pre-commit hook (`.husky/pre-commit`) runs, in order: `lint`, `lint:rust`, `format:check`, `format:rust:check`. All MUST pass to commit. Commit messages are validated by commitlint against Conventional Commits (`.husky/commit-msg`, `commitlint.config.js`).

## Git Conventions

All commits and pull requests MUST be atomic — one logical change per commit/PR, nothing unrelated bundled in.

Commit messages MUST follow Conventional Commits:

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

A blank line is REQUIRED before the body (if present) and before the footer(s) (if present).

`<type>` MUST be one of: `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`, `revert`, `style`, `test`.

## Architecture

- `src/` — React frontend (entry: `src/main.tsx`, root component: `src/App.tsx`). Communicates with the Rust backend via Tauri's `invoke` (`@tauri-apps/api`) calling `#[tauri::command]` functions.
- `src-tauri/src/lib.rs` — Rust backend entry point (`run()`), where Tauri commands are registered via `invoke_handler(tauri::generate_handler![...])` and plugins are registered via `.plugin(...)`.
- `src-tauri/capabilities/default.json` — Tauri's permission/ACL system: any new Tauri command or plugin capability used from the frontend MUST be explicitly allowed here for the `main` window.
- `src-tauri/tauri.conf.json` — app identifier, window config, dev server URL (fixed at `http://localhost:1420`, MUST match `vite.config.ts`), and bundling config.
- Vite dev server port is fixed to `1420` (`strictPort: true`) because Tauri's `devUrl` expects it; `src-tauri/**` is excluded from Vite's file watcher.

Since the plugin-based analytics architecture is not built yet, when implementing it, look for how new Tauri commands/plugins are wired (`invoke_handler`, `.plugin()` in `lib.rs`, and the corresponding capability entries) as the seam where new backend functionality gets exposed to the frontend. Keep the "a failing plugin MUST NOT crash the core app" requirement in mind when designing the plugin boundary — analysis plugins (especially any that shell out to or embed Python) should not be able to take down the Tauri process.
