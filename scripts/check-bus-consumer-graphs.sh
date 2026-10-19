#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$root"

for package in tinymemes-bus-default-consumer tinymemes-bus-no-default-consumer; do
  tree="$(cargo tree --locked \
    --manifest-path proof-workspaces/bus-contract/Cargo.toml \
    --package "$package" --edges normal,build --prefix none)"
  forbidden="$(printf '%s\n' "$tree" | grep -Ei 'tinybus|tinystoragedrivers|tokio|reqwest|ureq|hyper|rusqlite|git2' || true)"
  if [[ -n "$forbidden" ]]; then
    printf 'the %s consumer pulled in a transport, storage implementation, or runtime:\n%s\n' \
      "$package" "$forbidden" >&2
    exit 1
  fi
done

cargo test --locked -p tinymemes-bus --no-default-features --features storage
