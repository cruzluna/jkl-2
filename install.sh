#!/usr/bin/env bash
set -euo pipefail

BIN_NAME="jkl"
REPO="cruzluna/jkl-2"
INSTALL_DIR="${JKL_INSTALL_DIR:-$HOME/.local/bin}"
INSTALL_TAG="${JKL_INSTALL_TAG:-latest}"
DRY_RUN=false

if [[ -t 1 ]]; then
  MUTED='\033[0;2m'
  RED='\033[0;31m'
  ORANGE='\033[38;5;214m'
  NC='\033[0m'
else
  MUTED=''
  RED=''
  ORANGE=''
  NC=''
fi

usage() {
  cat <<EOF
jkl Installer

Usage: install.sh [options]

Options:
  -h, --help      Show this help message
      --dry-run   Print what would be installed without downloading or writing files

Examples:
  curl -fsSL https://raw.githubusercontent.com/${REPO}/master/install.sh | bash
  curl -fsSL https://raw.githubusercontent.com/${REPO}/master/install.sh | bash -s -- --dry-run
  JKL_INSTALL_DIR=/tmp/jkl-test ./install.sh --dry-run
EOF
}

print_message() {
  local level="$1"
  local message="$2"
  local color="$NC"

  case "$level" in
    info) color="$NC" ;;
    muted) color="$MUTED" ;;
    accent) color="$ORANGE" ;;
    error) color="$RED" ;;
  esac

  printf "%b%s%b\n" "$color" "$message" "$NC"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    *)
      print_message error "Unknown option: $1"
      usage
      exit 1
      ;;
  esac
done

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

if [[ "$DRY_RUN" == "true" ]]; then
  print_message accent "jkl installer dry run"
  print_message info "Resolved target: $target${asset_suffix}"
  print_message info "Archive: $archive"
  print_message info "Download URL: $url"
  print_message info "Install destination: $INSTALL_DIR/$BIN_NAME"
  if ! command -v "$BIN_NAME" >/dev/null 2>&1; then
    print_message info "PATH reminder: make sure $INSTALL_DIR is on your PATH."
  fi
else
  print_message accent "Installing jkl"
  print_message info "Downloading $url"
  curl -fsSL "$url" -o "$tmp_dir/$archive"
  tar -xzf "$tmp_dir/$archive" -C "$tmp_dir"

  mkdir -p "$INSTALL_DIR"
  mv "$tmp_dir/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
  chmod +x "$INSTALL_DIR/$BIN_NAME"

  print_message info "Installed $BIN_NAME to $INSTALL_DIR/$BIN_NAME"

  if ! command -v "$BIN_NAME" >/dev/null 2>&1; then
    print_message info "Make sure $INSTALL_DIR is on your PATH."
  fi
fi

printf "\n"
if [[ "$DRY_RUN" == "true" ]]; then
  print_message info "Dry run complete. No files were downloaded or modified."
else
  print_message info "jkl is installed and ready."
fi
printf "\n"

WORKING_HOOK_COMMAND='[ -n "$TMUX" ] || exit 0; jkl upsert "$(tmux display-message -p '"'"'#S'"'"')" --session-id "$(tmux display-message -p '"'"'#{session_id}'"'"')" --pane-id "$(tmux display-message -p '"'"'#{pane_id}'"'"')" --status working'
WAITING_HOOK_COMMAND='[ -n "$TMUX" ] || exit 0; jkl upsert "$(tmux display-message -p '"'"'#S'"'"')" --session-id "$(tmux display-message -p '"'"'#{session_id}'"'"')" --pane-id "$(tmux display-message -p '"'"'#{pane_id}'"'"')" --status waiting'

print_message accent "Helpful next steps ✨"
printf "\n"

print_message accent "Claude Code prompt"
print_message muted "Paste this into Claude Code to mark the current tmux pane as working when you send a prompt, and waiting when Claude stops."
print_message muted "UserPromptSubmit:"
print_message info "$WORKING_HOOK_COMMAND"
print_message muted "Stop:"
print_message info "$WAITING_HOOK_COMMAND"
printf "\n"

print_message accent "Kiro CLI prompt"
print_message muted "Paste this into Kiro CLI to mark the current tmux pane as working when you send a prompt, and waiting when Kiro stops."
print_message muted "userPromptSubmit:"
print_message info "$WORKING_HOOK_COMMAND"
print_message muted "stop:"
print_message info "$WAITING_HOOK_COMMAND"
printf "\n"

print_message accent "tmux setup prompt"
print_message muted "Paste this into your agent to update ~/.tmux.conf so the jkl-2 tmux plugin is enabled:"
print_message info "set -g @plugin 'cruzluna/jkl-2'"
print_message info "set -g @jkl_force_bind_keys 'on'"
print_message info "run '~/.tmux/plugins/tpm/tpm'"
print_message muted "Then reload tmux. If TPM still needs to install the plugin, run:"
print_message info "tmux run-shell \"~/.tmux/plugins/tpm/bin/install_plugins\""
print_message muted "After reloading, open the list right away with <prefix> f (or your configured agent view key)."
