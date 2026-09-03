#!/usr/bin/env bash
# Сборка установщика десктопа для ТЕКУЩЕЙ ОС.
#   macOS  → src-tauri/target/release/bundle/dmg/VoiceBridge.dmg
#   Windows → src-tauri/target/release/bundle/nsis/*.exe
# Windows-установщик нельзя собрать на macOS — используйте CI
# (см. docs/BUILD.md) или Windows-машину.
set -euo pipefail

cd "$(dirname "$0")/.."
npm run tauri build
