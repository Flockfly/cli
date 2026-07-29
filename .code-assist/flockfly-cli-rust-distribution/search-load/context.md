# Search `--load` Context

## Summary

Add `flockfly search "<query>" --load` without changing ordinary search or the existing standalone load contract. The flag selects the result with the best API rank, loads its default `SKILL.md`, and prints the same raw rendered output as `flockfly load <skill-id>`.

## Existing Documentation

- `README.md` documents the current Rust command surface and full validation commands.
- `tests/PARITY.md` maps all 21 original TypeScript CLI tests to explicit Rust tests; those tests remain a required regression gate.
- No `CODEASSIST.md`, `AGENTS.md`, or `CONTRIBUTING.md` exists.
- The tnote injects the authoritative feature semantics and requires unit plus real-API E2E coverage.

## Requirements

1. Clap accepts `flockfly search "<query>" --load` and documents the flag in search help.
2. Search still sends exactly one `POST /v1/search` request with the query.
3. Without `--load`, render the ranked result list exactly as before.
4. With `--load`, select the result with the lowest numeric `rank`, independent of array order.
5. Load the selected skill through `POST /v1/skills/<encoded-id>/load` with an empty `paths` array, matching standalone default load.
6. Render the loaded response through `format_loaded_files`; a single `SKILL.md` prints raw.
7. If search returns no results, print `No matching skills found.` and do not request a load.
8. Search and load API failures retain the normal CLI error prefix, stderr routing, and exit code 1.
9. Existing 21-case TypeScript parity, safety tests, E2E journeys, CI, release, and distribution validation remain green.

## Dependency Map

```text
Clap Search { query, load }
  -> authenticated API client
  -> POST /v1/search
     -> no results: existing empty-search renderer
     -> ordinary: existing ranked renderer
     -> --load: best rank -> shared default-load helper
                       -> POST /v1/skills/:id/load
                       -> format_loaded_files
```

## Implementation Paths

- `src/commands.rs`: flag parsing, branching, top-rank selection, shared load helper.
- `tests/cli_compat.rs`: fake API request tracking and focused feature/error tests.
- `tests/e2e/real-api.test.mjs`: compiled-binary coverage against the real Context API.
- `README.md`: public usage example and behavior.
- `tests/PARITY.md`: additive feature coverage note; original mapping remains unchanged.

## Risks

- Assuming response order equals rank could load the wrong skill; select by the explicit `rank`.
- Printing the search list before loaded content would violate the requested output contract.
- Reimplementing load rendering could drift from standalone `load`; use one helper.
- Empty results must not generate a malformed load request.

