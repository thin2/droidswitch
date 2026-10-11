#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
npm run tauri build -- --bundles app
bundle_root="$PWD/src-tauri/target/release/bundle"
package_stage="$(mktemp -d "${TMPDIR:-/tmp}/droid-switch-package.XXXXXX")"
trap 'rm -rf "$package_stage"' EXIT
/usr/bin/ditto "$bundle_root/macos/Droid Switch.app" "$package_stage/Droid Switch.app"
ln -s /Applications "$package_stage/Applications"
mkdir -p "$bundle_root/dmg"
package_arch="$(uname -m)"
hdiutil create -volname 'Droid Switch' -srcfolder "$package_stage" -ov -format UDZO "$bundle_root/dmg/Droid_Switch_0.1.0_${package_arch}.dmg"
codesign --verify --deep --strict "$bundle_root/macos/Droid Switch.app"
