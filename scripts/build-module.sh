#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$root"
crate="" version="" rust_target="" host="" output="release-assets" extras='[]'
while (($#)); do
  case "$1" in
    --crate) crate="${2:?missing crate}"; shift 2 ;;
    --version) version="${2:?missing version}"; shift 2 ;;
    --target) rust_target="${2:?missing target}"; shift 2 ;;
    --host) host="${2:?missing host}"; shift 2 ;;
    --extra-files-json) extras="${2:?missing extra file list}"; shift 2 ;;
    --output-dir) output="${2:?missing output directory}"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

[[ "$crate" == tinymemes-module && "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]] || {
  echo "usage: build-module.sh --crate tinymemes-module --version VERSION --target TARGET --host HOST [--output-dir DIR]" >&2
  exit 2
}
[[ "$host" =~ ^[A-Za-z0-9]([A-Za-z0-9._-]*[A-Za-z0-9])?$ ]] || { echo "invalid release host: $host" >&2; exit 2; }
jq -e 'type == "array" and length == 0' <<<"$extras" >/dev/null || {
  echo "this release profile does not package additional files" >&2
  exit 2
}
case "$rust_target" in
  x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu) library=libtinymemes_module.so ;;
  x86_64-apple-darwin|aarch64-apple-darwin) library=libtinymemes_module.dylib ;;
  x86_64-pc-windows-msvc|aarch64-pc-windows-msvc) library=tinymemes_module.dll ;;
  *) echo "unsupported module target: $rust_target" >&2; exit 2 ;;
esac
[[ "$output" != /* && "$output" != .. && "$output" != ../* && "$output" != */../* && "$output" != */.. ]] || {
  echo "output directory must stay inside the repository" >&2
  exit 2
}
cursor="$root"
IFS=/ read -r -a output_parts <<<"$output"
for part in "${output_parts[@]}"; do
  cursor="$cursor/$part"
  [[ ! -L "$cursor" ]] || { echo "output path contains a symbolic link: $output" >&2; exit 2; }
done

cargo build --locked --release --manifest-path Cargo.toml --package "$crate" --target "$rust_target"
target_root="$root/target"
stage="$target_root/module-package"
rm -rf -- "$stage"
mkdir -p -- "$stage" "$output"
output_path="$(cd -- "$output" && pwd -P)"
[[ "$output_path" == "$root"/* ]] || { echo "output directory escapes the repository" >&2; exit 2; }
install -m 755 "$target_root/$rust_target/release/$library" "$stage/$library"
if command -v sha256sum >/dev/null 2>&1; then
  library_hash="$(sha256sum "$stage/$library" | awk '{print $1}')"
else
  library_hash="$(shasum -a 256 "$stage/$library" | awk '{print $1}')"
fi
printf '"%s" = "%s"\n' "$library" "$library_hash" > "$stage/modules.toml"
install -m 644 LICENSE README.md "$stage/"
archive="$output_path/$crate-$version-$host.tar.gz"
tar -C "$stage" -czf "$archive" "$library" LICENSE README.md modules.toml
echo "$archive"
