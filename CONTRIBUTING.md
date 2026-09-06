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

## Architecture Notes

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
