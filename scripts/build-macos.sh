#!/usr/bin/env bash
# Сборка установщика macOS (.dmg).
# Требуется: Rust (rustup), Node 20+, cmake, Xcode CLT + clang.
# Настройки SDKROOT/MACOSX_DEPLOYMENT_TARGET уже заданы в
# src-tauri/.cargo/config.toml — дополнительных env не нужно.
# Первая сборка долгая — компилирует whisper.cpp из исходников.
set -euo pipefail

cd "$(dirname "$0")/.."
npm run tauri build

echo ""
echo "DMG: src-tauri/target/release/bundle/dmg/VoiceBridge.dmg"
