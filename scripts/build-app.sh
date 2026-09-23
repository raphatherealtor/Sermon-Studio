#!/usr/bin/env bash
# Build the Sermon Studio desktop application (AppImage + .deb).
#
# Prerequisites (Debian/Ubuntu):
#   sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
#        libayatana-appindicator3-dev librsvg2-dev patchelf build-essential \
#        pkg-config libssl-dev
#
# Usage:
#   scripts/build-app.sh            # build AppImage + .deb
#   scripts/build-app.sh --debug    # debug build (faster, larger)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PROFILE="release"
if [[ "${1:-}" == "--debug" ]]; then
    PROFILE="debug"
fi

echo "==> Installing frontend dependencies"
npm ci || npm install

echo "==> Building frontend bundle (Next.js static export -> out/)"
npm run build

echo "==> Building Tauri application ($PROFILE)"
if [[ "$PROFILE" == "release" ]]; then
    cargo tauri build --bundles appimage,deb
else
    cargo tauri build --debug --bundles appimage,deb
fi

echo "==> Artifacts:"
find target/release/bundle target/debug/bundle -maxdepth 3 -type f \
    \( -name '*.AppImage' -o -name '*.deb' \) 2>/dev/null || true
