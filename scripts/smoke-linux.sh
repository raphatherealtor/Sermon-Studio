#!/usr/bin/env bash
# Sermon Studio — Linux package smoke test.
#
# Verifies the freshly built .deb and AppImage:
#   1. bundled canon.db present + correct SHA-256
#   2. .deb installs and the installed binary launches headless
#   3. AppImage launches headless (extract-and-run, no FUSE)
#   4. real Passage / Strong's / X-Ref lookups against the BUNDLED canon.db
#
# Designed for the pinned ubuntu-22.04 CI image (xvfb, no FUSE assumed).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CANON_SHA="2eebe198ca17f8b5ecf7b9b378f4a2e56a76fb0e53a9d02e2501b3653d3d23b8"
BUNDLE="$ROOT/target/release/bundle"
DEB="$(ls "$BUNDLE"/deb/*.deb | head -n1)"
APPIMAGE="$(ls "$BUNDLE"/appimage/*.AppImage | head -n1)"
EXE_NAME="sermon-studio"

echo "==> Artifacts"
echo "    deb      = $DEB"
echo "    appimage = $APPIMAGE"
ls -lh "$DEB" "$APPIMAGE"

# ── 1. canon.db inside the .deb ──────────────────────────────────────────────
echo "==> Verifying canon.db inside the .deb"
DEB_X="$(mktemp -d)"
dpkg-deb -x "$DEB" "$DEB_X"
DEB_CANON=$(find "$DEB_X" -name canon.db -type f | head -n1)
if [[ -z "$DEB_CANON" ]]; then
  echo "BLOCKER: canon.db not found in .deb payload"; exit 1
fi
DEB_SHA=$(sha256sum "$DEB_CANON" | awk '{print $1}')
echo "    .deb canon.db path = ${DEB_CANON#$DEB_X}"
echo "    .deb canon.db sha  = $DEB_SHA"
[[ "$DEB_SHA" == "$CANON_SHA" ]] || { echo "BLOCKER: .deb canon.db SHA mismatch"; exit 1; }

# ── 2. canon.db inside the AppImage ──────────────────────────────────────────
echo "==> Verifying canon.db inside the AppImage"
APP_X="$(mktemp -d)"
( cd "$APP_X" && "$APPIMAGE" --appimage-extract >/dev/null 2>&1 )
APP_CANON=$(find "$APP_X" -name canon.db -type f | head -n1)
if [[ -z "$APP_CANON" ]]; then
  echo "BLOCKER: canon.db not found in AppImage payload"; exit 1
fi
APP_SHA=$(sha256sum "$APP_CANON" | awk '{print $1}')
echo "    AppImage canon.db path = ${APP_CANON#$APP_X}"
echo "    AppImage canon.db sha  = $APP_SHA"
[[ "$APP_SHA" == "$CANON_SHA" ]] || { echo "BLOCKER: AppImage canon.db SHA mismatch"; exit 1; }

# ── 3. real lookups against the BUNDLED canon.db via the headless CLI ────────
echo "==> Building the headless CLI"
cargo build --release --locked -p sermon

echo "==> Passage lookup (John 3:16)"
"$ROOT/target/release/sermon" verse "John 3:16" --canon "$DEB_CANON" | head -n1

echo "==> Strong's lookup (G25)"
"$ROOT/target/release/sermon" strong G25 --canon "$DEB_CANON" | head -n1

echo "==> Cross-reference lookup (Gen 1:1)"
"$ROOT/target/release/sermon" verse "Gen 1:1" --canon "$DEB_CANON" | tail -n +2 | head -n3

# ── 4. install the .deb and launch headless ──────────────────────────────────
echo "==> Installing the .deb"
sudo dpkg -i "$DEB" || sudo apt-get -f install -y
INSTALLED_BIN=$(command -v "$EXE_NAME" || echo "/usr/bin/$EXE_NAME")
echo "    installed binary = $INSTALLED_BIN"

echo "==> Launching installed binary headless (xvfb)"
xvfb-run -a -s "-screen 0 1280x820x24" "$INSTALLED_BIN" &
PID=$!
sleep 8
if kill -0 "$PID" 2>/dev/null; then
  echo "    installed .deb app launched OK (pid $PID)"
  kill "$PID" 2>/dev/null || true
else
  echo "BLOCKER: installed .deb app exited during launch"; wait "$PID" || true; exit 1
fi

# ── 5. launch the AppImage headless (no FUSE) ────────────────────────────────
echo "==> Launching AppImage headless (extract-and-run, xvfb)"
xvfb-run -a -s "-screen 0 1280x820x24" "$APPIMAGE" --appimage-extract-and-run &
PID2=$!
sleep 8
if kill -0 "$PID2" 2>/dev/null; then
  echo "    AppImage launched OK (pid $PID2)"
  kill "$PID2" 2>/dev/null || true
else
  echo "BLOCKER: AppImage exited during launch"; wait "$PID2" || true; exit 1
fi

echo "==> SMOKE TEST PASSED"
