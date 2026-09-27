#!/usr/bin/env python3
"""Refuse to ship a module the page cannot start.

    Uncaught RangeError: failed to grow table
        __wbindgen_init_externref_table
    stopping it growing;
    tools/wasm_verify.py web-output/hardened/site/torch-client_bg.wasm
"""

import sys

from wasm_symbols import sections, uleb

FUNCREF = 0x70
EXTERNREF = 0x6F
ELEMENT = {FUNCREF: "funcref", EXTERNREF: "externref"}

GROWN_TABLE = "__wbindgen_externrefs"

REQUIRED_EXPORTS = ("start_web", "take_web_assets", "asset_cache_key")

def name_at(module, at):
    length, at = uleb(module, at)
    return module[at : at + length].decode("utf-8", "replace"), at + length

def tables(module):
    imported = []
    defined = []
    for kind, _start, body, size, _end in sections(module):
        if kind == 2:
            at = body
            count, at = uleb(module, at)
            for _ in range(count):
                _module, at = name_at(module, at)
                _field, at = name_at(module, at)
                what = module[at]
                at += 1
                if what == 0x00:
                    _index, at = uleb(module, at)
                elif what == 0x01:
                    element = module[at]
                    at += 1
                    flags, at = uleb(module, at)
                    low, at = uleb(module, at)
                    high = None
                    if flags & 1:
                        high, at = uleb(module, at)
                    imported.append((element, low, high))
                elif what == 0x02:
                    flags, at = uleb(module, at)
                    _low, at = uleb(module, at)
                    if flags & 1:
                        _high, at = uleb(module, at)
                elif what == 0x03:
                    at += 1
                    at += 1
                else:
                    raise SystemExit(f"unknown import kind {what}")
        elif kind == 4:
            at = body
            count, at = uleb(module, at)
            for _ in range(count):
                element = module[at]
                at += 1
                flags, at = uleb(module, at)
                low, at = uleb(module, at)
                high = None
                if flags & 1:
                    high, at = uleb(module, at)
                defined.append((element, low, high))
    return imported + defined

def exports(module):
    out = {}
    for kind, _start, body, _size, _end in sections(module):
        if kind != 7:
            continue
        at = body
        count, at = uleb(module, at)
        for _ in range(count):
            label, at = name_at(module, at)
            what = module[at]
            at += 1
            index, at = uleb(module, at)
            out[label] = (what, index)
    return out

def verify(path):
    module = open(path, "rb").read()
    every = tables(module)
    exported = exports(module)
    problems = []

    for missing in (name for name in REQUIRED_EXPORTS if name not in exported):
        problems.append(f"{missing} is not exported")

    if GROWN_TABLE not in exported:
        problems.append(f"{GROWN_TABLE} is not exported; the glue cannot start")
    else:
        what, index = exported[GROWN_TABLE]
        if what != 0x01:
            problems.append(f"{GROWN_TABLE} is exported, but not as a table")
        elif index >= len(every):
            problems.append(f"{GROWN_TABLE} names table {index}, and there are {len(every)}")
        else:
            element, low, high = every[index]
            kind = ELEMENT.get(element, hex(element))
            if element != EXTERNREF:
                problems.append(
                    f"{GROWN_TABLE} names table {index}, which holds {kind}, not externref"
                )
            elif high is not None:
                problems.append(
                    f"{GROWN_TABLE} is table {index}, {kind} {low}..{high}, "
                    f"and the glue grows it past {high}"
                )

    if problems:
        print(f"{path}: this module would not start:", file=sys.stderr)
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        for index, (element, low, high) in enumerate(every):
            print(
                f"  table {index}: {ELEMENT.get(element, hex(element))} "
                f"min={low} max={high}",
                file=sys.stderr,
            )
        raise SystemExit(1)

    what, index = exported[GROWN_TABLE]
    element, low, high = every[index]
    print(f"verify: {GROWN_TABLE} is table {index}, {ELEMENT[element]} min={low}, growable")

def main(argv):
    if len(argv) != 2:
        raise SystemExit(__doc__)
    verify(argv[1])

if __name__ == "__main__":
    main(sys.argv)
