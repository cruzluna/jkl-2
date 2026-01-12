#!/usr/bin/env bash

bind-key f display-popup -E "jkl tui"
bind-key c command-prompt -p "Context for #S:" "run-shell \"jkl upsert '#S' '#S' --context '%%'\""
bind-key e display-popup -E "nvim ~/.config/jkl/session_context.json"
