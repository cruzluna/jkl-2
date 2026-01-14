#!/usr/bin/env bash

tmux unbind-key f
tmux unbind-key c
tmux unbind-key e

tmux bind-key f display-popup -E -w 40% -h 40% "jkl tui"
tmux bind-key c command-prompt -p "Context for #S:" "run-shell \"jkl upsert '#S' --session-id '#{session_id}' --context '%%'\""
tmux bind-key e display-popup -E -w 40% -h 40% "nvim ~/.config/jkl/session_context.json"
tmux bind-key S run-shell 'tmux display-popup -E -w 30% -h 30% "jkl tui --pane-state --session-name \"#{session_name}\" --pane-id \"#{pane_id}\""'

tmux set-hook -g session-renamed "run-shell \"jkl rename '#{hook_session}' '#{hook_session_name}'\""
