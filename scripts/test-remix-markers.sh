#!/usr/bin/env bash
# Actual SDK module, hash admitted, using captured native Memory/SQLite ports.
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
# Optional global Cargo arguments allow an explicitly recorded unpublished
# runtime override; the checked-in dependency pins remain canonical.
cargo_options=("$@")
cargo "${cargo_options[@]}" build -p tinymemes-module
module_target="$(cargo "${cargo_options[@]}" metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
case "$(uname -s)" in
  Darwin) library="libtinymemes_module.dylib" ;;
  MINGW*|MSYS*|CYGWIN*) library="tinymemes_module.dll" ;;
  *) library="libtinymemes_module.so" ;;
esac
TINYMEMES_MODULE="$module_target/debug/$library" cargo "${cargo_options[@]}" test --manifest-path proof-workspaces/remix-markers/Cargo.toml -p tinymemes-remix-marker-proof
