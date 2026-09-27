#!/usr/bin/env python3
"""Repair what the optimiser breaks on this module.

    tools/wasm_repair.py web-output/hardened/site/torch-client_bg.wasm
"""

import sys

from wasm_symbols import sections, uleb
from wasm_verify import EXTERNREF, GROWN_TABLE, name_at, tables

TABLE_EXPORT = 0x01

def leb(value):
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)

def export_entries(module, body):
    at = body
    count, at = uleb(module, at)
    entries = []
    for _ in range(count):
        label, at = name_at(module, at)
        kind = module[at]
        at += 1
        index, at = uleb(module, at)
        entries.append((label, kind, index))
    return entries

def repair(path):
    module = open(path, "rb").read()

    externrefs = [i for i, (element, _, _) in enumerate(tables(module)) if element == EXTERNREF]
    if len(externrefs) != 1:
        raise SystemExit(
            f"{path}: expected exactly one externref table, found {len(externrefs)}; "
            "not guessing which one the glue means"
        )
    correct = externrefs[0]

    rebuilt = bytearray(module[:8])
    changed = None
    for kind, start, body, size, end in sections(module):
        if kind != 7:
            rebuilt += module[start:end]
            continue

        entries = export_entries(module, body)
        payload = bytearray(leb(len(entries)))
        for label, what, index in entries:
            if label == GROWN_TABLE and what == TABLE_EXPORT and index != correct:
                changed = (index, correct)
                index = correct
            encoded = label.encode("utf-8")
            payload += leb(len(encoded)) + encoded + bytes([what]) + leb(index)
        rebuilt += bytes([kind]) + leb(len(payload)) + payload

    if changed is None:
        print(f"repair: nothing to repair, {GROWN_TABLE} already names table {correct}")
        return

    was, now = changed
    open(path, "wb").write(rebuilt)
    print(f"repair: {GROWN_TABLE} pointed at table {was}, now table {now} (the externref one)")

def main(argv):
    if len(argv) != 2:
        raise SystemExit(__doc__)
    repair(argv[1])

if __name__ == "__main__":
    main(sys.argv)
