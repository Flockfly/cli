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
| format: teaches search, load, and progressive disclosure | `ts_teaches_search_load_and_progressive_disclosure` |
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
