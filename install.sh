#!/usr/bin/env bash
set -euo pipefail

BIN_NAME="jkl"
REPO="cruzluna/jkl-2"
SYNC_SCRIPT_NAME="jkl-sync-fig-autocomplete"
INSTALL_DIR="${JKL_INSTALL_DIR:-$HOME/.local/bin}"
INSTALL_TAG="${JKL_INSTALL_TAG:-latest}"

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

is_amazon_linux_2() {
  if [[ -r /etc/os-release ]]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    if [[ "${ID:-}" == "amzn" && ( "${VERSION_ID:-}" == "2" || "${VERSION_ID:-}" == 2.* ) ]]; then
      return 0
    fi
  fi

  if [[ -r /etc/system-release ]] && grep -qi "Amazon Linux release 2" /etc/system-release; then
    return 0
  fi

  return 1
}

uname_s="$(uname -s)"
uname_m="$(uname -m)"

case "$uname_s" in
  Darwin) os="apple-darwin" ;;
  Linux)
    os="unknown-linux-gnu"
    if [[ -f /etc/alpine-release ]]; then
      os="unknown-linux-musl"
    fi
    ;;
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

asset_suffix=""
target="${arch}-${os}"
if [[ -n "${JKL_INSTALL_TARGET:-}" ]]; then
  target="${JKL_INSTALL_TARGET}"
fi

if [[ "$target" == *"-al2" ]]; then
  target="${target%-al2}"
  if [[ -z "${JKL_INSTALL_ASSET_SUFFIX:-}" ]]; then
    asset_suffix="-al2"
  fi
fi

if [[ -n "${JKL_INSTALL_ASSET_SUFFIX:-}" ]]; then
  asset_suffix="${JKL_INSTALL_ASSET_SUFFIX}"
elif [[ "$target" == *"-unknown-linux-gnu" ]] && is_amazon_linux_2; then
  asset_suffix="-al2"
fi

archive="${BIN_NAME}-${target}${asset_suffix}.tar.gz"
if [[ "$INSTALL_TAG" == "latest" ]]; then
  url="https://github.com/${REPO}/releases/latest/download/${archive}"
else
  url="https://github.com/${REPO}/releases/download/${INSTALL_TAG}/${archive}"
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

echo "Downloading $url"
curl -fsSL "$url" -o "$tmp_dir/$archive"
tar -xzf "$tmp_dir/$archive" -C "$tmp_dir"

mkdir -p "$INSTALL_DIR"
mv "$tmp_dir/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
chmod +x "$INSTALL_DIR/$BIN_NAME"

echo "Installed $BIN_NAME to $INSTALL_DIR/$BIN_NAME"

sync_script_url="https://raw.githubusercontent.com/${REPO}/master/scripts/sync-fig-autocomplete.sh"
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
