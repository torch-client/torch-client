#!/usr/bin/env python3
"""Take the comments out of the shaders a wasm module carries.

    tools/wgsl_strip.py web-output/default/site/torch-client_bg.wasm [dir ...]
"""

import os
import sys

from wasm_symbols import data_ranges

FILL = b" "

def comment_spans(source):
    spans = []
    at = 0
    end = len(source)
    while at < end:
        if source[at : at + 2] == b"//":
            stop = source.find(b"\n", at)
            stop = end if stop < 0 else stop
            spans.append((at, stop))
            at = stop
        elif source[at : at + 2] == b"/*":
            depth = 1
            stop = at + 2
            while stop < end and depth:
                if source[stop : stop + 2] == b"/*":
                    depth += 1
                    stop += 2
                elif source[stop : stop + 2] == b"*/":
                    depth -= 1
                    stop += 2
                else:
                    stop += 1
            spans.append((at, stop))
            at = stop
        else:
            at += 1
    return spans

def shader_files(roots):
    for root in roots:
        for where, _dirs, names in os.walk(root):
            for name in names:
                if name.endswith(".wgsl"):
                    yield os.path.join(where, name)

def default_roots():
    home = os.environ.get("CARGO_HOME") or os.path.expanduser("~/.cargo")
    roots = ["src"]
    roots += [d for d in (f"{home}/registry/src", f"{home}/git/checkouts") if os.path.isdir(d)]
    return roots

def strip(path, roots):
    module = bytearray(open(path, "rb").read())
    segments = list(data_ranges(module))

    found = 0
    blanked = 0
    for shader in sorted(set(shader_files(roots))):
        source = open(shader, "rb").read()
        if not source:
            continue
        spans = comment_spans(source)
        if not spans:
            continue
        hit = False
        for start, end in segments:
            at = module.find(source, start, end)
            while at >= 0:
                hit = True
                for begin, stop in spans:
                    module[at + begin : at + stop] = FILL * (stop - begin)
                    blanked += stop - begin
                at = module.find(source, at + len(source), end)
        found += hit

    open(path, "wb").write(module)
    print(f"shaders: {found} found, {blanked:,} bytes of comment blanked")

def main(argv):
    if len(argv) < 2:
        raise SystemExit(__doc__)
    strip(argv[1], argv[2:] or default_roots())

if __name__ == "__main__":
    main(sys.argv)
