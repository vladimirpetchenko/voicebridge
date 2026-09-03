#!/usr/bin/env bash
# Сборка Android APK (release).
# Требуется: Flutter, Android SDK (platform 37, build-tools, NDK), JDK 17.
set -euo pipefail

# Java 17 нужен для сборки (homebrew openjdk@17). Если JAVA_HOME не задан —
# подставляем путь homebrew автоматически.
if [ -z "${JAVA_HOME:-}" ] && [ -x /opt/homebrew/opt/openjdk@17/bin/java ]; then
  export JAVA_HOME=/opt/homebrew/opt/openjdk@17
fi

cd "$(dirname "$0")/../mobile"
flutter build apk --release

echo ""
echo "APK: mobile/build/app/outputs/flutter-apk/app-release.apk"
