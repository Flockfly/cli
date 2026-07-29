# TypeScript-to-Rust CLI Cutover

The TypeScript CLI remains the behavior reference and fallback until a Rust release completes every gate below. Do not delete the TypeScript package as part of the first Rust release.

## Compatibility gates

1. Keep the one-to-one inventory in `.code-assist/flockfly-cli-rust-distribution/plan.md` current.
2. Require all transferred Rust tests, Clippy, formatting, and release builds to pass on macOS and Linux.
3. Run `node --test tests/e2e/real-api.test.mjs` against a clean local Context API checkout.
4. Run `dist generate --check` and `dist plan`.
5. Install a release candidate into a clean environment and exercise login, whoami, publish, search, load, team attachment, and list operations against a non-production API.
6. Compare stdout, stderr, and exit codes from the TypeScript and Rust CLIs for the same scripted inputs.

## Safe rollout

- Publish the Rust binary under the same `flockfly` command only after the compatibility gates pass.
- Keep the source-checkout TypeScript command documented as the rollback path for at least one release cycle.
- Start with prerelease artifacts and internal users before declaring the Rust installer generally available.
- Do not change the production API contract during the CLI cutover.
- If a compatibility regression appears, direct users back to the TypeScript CLI while fixing and retesting Rust. Do not overwrite or remove existing credentials.

## Completion

After one stable release cycle with no unresolved compatibility regressions, remove the Node 24 runtime requirement from user-facing installation paths. Retire the TypeScript CLI only in a separately reviewed change.

