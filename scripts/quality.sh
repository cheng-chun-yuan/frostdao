#!/usr/bin/env bash
# Run the standard FrostDAO quality gate.
# Usage:
#   ./scripts/quality.sh
#   ./scripts/quality.sh --full
#   ./scripts/quality.sh --with-docs

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

RUN_BUILD=0
RUN_DOCS=0

for arg in "$@"; do
    case "$arg" in
        --full)
            RUN_BUILD=1
            RUN_DOCS=1
            ;;
        --with-docs)
            RUN_DOCS=1
            ;;
        -h|--help)
            sed -n '1,8p' "$0"
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg" >&2
            exit 2
            ;;
    esac
done

run_step() {
    local title="$1"
    shift
    echo ""
    echo "==> $title"
    "$@"
}

echo "FrostDAO Quality Gate"
echo "===================="

run_step "Running doctor checks" ./scripts/doctor.sh
run_step "Checking formatting" cargo fmt --all -- --check
run_step "Running clippy" cargo clippy --all-targets --all-features -- -D warnings
run_step "Running tests" cargo test
run_step "Running all-feature tests" cargo test --all-features

if [[ "$RUN_BUILD" -eq 1 ]]; then
    run_step "Building debug binary" cargo build
fi

if [[ "$RUN_DOCS" -eq 1 ]]; then
    run_step "Building Rust docs" env RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
fi

echo ""
echo "Quality gate passed."
