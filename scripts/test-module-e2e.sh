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

source_path="$module_path"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) source_path="$(cygpath -u "$module_path")" ;;
esac
[[ -f "$source_path" && -r "$source_path" ]] || {
  echo "module artifact is not a readable file" >&2
  exit 2
}

stage_dir="$(mktemp -d "$root/target/module-e2e-private-XXXXXX")"
cleanup() { rm -rf -- "$stage_dir"; }
trap cleanup EXIT
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    stage_windows="$(cygpath -w "$stage_dir")"
    if ! sid="$(powershell.exe -NoLogo -NoProfile -NonInteractive -Command '[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value')"; then
      echo "could not resolve the current Windows user for the private module fixture" >&2
      exit 2
    fi
    sid="${sid//$'\r'/}"
    sid="${sid//$'\n'/}"
    [[ "$sid" =~ ^S-1-[0-9]+(-[0-9]+)+$ ]] || {
      echo "PowerShell returned an invalid current-user SID" >&2
      exit 2
    }
    if ! MSYS2_ARG_CONV_EXCL='*' icacls "$stage_windows" /inheritance:r /grant:r "*${sid}:(OI)(CI)F" >/dev/null; then
      echo "could not make the module fixture directory private on Windows" >&2
      exit 2
    fi
    ;;
  *)
    chmod 700 "$stage_dir"
    ;;
esac

library_name="$(basename -- "$source_path")"
staged_module="$stage_dir/$library_name"
file_hash() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}
source_hash="$(file_hash "$source_path")"
cp -- "$source_path" "$staged_module"
staged_hash="$(file_hash "$staged_module")"
[[ "$source_hash" == "$staged_hash" ]] || {
  echo "staged module artifact hash differs from its source" >&2
  exit 2
}
staged_env_path="$staged_module"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) staged_env_path="$(cygpath -w "$staged_module")" ;;
esac

TINYMEMES_MODULE="$staged_env_path" cargo test --locked --package tinymemes-module --test compiled_remix
TINYMEMES_MODULE="$staged_env_path" cargo run --locked --package tinymemes-client --example compiled_state -- "$staged_module"
TINYMEMES_MODULE="$staged_env_path" \
  scripts/test-remix-markers.sh
