# Release Guide

Public release actions require explicit product-owner approval. Preparing and validating artifacts locally does not authorize creating a tag, GitHub Release, or Homebrew tap commit.

## Before you begin

- All changes are on the main branch. Check the *actual* latest CI run for `main` (e.g. `gh run list --branch main --limit 1`) - don't assume it's green just because nobody mentioned otherwise. A tag pushed on top of an already-red main will just fail the same way.
- Confirm `Cargo.toml` contains the intended semantic version.
- Run:

  ```sh
  cargo fmt --all --check
  cargo clippy --all-targets --all-features --locked -- -D warnings
  cargo test --locked
  cargo build --release --locked
  bash tests/integration/smoke.sh target/release/flockfly
  dist generate --check
  dist plan
  ```

- **These are also verified on Linux, not just your local OS.** CI's `test` job is a matrix over `ubuntu-latest` and `macos-latest`. If developing on macOS, additionally run the Docker integration suite locally to cover the Linux leg and the offline command surface end-to-end:

  ```sh
  docker build -f tests/integration/Dockerfile -t flockfly-integration . && \
    docker run --rm flockfly-integration
  ```

- `cargo install --path .` produces a working binary (`flockfly --version` prints the expected version).
- **Run the real-API e2e suite.** This is a hard requirement, not optional polish: it is the only coverage that exercises publish/search/load/router flows against a live API. It must always run with the API URL overridden to a local Context Router instance - `tests/e2e/real-api.test.mjs` does this automatically for every request it makes (`FLOCKFLY_API_URL` is set to the in-process local server's address before each CLI invocation), so it can never accidentally hit production. Run it with the Context Router checkout available:

  ```sh
  FLOCKFLY_CONTEXT_ROUTER_DIR=/path/to/context-router \
    node --test tests/e2e/real-api.test.mjs
  ```

  This can't run in CI today since the Context Router isn't checked into this repo or published, so it stays a required local pre-tag step rather than an automated gate. Do not tag a release without having run it.
- Review `MIGRATION.md` and preserve the TypeScript rollback path.
- Confirm repository release permissions and the `HOMEBREW_TAP_TOKEN` secret for `flockfly/homebrew-tap`.

## 1. Decide the new version

Follow [Semantic Versioning](https://semver.org/):

| Change type | Example | Version bump |
|---|---|---|
| Bug fix, docs, internals | Fix search ranking tie-break | Patch — `0.1.0 → 0.1.1` |
| New command or flag, new config key | Add `flockfly skills list --router` | Minor — `0.1.0 → 0.2.0` |
| Breaking CLI change, API contract change | Rename `--load` flag, change `credentials.json` schema | Major — `0.1.0 → 1.0.0` |

**When in doubt, bump minor.** It is always safe to do so.

## 2. Check backwards compatibility

Before bumping the version, answer these questions:

- **CLI flags** — are any existing flags renamed or removed? If yes, add a deprecation note in the help text for at least one minor release before removing.
- **Config and credentials** — does `~/.flockfly/credentials.json` still parse under the old schema? `FLOCKFLY_CONFIG_DIR`, `FLOCKFLY_API_URL`, and `FLOCKFLY_TOKEN` must keep working exactly as documented in the README.
- **Skill package contract** — does a `SKILL.md` with valid `name`/`description` frontmatter that publishes today still publish under the new version? Don't tighten package validation in a patch or minor release without a deprecation window.
- **TypeScript CLI parity** — per `MIGRATION.md`, the Rust and TypeScript CLIs must produce comparable stdout/stderr/exit codes for the same inputs during the cutover window.

If any of these require a migration, document the migration steps before releasing.

## 3. Bump the version

Edit `Cargo.toml`:

```toml
version = "X.Y.Z"
```

Commit:

```sh
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to X.Y.Z"
```

## 4. Tag the release

Only after explicit product-owner approval, create and push a version tag matching `Cargo.toml` exactly (with a `v` prefix):

```sh
git tag vX.Y.Z
git push origin vX.Y.Z
```

Pushing the tag triggers the cargo-dist CI workflow, which builds binaries for all targets and publishes a GitHub Release with a shell installer and Homebrew formula.

## 5. Verify the release

Once CI finishes:

1. Open the GitHub Releases page and confirm the release notes and attached binaries look correct.
2. The `Verify Release Install` workflow (`.github/workflows/verify-release.yml`) runs automatically when the release is published - and daily thereafter on a schedule - and installs the real shell installer in a clean container, confirming `flockfly --version` reports the new version and a basic smoke command works. Check that it's green (`gh run list --workflow=verify-release.yml --limit 1`) rather than re-doing this by hand; only fall back to running the installer yourself if that workflow is red or didn't fire.
3. Run a manual macOS install/login smoke test against the production API.

## 6. If something goes wrong

**Bad binary / wrong version printed** — delete the tag locally and remotely, fix the issue, and re-tag:

```sh
git tag -d vX.Y.Z
git push origin :refs/tags/vX.Y.Z
# fix, then re-tag and push
```

**Breaking change shipped by mistake** — issue a patch release immediately that either reverts the change or restores the old behavior under the old flag/format. Do not leave users on a broken version. Do not replace an existing stable tag.

## Release targets

cargo-dist builds for the following targets (see `Cargo.toml`):

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-musl`
- `x86_64-unknown-linux-musl`

It publishes GitHub Release archives, `flockfly-installer.sh`, and a Homebrew formula.
