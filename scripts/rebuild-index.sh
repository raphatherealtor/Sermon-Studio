#!/usr/bin/env bash
# Rebuild the derived pastor index (pastor.db) from the plain-text vault.
#
# This is the "delete the cache, lose nothing" guarantee: pastor.db is a pure
# derived artifact. Deleting it and rebuilding from disk takes well under 3
# seconds for a normal-sized archive and loses zero data.
#
# Usage:
#   scripts/rebuild-index.sh [VAULT_DIR] [PASTOR_DB]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VAULT_DIR="${1:-$HOME/Sermons}"
PASTOR_DB="${2:-$HOME/.local/share/sermon-studio/pastor.db}"

echo "==> Rebuilding pastor.db from $VAULT_DIR"
time cargo run --release --manifest-path "$ROOT/Cargo.toml" -p sermon -- \
    index --vault "$VAULT_DIR" --db "$PASTOR_DB"

echo "==> Done. pastor.db at $PASTOR_DB"
