#!/usr/bin/env bash
# M12.10 — Pixiekit desktop bundle build script.
#
# Wraps `tauri build` to produce platform-native bundles:
#   - macOS  : .app + .dmg under apps/web/src-tauri/target/release/bundle/{macos,dmg}/
#   - Linux  : .AppImage + .deb under apps/web/src-tauri/target/release/bundle/{appimage,deb}/
#   - Windows: .msi + .exe under apps/web/src-tauri/target/release/bundle/{msi,nsis}/
#
# Usage:
#   ./scripts/build-desktop.sh                # auto-detect targets for current OS
#   ./scripts/build-desktop.sh --bundles dmg  # explicit bundle list (comma-separated)
#
# Requires: Rust toolchain, pnpm, system webview deps (see README.md).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB_DIR="$REPO_ROOT/apps/web"

BUNDLES=""
if [[ "${1:-}" == "--bundles" && -n "${2:-}" ]]; then
  BUNDLES="$2"
fi

cd "$WEB_DIR"

if ! command -v pnpm >/dev/null 2>&1; then
  echo "error: pnpm not found. Install with: npm i -g pnpm" >&2
  exit 5
fi

echo "==> Installing frontend deps"
pnpm install --frozen-lockfile

echo "==> Building Tauri bundle"
if [[ -n "$BUNDLES" ]]; then
  pnpm tauri build --bundles "$BUNDLES"
else
  pnpm tauri build
fi

echo
echo "Done. Artifacts in:"
echo "  $REPO_ROOT/apps/web/src-tauri/target/release/bundle/"
