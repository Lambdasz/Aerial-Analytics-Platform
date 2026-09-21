# Contributing to Aerial Analytics Platform

This document covers the workflow, conventions, and quality gates you'll need to know before opening a pull request.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Branching](#branching)
- [Commit Conventions](#commit-conventions)
- [Code Style & Quality Gates](#code-style--quality-gates)
- [Architecture Notes](#architecture-notes)
- [Keeping Docs in Sync](#keeping-docs-in-sync)
- [Pull Requests](#pull-requests)
- [Reporting Bugs & Requesting Features](#reporting-bugs--requesting-features)

## Getting Started

Follow the [README's Getting Started guide](README.md#getting-started) to install the Tauri system
dependencies, Rust/cargo, and Node.js/npm, then clone the repo.

Once cloned, install JS dependencies with:

```bash
npm ci
```

> [!CAUTION]
> ALWAYS use `npm ci` (clean install), NOT `npm install`, so you get the lockfile-locked versions and
> avoid merge conflicts on `package-lock.json`.

## Development Workflow

- `npm run dev` — Vite dev server only (web, no Tauri).
- `npm run tauri dev` — full desktop dev (Vite + Tauri/Rust). The first run downloads and builds all
  Rust dependencies from `src-tauri/Cargo.lock`, so it can take a while.
- `npm run build` — typecheck + Vite build (web).
- `npm run tauri build` — build the distributable desktop app.

## Branching

Branch off `main` using the same `<type>/<short-kebab-description>` pattern used throughout this repo
(e.g. `chore/scaffold-project`, `feat/plugin-loader`), where `<type>` matches one of the commit types
below.

## Commit Conventions

All commits and pull requests MUST be **atomic** — one logical change per commit/PR, nothing unrelated
bundled in.

Commit messages MUST follow [Conventional Commits](https://www.conventionalcommits.org/), enforced by
commitlint (`.husky/commit-msg`, `commitlint.config.js`):

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

- A blank line is REQUIRED before the body (if present) and before the footer(s) (if present).
- `<type>` MUST be one of: `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`, `revert`,
  `style`, `test`.
- The `body-max-line-length` rule is disabled, so wrap body text however reads best.

## Code Style & Quality Gates

| Task                            | Command                     |
| ------------------------------- | --------------------------- |
| Run ESLint                      | `npm run lint`              |
| Fix ESLint issues               | `npm run lint:fix`          |
| Run Rust linting (clippy)       | `npm run lint:rust`         |
| Check Prettier formatting       | `npm run format:check`      |
| Fix Prettier formatting issues  | `npm run format`            |
| Check Rust formatting (rustfmt) | `npm run format:rust:check` |
| Fix Rust formatting issues      | `npm run format:rust`       |

The `.husky/pre-commit` hook runs, in order: `lint`, `lint:rust`, `format:check`, `format:rust:check`.
**All four MUST pass to commit.** Before committing, it's easiest to just run:

```bash
npm run lint:fix
npm run format
npm run format:rust
```

There is no test runner configured yet in this repo — don't reference one in your changes or PR
description.

These same four checks — plus a Vite/Tauri build check on macOS, Ubuntu, and Windows — are enforced in
CI (`.github/workflows/ci.yml`) on every push and pull request to `main`. Bypassing the pre-commit hook
(e.g. with `--no-verify`) only skips the local check; the PR will still fail CI if the gates don't pass.

## Architecture Notes

See [README.md § Project Structure](README.md#project-structure) for the full annotated directory
tree, including planned-but-not-yet-created directories. The notes below cover where new code
belongs within that layout — keep both docs up to date as the layout evolves.

- `src/` is the React frontend (entry: `src/main.tsx`, root component: `src/App.tsx`). It talks to the
  Rust backend via Tauri's `invoke` (`@tauri-apps/api`), which calls `#[tauri::command]` functions.
- `src-tauri/src/lib.rs` is the Rust backend entry point (`run()`), where commands are registered via
  `invoke_handler(tauri::generate_handler![...])` and plugins via `.plugin(...)`.
- Any new Tauri command or plugin capability used from the frontend MUST be explicitly allowed in
  `src-tauri/capabilities/default.json` for the `main` window.
- The Vite dev server port is fixed at `1420` (`strictPort: true`) and MUST match `devUrl` in
  `src-tauri/tauri.conf.json`.
- The project favors **functional programming principles** where practical (a stated project goal) —
  prefer pure functions and immutable data over mutation and side effects when designing new modules.
- This project uses a plugin-based architecture for analytical capabilities. A failing plugin
  (including one that shells out to or embeds Python) **MUST NOT** crash the core app — keep that
  boundary in mind when touching plugin-related code.

**Where new code goes:**

- **New Tauri command** → a module in `src-tauri/src/commands/`, registered in the
  `invoke_handler(tauri::generate_handler![...])` list in `src-tauri/src/lib.rs`, **and** allowed for
  the `main` window in `src-tauri/capabilities/default.json`. All three steps are REQUIRED.
- **Command bodies MUST stay thin** — parse/validate input, call into `services/`, map errors. Logic
  that could be unit-tested belongs in `services/`, not in a `#[tauri::command]` function.
- **New data model (struct/enum)** → `src-tauri/src/models/`. If it crosses the IPC boundary, derive
  `Serialize`/`Deserialize` and add the matching TS type next to the caller
  (`src/features/<feature>/types.ts`, or `src/types/` once 2+ features need it).
- **New helper function** → `src-tauri/src/services/` (Rust) or `src/lib/` (TypeScript). Prefer pure
  functions and immutable data — functional programming is a stated project goal.
- **New UI** → start inside `src/features/<feature>/`. Promote a component/hook to the shared
  `src/components/`, `src/hooks/`, `src/lib/`, or `src/types/` only once a second feature needs it.
- **Analysis plugins live in their own, separate repositories** — this repo is the platform core, not
  a home for plugin implementations. `src-tauri/src/plugins/` is the plugin _manager_: discovery,
  loading, and process isolation for externally developed plugins. A failing plugin MUST NOT crash
  the core app, so all plugin execution MUST stay behind that boundary.

## Keeping Docs in Sync

All documentation (`README.md`, `AGENTS.md`, `CONTRIBUTING.md`, and any future docs) MUST be kept up to
date as the code changes. If your change affects a documented command, architecture, or behavior,
update the relevant doc in the **same** change.

## Pull Requests

- Keep PRs atomic — one logical change, nothing unrelated bundled in.
- Fill in the pull request template.
- Make sure all four pre-commit checks pass locally before pushing.
- Link related issues (e.g. `Closes #123`).

## Reporting Bugs & Requesting Features

Use the issue templates (Bug Report / Feature Request) when opening a new issue — they'll prompt you
for the details maintainers need to triage.

For security vulnerabilities, **do not** open a public issue — see [SECURITY.md](SECURITY.md) instead.
