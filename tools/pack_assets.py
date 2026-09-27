#!/usr/bin/env python3
"""Pack the bundled assets into the single archive the web build fetches.

    tools/pack_assets.py
    tools/pack_assets.py --out somewhere.bin
    tools/pack_assets.py --root path/to/assets/minecraft
    tools/pack_assets.py --data path/to/data/minecraft
    magic       8 bytes   b"MCASSET2"
    count       u32 LE    number of entries
    per entry:  u32 LE    path length, then that many UTF-8 bytes
                u32 LE    blob length
    blobs       concatenated, in table order
"""

import argparse
import os
import struct
import sys
from pathlib import Path

MAGIC = b"MCASSET2"

KEEP_LIST = Path(__file__).resolve().parent / "asset_keep_list.txt"

def keep_rules() -> list[str]:
    lines = (line.strip() for line in KEEP_LIST.read_text().splitlines())
    return [line for line in lines if line and not line.startswith("#")]

def kept(key: str, rules: list[str]) -> bool:
    return any(key.startswith(r) if r.endswith("/") else key == r for r in rules)

ASSET_KEY_PREFIX = "assets/minecraft/"
DATA_KEY_PREFIX = "data/minecraft/"

def collect(root: Path, prefix: str = "") -> list[tuple[str, Path]]:
    rules = keep_rules()
    out = []
    for dirpath, dirnames, filenames in os.walk(root):
        for name in filenames:
            path = Path(dirpath) / name
            key = prefix + path.relative_to(root).as_posix()
            if kept(key, rules):
                out.append((key, path))
    out.sort()
    return out

PACK_ICON_KEY = "assets/minecraft/textures/gui/pack.png"

def collect_pack_icon(root: Path) -> list[tuple[str, Path]]:
    icon = root.parent.parent / "pack.png"
    return [(PACK_ICON_KEY, icon)] if icon.is_file() else []

def pack(entries: list[tuple[str, Path]]) -> bytes:
    table = bytearray()
    blobs = bytearray()
    table += MAGIC
    table += struct.pack("<I", len(entries))
    for key, path in entries:
        data = path.read_bytes()
        encoded = key.encode("utf-8")
        table += struct.pack("<I", len(encoded))
        table += encoded
        table += struct.pack("<I", len(data))
        blobs += data
    return bytes(table + blobs)

def main() -> int:
    here = Path(__file__).resolve().parent.parent
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--root", type=Path,
                    default=here / "reference/minecraft-26.1.1/assets/minecraft",
                    help="assets/minecraft directory to pack")
    ap.add_argument("--data", type=Path,
                    default=here / "reference/minecraft-26.1.1/data/minecraft",
                    help="data/minecraft (vanilla datapack) directory to pack")
    ap.add_argument("--out", type=Path, default=here / "dist/assets.bin",
                    help="archive to write")
    args = ap.parse_args()

    if not args.root.is_dir():
        print(f"no such assets root: {args.root}", file=sys.stderr)
        return 1

    entries = collect(args.root, ASSET_KEY_PREFIX)
    if args.data.is_dir():
        entries += collect(args.data, DATA_KEY_PREFIX)
        entries.sort()
    else:
        print(f"no datapack at {args.data}; packing assets only", file=sys.stderr)
    if not any(key == PACK_ICON_KEY for key, _ in entries):
        entries += collect_pack_icon(args.root)
        entries.sort()
    if not entries:
        print(f"nothing to pack under {args.root}", file=sys.stderr)
        return 1

    packed = pack(entries)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(packed)

    raw = sum(p.stat().st_size for _, p in entries)
    print(f"{len(entries)} files, {raw / 1e6:.1f} MB -> {args.out} "
          f"({len(packed) / 1e6:.1f} MB)")
    print("serve it with gzip or brotli; the JSON compresses about ten to one")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
