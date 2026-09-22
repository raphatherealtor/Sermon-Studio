#!/usr/bin/env bash
# Verify the canon.db pipeline end-to-end.
#
#   1. ETL self-test (determinism, checksums, manifest) — no network.
#   2. Rust canon builder + adapter + validation tests.
#   3. (Production-only) build a real canon.db from data/raw when present and
#      run the data-quality gates against it.
#
# Usage:
#   scripts/verify-canon.sh            # self-test + unit tests
#   scripts/verify-canon.sh --build    # also build data/canon.db from data/raw
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD="${1:-}"

echo "==> ETL self-test"
python3 "$ROOT/tools/etl.py" --self-test

echo "==> Rust canon tests"
cargo test --locked -p sermon_core canon::

if [[ "$BUILD" == "--build" ]]; then
  echo "==> Building canon.db from data/raw"
  python3 "$ROOT/tools/etl.py" --raw "$ROOT/data/raw" --out "$ROOT/data/clean"
  cargo run --release --locked --manifest-path "$ROOT/Cargo.toml" -p sermon -- \
    build-canon --clean "$ROOT/data/clean" --out "$ROOT/data/canon.db"
  echo "==> canon.db built: $(ls -lh "$ROOT/data/canon.db" | awk '{print $5}')"
  echo "==> Copying into the desktop resource dir for packaging"
  mkdir -p "$ROOT/data/resources"
  cp "$ROOT/data/canon.db" "$ROOT/data/resources/canon.db"
fi

echo "==> OK"
