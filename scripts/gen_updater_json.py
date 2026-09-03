#!/usr/bin/env python3
"""Генерирует latest.json — манифест автообновления Tauri (static JSON).

Используется в CI после `npm run tauri build` (при включённом
`bundle.createUpdaterArtifacts`). Читает установщик NSIS (`.exe`) и его
подпись (`.exe.sig`) из каталога bundle/nsis и формирует запись
`windows-x86_64`.

Аргументы:
  --bundle-dir PATH   каталог bundle/nsis (обязательно)
  --repo owner/name   репозиторий GitHub (для URL)
  --tag vX.Y.Z        тег релиза (для URL); версия = тег без ведущего 'v'
  --out PATH          куда записать latest.json
"""

import argparse
import datetime
import glob
import json
import os


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--bundle-dir", required=True)
    p.add_argument("--repo", required=True)
    p.add_argument("--tag", required=True)
    p.add_argument("--out", required=True)
    args = p.parse_args()

    ns = args.bundle_dir
    exes = sorted(glob.glob(os.path.join(ns, "*.exe")))
    if not exes:
        raise SystemExit("не найден установщик .exe в " + ns)

    exe = exes[0]
    sig_path = exe + ".sig"
    if not os.path.exists(sig_path):
        raise SystemExit("нет подписи обновления: " + sig_path)

    with open(sig_path, "r", encoding="utf-8") as f:
        signature = f.read().strip()

    name = os.path.basename(exe)
    url = f"https://github.com/{args.repo}/releases/download/{args.tag}/{name}"
    version = args.tag[1:] if args.tag.startswith("v") else args.tag

    manifest = {
        "version": version,
        "notes": f"VoiceBridge {version}",
        "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime(
            "%Y-%m-%dT%H:%M:%SZ"
        ),
        "platforms": {
            "windows-x86_64": {
                "signature": signature,
                "url": url,
            }
        },
    }

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)

    print(f"latest.json -> {args.out}")
    print(f"installer: {name}")
    print(f"url: {url}")


if __name__ == "__main__":
    main()
