#!/usr/bin/env python3
"""Keep a stripped wasm module debuggable.

    tools/wasm_symbols.py dump   dist/torch-client_bg.wasm dist-symbols.txt
    tools/wasm_symbols.py strip  dist/torch-client_bg.wasm
    tools/wasm_symbols.py resolve dist-symbols.txt 112876 98304
"""

import sys

def uleb(data, at):
    result = 0
    shift = 0
    while True:
        byte = data[at]
        at += 1
        result |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return result, at
        shift += 7

def sections(data):
    if data[:4] != b"\0asm":
        raise SystemExit("not a wasm module")
    at = 8
    while at < len(data):
        start = at
        kind = data[at]
        at += 1
        size, at = uleb(data, at)
        body = at
        at = body + size
        yield kind, start, body, size, at

def data_ranges(module):
    for kind, _start, body, size, _end in sections(module):
        if kind != 11:
            continue
        at = body
        count, at = uleb(module, at)
        for _ in range(count):
            flags, at = uleb(module, at)
            if flags in (0, 2):
                if flags == 2:
                    _memory, at = uleb(module, at)
                while module[at] != 0x0B:
                    at += 1
                at += 1
            length, at = uleb(module, at)
            yield at, at + length
            at += length
        if at != body + size:
            raise SystemExit("the data section did not parse; refusing to edit it")

def name_section(data):
    for kind, _start, body, size, _end in sections(data):
        if kind != 0:
            continue
        length, at = uleb(data, body)
        if data[at : at + length] == b"name":
            return data[at + length : body + size]
    return None

def function_names(payload):
    at = 0
    while at < len(payload):
        sub = payload[at]
        at += 1
        size, at = uleb(payload, at)
        end = at + size
        if sub != 1:
            at = end
            continue
        count, at = uleb(payload, at)
        for _ in range(count):
            index, at = uleb(payload, at)
            length, at = uleb(payload, at)
            yield index, payload[at : at + length].decode("utf-8", "replace")
            at += length
        return

def dump(module, out):
    data = open(module, "rb").read()
    payload = name_section(data)
    if payload is None:
        raise SystemExit(f"{module} has no name section; nothing to dump")
    written = 0
    with open(out, "w", encoding="utf-8") as handle:
        for index, name in function_names(payload):
            handle.write(f"{index}\t{name}\n")
            written += 1
    print(f"{out}: {written} names")

def strip(module):
    data = open(module, "rb").read()
    kept = bytearray(data[:8])
    removed = 0
    for kind, start, body, size, end in sections(data):
        if kind == 0:
            length, at = uleb(data, body)
            if data[at : at + length] == b"name":
                removed = size
                continue
        kept += data[start:end]
    if not removed:
        print(f"{module}: no name section, left alone")
        return
    open(module, "wb").write(kept)
    print(f"{module}: {len(data):,} -> {len(kept):,} bytes ({removed:,} of names)")

def resolve(table, indices):
    names = {}
    with open(table, encoding="utf-8") as handle:
        for line in handle:
            index, _, name = line.rstrip("\n").partition("\t")
            names[index] = name
    for index in indices:
        print(f"{index}\t{names.get(index, '<not in the table>')}")

def main(argv):
    if len(argv) < 3:
        raise SystemExit(__doc__)
    command = argv[1]
    if command == "dump" and len(argv) == 4:
        dump(argv[2], argv[3])
    elif command == "strip" and len(argv) == 3:
        strip(argv[2])
    elif command == "resolve" and len(argv) >= 4:
        resolve(argv[2], argv[3:])
    else:
        raise SystemExit(__doc__)

if __name__ == "__main__":
    main(sys.argv)
