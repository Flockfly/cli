#!/bin/bash
set -euo pipefail

REPOSITORY="flockfly/cli"
INSTALLER_URL="https://github.com/${REPOSITORY}/releases/latest/download/flockfly-installer.sh"
AUTH_HEADER=()
if [ -n "${GITHUB_TOKEN:-${GH_TOKEN:-}}" ]; then
  TOKEN="${GITHUB_TOKEN:-${GH_TOKEN:-}}"
  AUTH_HEADER=(-H "authorization: Bearer ${TOKEN}")
  export FLOCKFLY_GITHUB_TOKEN="$TOKEN"
fi

release_json="$(curl -fsSL "${AUTH_HEADER[@]}" "https://api.github.com/repos/${REPOSITORY}/releases/latest")"
latest_version="$(
  echo "$release_json" |
    grep -m1 '"tag_name"' |
    sed -E 's/.*"tag_name": *"v?([^"]+)".*/\1/'
)"
if [ -z "$latest_version" ]; then
  echo "Could not determine the latest Flockfly CLI release." >&2
  exit 1
fi

install_root="$(mktemp -d)"
export CARGO_HOME="${install_root}/cargo"
export HOME="$install_root"

curl --proto '=https' --tlsv1.2 -LsSf "${AUTH_HEADER[@]}" "$INSTALLER_URL" | sh
export PATH="${HOME}/.local/bin:${PATH}"

command -v flockfly >/dev/null
actual_version="$(flockfly --version | awk '{print $2}')"
if [ "$actual_version" != "$latest_version" ]; then
  echo "Expected flockfly ${latest_version}, got ${actual_version}." >&2
  exit 1
fi
flockfly --help >/dev/null
flockfly init | grep -q 'flockfly search "<task>"'
