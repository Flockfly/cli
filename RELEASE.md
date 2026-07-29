# Release Guide

Public release actions require explicit product-owner approval. Preparing and validating artifacts locally does not authorize creating a tag, GitHub Release, or Homebrew tap commit.

## Before tagging

1. Confirm `Cargo.toml` contains the intended semantic version.
2. Run:

   ```sh
   cargo fmt --all --check
   cargo clippy --all-targets --all-features --locked -- -D warnings
   cargo test --locked
   cargo build --release --locked
   bash tests/integration/smoke.sh target/release/flockfly
   dist generate --check
   dist plan
   ```

3. With the Context Router checkout available, run:

   ```sh
   FLOCKFLY_CONTEXT_ROUTER_DIR=/path/to/context-router \
     node --test tests/e2e/real-api.test.mjs
   ```

4. Review `MIGRATION.md` and preserve the TypeScript rollback path.
5. Confirm repository release permissions and the `HOMEBREW_TAP_TOKEN` secret for `flockfly/homebrew-tap`.

## Authorized release

Only after explicit approval, create and push a version tag matching `Cargo.toml`:

```sh
git tag vX.Y.Z
git push origin vX.Y.Z
```

The generated cargo-dist workflow builds:

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-musl`
- `x86_64-unknown-linux-musl`

It publishes GitHub Release archives, `flockfly-installer.sh`, and a Homebrew formula. The `Verify Release Install` workflow then installs the published artifact in a clean Ubuntu container and checks its version and smoke commands.

## Verification and rollback

- Check every release and installer workflow job before announcing availability.
- Run a manual macOS install/login smoke test.
- If publishing or compatibility validation fails, do not replace an existing stable tag. Fix the problem, rerun all gates, and release a new patch version.

