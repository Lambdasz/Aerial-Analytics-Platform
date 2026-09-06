# Aerial-Analytics-Platform

The Aerial Analytics Platform is designed to process and analyze RGB aerial imagery captured using standard drones. The platform uses a plugin-based architecture so that analytical capabilities can be added independently without modifying the core system.

[![TypeScript](https://img.shields.io/badge/TypeScript-6-3178C6?logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Rust](https://img.shields.io/badge/Rust-latest-CE422B?logo=rust&logoColor=white)](https://rust-lang.org/)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=white)](https://react.dev/)
[![Vite](https://img.shields.io/badge/Vite-8-646CFF?logo=vite&logoColor=white)](https://vite.dev/)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![ESLint](https://img.shields.io/badge/ESLint-9-4B3BDB?logo=eslint&logoColor=white)](https://eslint.org/)
[![Prettier](https://img.shields.io/badge/Prettier-3-F7B93E?logo=prettier&logoColor=white)](https://prettier.io/)
[![Husky](https://img.shields.io/badge/Husky-9-000?logo=husky&logoColor=white)](https://typicode.github.io/husky/)

## Table of Contents

<!-- prettier-ignore-start -->
<!-- TOC -->
* [Aerial-Analytics-Platform](#aerial-analytics-platform)
  * [Table of Contents](#table-of-contents)
  * [Tech Stack](#tech-stack)
  * [Getting Started](#getting-started)
    * [1. Clone the repo](#1-clone-the-repo)
    * [2. Install project dependencies](#2-install-project-dependencies)
    * [3. Development](#3-development)
  * [Available Scripts](#available-scripts)
  * [Project Structure](#project-structure)
  * [Contributing](#contributing)
  * [Security](#security)
  * [License](#license)
<!-- TOC -->
<!-- prettier-ignore-end -->

## Tech Stack

- **Frontend Language**: [TypeScript 6](https://www.typescriptlang.org/)
- **Backend Language**: [Rust](https://rust-lang.org/)
- **UI Framework**: [React 19](https://react.dev/)
- **UI Component Library**: [Blueprint.js](https://blueprintjs.com/)
- **Build Tool & Web Server**: [Vite 8](https://vite.dev/)
- **Desktop App Framework**: [Tauri 2](https://v2.tauri.app/)
- **Development & Tooling**:
  - Linting: [ESLint 9](https://eslint.org/), [Clippy](https://github.com/rust-lang/rust-clippy)
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

To manually update dependencies, run:

```bash
# For Node.js dependencies:
npm update

# And for Rust dependencies:
cargo update
```

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

## Project Structure

```bash
.
├── .github/                      # GitHub community health files
│   ├── ISSUE_TEMPLATE/
│   │   ├── bug_report.yml
│   │   ├── config.yml
│   │   └── feature_request.yml
│   └── pull_request_template.md
├── CONTRIBUTING.md               # Contribution guidelines
├── eslint.config.js              # JavaScript/TypeScript linting rules
├── index.html                    # HTML entry point for the web app
├── LICENSE                       # MIT License
├── package.json                  # Node.js dependencies and scripts
├── package-lock.json             # Locked versions of npm dependencies
├── public/                       # Static assets served directly
│   ├── tauri.svg
│   └── vite.svg
├── README.md                     # Project documentation
├── SECURITY.md                   # Vulnerability reporting policy
├── src/                          # React frontend application source
│   ├── App.css                   # Main component styling
│   ├── App.tsx                   # Main React component
│   ├── assets/                   # Application assets
│   │   └── react.svg
│   ├── main.tsx                  # Application entry point
│   └── vite-env.d.ts             # Vite type definitions
├── src-tauri/                    # Rust backend and Tauri desktop configuration
│   ├── build.rs                  # Rust build script
│   ├── capabilities/             # ACL capability definitions
│   │   └── default.json
│   ├── Cargo.lock                # Locked versions of Rust dependencies
│   ├── Cargo.toml                # Rust project manifest
│   ├── gen/                      # Generated JSON schemas
│   │   └── schemas/
│   │       ├── acl-manifests.json
│   │       ├── capabilities.json
│   │       ├── desktop-schema.json
│   │       └── linux-schema.json
│   ├── icons/                    # App icons for different platforms
│   │   ├── icon.icns             # macOS icon
│   │   ├── icon.ico              # Windows icon
│   │   └── icon.png              # Linux icon
│   ├── rustfmt.toml              # Rust code formatting rules
│   ├── src/                      # Rust application source code
│   │   ├── lib.rs
│   │   └── main.rs
│   └── tauri.conf.json           # Tauri app configuration
├── tsconfig.json                 # TypeScript configuration
├── tsconfig.node.json            # TypeScript configuration for Vite
└── vite.config.ts                # Vite bundler and dev server configuration
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) before working on development. Use the
[issue templates](.github/ISSUE_TEMPLATE) to report bugs or request features.

## Security

See [SECURITY.md](SECURITY.md) for how to report vulnerabilities.

## License

This software project is licensed under the MIT License. See [LICENSE](LICENSE).
