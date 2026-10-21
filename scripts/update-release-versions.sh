#!/usr/bin/env bash
set -euo pipefail

root="${1:?usage: update-release-versions.sh <repository-root> <next-version>}"
root="$(cd -- "$root" && pwd -P)"
next_version="${2:?usage: update-release-versions.sh <repository-root> <next-version>}"
[[ "$next_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "next version must be numeric X.Y.Z" >&2
  exit 2
}
cd -- "$root"
export NEXT_VERSION="$next_version"

update_package_version() {
  perl -0pi -e 's/(\[package\][\s\S]*?\nversion = ")[^"]+("\s*\n)/$1$ENV{NEXT_VERSION}$2/' "$1"
}

update_package_version Cargo.toml
update_package_version crates/tinymemes-module/Cargo.toml
update_package_version crates/tinymemes-bus/Cargo.toml
update_package_version crates/tinymemes-client/Cargo.toml

perl -0pi -e 's/(tinymemes-bus = \{ version = ")[^"]+("\s*,\s*path = ")/$1$ENV{NEXT_VERSION}$2/g' \
  Cargo.toml \
  crates/tinymemes-client/Cargo.toml \
  proof-workspaces/remix-markers/runner/Cargo.toml
perl -0pi -e 's/(tinymemes-client = \{ version = ")[^"]+("\s*,\s*path = ")/$1$ENV{NEXT_VERSION}$2/g' \
  proof-workspaces/remix-markers/runner/Cargo.toml
