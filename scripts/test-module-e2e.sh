#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$root"
module_path="${TINYMEMES_MODULE:-}"
if [[ -z "$module_path" ]]; then
  cargo build --locked --package tinymemes-module
  case "$(uname -s)" in
    Darwin) library="$root/target/debug/libtinymemes_module.dylib" ;;
    MINGW*|MSYS*|CYGWIN*) library="$root/target/debug/tinymemes_module.dll" ;;
    *) library="$root/target/debug/libtinymemes_module.so" ;;
  esac
  module_path="$library"
fi
TINYMEMES_MODULE="$module_path" cargo test --locked --package tinymemes-module --test compiled_remix
cargo run --locked --package tinymemes-client --example compiled_state -- "$module_path"
TINYMEMES_MODULE="$module_path" \
  scripts/test-remix-markers.sh
