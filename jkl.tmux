#!/usr/bin/env bash

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

bind_prefix_key() {
  local option_name="$1"
  local default_key="$2"
  shift 2

  local key
  key="$(get_tmux_option "$option_name" "$default_key")"

  # Users can disable a binding by setting the key option to an empty value or "none".
  if [ -z "$key" ] || [ "$key" = "none" ]; then
    return
  fi

  local force_bind
  force_bind="$(get_tmux_option "@jkl_force_bind_keys" "off")"

  # Avoid overriding existing user/default tmux keybindings unless explicitly requested.
  if [ "$force_bind" != "on" ] && tmux list-keys -T prefix "$key" >/dev/null 2>&1; then
    return
  fi

  tmux bind-key "$key" "$@"
}

bind_prefix_key "@jkl_key_tui" "f" display-popup -E -w 40% -h 40% "jkl tui"
bind_prefix_key "@jkl_key_context" "W" command-prompt -p "Context for #S:" "run-shell \"jkl upsert '#S' --session-id '#{session_id}' --context '%%'\""
bind_prefix_key "@jkl_key_edit" "e" display-popup -E -w 40% -h 40% "nvim ~/.config/jkl/session_context.json"
bind_prefix_key "@jkl_key_pane_state" "S" run-shell 'tmux display-popup -E -w 30% -h 20% "jkl tui --pane-state --session-name \"#{session_name}\" --pane-id \"#{pane_id}\""'

tmux set-hook -g session-renamed "run-shell \"jkl rename '#{hook_session}' '#{hook_session_name}'\""
