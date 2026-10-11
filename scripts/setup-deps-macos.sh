#!/usr/bin/env bash
# References: https://rust-lang.org/tools/install/
#             https://v2.tauri.app/start/prerequisites/
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

if [[ "$(uname -s)" != Darwin ]]; then
  echo '此脚本仅适用于 macOS。' >&2
  exit 1
fi
for required in node npm curl xcrun; do
  if ! command -v "$required" >/dev/null 2>&1; then
    echo "缺少命令：$required" >&2
    exit 1
  fi
done
xcrun --show-sdk-path >/dev/null
node -e 'const [major, minor] = process.versions.node.split(".").map(Number); if (major < 22 || (major === 22 && minor < 12)) { console.error("需要 Node.js >=22.12"); process.exit(1); }'

# Preserve the user's proxy and registry configuration.
echo '安装 npm 依赖（已有锁文件时使用 npm ci）…'
if [[ -f package-lock.json ]]; then
  npm ci --no-fund --fetch-timeout=30000 --fetch-retries=1
else
  npm install --no-fund --fetch-timeout=30000 --fetch-retries=1
fi

# Discover an existing rustup installation before attempting a new install.
rust_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
export PATH="$rust_bin:$PATH"
if ! command -v cargo >/dev/null 2>&1; then
  if command -v rustup >/dev/null 2>&1; then
    rustup toolchain install stable --profile minimal
  else
    echo '通过 Rust 官方 rustup 安装 stable（不修改 shell 启动文件）…'
    installer="$(mktemp -t droidswitch-rustup)"
    trap 'rm -f "$installer"' EXIT
    curl --proto '=https' --tlsv1.2 --fail --show-error --location \
      --connect-timeout 15 --max-time 120 https://sh.rustup.rs -o "$installer"
    sh "$installer" -y --profile minimal --default-toolchain stable --no-modify-path
  fi
fi
cargo --version
rustc --version

echo '执行前端及 Rust 构建…'
npm run build
cargo build --manifest-path src-tauri/Cargo.toml
printf '\n依赖安装与构建通过。新终端若找不到 cargo，请执行：\nexport PATH="%s:$PATH"\n' "$rust_bin"
