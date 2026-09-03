#!/usr/bin/env bash
# Сборка установщика десктопа для ТЕКУЩЕЙ ОС.
#   macOS  → src-tauri/target/release/bundle/dmg/VoiceBridge.dmg
#   Windows → src-tauri/target/release/bundle/nsis/*.exe
# Windows-установщик нельзя собрать на macOS — используйте CI
# (см. docs/BUILD.md) или Windows-машину.
set -euo pipefail

cd "$(dirname "$0")/.."

# Ключ подписи артефактов обновления (нужен при bundle.createUpdaterArtifacts).
# Если локальный ключ есть — подставляем путь автоматически.
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && [ -f src-tauri/voicebridge-signing.key ]; then
  export TAURI_SIGNING_PRIVATE_KEY_PATH="$PWD/src-tauri/voicebridge-signing.key"
fi

npm run tauri build
