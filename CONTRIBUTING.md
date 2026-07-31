# Contributing to Flockfly CLI

See `MIGRATION.md` for the TypeScript-to-Rust cutover policy this repo is operating under.

## Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable)
- [Docker](https://docs.docker.com/get-docker/) (for integration tests)
- Node.js 24+ and a local [Context Router](MIGRATION.md) checkout (for the real-API e2e suite)

## Setup

```sh
git clone https://github.com/flockfly/cli.git
cd cli
cargo build
```

## Development commands

```sh
cargo build              # Debug build
cargo build --release    # Release build
cargo run -- help        # Run locally
cargo install --path .   # Install to ~/.cargo/bin
```

## Testing

### Unit tests

```sh
cargo test
```

No external dependencies required.

### Formatting and Clippy

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
```

CI enforces zero warnings and exact formatting. Run these before pushing.

### Smoke test

```sh
cargo build --release
bash tests/integration/smoke.sh target/release/flockfly
```

Runs on every CI push as part of the `test` job. Confirms the release binary starts and its top-level help text is intact.

### Integration tests

```sh
make integration-test
```

Builds a Docker container with the release binary and runs the offline command surface: `--help`/`--version`, `init`, the auth gate on every credentialed command, and credential/config-dir discovery and precedence (`FLOCKFLY_CONFIG_DIR`, `FLOCKFLY_TOKEN`, `FLOCKFLY_API_URL`). Runs on every CI push as the `integration` job.

### Real-API e2e tests

```sh
FLOCKFLY_CONTEXT_ROUTER_DIR=/path/to/context-router \
  node --test tests/e2e/real-api.test.mjs
```

The only coverage that exercises publish/search/load/team flows against a live API. It always runs with `FLOCKFLY_API_URL` overridden to a local, in-process Context Router instance - it can never hit production. Requires a local Context Router checkout, so it isn't part of CI; it's a required step before tagging a release (see `RELEASE.md`).

## Project structure

```
src/
  main.rs      CLI entry point, runtime wiring
  commands.rs  Subcommand dispatch and command implementations
  api.rs       HTTP client and error mapping
  config.rs    Credentials and config resolution (FLOCKFLY_CONFIG_DIR, FLOCKFLY_API_URL, FLOCKFLY_TOKEN)
  package.rs   Skill directory packaging and SKILL.md validation
  format.rs    Output rendering (search results, loaded files, init snippet)
  lib.rs       Module exports

tests/
  *_compat.rs         TypeScript-parity regression tests
  PARITY.md           Compatibility inventory
  e2e/
    real-api.test.mjs Real-API tests against a local Context Router
  integration/
    Dockerfile        Docker image for the offline integration suite
    run.sh            Integration test script
    smoke.sh           Minimal binary smoke test (also run in CI's test job)
    verify-install.sh Post-release installer verification
```

## Release process

See [RELEASE.md](RELEASE.md) for the full checklist. Summary:

1. Confirm CI is green on `main` and the e2e suite passes locally.
2. Bump the version in `Cargo.toml` following semver; check backwards compatibility.
3. Commit, then (only after explicit product-owner approval) tag `vX.Y.Z` and push the tag.
4. The release workflow builds binaries for all targets, creates a GitHub Release, and publishes the Homebrew formula.
5. Verify via the `Verify Release Install` workflow rather than re-testing by hand.
