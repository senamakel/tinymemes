#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$root"
python3 scripts/test-source-profile.py
python3 scripts/test-module-package.py
if [[ "$(uname -s)" == Linux ]]; then
  python3 scripts/test-remix-markers-tests.py
fi
scripts/check-bus-consumer-graphs.sh
cargo test --locked --workspace --no-default-features
cargo test --locked --workspace --all-features
cargo fmt --manifest-path proof-workspaces/remix-markers/Cargo.toml -- --check
cargo clippy --locked --manifest-path proof-workspaces/remix-markers/Cargo.toml --all-targets -- -D warnings
