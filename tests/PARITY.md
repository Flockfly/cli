# TypeScript CLI Test Parity

Source: `/Users/jkim/Documents/flockfly/context-router/cli/test`

The four original non-E2E files contain 21 `it(...)` cases. The Rust suite contains 21 corresponding `ts_*` tests; none are ignored or consolidated away.

| TypeScript test | Rust test |
|---|---|
| config: connects installed clients to the production API by default | `ts_connects_installed_clients_to_the_production_api_by_default` |
| config: allows local development to override the API URL | `ts_allows_local_development_to_override_the_api_url` |
| format: prints rank, skill ID, and frontmatter only | `ts_prints_rank_skill_id_and_frontmatter_only` |
| format: handles empty results | `ts_handles_empty_results` |
| format: prints a single file raw | `ts_prints_a_single_file_raw` |
| format: adds boundaries for multiple files | `ts_adds_boundaries_for_multiple_files` |
| format: loads the Flockfly discovery skill with its activation description | `ts_loads_the_flockfly_discovery_skill_with_its_activation_description` |
| package: packages a fixture skill with sorted relative paths | `ts_packages_a_fixture_skill_with_sorted_relative_paths` |
| package: rejects a missing path and non-directories | `ts_rejects_a_missing_path_and_non_directories` |
| package: rejects a directory without SKILL.md | `ts_rejects_a_directory_without_skill_md` |
| package: rejects missing frontmatter fields with actionable messages | `ts_rejects_missing_frontmatter_fields_with_actionable_messages` |
| package: rejects symlinks that escape the skill directory | `ts_rejects_symlinks_that_escape_the_skill_directory` |
| package: allows symlinks that stay inside the skill directory | `ts_allows_symlinks_that_stay_inside_the_skill_directory` |
| CLI: logs in through the browser flow and stores a token without printing it | `ts_logs_in_through_the_browser_flow_and_stores_a_token_without_printing_it` |
| CLI: prints an actionable error when not logged in | `ts_prints_an_actionable_error_when_not_logged_in` |
| CLI: prints the init snippet without touching files | `ts_prints_the_init_snippet_without_touching_files` |
| CLI: publishes, searches, and loads a skill end to end | `ts_publishes_searches_and_loads_a_skill_end_to_end` |
| CLI: publishes with --router and finds the skill via search immediately | `ts_publishes_with_router_and_finds_the_skill_via_search_immediately` |
| CLI: asks before replacing an existing skill and honors the answer | `ts_asks_before_replacing_an_existing_skill_and_honors_the_answer` |
| CLI: prints actionable errors for unknown routers and invalid packages | `ts_prints_actionable_errors_for_unknown_routers_and_invalid_packages` |
| CLI: lists public collection and router skills | `ts_lists_public_collection_and_router_skills` |

The separately classified original E2E cases are also ported in `tests/e2e/real-api.test.mjs`:

- `TS E2E: covers the full router journey with visibility rules and telemetry`
- `TS E2E: replaces a skill after confirmation and search returns the new version`

Additive Rust coverage for `search --load` preserves the original 21-case mapping and verifies:

- Clap help and parsing for `search "<query>" --load`
- explicit best-rank selection independent of response order
- byte-for-byte parity with standalone default load rendering
- unchanged ordinary search and no-result behavior
- no load request for empty results
- search- and load-stage API failure propagation
- real-API top selection, correlated telemetry, empty results, and authentication failure

## Skills/Collections/Routers redesign (2026-08-02)

The source TypeScript CLI was rebuilt around Skill Collections and Skill
Routers (skills live in a collection, e.g. the public collection; a "router"
replaces "team" as the named, shareable set of skills an agent searches
against — no more org-scoped `/v1/orgs/:orgId/skills` or `/v1/teams`). The
Rust CLI's command surface, endpoints, and test suite were updated to match:

- `--team` → `--router` on `publish` and `skills list`
- `flockfly team add` → `flockfly router add` (`/v1/teams/:id/skills/:skillId` → `/v1/routers/:id/skills/:skillId`)
- `flockfly teams list` → `flockfly routers list` (`/v1/teams` → `/v1/routers`)
- `flockfly skills list --org` → `flockfly skills list` (publish always targets the public collection, discovered via `GET /v1/collections`; the CLI resolves it once per command rather than assuming a fixed ID)
- Publish moved from `POST /v1/orgs/:orgId/skills` to `POST /v1/collections/:collectionId/skills`; the request body's `teamId` became `routerId`, and the publish response's `attachedTeamId` became `attachedRouterId`
- `GET /v1/me`'s `personalTeam` became `personalRouter` (unused by the Rust CLI directly, but the fake test backend's response shape was updated for realism)
- List endpoints (`/v1/collections/:id/skills`, `/v1/routers/:id/skills`) are now cursor-paginated (`{ skills, page: { limit, hasMore, nextCursor } }`); `skills list` loops pages (100/request) the same way the TypeScript CLI's `fetchAllSkillPages` does, rather than assuming one response has every row
- `api.rs`'s actionable-error mapping: `team_not_found`/`not_team_member` → `router_not_found`/`router_access_denied`, plus new `collection_not_found`/`collection_access_denied` cases
- `format.rs`'s `INIT_SNIPPET` now says "search your routed skills" instead of "search for relevant team skills"
- `tests/e2e/real-api.test.mjs` grants each test user `create` access on the public collection before publishing (`entity_access` table) — the public collection's real allowlist only covers `jonathan@flockfly.ai`, and this requirement didn't exist under the old org-scoped model

`config.rs`, `package.rs`, and their test files (`config_compat.rs`, `package_compat.rs`) are untouched — neither concept touches collections, routers, or orgs.

Current evidence is recorded in `.code-assist/flockfly-cli-rust-distribution/progress.md`.

## `init` loads a discovery skill instead of teaching search/load (2026-08-03)

The source TypeScript CLI's `init` command no longer prints instructions to
run `flockfly search`/`flockfly load` manually. Instead it tells the agent to
load one fixed, published meta-skill (`skill_pxJxZr7CMBMk`, the "Flockfly
discovery" skill) and includes that skill's activation description inline so
the agent can decide when to load it without an extra round trip. The Rust
CLI's `init` output and command description were updated to match:

- `format.rs`'s `INIT_SNIPPET` now says `flockfly load skill_pxJxZr7CMBMk` plus the discovery skill's description, instead of teaching `flockfly search "<task>"` / `flockfly load <skillId>` / `flockfly load <skillId> <path...>`
- `commands.rs`'s `init` action prints `INIT_SNIPPET` directly (no more "Add the following snippet to your CLAUDE.md or AGENTS.md:" wrapper), and its clap `about` text changed from "Print the snippet to add to CLAUDE.md or AGENTS.md" to "Print the Flockfly discovery instructions"
- `search` and `load` remain unchanged as subcommands; only the `init` snippet's guidance text changed

## Claude Code session sync (2026-08-05)

New feature, not a ported TS behavior — this is the first case where the
source TypeScript CLI grew a capability that didn't exist yet anywhere, so
per `MIGRATION.md`'s "TS is the reference" workflow it was built first in
`context-router/cli` (`src/{hooks,hookSync,session}.ts`, extended
`config.ts`/`commands.ts`, and new `test/{hooks,hookSync}.test.ts` plus
extensions to `test/cli.test.ts`) and then ported here 1:1. It does not
extend the original 21-case table above; it's additive coverage the same
way `search --load` was.

`flockfly init --collection <id>` installs a global Claude Code hook
(`~/.claude/settings.json`'s `Stop`/`SubagentStop`/`SessionEnd` events) that
runs `flockfly session sync --hook` on every Claude Code session, anywhere
on the machine. The hook reads new transcript lines since the last recorded
byte offset, pushes them (harness-native, unnormalized) to the Context
API's `POST /v1/collections/:id/sessions/logs/native`, and on `SessionEnd`
does a full-file `POST .../sessions/reconcile/native` catch-up sweep
instead (dedup-by-id, guards against any incremental push that got missed).
Normalization of raw Claude JSONL into Murmur's unified event shape happens
server-side in Murmur (`normalize_claude_entry`), not in either CLI — both
CLIs stay dumb, uploading transcript bytes unmodified.

New Rust modules, each a direct port of its TypeScript source:

- `src/hooks.rs` (from `hooks.ts`): marker-based idempotent install/remove
  of the three managed hook entries in `~/.claude/settings.json`, reading
  the home directory from `env["HOME"]` (tests inject a temp dir there,
  same as `config_compat.rs` already does for `FLOCKFLY_CONFIG_DIR`) rather
  than always calling `dirs::home_dir()` directly, so it's testable the
  same way.
- `src/hook_sync.rs` (from `hookSync.ts`, itself a port of
  `murmur/scripts/claude-code-hook-sync.py`): `derive_claude_session_key`
  (path → collection-session key + subagent subpath) and
  `read_complete_new_lines` (byte-offset incremental JSONL reader — only
  complete trailing lines, resets on truncation, skips malformed lines),
  plus `SyncState` persistence under `config_dir()/session-sync-state.json`.
- `src/session.rs` (from `session.ts`): `session_sync_command` — the
  `--hook` entrypoint. Best-effort by construction (`Result` errors are
  caught by the caller and written to `runtime.err()`, never propagated),
  so a sync failure can never block Claude Code.
- `src/config.rs` gained `SyncConfig`/`load_sync_config`/`save_sync_config`
  (`~/.flockfly/sync-config.json`, mirroring `credentials.json`'s 0600
  write pattern) — kept separate from credentials so `logout` doesn't
  discard the configured collection.
- `src/commands.rs`: `Init` gained an optional `--collection <id>` (bare
  `init` is unchanged — still prints `INIT_SNIPPET`); new `Hooks { Remove }`
  and `Session { Sync { hook, reconcile, collection } }` subcommands; the
  `Runtime` trait gained `read_stdin(&mut self) -> String` (implemented via
  `io::stdin().read_to_string` in `main.rs`'s `StdRuntime`, and via an
  injectable `stdin: String` field on every fake `Runtime` used in tests).

New Rust test files, each with its own coverage (no 1:1 name mapping to
TypeScript test names, since this is additive, not ported):

- `tests/hooks_compat.rs` — install/idempotent-reinstall/preserves-
  unrelated-hooks/remove, against a temp `HOME`.
- `tests/hook_sync_compat.rs` — key derivation table cases (main transcript,
  subagent subpath, no-`projects/`-segment fallback) and
  `read_complete_new_lines` fixture-file cases (partial trailing line,
  resumed offset, truncation reset, malformed line skipped), plus one case
  reading the shared realistic fixture at
  `tests/fixtures/claude-transcripts/sample-session.jsonl` — byte-identical
  to `context-router/cli/test/fixtures/claude-transcripts/sample-session.jsonl`,
  so both suites exercise the same input.
- `tests/session_sync_compat.rs` — its own minimal fake `Api`/`ApiFactory`
  backend (not `cli_compat.rs`'s skills/routers-oriented one, which doesn't
  model collection-session routes) covering: `init --collection` with/
  without session-publish access, `hooks remove` idempotency, an end-to-end
  `session sync --hook` push against the shared fixture transcript,
  best-effort exit-0 behavior (no collection configured; missing transcript
  file), offset persistence across two incremental hook fires, and
  `--reconcile` backfilling entries a prior incremental push missed.

`tests/PARITY.md` is the living record for changes like this one;
`.code-assist/flockfly-cli-rust-distribution/plan.md` is a frozen snapshot
of the initial TS→Rust transfer checklist and was not updated for the two
redesigns above either — this entry follows that same precedent.
