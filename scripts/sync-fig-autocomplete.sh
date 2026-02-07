#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO="cruzluna/jkl-2"
DEFAULT_SPEC_URL="https://raw.githubusercontent.com/${REPO}/main/completions/fig/jkl.ts"
SPEC_URL="${JKL_FIG_SPEC_URL:-$DEFAULT_SPEC_URL}"
SPEC_SOURCE="$REPO_ROOT/completions/fig/jkl.ts"

FIG_REPO_DIR="${FIG_AUTOCOMPLETE_DIR:-$HOME/.fig/autocomplete}"
FALLBACK_FIG_REPO_DIR="$HOME/.fig/.fig/autocomplete"
FIG_SPEC_DEST="$FIG_REPO_DIR/src/jkl.ts"
AUTO_INIT="${FIG_AUTO_INIT:-1}"
TMP_SPEC=""

cleanup() {
  if [[ -n "$TMP_SPEC" && -f "$TMP_SPEC" ]]; then
    rm -f "$TMP_SPEC"
  fi
}
trap cleanup EXIT

resolve_spec_source() {
  if [[ -f "$SPEC_SOURCE" ]]; then
    echo "$SPEC_SOURCE"
    return 0
  fi

  if ! command -v curl >/dev/null 2>&1; then
    echo "curl is required to download the Fig spec when repo files are unavailable." >&2
    exit 1
  fi

  TMP_SPEC="$(mktemp "${TMPDIR:-/tmp}/jkl-fig-spec.XXXXXX.ts")"
  curl -fsSL "$SPEC_URL" -o "$TMP_SPEC"
  echo "$TMP_SPEC"
}

ensure_fig_repo_dir() {
  if [[ -d "$FIG_REPO_DIR" ]]; then
    return 0
  fi

  if [[ "$FIG_REPO_DIR" == "$HOME/.fig/autocomplete" && -d "$FALLBACK_FIG_REPO_DIR" ]]; then
    FIG_REPO_DIR="$FALLBACK_FIG_REPO_DIR"
    FIG_SPEC_DEST="$FIG_REPO_DIR/src/jkl.ts"
    return 0
  fi

  if [[ "$AUTO_INIT" != "1" ]]; then
    cat >&2 <<EOF
Fig autocomplete repo not found at: $FIG_REPO_DIR
Initialize it first:
  npx @withfig/autocomplete-tools@latest init
EOF
    exit 1
  fi

  if ! command -v npx >/dev/null 2>&1; then
    echo "npx is required to initialize Fig autocomplete at $FIG_REPO_DIR." >&2
    exit 1
  fi

  parent_dir="$(dirname "$FIG_REPO_DIR")"
  mkdir -p "$parent_dir"
  (
    cd "$parent_dir"
    npx @withfig/autocomplete-tools@latest init
  )

  if [[ -d "$FIG_REPO_DIR" ]]; then
    return 0
  fi

  if [[ "$FIG_REPO_DIR" == "$HOME/.fig/autocomplete" && -d "$FALLBACK_FIG_REPO_DIR" ]]; then
    FIG_REPO_DIR="$FALLBACK_FIG_REPO_DIR"
    FIG_SPEC_DEST="$FIG_REPO_DIR/src/jkl.ts"
    return 0
  fi

  echo "Failed to initialize Fig autocomplete repo at: $FIG_REPO_DIR" >&2
  exit 1
}

SPEC_PATH="$(resolve_spec_source)"
ensure_fig_repo_dir

if ! command -v npm >/dev/null 2>&1; then
  echo "npm is required but was not found in PATH." >&2
  exit 1
fi

mkdir -p "$(dirname "$FIG_SPEC_DEST")"
cp "$SPEC_PATH" "$FIG_SPEC_DEST"
echo "Synced spec to: $FIG_SPEC_DEST"

(
  cd "$FIG_REPO_DIR"
  npm run build
)

echo "Fig autocomplete build complete."
