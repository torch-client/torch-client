#!/usr/bin/env python3
"""Strip the comments out of the JavaScript wasm-bindgen generates.

    tools/js_strip.py web-output/default/site/torch-client.js
"""

import sys

VALUE_ENDS = ")]}"
REGEX_PRECEDES = {
    "return",
    "typeof",
    "instanceof",
    "in",
    "of",
    "new",
    "delete",
    "void",
    "throw",
    "case",
    "do",
    "else",
    "yield",
    "await",
}

def opens_regex(text, at):
    i = at - 1
    while i >= 0 and text[i].isspace():
        i -= 1
    if i < 0:
        return True
    c = text[i]
    if c in VALUE_ENDS:
        return False
    if c.isalnum() or c in "_$":
        end = i + 1
        while i >= 0 and (text[i].isalnum() or text[i] in "_$"):
            i -= 1
        return text[i + 1 : end] in REGEX_PRECEDES
    return True

def strip(text):
    out = []
    state = "code"
    at_line_start = True
    at = 0
    end = len(text)
    while at < end:
        c = text[at]
        pair = text[at : at + 2]
        if state == "code":
            if pair == "//":
                state = "line"
                at += 2
                continue
            if pair == "/*":
                state = "block"
                at += 2
                continue
            if c == "\n":
                if not at_line_start:
                    out.append(c)
                    at_line_start = True
                at += 1
                continue
            if at_line_start and c in " \t":
                at += 1
                continue
            at_line_start = False
            if c in "'\"`":
                state = c
                out.append(c)
                at += 1
                continue
            if c == "/" and opens_regex(text, at):
                state = "regex"
                out.append(c)
                at += 1
                continue
            out.append(c)
            at += 1
        elif state in "'\"`":
            if c == "\\":
                out.append(text[at : at + 2])
                at += 2
                continue
            if c == state:
                state = "code"
            out.append(c)
            at += 1
        elif state == "regex":
            if c == "\\":
                out.append(text[at : at + 2])
                at += 2
                continue
            if c == "[":
                close = at
                while close < end and text[close] != "]":
                    close += 2 if text[close] == "\\" else 1
                out.append(text[at : close + 1])
                at = close + 1
                continue
            if c == "/":
                state = "code"
            out.append(c)
            at += 1
        elif state == "line":
            if c == "\n":
                state = "code"
            at += 1
        else:
            if pair == "*/":
                state = "code"
                at += 2
                continue
            at += 1
    if state != "code":
        raise SystemExit(f"ended inside {state}; the scan lost the grammar")
    return "".join(out)

def main(argv):
    if len(argv) < 2:
        raise SystemExit(__doc__)
    for path in argv[1:]:
        source = open(path, encoding="utf-8").read()
        result = strip(source).lstrip("\n")
        open(path, "w", encoding="utf-8").write(result)
        before = len(source.encode("utf-8"))
        after = len(result.encode("utf-8"))
        print(f"    {path}: {before:,} -> {after:,} bytes")

if __name__ == "__main__":
    main(sys.argv)
