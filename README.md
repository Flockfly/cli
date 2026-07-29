# Flockfly CLI

The Flockfly CLI publishes, searches, and progressively loads team skill packages. It is a native Rust binary and does not require Node.js.

## Install

After the first public release, install the latest macOS or Linux binary with:

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/flockfly/cli/releases/latest/download/flockfly-installer.sh | sh
```

The installer places `flockfly` in `~/.local/bin`. Make sure that directory is on `PATH`.

Homebrew installation will be available from the Flockfly tap:

```sh
brew install flockfly/tap/flockfly
```

No public release has been authorized yet; until one exists, build from source.

## Build from source

Install a stable Rust toolchain, then:

```sh
cargo install --path .
flockfly --version
```

## Usage

```sh
flockfly login
flockfly whoami
flockfly init
flockfly publish ./my-skill
flockfly publish ./my-skill --team engineering
flockfly search "prepare an incident review"
flockfly load skill_abc123
flockfly load skill_abc123 references/guide.md
flockfly team add --skill skill_abc123 --team engineering
flockfly teams list
flockfly skills list --org
flockfly skills list --team engineering
```

A published skill directory must contain `SKILL.md` with `name` and `description` YAML frontmatter. Symlinks that leave the package directory are rejected.

## Configuration

| Variable | Purpose |
|---|---|
| `FLOCKFLY_API_URL` | Override the API; defaults to `https://api.flockfly.ai`. |
| `FLOCKFLY_CONFIG_DIR` | Override the credential directory; defaults to `~/.flockfly`. |
| `FLOCKFLY_TOKEN` | Inject a token for CI or agents without a credential file. |

Browser login stores credentials in `credentials.json` with mode `0600` on Unix. Tokens are never printed by normal commands.

## Upgrade and uninstall

For installer-managed copies, rerun the installer to upgrade. For Homebrew:

```sh
brew update
brew upgrade flockfly
```

To uninstall an installer-managed copy:

```sh
rm ~/.local/bin/flockfly
```

Credentials are intentionally retained. Remove `~/.flockfly` separately only if you also want to sign out and delete local CLI configuration.

## Development

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked
cargo build --release --locked
bash tests/integration/smoke.sh target/release/flockfly
```

To exercise the compiled Rust binary against the real local Context API:

```sh
FLOCKFLY_CONTEXT_ROUTER_DIR=/path/to/context-router \
  node --test tests/e2e/real-api.test.mjs
```

See [MIGRATION.md](MIGRATION.md) for the TypeScript fallback/cutover policy and [RELEASE.md](RELEASE.md) for the release checklist.
