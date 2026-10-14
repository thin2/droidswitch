#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
# Avoid embedding the local builder's home directory in Rust diagnostics.
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--remap-path-prefix=$HOME=/build"
python3 scripts/prepare-codex-proxy.py
npm run tauri build -- --bundles app
bundle_root="$PWD/src-tauri/target/release/bundle"
package_stage="$(mktemp -d "${TMPDIR:-/tmp}/droid-switch-package.XXXXXX")"
trap 'rm -rf "$package_stage"' EXIT
/usr/bin/ditto "$bundle_root/macos/Droid Switch.app" "$package_stage/Droid Switch.app"
ln -s /Applications "$package_stage/Applications"
mkdir -p "$bundle_root/dmg"
package_arch="$(uname -m)"
package_version="$(node -p 'JSON.parse(require("fs").readFileSync("src-tauri/tauri.conf.json", "utf8")).version')"
hdiutil create -volname 'Droid Switch' -srcfolder "$package_stage" -ov -format UDZO "$bundle_root/dmg/Droid_Switch_${package_version}_${package_arch}.dmg"
codesign --verify --deep --strict "$bundle_root/macos/Droid Switch.app"
