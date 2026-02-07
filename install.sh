#!/usr/bin/env bash
set -euo pipefail

BIN_NAME="jkl"
REPO="cruzluna/jkl-2"
SYNC_SCRIPT_NAME="jkl-sync-fig-autocomplete"
INSTALL_DIR="${JKL_INSTALL_DIR:-$HOME/.local/bin}"

should_install_fig_helper() {
  case "${JKL_INSTALL_FIG_COMPLETIONS:-}" in
    y|Y|yes|YES|true|TRUE|1) return 0 ;;
    n|N|no|NO|false|FALSE|0) return 1 ;;
    "")
      ;;
    *)
      echo "Invalid JKL_INSTALL_FIG_COMPLETIONS value: ${JKL_INSTALL_FIG_COMPLETIONS}" >&2
      echo "Expected one of: y/n, yes/no, true/false, 1/0" >&2
      return 1
      ;;
  esac

  if [[ -e /dev/tty ]]; then
    while true; do
      read -r -p "Install Fig autocomplete helper (${SYNC_SCRIPT_NAME})? [y/n]: " answer </dev/tty
      case "$answer" in
        y|Y) return 0 ;;
        n|N) return 1 ;;
        *) echo "Please answer y or n." ;;
      esac
    done
  fi

  return 1
}

uname_s="$(uname -s)"
uname_m="$(uname -m)"

case "$uname_s" in
  Darwin) os="apple-darwin" ;;
  Linux) os="unknown-linux-gnu" ;;
  *)
    echo "Unsupported OS: $uname_s" >&2
    exit 1
    ;;
esac

case "$uname_m" in
  x86_64|amd64) arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
  *)
    echo "Unsupported architecture: $uname_m" >&2
    exit 1
    ;;
esac

target="${arch}-${os}"
archive="${BIN_NAME}-${target}.tar.gz"
url="https://github.com/${REPO}/releases/latest/download/${archive}"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

echo "Downloading $url"
curl -fsSL "$url" -o "$tmp_dir/$archive"
tar -xzf "$tmp_dir/$archive" -C "$tmp_dir"

mkdir -p "$INSTALL_DIR"
mv "$tmp_dir/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
chmod +x "$INSTALL_DIR/$BIN_NAME"

echo "Installed $BIN_NAME to $INSTALL_DIR/$BIN_NAME"

sync_script_url="https://raw.githubusercontent.com/${REPO}/main/scripts/sync-fig-autocomplete.sh"
if should_install_fig_helper; then
  if curl -fsSL "$sync_script_url" -o "$INSTALL_DIR/$SYNC_SCRIPT_NAME"; then
    chmod +x "$INSTALL_DIR/$SYNC_SCRIPT_NAME"
    echo "Installed $SYNC_SCRIPT_NAME to $INSTALL_DIR/$SYNC_SCRIPT_NAME"
  else
    echo "Warning: failed to install $SYNC_SCRIPT_NAME from $sync_script_url" >&2
  fi
else
  echo "Skipped $SYNC_SCRIPT_NAME installation."
fi

if ! command -v "$BIN_NAME" >/dev/null 2>&1; then
  echo "Make sure $INSTALL_DIR is on your PATH."
fi
