#!/bin/bash
# Compare Python vs Rust snapshot library performance
# Usage: ./bench_compare.sh [--preset tiny|small] [extra args...]

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUST_CRATE="$HOME/openjd-rs/crates/openjd-snapshots"
PYTHON_SCRIPT="$HOME/deadline-cloud/scripted_tests/snapshots_scale_test.py"
RUST_BIN="$RUST_CRATE/../../target/release/snapshots-bench"

# Default args
PRESET="${1:---preset}"
PRESET_VAL="${2:-tiny}"
shift 2 2>/dev/null || true

# Build Rust binary in release mode
echo "=== Building Rust benchmark (release) ==="
cd "$RUST_CRATE"
RUSTUP_TOOLCHAIN=stable cargo build --release --features bench --bin snapshots-bench 2>&1 | tail -3

# Generate shared test data
SOURCE_DIR=$(mktemp -d /tmp/snapshots_bench_XXXXXX)
echo ""
echo "=== Generating test data in $SOURCE_DIR ==="
"$RUST_BIN" --local-only $PRESET $PRESET_VAL --source-dir "$SOURCE_DIR" --skip-download --no-verify "$@" 2>&1 | grep -E "^(  |Creating|  Total:)"

echo ""
echo "================================================================"
echo "  PYTHON BENCHMARK"
echo "================================================================"
python "$PYTHON_SCRIPT" --local-only $PRESET $PRESET_VAL --source-dir "$SOURCE_DIR" --no-verify "$@" 2>&1

echo ""
echo "================================================================"
echo "  RUST BENCHMARK"
echo "================================================================"
"$RUST_BIN" --local-only $PRESET $PRESET_VAL --source-dir "$SOURCE_DIR" --no-verify "$@" 2>&1

# Cleanup
echo ""
echo "=== Cleaning up $SOURCE_DIR ==="
rm -rf "$SOURCE_DIR"
echo "Done."
