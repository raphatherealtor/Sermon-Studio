#!/usr/bin/env bash
# Build the static canon vault (canon.db) from raw public-domain sources.
#
# The canon vault is READ-ONLY at runtime and contains:
#   * KJV Bible text (31,103 verses)
#   * OpenBible cross-references (ranges expanded to verse-level links)
#   * (future) Strong's lexicon, STEPBible morphology, topical indexes
#
# Usage:
#   scripts/build-canon.sh [RAW_DIR] [OUT_DB]
#
# Defaults: RAW_DIR=data/raw  OUT_DB=data/canon.db
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RAW_DIR="${1:-$ROOT/data/raw}"
OUT_DB="${2:-$ROOT/data/canon.db}"
CLEAN_DIR="$ROOT/data/clean"

echo "==> Acquiring raw sources (build-time only) -> $RAW_DIR"
python3 "$ROOT/tools/acquire.py" --out "$RAW_DIR"

echo "==> Normalizing raw sources -> $CLEAN_DIR"
python3 "$ROOT/tools/etl.py" --raw "$RAW_DIR" --out "$CLEAN_DIR"

echo "==> Building canon.db -> $OUT_DB"
cargo run --release --manifest-path "$ROOT/Cargo.toml" -p sermon -- \
    build-canon --clean "$CLEAN_DIR" --out "$OUT_DB"

echo "==> Copying into the desktop resource dir for packaging"
mkdir -p "$ROOT/data/resources"
cp "$OUT_DB" "$ROOT/data/resources/canon.db"

echo "==> Done. canon.db at $OUT_DB"
ls -lh "$OUT_DB"
