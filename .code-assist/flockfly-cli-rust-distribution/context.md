# Flockfly Rust CLI Context

## Summary

Replace the private Node 24/TypeScript CLI with an idiomatic Rust binary while preserving its observable command, API, configuration, packaging, safety, output, and exit-code behavior. The original CLI remains the behavioral oracle until parity is demonstrated.

## Existing Documentation

- `README.md` currently identifies this repository as the Flockfly CLI and is user-owned.
- No `CODEASSIST.md`, `AGENTS.md`, `CONTRIBUTING.md`, or other project instruction file exists.
- The coordination note requires a complete, explicit transfer of every original unit test and forbids public release/tag/tap changes without approval.
- `/Users/jkim/Documents/vibe/tnote` demonstrates the desired Clap, cargo-dist, CI, installer, Homebrew, and post-release verification shape.

## Source System

- TypeScript implementation: `/Users/jkim/Documents/flockfly/context-router/cli`
- Shared contracts and validation: `/Users/jkim/Documents/flockfly/context-router/shared`
- Fixture skills: `/Users/jkim/Documents/flockfly/context-router/fixtures/skills`
- Original suites:
  - `config.test.ts`: 2 cases
  - `format.test.ts`: 5 cases
  - `package.test.ts`: 6 cases
  - `cli.test.ts`: 8 cases
  - `e2e.test.ts`: 2 separately classified full-system cases

## Functional Requirements

1. Provide the `flockfly` commands `login`, `whoami`, `init`, `publish`, `search`, `load`, `team add`, `teams list`, and `skills list`.
2. Match relevant arguments and flags, output text, stderr behavior, and successful/error exit codes.
3. Use `https://api.flockfly.ai` by default and honor `FLOCKFLY_API_URL`, `FLOCKFLY_CONFIG_DIR`, and `FLOCKFLY_TOKEN`.
4. Store JSON credentials with mode `0600`, never print tokens, and support browser login polling.
5. Preserve structured API error handling and actionable messages.
6. Package skill directories recursively with stable sorted paths and Base64 contents.
7. Validate SKILL.md YAML frontmatter and package paths.
8. Reject symlinks escaping the package root while allowing in-root symlinks.
9. Preserve search/load/list rendering and replacement confirmation semantics.
10. Explicitly map and pass all 21 original non-E2E cases; port the 2 E2E cases separately.
11. Add macOS/Linux CI, cargo-dist planning/release workflow, shell/Homebrew distribution configuration, installer verification, and release documentation without publishing.

## Architecture and Dependency Map

```text
main.rs
  -> clap command parsing
  -> commands.rs orchestration
       -> config.rs credentials/environment
       -> api.rs HTTP and structured errors
       -> package.rs filesystem/frontmatter/path safety
       -> format.rs deterministic rendering
  -> process adapters (browser, prompt, sleep, stdout/stderr)

Rust tests
  -> pure module tests for config/format/package
  -> local HTTP compatibility server for command flows
  -> real compiled binary/API E2E harness where available
```

## Implementation Paths

- `Cargo.toml`, `Cargo.lock`
- `src/lib.rs`, `src/main.rs`
- `src/api.rs`, `src/commands.rs`, `src/config.rs`, `src/format.rs`, `src/package.rs`
- `tests/` for explicit transferred compatibility and E2E tests
- `tests/fixtures/skills/` for self-contained fixture parity
- `.github/workflows/` and `RELEASE.md` for distribution operations

## Constraints and Risks

- The repository has no commits yet; preserve the existing untracked README.
- `.agents/` is read-only, so workflow artifacts live under `.code-assist/`.
- Tests must not rely on production or mutate production state.
- Login time must be injectable so tests do not wait five minutes.
- Symlink tests are Unix-oriented; release targets are macOS/Linux only.
- cargo-dist is not installed locally; configuration can be validated structurally and, if installed later, with `dist plan`.

