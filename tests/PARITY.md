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
| CLI: publishes with --team and finds the skill via search immediately | `ts_publishes_with_team_and_finds_the_skill_via_search_immediately` |
| CLI: asks before replacing an existing skill and honors the answer | `ts_asks_before_replacing_an_existing_skill_and_honors_the_answer` |
| CLI: prints actionable errors for unknown teams and invalid packages | `ts_prints_actionable_errors_for_unknown_teams_and_invalid_packages` |
| CLI: lists org and team skills | `ts_lists_org_and_team_skills` |

The separately classified original E2E cases are also ported in `tests/e2e/real-api.test.mjs`:

- `TS E2E: covers the full team journey with visibility rules and telemetry`
- `TS E2E: replaces a skill after confirmation and search returns the new version`

Current evidence is recorded in `.code-assist/flockfly-cli-rust-distribution/progress.md`.

