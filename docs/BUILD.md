# VoiceBridge — сборка и запуск

Все команды и скрипты для сборки/запуска проекта. Чтобы не искать заново.

## Требования

- **Rust** (rustup, stable) — проверено на 1.98.
- **Node.js** 20+ и npm.
- **cmake** — для `whisper.cpp` (первая сборка десктопа долгая).
- **macOS**: Xcode CLT (`xcode-select --install`) + clang. Apple Silicon желателен
  (Metal-ускорение whisper).
- **Windows**: MSVC Build Tools (workload C++) и LLVM (`libclang.dll` для bindgen
  whisper-rs; `LIBCLANG_PATH` на `<LLVM>\bin`).
- **Android**: Flutter (stable, 3.38.7), JDK 17, Android SDK (platform 37,
  build-tools, NDK — Gradle доставит сам по лицензиям).

## Запуск в dev-режиме

```bash
npm install
npm run tauri dev        # десктоп (vite + cargo run)

cd mobile && flutter run # мобилка (нужен эмулятор/устройство)
```

## Сборка установщиков

### Десктоп (текущая ОС)

```bash
npm run tauri build               # или: scripts/build-desktop.sh
```

Результат:
- macOS → `src-tauri/target/release/bundle/dmg/VoiceBridge.dmg`
- Windows → `src-tauri/target/release/bundle/nsis/*.exe`

> ⚠️ Windows-установщик **нельзя** собрать на macOS (нет MSVC/NSIS для
> кросс-компиляции). Используйте CI (см. ниже) или Windows-машину.

### Android APK

```bash
scripts/build-android.sh          # или: cd mobile && flutter build apk --release
```

Результат: `mobile/build/app/outputs/flutter-apk/app-release.apk`

Примечание: для `flutter_secure_storage` в `mobile/android/app/build.gradle.kts`
зафиксирован `compileSdk = 37`; нужен установленный `platforms/android-37`
(при отсутствии — поставить через SDK Manager, либо `ln -s android-37.0 android-37`
в папке платформ).

### Windows + macOS + Android через CI (рекомендуется для релизов)

Workflow `.github/workflows/build.yml` собирает `.exe`, `.dmg` и APK в облаке.
Запуск:

```bash
# вручную (нужен gh + авторизация):
gh workflow run build.yml

# либо по тегу:
git tag v0.1.4 && git push --tags
```

Альтернатива без CLI: GitHub → вкладка **Actions** → **Release** → **Run workflow**.
Артефакты появляются во вкладке Actions; по тегу `v*` автоматически прикрепляются
к GitHub Release.

Имена релизных артефактов (версия из тега):
- Windows → `VoiceBridge_<version>_x64-setup.exe`
- macOS → `VoiceBridge_<version>_aarch64.dmg`
- Android → `voicebridge-<version>.apk` (CI переименовывает `app-release.apk`)

## Полезные команды (фронтенд / бэкенд отдельно)

```bash
npm run build            # tsc + vite build (фронтенд)
npm run dev              # только vite dev-сервер

# в src-tauri/:
cargo check              # быстрая проверка
cargo build              # полная сборка
```

## Иконка

```bash
python3 scripts/gen_icon.py && npx tauri icon
```

## Проверки

```bash
npx tsc --noEmit         # типы фронтенда
cd mobile && flutter analyze   # статанализ мобилки
cd src-tauri && cargo check    # проверка Rust
```
