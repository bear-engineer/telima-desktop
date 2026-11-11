#!/usr/bin/env bash
# Regenerates every derived icon from the one master, assets/icon/telima.png (1024x1024).
# macOS only (sips, iconutil). The app id comes from crates/telima-ui/src/lib.rs.
set -euo pipefail
cd "$(dirname "$0")/.."
APP_ID=$(sed -n 's/^pub const APP_ID: &str = "\(.*\)";$/\1/p' crates/telima-ui/src/lib.rs)
[ -n "$APP_ID" ] || { echo "APP_ID not found in crates/telima-ui/src/lib.rs" >&2; exit 1; }
MASTER=assets/icon/telima.png
rm -rf assets/icon/hicolor
for s in 16 32 48 64 128 256 512; do
  d="assets/icon/hicolor/${s}x${s}/apps"
  mkdir -p "$d"
  sips -z "$s" "$s" "$MASTER" --out "$d/$APP_ID.png" >/dev/null
done
SET=$(mktemp -d)/telima.iconset
mkdir -p "$SET"
for s in 16 32 128 256 512; do
  sips -z "$s" "$s" "$MASTER" --out "$SET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z "$d" "$d" "$MASTER" --out "$SET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$SET" -o assets/icon/telima.icns
rm -rf "$(dirname "$SET")"
echo "icons regenerated for $APP_ID"
