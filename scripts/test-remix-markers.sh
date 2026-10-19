#!/usr/bin/env bash
# Actual SDK module, hash admitted, using captured native Memory/SQLite ports.
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
# Optional global Cargo arguments allow an explicitly recorded unpublished
# runtime override during development; publication CI uses public locked sources.
module_path="${TINYMEMES_MODULE:-}"
if [[ -z "$module_path" ]]; then
  cargo "$@" build --locked -p tinymemes-module
  module_target="$(cargo "$@" metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
  case "$(uname -s)" in
    Darwin) library="libtinymemes_module.dylib" ;;
    MINGW*|MSYS*|CYGWIN*) library="tinymemes_module.dll" ;;
    *) library="libtinymemes_module.so" ;;
  esac
  module_path="$module_target/debug/$library"
fi
TINYMEMES_MODULE="$module_path" cargo "$@" test --locked --manifest-path proof-workspaces/remix-markers/Cargo.toml -p tinymemes-remix-marker-proof
