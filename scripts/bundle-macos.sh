#!/usr/bin/env bash
# Builds an unsigned telima.app from the release binary (no signing or notarisation, by decision).
# Usage: scripts/bundle-macos.sh   ->  target/release/bundle/telima.app
set -euo pipefail
cd "$(dirname "$0")/.."
APP_ID=$(sed -n 's/^pub const APP_ID: &str = "\(.*\)";$/\1/p' crates/telima-ui/src/lib.rs)
[ -n "$APP_ID" ] || { echo "APP_ID not found in crates/telima-ui/src/lib.rs" >&2; exit 1; }
VERSION=$(cargo metadata --no-deps --format-version 1 \
  | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "telima"))')
cargo build --release -p telima
APP=target/release/bundle/telima.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/telima "$APP/Contents/MacOS/telima"
cp assets/icon/telima.icns "$APP/Contents/Resources/telima.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>telima</string>
  <key>CFBundleDisplayName</key><string>telima</string>
  <key>CFBundleIdentifier</key><string>${APP_ID}</string>
  <key>CFBundleExecutable</key><string>telima</string>
  <key>CFBundleIconFile</key><string>telima</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
echo "$APP"
