#!/usr/bin/env bash
set -euo pipefail

if command -v dnf >/dev/null 2>&1; then
  dnf install -y gcc gcc-c++ cmake curl git gzip jq make openssl-devel perl python3 tar
elif command -v pacman >/dev/null 2>&1; then
  pacman -Syu --disable-sandbox-filesystem --noconfirm base-devel cmake curl git gzip jq openssl perl python tar
else
  echo "unsupported distro package manager" >&2
  exit 2
fi
