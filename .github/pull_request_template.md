## Summary

<!-- Describe what this PR changes and why. -->

## Related issues

<!-- e.g. Closes #123 -->

## Type of change

- [ ] `build` — changes to the build system or external dependencies
- [ ] `chore` — maintenance tasks that don't affect src or test files
- [ ] `ci` — changes to CI configuration or scripts
- [ ] `docs` — documentation only
- [ ] `feat` — a new feature
- [ ] `fix` — a bug fix
- [ ] `perf` — a performance improvement
- [ ] `refactor` — a code change that neither fixes a bug nor adds a feature
- [ ] `revert` — reverts a previous commit
- [ ] `style` — formatting/style only, no code logic change
- [ ] `test` — adding or correcting tests

## How this was tested

<!-- There is no test runner configured yet, so describe the manual steps you took to verify this
change (e.g. `npm run tauri dev` and reproduced X, or `npm run build` succeeded). -->

## Checklist

- [ ] This PR is atomic — one logical change, nothing unrelated bundled in.
- [ ] Commit message(s) follow [Conventional Commits](https://www.conventionalcommits.org/).
- [ ] `npm run lint` passes.
- [ ] `npm run lint:rust` passes.
- [ ] `npm run format:check` passes.
- [ ] `npm run format:rust:check` passes.
- [ ] Any new Tauri command or plugin capability is added to `src-tauri/capabilities/default.json`.
- [ ] Documentation (README.md, AGENTS.md, etc.) is updated if this change affects what they describe.
