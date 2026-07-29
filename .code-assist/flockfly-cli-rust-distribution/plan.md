# Flockfly Rust CLI Plan

## Test Strategy and Explicit Transfer Inventory

Each checkbox is one original TypeScript test and must map to one explicitly named Rust test.

### Configuration (`config.test.ts`)

- [x] `connects installed clients to the production API by default`: missing credential directory -> `https://api.flockfly.ai`.
- [x] `allows local development to override the API URL`: `FLOCKFLY_API_URL=http://127.0.0.1:8799` -> same URL.

### Formatting (`format.test.ts`)

- [x] `prints rank, skill ID, and frontmatter only`: two result records -> exact seven-line output without score/body.
- [x] `handles empty results`: empty results -> `No matching skills found.`
- [x] `prints a single file raw`: one `SKILL.md` file -> content unchanged, including trailing newline.
- [x] `adds boundaries for multiple files`: two reference files -> exact boundary output with trailing content newline removed.
- [x] `teaches search, load, and progressive disclosure`: init snippet contains all three canonical invocations.

### Package Safety (`package.test.ts`)

- [x] `packages a fixture skill with sorted relative paths`: PDD fixture -> parsed name and exact sorted paths.
- [x] `rejects a missing path and non-directories`: absent path and SKILL.md file path -> typed/actionable errors.
- [x] `rejects a directory without SKILL.md`: notes-only directory -> `No SKILL.md found`.
- [x] `rejects missing frontmatter fields with actionable messages`: missing description and absent frontmatter -> specific messages.
- [x] `rejects symlinks that escape the skill directory`: external file symlink -> `Symlink escapes`.
- [x] `allows symlinks that stay inside the skill directory`: internal file symlink -> packaged alias path.

### CLI Compatibility (`cli.test.ts`)

- [x] `logs in through the browser flow and stores a token without printing it`: approved local auth -> code 0, mode 0600 credentials, token absent from output, whoami succeeds.
- [x] `prints an actionable error when not logged in`: whoami without credentials -> code 1 and login guidance on stderr.
- [x] `prints the init snippet without touching files`: init -> code 0 and canonical snippet.
- [x] `publishes, searches, and loads a skill end to end`: local API journey -> publish/team attach/search/default load/multi-load outputs.
- [x] `publishes with --team and finds the skill via search immediately`: named personal team -> attachment and search visibility.
- [x] `asks before replacing an existing skill and honors the answer`: name conflict -> decline error, accept version 2 and captured question.
- [x] `prints actionable errors for unknown teams and invalid packages`: unknown team and missing directory -> code 1 and exact guidance.
- [x] `lists org and team skills`: published attached fixture -> both list variants render `pdd v1`.

### Original E2E (`e2e.test.ts`)

- [x] `covers the full team journey with visibility rules and telemetry`: real API/database environment -> two-user visibility, progressive load, correlated telemetry, feedback.
- [x] `replaces a skill after confirmation and search returns the new version`: real API -> version replacement, updated search result, persistent attachment.

## TDD and Implementation Sequence

- [x] Create the Rust crate and fixture layout.
- [x] Add all 21 named compatibility tests before production implementation and record expected RED output.
- [x] Implement configuration, formatting, frontmatter/path validation, and packaging.
- [x] Implement HTTP client, structured errors, command orchestration, dependency injection, and binary adapters.
- [x] Make all 21 transferred non-E2E tests green without weakening assertions.
- [x] Port and run the 2 original E2E cases against an available real local API.
- [x] Add cargo-dist metadata, CI, release/installer verification, and documentation.
- [x] Run format, clippy with warnings denied, tests, release build, and local install smoke checks.
- [x] Review the explicit inventory against the TypeScript source and record final evidence.

## Security and Maintainability

- Never include token values in errors or logs.
- Resolve every symlink and require its target to stay beneath the canonical package root.
- Keep API DTOs local and narrow; ignore unknown response fields.
- Keep rendering pure and separately tested.
- Use a mockable command runtime for browser/prompt/sleep behavior.
- Do not publish tags, releases, or tap changes.
