#!/usr/bin/env bash

set -euo pipefail

TOGGLE_EXIT_CODE=42
SOURCE_PANE="${1:-}"
TARGET_FILE="$(mktemp "${TMPDIR:-/tmp}/jkl-preview-target.XXXXXX")"
PREVIEW_PANE_ID=""
PREVIEW_ENABLED=0

get_tmux_option() {
  local option="$1"
  local default="$2"
  local value
  value="$(tmux show-option -gqv "$option")"
  if [ -n "$value" ]; then
    echo "$value"
  else
    echo "$default"
  fi
}

cleanup() {
  if [ -n "$PREVIEW_PANE_ID" ]; then
    tmux kill-pane -t "$PREVIEW_PANE_ID" >/dev/null 2>&1 || true
  fi
  rm -f "$TARGET_FILE"
}
trap cleanup EXIT

ensure_preview_pane() {
  local preview_width
  preview_width="$(get_tmux_option "@jkl_live_preview_width" "40%")"
  local split_target=()
  if [ -n "$SOURCE_PANE" ]; then
    split_target=(-t "$SOURCE_PANE")
  fi

  PREVIEW_PANE_ID="$(
    tmux split-window "${split_target[@]}" -h -l "$preview_width" -P -F "#{pane_id}" \
      "jkl preview --target-file \"$TARGET_FILE\" --lines 120"
  )"

  if [ -n "$SOURCE_PANE" ]; then
    tmux select-pane -t "$SOURCE_PANE" >/dev/null 2>&1 || true
  fi
}

show_list_popup_normal() {
  local popup_width popup_height
  popup_width="$(get_tmux_option "@jkl_popup_width" "80%")"
  popup_height="$(get_tmux_option "@jkl_popup_height" "70%")"
  tmux display-popup -E -w "$popup_width" -h "$popup_height" \
    "jkl tui --external-preview --preview-target-file \"$TARGET_FILE\""
}

show_list_popup_with_external_preview() {
  local popup_width popup_height
  popup_width="$(get_tmux_option "@jkl_live_popup_width" "52%")"
  popup_height="$(get_tmux_option "@jkl_popup_height" "70%")"
  tmux display-popup -E -w "$popup_width" -h "$popup_height" -x 0 \
    "jkl tui --external-preview --preview-target-file \"$TARGET_FILE\""
}

while true; do
  if [ "$PREVIEW_ENABLED" -eq 1 ]; then
    if show_list_popup_with_external_preview; then
      EXIT_CODE=0
    else
      EXIT_CODE=$?
    fi
  else
    if show_list_popup_normal; then
      EXIT_CODE=0
    else
      EXIT_CODE=$?
    fi
  fi

  if [ "$EXIT_CODE" -eq "$TOGGLE_EXIT_CODE" ]; then
    if [ "$PREVIEW_ENABLED" -eq 0 ]; then
      PREVIEW_ENABLED=1
      ensure_preview_pane
    else
      PREVIEW_ENABLED=0
      if [ -n "$PREVIEW_PANE_ID" ]; then
        tmux kill-pane -t "$PREVIEW_PANE_ID" >/dev/null 2>&1 || true
        PREVIEW_PANE_ID=""
      fi
    fi
    continue
  fi

  break
done
