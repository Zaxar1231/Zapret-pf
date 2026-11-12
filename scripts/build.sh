#!/usr/bin/env bash
# Кросс-сборка ZapretPF.exe и установщика из Linux.
# Нужны: rustup target x86_64-pc-windows-gnu, gcc-mingw-w64-x86-64, nsis, git.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
TARGET=x86_64-pc-windows-gnu

echo "==> Сборка ZapretPF.exe v$VERSION"
cargo build --release --target "$TARGET"

mkdir -p dist
if [ ! -f dist/zapret/bin/winws.exe ]; then
  echo "==> Загрузка zapret-discord-youtube"
  rm -rf dist/zapret
  git clone --depth 1 https://github.com/Flowseal/zapret-discord-youtube dist/zapret
  rm -rf dist/zapret/.git dist/zapret/.github
fi

cp "target/$TARGET/release/ZapretPF.exe" dist/ZapretPF.exe

echo "==> Сборка установщика"
makensis -V2 -DVERSION="$VERSION" \
  -DAPP_EXE="../dist/ZapretPF.exe" \
  -DZAPRET_DIR="../dist/zapret" \
  -DOUT_FILE="../dist/ZapretPF-Setup-$VERSION.exe" \
  installer/installer.nsi

echo "==> Портативный архив"
(cd dist && rm -f "ZapretPF-$VERSION-portable.zip" && mkdir -p portable/ZapretPF && \
  cp ZapretPF.exe portable/ZapretPF/ && cp -r zapret portable/ZapretPF/ && \
  (cd portable && zip -qr "../ZapretPF-$VERSION-portable.zip" ZapretPF) && rm -rf portable)

ls -la dist
