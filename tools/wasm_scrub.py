#!/usr/bin/env python3
"""Drop the toolchain fingerprint from a wasm module.

    Failed to find src_prefix "???" in "????????????????????????????????"
    `file!()` produces at compile time. The paths never enter the module, and
    they stay well-formed, so bevy still finds its `src` component.
    file on disk.
    tools/wasm_scrub.py web-output/hardened/site/torch-client_bg.wasm
"""

import sys

from wasm_symbols import sections, uleb

def scrub(path):
    module = open(path, "rb").read()

    kept = bytearray(module[:8])
    dropped = 0
    for kind, start, body, size, end in sections(module):
        if kind == 0:
            length, at = uleb(module, body)
            if module[at : at + length] == b"producers":
                dropped = size
                continue
        kept += module[start:end]

    if not dropped:
        print("scrub: no producers section, nothing to drop")
        return

    open(path, "wb").write(kept)
    print(f"scrub: producers dropped, {len(module):,} -> {len(kept):,} bytes")

def main(argv):
    if len(argv) != 2:
        raise SystemExit(__doc__)
    scrub(argv[1])

if __name__ == "__main__":
    main(sys.argv)
