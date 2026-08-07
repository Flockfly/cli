# Flockfly CLI

The Flockfly CLI publishes, searches, and progressively loads skill packages. It is a native Rust binary and does not require Node.js.

## Install

Install the latest macOS or Linux binary with:

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/flockfly/cli/releases/latest/download/flockfly-installer.sh | sh
```

The installer places `flockfly` in `~/.local/bin`. Make sure that directory is on `PATH`.

Or install via Homebrew from the Flockfly tap:

```sh
brew install flockfly/tap/flockfly
```

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
flockfly publish ./my-skill --router engineering
flockfly search "prepare an incident review"
flockfly search "prepare an incident review" --load
flockfly load skill_abc123
flockfly load skill_abc123 references/guide.md
flockfly router add --skill skill_abc123 --router engineering
flockfly routers list
flockfly skills list
flockfly skills list --router engineering
flockfly init --collection coll_abc123
flockfly hooks remove
```

A published skill directory must contain `SKILL.md` with `name` and `description` YAML frontmatter. Symlinks that leave the package directory are rejected. `publish` always targets the public skill collection; `--router` additionally attaches the published skill to a router so agents routed to it can find it immediately.

`search --load` loads and prints the highest-ranked match immediately. Its output is the same raw `SKILL.md` content produced by `flockfly load <skill-id>`. If nothing matches, it prints `No matching skills found.` without issuing a load request.

### Automatic Claude Code session sync

`flockfly init --collection <id>` configures automatic session capture for
Claude Code. You must already hold session-publish access on the given
collection (ask its owner to grant it if you don't). Once configured, it
installs a global hook in `~/.claude/settings.json` — every Claude Code
session on the machine, in any repo, gets pushed into that collection from
then on: incrementally after each turn (`Stop`/`SubagentStop`), and a full
catch-up reconcile when the session ends (`SessionEnd`), so nothing is lost
even if an incremental push is missed. `flockfly session sync --hook` is
the hook's own entrypoint (Claude Code invokes it; you shouldn't need to
run it directly). `flockfly hooks remove` uninstalls the hook without
touching the configured collection, so re-running `init --collection`
later doesn't require picking it again.

## Configuration

| Variable | Purpose |
|---|---|
| `FLOCKFLY_API_URL` | Override the API; defaults to `https://api.flockfly.ai`. |
| `FLOCKFLY_CONFIG_DIR` | Override the credential directory; defaults to `~/.flockfly`. |
| `FLOCKFLY_TOKEN` | Inject a token for CI or agents without a credential file. |

Browser login stores credentials in `credentials.json` with mode `0600` on Unix. Tokens are never printed by normal commands. The collection configured by `flockfly init --collection <id>` is stored alongside it, in `sync-config.json`.

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

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development workflow and [RELEASE.md](RELEASE.md) for the release checklist.
