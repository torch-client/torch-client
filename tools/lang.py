#!/usr/bin/env python3
"""Check and annotate the client's own strings (lang/<code>/<area>.json).

    tools/lang.py check
    tools/lang.py sync
"""

import json
import pathlib
import re
import sys
from collections import Counter

ROOT = pathlib.Path.cwd()
LANG = ROOT / "lang"
SRC = ROOT / "src"
ENGLISH = "en_us"
RUSSIAN = "ru_ru"

USE = re.compile(r'(?<![A-Za-z0-9_])(tr|trf|trn)!\(\s*"([^"\\]*)"|Key::of\(\s*"([^"\\]*)"')
COMMENT = re.compile(r"^(\s*)// en:(?: (.*))?$")
KEY = re.compile(r"[A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)*")
PLURAL_FORMS = ("one", "few", "many")
PLACEHOLDER = re.compile(r"%(?:%|\d+\$s|s)")

REGISTRY = SRC / "modules" / "registry.rs"
MODULES_DIR = SRC / "modules"
GENERATED_AREAS = ("modules", "options")
HAND_WRITTEN = ("modules.category.", "modules.mode.")

class Report:
    def __init__(self):
        self.errors = 0
        self.warnings = 0

    def error(self, where, what):
        self.errors += 1
        print(f"error: {where}: {what}")

    def warn(self, where, what):
        self.warnings += 1
        print(f"warning: {where}: {what}")

def read_language(code, report):
    table = {}
    folder = LANG / code
    if not folder.is_dir():
        return table
    for path in sorted(folder.glob("*.json")):
        rel = path.relative_to(ROOT)
        area = path.stem

        def pairs_hook(pairs, rel=rel):
            seen = set()
            for key, _ in pairs:
                if key in seen:
                    report.error(rel, f"`{key}` appears twice")
                seen.add(key)
            return dict(pairs)

        try:
            data = json.loads(path.read_text(encoding="utf-8-sig"), object_pairs_hook=pairs_hook)
        except (OSError, ValueError) as e:
            report.error(rel, f"unreadable: {e}")
            continue
        if not isinstance(data, dict):
            report.error(rel, "not a JSON object")
            continue
        for key, text in data.items():
            if not isinstance(text, str):
                report.error(rel, f"the value of `{key}` is not a string")
                continue
            if any(0xD800 <= ord(c) <= 0xDFFF for c in key + text):
                report.error(rel, f"`{key}` has a \\u escape of an unpaired surrogate")
                continue
            if not KEY.fullmatch(key):
                report.error(rel, f"`{key}` is not dot-separated segments of A-Z, a-z, 0-9 and _")
            elif key.split(".")[0] != area:
                report.error(rel, f"`{key}` does not start with `{area}.`, the file's area")
            if key in table:
                report.error(rel, f"`{key}` is also in {table[key][1]}")
            table[key] = (text, rel)
    return table

ESCAPED = {"\n": "\\n", "\r": "\\r", "\t": "\\t"}
UNESCAPED = {"n": "\n", "r": "\r", "t": "\t", "s": " ", "\\": "\\"}

def comment_text(text):
    lead = len(text) - len(text.lstrip(" "))
    trail = len(text) - len(text.rstrip(" ")) if lead < len(text) else 0
    edge = set(range(lead)) | set(range(len(text) - trail, len(text)))

    def starts_escape(i):
        if i >= len(text):
            return True
        return text[i] in ESCAPED or text[i] in "nrst\\" or i in edge

    out = []
    for i, c in enumerate(text):
        if i in edge:
            out.append("\\s")
        elif c in ESCAPED:
            out.append(ESCAPED[c])
        elif c == "\\" and starts_escape(i + 1):
            out.append("\\\\")
        else:
            out.append(c)
    return "".join(out)

def comment_value(written):
    out = []
    i = 0
    while i < len(written):
        c = written[i]
        if c == "\\" and i + 1 < len(written) and written[i + 1] in UNESCAPED:
            out.append(UNESCAPED[written[i + 1]])
            i += 2
        else:
            out.append(c)
            i += 1
    return "".join(out)

def call_keys(kind, key):
    if kind == "trn":
        return [f"{key}.{form}" for form in PLURAL_FORMS]
    return [key]

def plural_bases(table, report, code):
    bases = {}
    for key, (_, rel) in table.items():
        base, _, form = key.rpartition(".")
        if base and form in PLURAL_FORMS:
            bases.setdefault(base, (set(), rel))[0].add(form)
    for base, (forms, rel) in sorted(bases.items()):
        missing = [f for f in PLURAL_FORMS if f not in forms]
        if missing:
            report.error(rel, f"plural `{base}` has no {', '.join(f'`.{f}`' for f in missing)} in lang/{code}")

WHITE_SPACE = set("\t\n\v\f\r \x85\xa0     　") | {
    chr(c) for c in range(0x2000, 0x200B)
}

def norm(name):
    return "".join(c for c in name if c not in WHITE_SPACE).lower()

def module_names(english, russian, report):
    modules = {}
    for key in english:
        parts = key.split(".")
        if parts[0] != "modules" or parts[-1] != "name" or any(key.startswith(p) for p in HAND_WRITTEN):
            continue
        if len(parts) == 3:
            modules.setdefault(parts[1], {"names": None, "settings": {}})["names"] = key
        elif len(parts) == 4:
            modules.setdefault(parts[1], {"names": None, "settings": {}})["settings"][parts[2]] = key

    def names(key):
        out = [("English", norm(english[key][0]))]
        if key in russian and russian[key][0]:
            out.append(("Russian", norm(russian[key][0])))
        return out

    def clashes(rows, what):
        for label, key in sorted(rows.items()):
            if key not in russian or not russian[key][0]:
                continue
            mine = norm(russian[key][0])
            for other, other_key in sorted(rows.items()):
                if other == label:
                    continue
                for lang, name in names(other_key):
                    if name == mine:
                        report.error(
                            russian[key][1],
                            f"`{key}` ({russian[key][0]!r}) is the {lang} name of {what} `{other}` once "
                            "whitespace and case are dropped, so a command could not tell them apart",
                        )

    clashes({index: m["names"] for index, m in modules.items() if m["names"]}, "module")
    for index, m in sorted(modules.items()):
        clashes(m["settings"], f"`{index}`'s setting")

def placeholders(text):
    return Counter(m for m in PLACEHOLDER.findall(text) if m != "%%")

def scan(path):
    text = path.read_text(encoding="utf-8")
    lines = text.split("\n")
    starts = [0]
    for line in lines:
        starts.append(starts[-1] + len(line) + 1)
    uses = {}
    for m in USE.finditer(text):
        lo, hi = 0, len(lines) - 1
        while lo < hi:
            mid = (lo + hi + 1) // 2
            if starts[mid] <= m.start():
                lo = mid
            else:
                hi = mid - 1
        if lines[lo].lstrip().startswith("//"):
            continue
        if m.group(1):
            uses.setdefault(lo, []).append((m.group(1), m.group(2)))
        else:
            uses.setdefault(lo, []).append(("Key::of", m.group(3)))
    return lines, uses

def comments_above(lines, index):
    found = []
    i = index - 1
    while i >= 0:
        m = COMMENT.match(lines[i])
        if not m:
            break
        found.append((i, m.group(2) or ""))
        i -= 1
    found.reverse()
    return found

def rust_files():
    return sorted(p for p in SRC.rglob("*.rs") if p.is_file())

class SourceError(Exception):
    pass

class Tok:
    __slots__ = ("kind", "text", "line")

    def __init__(self, kind, text, line):
        self.kind = kind
        self.text = text
        self.line = line

    def __repr__(self):
        return f"{self.kind}:{self.text!r}@{self.line}"

def unescape(body, line):
    out = []
    i = 0
    n = len(body)
    while i < n:
        c = body[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        i += 1
        if i >= n:
            raise SourceError(f"line {line}: a string ending in a lone backslash")
        e = body[i]
        i += 1
        simple = {"n": "\n", "r": "\r", "t": "\t", "\\": "\\", "0": "\0", "'": "'", '"': '"'}
        if e in simple:
            out.append(simple[e])
        elif e == "x":
            out.append(chr(int(body[i : i + 2], 16)))
            i += 2
        elif e == "u":
            close = body.index("}", i)
            out.append(chr(int(body[i + 1 : close].replace("_", ""), 16)))
            i = close + 1
        elif e == "\n" or e == "\r":
            while i < n and body[i] in " \t\r\n":
                i += 1
        else:
            raise SourceError(f"line {line}: unknown escape `\\{e}`")
    return "".join(out)

def tokenize(text):
    toks = []
    i = 0
    n = len(text)
    line = 1
    while i < n:
        c = text[i]
        if c == "\n":
            line += 1
            i += 1
        elif c.isspace():
            i += 1
        elif text.startswith("//", i):
            end = text.find("\n", i)
            i = n if end < 0 else end
        elif text.startswith("/*", i):
            depth = 0
            while i < n:
                if text.startswith("/*", i):
                    depth += 1
                    i += 2
                elif text.startswith("*/", i):
                    depth -= 1
                    i += 2
                    if depth == 0:
                        break
                else:
                    if text[i] == "\n":
                        line += 1
                    i += 1
        elif c == '"' or (c == "b" and text.startswith('b"', i)):
            start_line = line
            j = i + (2 if c == "b" else 1)
            while j < n and text[j] != '"':
                if text[j] == "\\":
                    j += 1
                if j < n and text[j] == "\n":
                    line += 1
                j += 1
            if j >= n:
                raise SourceError(f"line {start_line}: a string that never ends")
            body = text[i + (2 if c == "b" else 1) : j]
            toks.append(Tok("str", unescape(body, start_line), start_line))
            i = j + 1
        elif re.match(r'b?r#*"', text[i:i + 70]) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            m = re.match(r'(b?r)(#*)"', text[i:])
            hashes = m.group(2)
            start = i + len(m.group(0))
            end = text.find('"' + hashes, start)
            if end < 0:
                raise SourceError(f"line {line}: a raw string that never ends")
            body = text[start:end]
            toks.append(Tok("str", body, line))
            line += body.count("\n")
            i = end + 1 + len(hashes)
        elif c == "'":
            m = re.match(r"'(?:\\(?:u\{[0-9A-Fa-f_]+\}|x[0-9A-Fa-f]{2}|.)|[^\\'])'", text[i:])
            if m:
                toks.append(Tok("char", m.group(0), line))
                i += len(m.group(0))
            else:
                toks.append(Tok("punct", "'", line))
                i += 1
        elif c.isalpha() or c == "_":
            m = re.match(r"(?:r#)?[A-Za-z_][A-Za-z0-9_]*", text[i:])
            toks.append(Tok("ident", m.group(0), line))
            i += len(m.group(0))
        elif c.isdigit():
            m = re.match(r"[0-9][0-9A-Za-z_]*(?:\.[0-9][0-9A-Za-z_]*)?", text[i:])
            toks.append(Tok("num", m.group(0), line))
            i += len(m.group(0))
        else:
            toks.append(Tok("punct", c, line))
            i += 1
    return toks

class Cursor:
    def __init__(self, toks, where):
        self.toks = toks
        self.at = 0
        self.where = where

    def peek(self, ahead=0):
        i = self.at + ahead
        return self.toks[i] if i < len(self.toks) else None

    def fail(self, what):
        tok = self.peek()
        line = tok.line if tok else "end"
        raise SourceError(f"{self.where}:{line}: {what}, found {tok.text if tok else 'the end'!r}")

    def is_punct(self, text, ahead=0):
        tok = self.peek(ahead)
        return tok is not None and tok.kind == "punct" and tok.text == text

    def is_ident(self, text=None, ahead=0):
        tok = self.peek(ahead)
        return tok is not None and tok.kind == "ident" and (text is None or tok.text == text)

    def punct(self, text):
        if not self.is_punct(text):
            self.fail(f"expected `{text}`")
        self.at += 1

    def ident(self):
        if not self.is_ident():
            self.fail("expected an identifier")
        tok = self.toks[self.at]
        self.at += 1
        if tok.text.startswith("r#"):
            raise SourceError(f"{self.where}:{tok.line}: `{tok.text}` cannot be a key segment")
        return tok.text

    def string(self):
        tok = self.peek()
        if tok is None or tok.kind != "str":
            self.fail("expected a string literal")
        self.at += 1
        return tok.text

    def group(self, open_, close):
        self.punct(open_)
        depth = 0
        start = self.at
        pairs = {"(": ")", "[": "]", "{": "}"}
        if open_ == "<":
            pairs = {"<": ">"}
        while True:
            tok = self.peek()
            if tok is None:
                self.fail(f"no `{close}` for this `{open_}`")
            if tok.kind == "punct":
                if depth == 0 and tok.text == close:
                    inner = self.toks[start : self.at]
                    self.at += 1
                    return inner
                if tok.text in pairs:
                    depth += 1
                elif tok.text in pairs.values():
                    depth -= 1
            self.at += 1

    def skip_attributes(self):
        while self.is_punct("#"):
            self.at += 1
            if self.is_punct("!"):
                self.at += 1
            self.group("[", "]")

def find_invocations(toks, name):
    found = []
    for i in range(len(toks) - 2):
        a, b, c = toks[i], toks[i + 1], toks[i + 2]
        if a.kind == "ident" and a.text == name and b.text == "!" and c.text == "{" and b.kind == c.kind == "punct":
            found.append(i + 2)
    return found

def field_string(body, field, where):
    depth = 0
    for i, tok in enumerate(body):
        if tok.kind == "punct" and tok.text in "([{":
            depth += 1
        elif tok.kind == "punct" and tok.text in ")]}":
            depth -= 1
        elif depth == 0 and tok.kind == "ident" and tok.text == field:
            if i + 2 < len(body) and body[i + 1].text == ":" and body[i + 2].kind == "str":
                return body[i + 2].text
    raise SourceError(f"{where}: no `{field}: \"...\"` in the row's fields")

def read_modules_table():
    rel = REGISTRY.relative_to(ROOT)
    toks = tokenize(REGISTRY.read_text(encoding="utf-8"))
    starts = find_invocations(toks, "modules")
    if len(starts) != 1:
        raise SourceError(f"{rel}: expected one `modules! {{ ... }}` invocation, found {len(starts)}")
    c = Cursor(toks, rel)
    c.at = starts[0]
    body = c.group("{", "}")
    c = Cursor(body, rel)
    modules = []
    while c.peek() is not None:
        module = {"id": c.ident()}
        c.punct("/")
        module["index"] = c.ident()
        c.punct(":")
        c.ident()
        c.punct(",")
        c.ident()
        c.punct(",")
        module["desc"] = c.string()
        rows = Cursor(c.group("{", "}"), rel)
        settings = []
        while rows.peek() is not None:
            row = {"konst": rows.ident()}
            rows.punct(":")
            row["name"] = rows.string()
            rows.punct(",")
            row["tip"] = rows.string()
            rows.punct(",")
            row["kind"] = rows.ident()
            row["enum"] = None
            if rows.is_punct("<"):
                ty = [t.text for t in rows.group("<", ">") if t.kind == "ident"]
                if not ty:
                    rows.fail("expected a type inside `<...>`")
                row["enum"] = ty[-1]
            fields = rows.group("{", "}")
            where = f"{rel}:{fields[0].line if fields else '?'}"
            if row["kind"] == "Button":
                row["text"] = field_string(fields, "text", where)
            elif row["kind"] == "Text":
                row["hint"] = field_string(fields, "hint", where)
            if rows.is_ident("if"):
                rows.at += 1
                while not rows.is_punct(";"):
                    if rows.peek() is None:
                        rows.fail("expected `;` after the `if` condition")
                    if rows.peek().kind == "punct" and rows.peek().text in "([{":
                        open_ = rows.peek().text
                        rows.group(open_, {"(": ")", "[": "]", "{": "}"}[open_])
                    else:
                        rows.at += 1
            rows.punct(";")
            settings.append(row)
        module["settings"] = settings
        modules.append(module)
    return modules

def read_options():
    enums = {}
    for path in sorted(MODULES_DIR.rglob("*.rs")):
        rel = path.relative_to(ROOT)
        toks = tokenize(path.read_text(encoding="utf-8"))
        for start in find_invocations(toks, "options"):
            outer = Cursor(toks, rel)
            outer.at = start
            c = Cursor(outer.group("{", "}"), rel)
            c.skip_attributes()
            if c.is_ident("pub"):
                c.at += 1
                if c.is_punct("("):
                    c.group("(", ")")
            if not c.is_ident("enum"):
                c.fail("expected `enum`")
            c.at += 1
            name = c.ident()
            v = Cursor(c.group("{", "}"), rel)
            variants = []
            while v.peek() is not None:
                v.skip_attributes()
                variant = v.ident()
                v.punct("=")
                variants.append((variant, v.string()))
                if v.is_punct(","):
                    v.at += 1
            if name in enums and enums[name][0] != variants:
                raise SourceError(
                    f"{rel}: `{name}` is also an options enum in {enums[name][1]} with other labels; "
                    "the two would share `options.{name}.*` keys"
                )
            enums.setdefault(name, (variants, rel))
    return enums

def generate():
    modules = read_modules_table()
    enums = read_options()
    out = {"modules": {}, "options": {}}
    counts = Counter()

    def put(area, key, text):
        if any(key.startswith(p) for p in HAND_WRITTEN):
            raise SourceError(f"`{key}` would be generated, but keys under {', '.join(HAND_WRITTEN)} are hand-written")
        if key in out[area]:
            raise SourceError(f"`{key}` is generated twice")
        if not KEY.fullmatch(key):
            raise SourceError(f"`{key}` is not a valid key")
        out[area][key] = text

    for m in modules:
        base = f"modules.{m['index']}"
        put("modules", f"{base}.name", m["id"])
        put("modules", f"{base}.desc", m["desc"])
        counts["modules"] += 1
        for s in m["settings"]:
            row = f"{base}.{s['konst']}"
            put("modules", f"{row}.name", s["name"])
            put("modules", f"{row}.tip", s["tip"])
            counts["settings"] += 1
            if s["kind"] == "Button":
                put("modules", f"{row}.text", s["text"])
                counts["buttons"] += 1
            elif s["kind"] == "Text":
                put("modules", f"{row}.hint", s["hint"])
                counts["texts"] += 1
            elif s["kind"] == "Enum":
                if s["enum"] not in enums:
                    raise SourceError(f"{base}.{s['konst']}: `Enum<{s['enum']}>` is not an options! enum under src/modules/")
                counts["enum settings"] += 1
    for name, (variants, _) in enums.items():
        for variant, label in variants:
            put("options", f"options.{name}.{variant}", label)
            counts["options"] += 1
        counts["enums"] += 1
    return out, counts

def area_path(area):
    return LANG / ENGLISH / f"{area}.json"

def render(table):
    return json.dumps(table, indent=4, ensure_ascii=False) + "\n"

def wanted_files(generated, report):
    wanted = {}
    for area in GENERATED_AREAS:
        path = area_path(area)
        table = {}
        if path.is_file():
            try:
                current = json.loads(path.read_text(encoding="utf-8-sig"))
            except ValueError as e:
                report.error(path.relative_to(ROOT), f"unreadable, so not regenerated: {e}")
                continue
            if not isinstance(current, dict):
                report.error(path.relative_to(ROOT), "not a JSON object, so not regenerated")
                continue
            for key, text in current.items():
                if any(key.startswith(p) for p in HAND_WRITTEN):
                    table[key] = text
        table.update(generated[area])
        wanted[area] = (path, render(table))
    return wanted

def check_generated(report):
    try:
        generated, _ = generate()
    except SourceError as e:
        report.error("tools/lang.py", f"cannot read the module table: {e}")
        return set()
    for area, (path, text) in wanted_files(generated, report).items():
        rel = path.relative_to(ROOT)
        have = path.read_text(encoding="utf-8") if path.is_file() else None
        if have != text:
            report.error(rel, "out of date with the Rust source (run tools/lang.py sync)")
    return {key for area in generated.values() for key in area}

FONT_SOURCE = SRC / "text" / "mod.rs"
BUILTIN_FONT_AREA = "install"
FONT_CHAR = re.compile(r"\(\s*'(\\'|\\\\|[^'\\])'\s*,")

def builtin_font_chars(report):
    try:
        text = FONT_SOURCE.read_text(encoding="utf-8")
    except OSError as e:
        report.error(FONT_SOURCE.relative_to(ROOT), f"unreadable: {e}")
        return None
    chars = set()
    for name in ("BUILTIN", "BUILTIN_ALIASES"):
        m = re.search(r"^const " + name + r": \[[^\n]*\] = \[\n(.*?)^\];", text, re.S | re.M)
        if not m:
            report.error(FONT_SOURCE.relative_to(ROOT), f"no `const {name}` table to read the built-in font from")
            return None
        for row in m.group(1).split("\n"):
            c = FONT_CHAR.search(row)
            if c:
                chars.add(c.group(1)[-1])
    return chars

def check_builtin_font(english, russian, report):
    chars = builtin_font_chars(report)
    if chars is None:
        return
    for table in (english, russian):
        for key, (text, rel) in sorted(table.items()):
            if key.split(".")[0] != BUILTIN_FONT_AREA:
                continue
            bad = sorted({c for c in text if c not in chars})
            if bad:
                report.error(
                    rel,
                    f"`{key}` uses {''.join(bad)!r}, which the built-in font of the install screen "
                    f"cannot draw (src/text/mod.rs `BUILTIN`)",
                )

def check():
    report = Report()
    english = read_language(ENGLISH, report)
    russian = read_language(RUSSIAN, report)
    if not english:
        report.error(f"lang/{ENGLISH}", "no English table")
    check_builtin_font(english, russian, report)

    used = check_generated(report)
    for path in rust_files():
        rel = path.relative_to(ROOT)
        lines, uses = scan(path)
        for index, calls in sorted(uses.items()):
            keys = [key for kind, base in calls for key in call_keys(kind, base)]
            for key in keys:
                used.add(key)
                if key not in english:
                    report.error(f"{rel}:{index + 1}", f"`{key}` is not in lang/{ENGLISH}")
            if any(k not in english for k in keys):
                continue
            have = [text.rstrip() for _, text in comments_above(lines, index)]
            want = [comment_text(english[k][0]) for k in keys]
            if have != want:
                report.error(
                    f"{rel}:{index + 1}",
                    f"the `// en:` comments above say {have!r}, the table says {want!r} (run tools/lang.py sync)",
                )

    for key, (_, rel) in sorted(english.items()):
        if key not in used:
            report.warn(rel, f"`{key}` is not used in src/")
    plural_bases(english, report, ENGLISH)
    plural_bases(russian, report, RUSSIAN)
    module_names(english, russian, report)
    for key, (text, rel) in sorted(russian.items()):
        if key not in english:
            report.error(rel, f"`{key}` is not in lang/{ENGLISH}")
            continue
        if not text and english[key][0]:
            report.error(rel, f"`{key}` is empty, English is {english[key][0]!r}; translate it or remove the row")
            continue
        if placeholders(text) != placeholders(english[key][0]):
            report.error(
                rel,
                f"`{key}` has placeholders {sorted(placeholders(text).elements())}, "
                f"English has {sorted(placeholders(english[key][0]).elements())}",
            )
    missing_areas = Counter()
    for key, (text, rel) in sorted(english.items()):
        if not russian or key in russian or not text:
            continue
        if (LANG / RUSSIAN / rel.name).is_file():
            report.warn(f"lang/{RUSSIAN}", f"`{key}` has no Russian text ({rel})")
        else:
            missing_areas[rel.name] += 1
    for name, count in sorted(missing_areas.items()):
        report.warn(f"lang/{RUSSIAN}/{name}", f"does not exist; {count} key(s) of lang/{ENGLISH}/{name} show in English")

    print(f"{report.errors} error(s), {report.warnings} warning(s)")
    return 1 if report.errors else 0

def sync_generated(report):
    try:
        generated, counts = generate()
    except SourceError as e:
        report.error("tools/lang.py", f"cannot read the module table: {e}")
        return False
    for area, (path, text) in wanted_files(generated, report).items():
        rel = path.relative_to(ROOT)
        if not path.is_file() or path.read_text(encoding="utf-8") != text:
            path.write_text(text, encoding="utf-8")
            print(f"updated {rel}")
    print(
        "generated: {modules} modules, {settings} settings ({buttons} buttons, {texts} text fields, "
        "{enum settings} choices), {enums} options enums with {options} options".format_map(counts)
    )
    return True

def sync():
    report = Report()
    if not sync_generated(report):
        print(f"{report.errors} error(s)")
        return 1
    english = read_language(ENGLISH, report)
    changed_files = 0
    for path in rust_files():
        rel = path.relative_to(ROOT)
        lines, uses = scan(path)
        if not uses:
            continue
        out = list(lines)
        for index in sorted(uses, reverse=True):
            keys = [key for kind, base in uses[index] for key in call_keys(kind, base)]
            missing = [k for k in keys if k not in english]
            if missing:
                for k in missing:
                    report.warn(f"{rel}:{index + 1}", f"`{k}` is not in lang/{ENGLISH}; comments left alone")
                continue
            indent = re.match(r"\s*", lines[index]).group(0)
            want = [f"{indent}// en: {comment_text(english[k][0])}".rstrip() for k in keys]
            above = comments_above(lines, index)
            first = above[0][0] if above else index
            if lines[first:index] != want:
                out[first:index] = want
        if out != lines:
            path.write_text("\n".join(out), encoding="utf-8")
            changed_files += 1
            print(f"updated {rel}")
    print(f"{changed_files} file(s) updated, {report.warnings} warning(s)")
    return 1 if report.errors else 0

def main():
    if len(sys.argv) != 2 or sys.argv[1] not in ("check", "sync"):
        print(__doc__.strip(), file=sys.stderr)
        return 2
    if not (LANG.is_dir() and SRC.is_dir()):
        print("run from the repository root (lang/ and src/ not found)", file=sys.stderr)
        return 2
    return check() if sys.argv[1] == "check" else sync()

if __name__ == "__main__":
    sys.exit(main())
