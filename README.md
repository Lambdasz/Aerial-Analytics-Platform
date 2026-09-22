# Aerial Analytics Platform

The Aerial Analytics Platform is designed to process and analyze RGB aerial imagery captured using standard drones. The platform uses a plugin-based architecture so that analytical capabilities can be added independently without modifying the core system.

[![TypeScript](https://img.shields.io/badge/TypeScript-6-3178C6?logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Rust](https://img.shields.io/badge/Rust-2021-CE422B?logo=rust&logoColor=white)](https://rust-lang.org/)
[![Blueprint](https://img.shields.io/badge/Blueprint.js-6-2d72d2?style=flat&logo=blueprint&logoColor=white)](https://blueprintjs.com/)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=white)](https://react.dev/)
[![Vite](https://img.shields.io/badge/Vite-8-646CFF?logo=vite&logoColor=white)](https://vite.dev/)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![ESLint](https://img.shields.io/badge/ESLint-9-4B3BDB?logo=eslint&logoColor=white)](https://eslint.org/)
[![Prettier](https://img.shields.io/badge/Prettier-3-F7B93E?logo=prettier&logoColor=white)](https://prettier.io/)
[![commitlint](https://img.shields.io/badge/commitlint-20-a8b1ff?style=flat&logo=commitlint&logoColor=white)](https://commitlint.js.org/)
[![Husky](https://img.shields.io/badge/Husky-9-000?logo=husky&logoColor=white)](https://typicode.github.io/husky/)
[![Node](https://img.shields.io/badge/Node.js-%3E%3D24-339933?logo=nodedotjs&logoColor=white)](https://nodejs.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE.md)

## Table of Contents

<!-- prettier-ignore-start -->
<!-- TOC -->
* [Aerial Analytics Platform](#aerial-analytics-platform)
  * [Table of Contents](#table-of-contents)
  * [Tech Stack](#tech-stack)
  * [Getting Started](#getting-started)
    * [1. Clone the repo](#1-clone-the-repo)
    * [2. Install project dependencies](#2-install-project-dependencies)
    * [3. Development](#3-development)
  * [Dealing with Dependencies](#dealing-with-dependencies)
  * [Available Scripts](#available-scripts)
  * [Continuous Integration](#continuous-integration)
  * [Project Structure](#project-structure)
  * [Contributing](#contributing)
  * [Security](#security)
  * [Copyright & License](#copyright--license)
<!-- TOC -->
<!-- prettier-ignore-end -->

## Tech Stack

- **Frontend Language**: [TypeScript 6](https://www.typescriptlang.org/)
- **Backend Language**: [Rust](https://rust-lang.org/)
- **UI Framework**: [React 19](https://react.dev/)
- **UI Component Library**: [Blueprint.js](https://blueprintjs.com/)
- **Map**: [Leaflet](https://leafletjs.com/) / [react-leaflet](https://react-leaflet.js.org/)
- **Build Tool & Web Server**: [Vite 8](https://vite.dev/)
- **Desktop App Framework**: [Tauri 2](https://v2.tauri.app/)
- **Node**: [Node.js 24](https://nodejs.org/en)
- **Development & Tooling**:
  - Code Linting: [ESLint 9](https://eslint.org/), [Clippy](https://github.com/rust-lang/rust-clippy)
  - Commit Message Linting: [commitlint 20](https://commitlint.js.org/)
  - Formatting: [Prettier 3](https://prettier.io/), [rustfmt](https://github.com/rust-lang/rustfmt)
  - Git Hooks: [Husky 9](https://typicode.github.io/husky/)

## Getting Started

In order to start building this project, you'll first need to install a few dependencies:

1. Tauri System Dependencies
2. Rust with cargo package manager
3. Node.js with npm package manager

Refer to Tauri's short, very straightforward [Prerequisites Guide](https://v2.tauri.app/start/prerequisites/) to help you prepare.

### 1. Clone the repo

```bash
git clone https://github.com/Lambdasz/Aerial-Analytics-Platform.git

# Or if you prefer SSH:
git clone git@github.com:Lambdasz/Aerial-Analytics-Platform.git

# Then, enter the directory
cd Aerial-Analytics-Platform
```

### 2. Install project dependencies

```bash
npm ci
```

> [!CAUTION]
> ALWAYS USE `npm ci` (clean install), NOT `npm install` to ensure you used the lockfile-locked versions and avoid merge conflict on the lockfile.

### 3. Development

To start the development server only without Tauri, run:

```bash
npm run dev
```

To start both development server and run Tauri for Desktop development, run:

```bash
npm run tauri dev
```

> [!NOTE]
> This command will make the Rust package manager (cargo) to download and build all the required packages from `src-tauri/Cargo.lock`. First run may take a while since these packages are not cached on your machine yet.

To build the Tauri app for distribution, run:

```bash
npm run tauri build
```

## Dealing with Dependencies

> [!WARNING]
> **STOP!** If you do really want to do something with this project dependencies, you MUST:
>
> 1. Identify what dependencies you want to add. Check if they are compatible with current tech stack.
> 2. Notify [Project Lead](https://github.com/muhammadzaini213) or [Module 1 Lead](https://github.com/andinaufal120) that you want to add dependencies.
> 3. `git pull`, and create a new branch (e.g. `chore/add-react-router`, `chore/bump-eslint`) from `main`.
> 4. Update/add dependencies (steps described below), commit, push, and open a Pull Request (PR) against `main` immediately.
> 5. Notify [Project Lead](https://github.com/muhammadzaini213) or [Module 1 Lead](https://github.com/andinaufal120) that you have completed your work and one of them will merge your PR onto main.
> 6. Checkout to another local branch (such as `main`), and run `git pull` immediately.
>
> Dependency-related branches are intended to be **very short-lived** (i.e. must be merged and deleted under 1 day after creating the branch). Chaos forces us to put dependencies guideline under the root README instead on CONTRIBUTING.md.

To **ADD NEW** Node dependencies, run:

```bash
npm install package-1 package-2 ... package-n
```

For Rust dependencies (crates), run:

```bash
cargo add --manifest-path src-tauri/Cargo.toml package-1 package-2 ... package-n
```

To bump dependency versions (i.e. manually update dependencies), run:

```bash
# For Node.js dependencies:
npm update

# And for Rust dependencies:
cargo update
```

> [!TIP]
> Dependencies update is usually handled by bot or the Module 1 as repository maintainers, so you as developers don't have to.

## Available Scripts

| Task                            | Command                     |
| ------------------------------- | --------------------------- |
| Start dev server                | `npm run dev`               |
| Run Tauri desktop app           | `npm run tauri dev`         |
| Build the Vite app              | `npm run build`             |
| Build the Tauri app             | `npm run tauri build`       |
| Run ESLint                      | `npm run lint`              |
| Run Rust linting (clippy)       | `npm run lint:rust`         |
| Fix ESLint issues               | `npm run lint:fix`          |
| Check Prettier formatting       | `npm run format:check`      |
| Check Rust formatting (rustfmt) | `npm run format:rust:check` |
| Fix Prettier formatting issues  | `npm run format`            |
| Fix Rust formatting issues      | `npm run format:rust`       |
| Generate Rust docs              | `cargo doc --open`          |

> [!NOTE]
> There is no test runner configured yet. Do not reference one in changes or PR descriptions.

### Generating Rust Documentation

The backend uses rustdoc for API documentation. To generate and open the docs locally:

```bash
cargo doc --manifest-path src-tauri/Cargo.toml --document-private-items --open
```

- `--document-private-items` includes private functions, structs, and modules (useful during development).
- Output is written to `src-tauri/target/doc/aerial_analytics_platform_lib/index.html`.
- CI does not build or publish rustdoc — this is a local-only tool.

## Continuous Integration

Every push and pull request targeting `main` runs the [CI workflow](.github/workflows/ci.yml) via GitHub Actions:

- **Lint & format (JS/TS)**: ESLint (`npm run lint`) and Prettier (`npm run format:check`).
- **Lint & format (Rust)**: rustfmt (`npm run format:rust:check`) and Clippy (`npm run lint:rust`).
- **Build**: verifies the Vite build (`npm run build`) and a Tauri build without bundling
  (`npm run tauri build -- --no-bundle`) succeed on `macos-latest`, `ubuntu-24.04`, and `windows-latest`.

The build matrix only runs once both lint jobs pass. The workflow does not publish or deploy anything.

## Project Structure

```bash
.
├── .github/                      # GitHub community health files
│   ├── actions/                  # Reusable CI actions (e.g. install-tauri-linux-deps)
│   ├── ISSUE_TEMPLATE/
│   │   ├── bug_report.yml
│   │   ├── config.yml
│   │   └── feature_request.yml
│   ├── pull_request_template.md
│       └── workflows/
│           └── ci.yml                # CI pipeline: lint + format + cross-platform build
├── plugins/                      # Analytical plugins (Module 2: Extension System)
│   ├── mock/                     # Python mock plugin (rgb-vegetation-exg)
│   ├── mock_rust/                # Native Rust mock plugin (tree-canopy-density)
│   ├── rgb-vegetation-detection/ # Vegetation detection plugin (Python)
│   ├── rgb-landcover-classification/  # Land-cover classification plugin (Python)
│   └── template/                 # Canonical starter template for new plugins
├── schemas/                      # JSON Schema definitions for plugin contracts
│   ├── plugin.schema.json        # Plugin manifest schema
│   ├── execution_payload.schema.json
│   ├── execution_result.schema.json
│   ├── inputs.schema.json
│   ├── outputs.schema.json
│   └── parameters.schema.json
├── src/                          # React frontend application source
│   ├── main.tsx                  # Application entry point
│   ├── App.tsx                   # Root React component → AppShell
│   ├── shell/                    # App shell, sidebar, module routing
│   │   ├── AppShell.tsx
│   │   ├── Sidebar.tsx
│   │   └── modules.ts           # Module registry for sidebar navigation
│   ├── modules/                  # Per-module page components
│   │   ├── module-01/            # Image & Project Manager page
│   │   ├── module-02/            # Plugin System page
│   │   ├── module-03/            # Map Explorer page
│   │   └── ...
│   ├── map/                      # Leaflet map explorer, AOI drawing, layer management
│   │   ├── components/
│   │   ├── types/
│   │   ├── fixtures/
│   │   ├── docs/                 # Module docs: examples/, README_modul3.md, README_UMUM.md
│   │   └── error.ts
│   ├── features/                 # Feature modules (mostly scaffolding)
│   ├── component_plugin_manager/ # Plugin manager UI components
│   ├── components/               # Shared UI — only for components used by 2+ features
│   ├── hooks/                    # Shared React hooks
│   ├── lib/                      # Pure helper functions (no React, no Tauri)
│   ├── types/                    # Types shared across features
│   └── vite-env.d.ts             # Vite type definitions
├── src-tauri/                    # Rust backend and Tauri desktop configuration
│   ├── build.rs                  # Rust build script
│   ├── Cargo.toml                # Rust project manifest
│   ├── Cargo.lock                # Locked Rust dependency versions
│   ├── capabilities/             # ACL permissions — plugin/core only (custom commands are auto-allowed)
│   │   └── default.json
│   ├── rustfmt.toml              # Rust code formatting rules
│   ├── tauri.conf.json           # Tauri app config (devUrl: localhost:1420)
│   ├── icons/                    # App icons for each platform
│   ├── gen/schemas/              # Generated JSON schemas (do not edit by hand)
│   └── src/
│       ├── main.rs               # Binary entry point (calls lib::run())
│       ├── lib.rs                # run(): registers commands + Tauri plugins
│       ├── commands/             # #[tauri::command] fns — thin, no business logic
│       │   ├── session.rs        # Flight session CRUD commands
│       │   └── mod.rs
│       ├── models/               # Serde structs/enums shared with the frontend
│       │   ├── metadata.rs       # Image metadata types
│       │   ├── session.rs        # Flight session types
│       │   ├── image_import.rs   # Image import types
│       │   └── mod.rs
│       ├── services/             # Domain logic — pure functions where practical
│       ├── modules/
│       │   ├── module_01/        # Image metadata extraction
│       │   └── module_10/        # Temporal change contracts
│       ├── plugin_manager/       # Plugin discovery, lifecycle, execution, error isolation
│       │   ├── commands.rs       # Plugin Tauri IPC commands
│       │   ├── executor.rs       # Async subprocess supervision
│       │   ├── registry.rs       # Plugin discovery and indexing
│       │   ├── lifecycle.rs      # Install, remove, enable/disable
│       │   ├── payload.rs        # Execution payload assembly
│       │   ├── result_handler.rs # Result parsing and run history
│       │   ├── models.rs         # Plugin DTOs
│       │   ├── error.rs          # Plugin error types
│       │   └── DOCS/             # Detailed plugin system design docs
│       ├── map_controller/       # Map data provider and AOI storage
│       ├── reporting/            # Reporting module
│       ├── plot.rs               # GeoJSON plot import and validation
│       └── plot_api.rs           # Cross-module plot analysis API
├── test-data/                    # Test data files
├── commitlint.config.js          # Commitlint config (Conventional Commits)
├── eslint.config.js              # ESLint 9 flat config (TS/TSX)
├── .prettierrc.json              # Prettier config
├── .husky/                       # Git hooks (pre-commit, commit-msg)
├── index.html                    # HTML entry point for the web app
├── package.json                  # Node.js dependencies and scripts
├── package-lock.json             # Locked npm dependency versions
├── tsconfig.json                 # TypeScript config (strict mode)
├── tsconfig.node.json            # TypeScript config for Vite
├── vite.config.ts                # Vite bundler and dev server (port 1420)
├── AGENTS.md                     # AI agent instructions
├── CONTRIBUTING.md               # Contribution guidelines
├── SECURITY.md                   # Vulnerability reporting policy
└── LICENSE.md                    # MIT License
```

See [CONTRIBUTING.md § Architecture Notes](CONTRIBUTING.md#architecture-notes) for rules on where new
commands, models, helpers, and UI belong within this layout.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) before working on development. Use the
[issue templates](.github/ISSUE_TEMPLATE) to report bugs or request features.

## Security

See [SECURITY.md](SECURITY.md) for how to report vulnerabilities.

## Copyright & License

Copyright © 2026 Lambdaz.

This software project is licensed under the MIT License. See [LICENSE.md](LICENSE.md).
