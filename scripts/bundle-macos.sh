#!/usr/bin/env bash
# Builds a release binary and wraps it in table-plus-rs.app (unsigned).
#   scripts/bundle-macos.sh            -> target/release/bundle/table-plus-rs.app
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release -p table-plus-rs

APP=target/release/bundle/table-plus-rs.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/table-plus-rs "$APP/Contents/MacOS/table-plus-rs"
cp crates/app/assets/icon.icns "$APP/Contents/Resources/icon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>table-plus-rs</string>
  <key>CFBundleDisplayName</key><string>table-plus-rs</string>
  <key>CFBundleIdentifier</key><string>dev.zeeshan.table-plus-rs</string>
  <key>CFBundleExecutable</key><string>table-plus-rs</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

echo "Built $APP"
