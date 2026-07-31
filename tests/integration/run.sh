#!/bin/bash
set -euo pipefail

PASS=0
FAIL=0
ERRORS=""

pass() { PASS=$((PASS + 1)); echo "  ✓ $1"; }
fail() { FAIL=$((FAIL + 1)); ERRORS="${ERRORS}\n  ✗ $1"; echo "  ✗ $1"; }

echo "=== flockfly integration tests ($(flockfly --version)) ==="
echo ""

# ── Commands that never touch the network ──────────────────────────────────

echo "Offline commands:"

flockfly --version | grep -q "flockfly" && pass "--version" || fail "--version"
flockfly --help | grep -q "publish" && pass "--help lists publish" || fail "--help lists publish"
flockfly search --help | grep -q -- "--load" && pass "search --help lists --load" || fail "search --help lists --load"
flockfly init | grep -q 'flockfly search "<task>"' && pass "init prints snippet" || fail "init prints snippet"

! flockfly bogus-command >/dev/null 2>&1 && pass "unknown subcommand exits nonzero" || fail "unknown subcommand exits nonzero"

echo ""

# ── Auth gate: every credentialed command must refuse before hitting the network ──
#
# Note: these commands are *expected* to exit non-zero. Piping their output
# straight into `grep -q` would be wrong here - under `pipefail` (set above),
# the pipeline reports the producer's non-zero exit even when grep finds its
# match, so the assertion would always read as a failure. Capture output into
# a variable first (with `|| true` so `set -e` doesn't abort on the expected
# failure), then grep the variable instead.

echo "Auth gate (no credentials):"

export FLOCKFLY_CONFIG_DIR="$(mktemp -d)"
unset FLOCKFLY_TOKEN FLOCKFLY_API_URL 2>/dev/null || true

NOT_LOGGED_IN='You are not logged in. Run `flockfly login` first.'

OUT=$(flockfly whoami 2>&1 || true)
echo "$OUT" | grep -qF "$NOT_LOGGED_IN" && pass "whoami refuses without credentials" || fail "whoami refuses without credentials"

OUT=$(flockfly search "anything" 2>&1 || true)
echo "$OUT" | grep -qF "$NOT_LOGGED_IN" && pass "search refuses without credentials" || fail "search refuses without credentials"

OUT=$(flockfly load skill_abc 2>&1 || true)
echo "$OUT" | grep -qF "$NOT_LOGGED_IN" && pass "load refuses without credentials" || fail "load refuses without credentials"

OUT=$(flockfly teams list 2>&1 || true)
echo "$OUT" | grep -qF "$NOT_LOGGED_IN" && pass "teams list refuses without credentials" || fail "teams list refuses without credentials"

OUT=$(flockfly skills list 2>&1 || true)
echo "$OUT" | grep -qF "$NOT_LOGGED_IN" && pass "skills list refuses without credentials" || fail "skills list refuses without credentials"

OUT=$(flockfly team add --skill skill_abc --team eng 2>&1 || true)
echo "$OUT" | grep -qF "$NOT_LOGGED_IN" && pass "team add refuses without credentials" || fail "team add refuses without credentials"

# publish authenticates before it validates the skill directory, so a
# nonexistent directory should still surface the auth error, not a
# missing-directory error - this locks in that ordering.
OUT=$(flockfly publish /nonexistent/skill-dir 2>&1 || true)
echo "$OUT" | grep -qF "$NOT_LOGGED_IN" && pass "publish checks auth before the skill directory" || fail "publish checks auth before the skill directory"

echo ""

# ── Credential discovery and precedence ─────────────────────────────────────

echo "Credential discovery:"

UNREACHABLE="http://127.0.0.1:1"

# Malformed/incomplete credentials.json (empty token) must be treated as "not logged in".
cat > "$FLOCKFLY_CONFIG_DIR/credentials.json" <<EOF
{"apiUrl": "$UNREACHABLE", "token": ""}
EOF
OUT=$(flockfly whoami 2>&1 || true)
echo "$OUT" | grep -qF "$NOT_LOGGED_IN" && pass "empty token in credentials.json is ignored" || fail "empty token in credentials.json is ignored"

# A valid credentials.json should be read from FLOCKFLY_CONFIG_DIR, and its
# apiUrl used for the request - proven here by the network-unreachable error
# naming that exact URL.
cat > "$FLOCKFLY_CONFIG_DIR/credentials.json" <<EOF
{"apiUrl": "$UNREACHABLE", "token": "test-token"}
EOF
OUT=$(flockfly whoami 2>&1 || true)
echo "$OUT" | grep -qF "Could not reach the Flockfly API at $UNREACHABLE" && pass "credentials.json apiUrl is honored" || fail "credentials.json apiUrl is honored"

# FLOCKFLY_TOKEN + FLOCKFLY_API_URL must take precedence over credentials.json.
ENV_URL="http://127.0.0.1:2"
OUT=$(FLOCKFLY_TOKEN=env-token FLOCKFLY_API_URL="$ENV_URL" flockfly whoami 2>&1 || true)
echo "$OUT" | grep -qF "Could not reach the Flockfly API at $ENV_URL" && pass "FLOCKFLY_TOKEN/FLOCKFLY_API_URL override credentials.json" || fail "FLOCKFLY_TOKEN/FLOCKFLY_API_URL override credentials.json"

# An empty FLOCKFLY_TOKEN must be ignored, not treated as a real (empty) credential.
rm -f "$FLOCKFLY_CONFIG_DIR/credentials.json"
OUT=$(FLOCKFLY_TOKEN="" flockfly whoami 2>&1 || true)
echo "$OUT" | grep -qF "$NOT_LOGGED_IN" && pass "empty FLOCKFLY_TOKEN env var is ignored" || fail "empty FLOCKFLY_TOKEN env var is ignored"

echo ""

# ── Summary ──────────────────────────────────────────────────────────────────

echo "=== Results: $PASS passed, $FAIL failed ==="
if [ "$FAIL" -gt 0 ]; then
    echo -e "\nFailures:$ERRORS"
    exit 1
fi
