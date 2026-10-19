#!/usr/bin/env bash
set -euo pipefail

mode="${RELEASE_MODE:-main}"
event="${GITHUB_EVENT_NAME:-}"
ref="${GITHUB_REF:-}"
bump="${BUMP:-}"
tag="${TAG:-}"
version="${VERSION:-}"
reviewed_pr="${REVIEWED_PR:-}"

fail() {
  printf '%s\n' "$1" >&2
  exit 2
}

[[ "$event" == "workflow_dispatch" ]] || fail "releases must be manually dispatched"
[[ "$ref" == "refs/heads/main" ]] || fail "release workflow must be dispatched from main"

case "$mode" in
  main)
    [[ "$bump" == "patch" || "$bump" == "minor" || "$bump" == "major" ]] \
      || fail "main release requires bump=patch, minor, or major"
    [[ -z "$tag" && -z "$version" && -z "$reviewed_pr" ]] \
      || fail "tag, version, and reviewed-pr are only valid in reviewed-epic mode"
    {
      printf 'mode=main\n'
      printf 'bump=%s\n' "$bump"
    } >> "$GITHUB_OUTPUT"
    ;;
  reviewed-epic)
    [[ -z "$bump" ]] || fail "bump is only valid in main release mode"
    [[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]] \
      || fail "reviewed-epic tag must be v-prefixed semver"
    [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]] \
      || fail "reviewed-epic version must be semver without the v prefix"
    [[ "$tag" == "v$version" ]] || fail "reviewed-epic tag and version must match"
    [[ "$reviewed_pr" =~ ^[1-9][0-9]*$ ]] \
      || fail "reviewed-pr must be a positive canonical PR number"
    {
      printf 'mode=reviewed-epic\n'
      printf 'tag=%s\n' "$tag"
      printf 'version=%s\n' "$version"
      printf 'reviewed_pr=%s\n' "$reviewed_pr"
    } >> "$GITHUB_OUTPUT"
    ;;
  *)
    fail "release_mode must be main or reviewed-epic"
    ;;
esac
