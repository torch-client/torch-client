#!/usr/bin/env python3

import hashlib
import json
import os
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

import urllib.request

VERSION_MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest.json"
ASSET_BASE = "https://resources.download.minecraft.net"

def fetch_json(url: str) -> dict:
    with urllib.request.urlopen(url) as r:
        return json.loads(r.read())

def download_file(url: str, dest: Path, expected_hash: str) -> tuple[str, bool]:
    if dest.exists():
        data = dest.read_bytes()
        if hashlib.sha1(data).hexdigest() == expected_hash:
            return str(dest), True

    dest.parent.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(url) as r:
        data = r.read()

    actual = hashlib.sha1(data).hexdigest()
    if actual != expected_hash:
        raise ValueError(f"Hash mismatch for {dest}: expected {expected_hash}, got {actual}")

    dest.write_bytes(data)
    return str(dest), False

def main(version_id: str, output_dir: str = "minecraft_assets", workers: int = 16):
    output = Path(output_dir)
    print(f"Fetching version manifest...")
    manifest = fetch_json(VERSION_MANIFEST)

    version_entry = next(
        (v for v in manifest["versions"] if v["id"] == version_id), None
    )
    if not version_entry:
        available = [v["id"] for v in manifest["versions"][:20]]
        print(f"Version '{version_id}' not found. Recent versions: {available}")
        sys.exit(1)

    print(f"Fetching version JSON for {version_id}...")
    version_json = fetch_json(version_entry["url"])

    asset_index_info = version_json["assetIndex"]
    print(f"Fetching asset index '{asset_index_info['id']}'...")
    asset_index = fetch_json(asset_index_info["url"])

    objects = asset_index["objects"]
    print(f"Found {len(objects)} assets. Downloading to '{output}'...")

    tasks = []
    for name, info in objects.items():
        h = info["hash"]
        prefix = h[:2]
        url = f"{ASSET_BASE}/{prefix}/{h}"
        dest = output / "objects" / prefix / h
        tasks.append((url, dest, h, name))

    cached = 0
    downloaded = 0
    errors = []

    with ThreadPoolExecutor(max_workers=workers) as pool:
        futures = {
            pool.submit(download_file, url, dest, h): name
            for url, dest, h, name in tasks
        }
        for i, future in enumerate(as_completed(futures), 1):
            name = futures[future]
            try:
                _, was_cached = future.result()
                if was_cached:
                    cached += 1
                else:
                    downloaded += 1
            except Exception as e:
                errors.append((name, str(e)))
            if i % 500 == 0 or i == len(tasks):
                print(f"  {i}/{len(tasks)} — {downloaded} downloaded, {cached} cached, {len(errors)} errors")

    index_dest = output / "indexes" / f"{asset_index_info['id']}.json"
    index_dest.parent.mkdir(parents=True, exist_ok=True)
    index_dest.write_text(json.dumps(asset_index, indent=2))

    print(f"\nDone. {downloaded} downloaded, {cached} already cached.")
    if errors:
        print(f"{len(errors)} errors:")
        for name, err in errors[:10]:
            print(f"  {name}: {err}")

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python mc_assets.py <version_id> [output_dir] [workers]")
        print("Example: python mc_assets.py 26.1.2")
        sys.exit(1)

    version = sys.argv[1]
    out = sys.argv[2] if len(sys.argv) > 2 else "minecraft_assets"
    workers = int(sys.argv[3]) if len(sys.argv) > 3 else 16
    main(version, out, workers)
