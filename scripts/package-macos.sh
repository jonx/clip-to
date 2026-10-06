#!/bin/bash
# Build, sign, notarize and staple macOS distributions. Requires local Keychain credentials.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${APPLE_DEV_ID:?Set APPLE_DEV_ID to your Developer ID Application identity}"
: "${APPLE_NOTARY_PROFILE:?Set APPLE_NOTARY_PROFILE to a notarytool Keychain profile}"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
out="$PWD/target/distribution/v$version"
stage="$out/stage"
mkdir -p "$stage" "$out/arm64" "$out/x86_64"
export MACOSX_DEPLOYMENT_TARGET=11.0
for arch in aarch64 x86_64; do
    cargo build --release --locked --target "$arch-apple-darwin"
done
lipo -create target/aarch64-apple-darwin/release/ct target/x86_64-apple-darwin/release/ct -output "$stage/ct"
app="$stage/ClipTo.app"
mkdir -p "$app/Contents/MacOS"
cp "$stage/ct" "$app/Contents/MacOS/ClipTo"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>me.jkn.clipto</string>
<key>CFBundleName</key><string>ClipTo</string>
<key>CFBundleExecutable</key><string>ClipTo</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>LSMinimumSystemVersion</key><string>11.0</string>
<key>LSUIElement</key><true/>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
EOF
cp README.md LICENSE "$stage/"
ln -sfn /Applications "$stage/Applications"
xattr -cr "$app"
codesign --force --options runtime --timestamp --sign "$APPLE_DEV_ID" "$app"
codesign --force --options runtime --timestamp --identifier me.jkn.clipto.cli --sign "$APPLE_DEV_ID" "$stage/ct"

# Sign architecture-specific CLI archives too. Submit all Mach-O variants once.
for pair in 'aarch64 arm64' 'x86_64 x86_64'; do
    read -r rust_arch asset_arch <<< "$pair"
    cp "target/$rust_arch-apple-darwin/release/ct" "$out/$asset_arch/ct"
    cp README.md LICENSE "$out/$asset_arch/"
    codesign --force --options runtime --timestamp --identifier me.jkn.clipto.cli --sign "$APPLE_DEV_ID" "$out/$asset_arch/ct"
done
mkdir -p "$out/submission"
ditto "$app" "$out/submission/ClipTo.app"
cp "$stage/ct" "$out/submission/ct-universal"
cp "$out/arm64/ct" "$out/submission/ct-arm64"
cp "$out/x86_64/ct" "$out/submission/ct-x86_64"
ditto -c -k --keepParent "$out/submission" "$out/notarization.zip"
xcrun notarytool submit "$out/notarization.zip" --keychain-profile "$APPLE_NOTARY_PROFILE" --wait --output-format json > "$out/notarization.json"
python3 -c 'import json,sys; r=json.load(open(sys.argv[1])); print(r); sys.exit(0 if r.get("status")=="Accepted" else 1)' "$out/notarization.json"
xcrun stapler staple "$app"
xcrun stapler validate "$app"
codesign --verify --deep --strict --verbose=2 "$app"
spctl --assess --type execute --verbose=2 "$app"

dmg="$out/ClipTo-v$version-macos-universal.dmg"
hdiutil create -ov -volname "ClipTo $version" -srcfolder "$stage" -format UDZO "$dmg"
codesign --force --timestamp --sign "$APPLE_DEV_ID" "$dmg"
xcrun notarytool submit "$dmg" --keychain-profile "$APPLE_NOTARY_PROFILE" --wait --output-format json > "$out/dmg-notarization.json"
python3 -c 'import json,sys; r=json.load(open(sys.argv[1])); print(r); sys.exit(0 if r.get("status")=="Accepted" else 1)' "$out/dmg-notarization.json"
xcrun stapler staple "$dmg"
xcrun stapler validate "$dmg"
spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg"
for arch in arm64 x86_64; do
    codesign --verify --strict --verbose=2 "$out/$arch/ct"
    tar -C "$out/$arch" -czf "$out/ct-v$version-macos-$arch.tar.gz" ct README.md LICENSE
done
echo "Signed and notarized assets: $out"
