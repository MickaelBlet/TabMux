#!/usr/bin/env bash
# Build TabMux. Usage: scripts/build.sh [--debug] [--check]
#   --debug  build in debug mode (default: release)
#   --check  run fmt check, clippy and tests before building
set -euo pipefail

cd "$(dirname "$0")/.."

profile=release
check=false
for arg in "$@"; do
    case "$arg" in
        --debug) profile=debug ;;
        --check) check=true ;;
        -h|--help) sed -n '2,4p' "$0"; exit 0 ;;
        *) echo "unknown option: $arg" >&2; exit 1 ;;
    esac
done

if $check; then
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
fi

if [[ $profile == release ]]; then
    cargo build --release
else
    cargo build
fi

echo "Built: target/$profile/tabmux"
