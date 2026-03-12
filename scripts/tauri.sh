#!/usr/bin/env bash

set -euo pipefail

if [[ $# -eq 0 ]]; then
  exec tauri
fi

command="$1"
shift

case "$command" in
  dev)
    exec tauri dev --features devtools "$@"
    ;;
  build)
    cli_args=()
    cargo_args=(--no-default-features)
    saw_separator=0

    for arg in "$@"; do
      if [[ $saw_separator -eq 0 && "$arg" == "--" ]]; then
        saw_separator=1
        continue
      fi

      if [[ $saw_separator -eq 0 ]]; then
        cli_args+=("$arg")
      else
        cargo_args+=("$arg")
      fi
    done

    exec tauri build "${cli_args[@]}" -- "${cargo_args[@]}"
    ;;
  *)
    exec tauri "$command" "$@"
    ;;
esac
