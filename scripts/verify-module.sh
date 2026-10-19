#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
if [[ "${1:-}" == --archive ]]; then shift; fi
archive="${1:?usage: verify-module.sh --archive <module-archive>}"
archive="$(cd -- "$(dirname -- "$archive")" && pwd -P)/$(basename -- "$archive")"
cd -- "$root"
asset="${MODULE_ASSET:-tinymemes-module}"
version="${MODULE_VERSION:?MODULE_VERSION is required}"
host="${MODULE_HOST:?MODULE_HOST is required}"
expected="$asset-$version-$host.tar.gz"
[[ "$(basename -- "$archive")" == "$expected" ]] || {
  echo "unexpected module archive name: $(basename -- "$archive")" >&2
  exit 2
}
case "$host" in
  *macos*) library=libtinymemes_module.dylib ;;
  *windows*) echo "Windows archives use verify-module.ps1" >&2; exit 2 ;;
  *-x86_64|*-arm64) library=libtinymemes_module.so ;;
  *) echo "unsupported module host: $host" >&2; exit 2 ;;
esac
work="$root/target/verify-module-$$"
rm -rf -- "$work"
mkdir -p -- "$work"
trap 'rm -rf -- "$work"' EXIT
members="$(tar -tzf "$archive")"
while IFS= read -r member; do
  case "$member" in
    /*|../*|*/../*|*/..) echo "archive member escapes the extraction directory: $member" >&2; exit 2 ;;
    */*) echo "archive member is nested unexpectedly: $member" >&2; exit 2 ;;
  esac
done <<<"$members"
if ! tar -tvzf "$archive" | awk 'substr($0, 1, 1) != "-" { exit 1 }'; then
  echo "archive contains a link, directory, or special file" >&2
  exit 2
fi
tar -xzf "$archive" -C "$work"
expected_files=("$library" LICENSE README.md modules.toml)
shopt -s nullglob dotglob
actual_files=("$work"/*)
[[ "${#actual_files[@]}" -eq "${#expected_files[@]}" ]] || {
  echo "archive has an unexpected file set" >&2
  exit 1
}
for name in "${expected_files[@]}"; do
  [[ -f "$work/$name" && ! -L "$work/$name" ]] || { echo "archive is missing $name" >&2; exit 1; }
done
if command -v sha256sum >/dev/null 2>&1; then
  hash="$(sha256sum "$work/$library" | awk '{print $1}')"
else
  hash="$(shasum -a 256 "$work/$library" | awk '{print $1}')"
fi
line_count="$(wc -l < "$work/modules.toml")"
[[ "$line_count" -eq 1 ]] && grep -Fxq "\"$library\" = \"$hash\"" "$work/modules.toml" || {
  echo "modules.toml does not pin the packaged library hash" >&2
  exit 1
}

TINYMEMES_MODULE="$work/$library" scripts/test-module-e2e.sh
