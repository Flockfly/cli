# Search `--load` Plan

## Test Scenarios

- [x] Argument parsing/help: `search --help` -> code 0 and `--load` documented.
- [x] Top-result selection and rendering: unordered ranks `[2: skill_1, 1: skill_2]` plus `search query --load` -> load request targets `skill_2`; stdout is raw skill content, not ranked metadata.
- [x] Ordinary search regression: `search query` -> existing ranked list and no load request.
- [x] Empty result behavior: empty search plus `--load` -> code 0, `No matching skills found.`, zero load requests.
- [x] Search API failure: search endpoint error plus `--load` -> code 1, actionable stderr, zero load requests.
- [x] Load API failure: nonempty search followed by load error -> code 1 and the load error on stderr.
- [x] Real-API success: publish two attached skills, search for the intended top match with `--load` -> raw top SKILL.md only and correlated load telemetry.
- [x] Real-API empty search: unmatched query with `--load` -> existing no-match text and no new load event.

## TDD Sequence

- [x] Extend only the fake API test harness with ordered-result/error/request tracking.
- [x] Add all focused unit tests and real-API E2E assertions before production changes.
- [x] Run focused Rust tests and capture expected RED failures.
- [x] Add the Clap flag and refactor standalone/default load into one helper.
- [x] Select the minimum `rank`, branch correctly for ordinary/empty searches, and propagate failures.
- [x] Run focused tests to GREEN and refactor without changing output.
- [x] Update README and parity documentation.
- [x] Run formatting, full Rust tests, real-API E2E, Clippy, release build/smoke, cargo-dist generation/plan/artifact validation.
- [x] Commit only after every gate passes.
