#!/usr/bin/env bash
set -euo pipefail

BIN_NAME="jkl"
REPO="cruzluna/jkl-2"
INSTALL_DIR="${JKL_INSTALL_DIR:-$HOME/.local/bin}"
INSTALL_TAG="${JKL_INSTALL_TAG:-latest}"

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

if ! command -v "$BIN_NAME" >/dev/null 2>&1; then
  echo "Make sure $INSTALL_DIR is on your PATH."
fi
