# Search `--load` Progress

## Setup

- [x] Reopened and reconciled `agent-flockfly-cli-rust-distribution`.
- [x] Selected autonomous mode and created the feature artifact/log directory.
- [x] Discovered repository instructions (README only; no CODEASSIST.md).
- [x] Inspected command parsing, shared load rendering, fake API harness, parity inventory, and real-API E2E harness.

## Implementation Checklist

- [x] Document requirements, dependency map, risks, and explicit scenarios.
- [x] Add unit and real-API E2E tests.
- [x] Capture focused RED failures.
- [x] Implement `search --load` through the shared load path.
- [x] Make focused tests GREEN.
- [x] Update user/parity documentation.
- [x] Pass complete Rust parity and safety suite.
- [x] Pass real-API E2E suite.
- [x] Pass formatting, Clippy, release build, smoke, and distribution gates.
- [ ] Commit and record the commit hash.

## TDD Cycles

### Test design

- Decision: use the explicit numeric rank rather than API array order.
- Decision: factor a default-load helper used by both `load` and `search --load`, preventing rendering/request drift.
- Decision: test both search-stage and load-stage failures with request tracking so the empty-result case proves it never loads.

### RED

- Added six focused Rust cases covering help/parsing, top rank and raw rendering, ordinary search, no results, search failure, and load failure.
- Five new behavior cases failed because Clap rejected the absent `--load` flag with exit code 2; the ordinary-search regression passed.
- The focused real-API E2E failed for the same expected reason: `unexpected argument '--load'`.

### GREEN

- Added the Clap `--load` flag to search.
- Selected the lowest numeric rank rather than trusting result array order.
- Extracted one default-load request/render helper shared by standalone load and search-load.
- All focused Rust tests pass.
- The focused real-API E2E passes with raw rendering, top-impression and correlated-load assertions, no-result/no-load proof, and real authentication failure coverage.

## Validation

- Original TypeScript/Rust parity count: 21 source cases and 21 explicit `ts_*` Rust cases.
- `cargo fmt --all --check`: passed.
- `cargo test --locked`: 32 passed, 0 failed, 0 ignored.
- Real Context API E2E: 3 passed, 0 failed.
- `cargo clippy --all-targets --all-features --locked -- -D warnings`: passed.
- `cargo build --release --locked`: passed.
- Release binary smoke, including `search --help` flag documentation: passed.
- Offline source install and installed-binary smoke: passed.
- `dist generate --check`: passed.
- `dist plan`: passed for all six configured targets plus shell/Homebrew installers.
- Pre-commit ARM64 macOS local artifact build: passed.

## Commit

- Pending.
