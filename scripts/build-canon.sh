#!/usr/bin/env bash
# Build the static canon vault (canon.db) from raw public-domain sources.
#
# The canon vault is READ-ONLY at runtime and contains:
#   * KJV Bible text (31,102 verses)
#   * Strong's Greek/Hebrew lexicon (14,197 entries)
#   * Per-word Strong's tagging (verse_words)
#   * TSK / OpenBible cross-references (256,648 links)
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

echo "==> Normalizing raw sources -> $CLEAN_DIR"
python3 "$ROOT/tools/etl.py" --raw "$RAW_DIR" --out "$CLEAN_DIR"

echo "==> Building canon.db -> $OUT_DB"
cargo run --release --manifest-path "$ROOT/Cargo.toml" -p sermon -- \
    build-canon --clean "$CLEAN_DIR" --out "$OUT_DB"

echo "==> Done. canon.db at $OUT_DB"
ls -lh "$OUT_DB"
