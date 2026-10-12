#!/usr/bin/env bash
# Actual SDK module, hash admitted, using captured native Memory/SQLite ports.
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
cargo build -p tinymemes-module
module_target="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
case "$(uname -s)" in
  Darwin) library="libtinymemes_module.dylib" ;;
  MINGW*|MSYS*|CYGWIN*) library="tinymemes_module.dll" ;;
  *) library="libtinymemes_module.so" ;;
esac
TINYMEMES_MODULE="$module_target/debug/$library" cargo test --manifest-path proof-workspaces/remix-markers/Cargo.toml -p tinymemes-remix-marker-proof
