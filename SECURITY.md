# Security Policy

## Supported Versions

This project is pre-1.0 and under active development as a university PBL project. Only the latest
commit on `main` is supported with security fixes; there are no maintained release branches.

| Version         | Supported |
| --------------- | --------- |
| `main` (latest) | ✅        |
| Anything else   | ❌        |

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Instead, report it privately using GitHub's private vulnerability reporting feature:

<https://github.com/Lambdasz/Aerial-Analytics-Platform/security/advisories/new>

When reporting, please include:

- The affected component (e.g. a specific Tauri command, a plugin, the capability/ACL configuration).
- The version or commit SHA you tested against.
- Steps to reproduce, and any proof-of-concept if available.
- The potential impact as you understand it.

This is a student project without a dedicated security team, so response times are best-effort. We
will acknowledge reports as soon as we're able and keep you updated as we investigate and fix the
issue.

## Scope

**In scope:**

- The Tauri core application and its Rust `#[tauri::command]` handlers.
- The capability/ACL configuration (`src-tauri/capabilities/default.json`) and how it's applied.
- The plugin boundary — including plugins that shell out to or embed Python — and whether a failing or
  malicious plugin can affect the stability or security of the core app.
- Handling of user-supplied imagery, project files, and any other untrusted input.

**Out of scope:**

- Vulnerabilities in third-party dependencies — please report those directly to the upstream project.
- Issues in third-party plugins that are not distributed as part of this repository.
