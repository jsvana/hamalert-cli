#!/bin/sh
set -eu

run_cli() {
  if command -v hamalert-cli >/dev/null 2>&1; then
    hamalert-cli "$@"
  elif [ -f Cargo.toml ]; then
    cargo run -- "$@"
  else
    echo "hamalert-cli binary not found and no Cargo.toml is present" >&2
    exit 1
  fi
}

run_help() {
  printf '\n## hamalert-cli'
  if [ "$#" -gt 0 ]; then
    printf ' %s' "$@"
  fi
  printf ' --help\n\n'
  run_cli "$@" --help
}

run_help
run_help auth
run_help auth login
run_help add-trigger
run_help import-polo-notes
run_help import-file
run_help backup
run_help restore
run_help edit
run_help pause
run_help resume
run_help bulk-delete
run_help profile
run_help profile save
run_help profile switch
run_help profile set-permanent
