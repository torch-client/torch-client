#!/usr/bin/env python3
"""Strip `web/index.html` down to what a browser needs.

    tools/minify_page.py web/index.html dist/index.html
"""

import re
import sys

INSTALLABLE = (
    '<link rel="manifest" href="manifest.webmanifest">\n'
    '<meta name="theme-color" content="#000000">'
)

CHROMEBOOK = (
    '<script>if ("serviceWorker" in navigator) navigator.serviceWorker.register("./sw.js");</script>'
)

APPLE_HEAD = (
    '<meta name="apple-mobile-web-app-capable" content="yes">\n'
    '<meta name="mobile-web-app-capable" content="yes">\n'
    '<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">\n'
    '<meta name="apple-mobile-web-app-title" content="Torch">\n'
    '<link rel="apple-touch-icon" href="icon-192.png">'
)

APPLE_TAIL = """<style>
html, body { height: 100dvh; }
body {
box-sizing: border-box;
padding: env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left);
}
#applehint {
position: fixed; z-index: 1; left: 50%; transform: translateX(-50%);
top: calc(env(safe-area-inset-top) + 8px); width: max-content; max-width: 90vw;
box-sizing: border-box; padding: 8px 12px;
font: 12px ui-monospace, monospace; color: #fff; text-align: center;
background: rgba(0, 0, 0, 0.85); border: 1px solid #555; border-radius: 3px;
}
</style>
<script>
(() => {
if (navigator.standalone !== false || document.fullscreenEnabled) return;
const KEY = "apple-hint-dismissed";
try { if (localStorage.getItem(KEY)) return; } catch {}
const hint = document.createElement("div");
hint.id = "applehint";
hint.textContent = "For full screen: tap Share, then Add to Home Screen. Tap to dismiss.";
hint.addEventListener("click", () => {
hint.remove();
try { localStorage.setItem(KEY, "1"); } catch {}
});
document.body.append(hint);
})();
</script>
"""

def strip_block_comments(text, opener="/*", closer="*/"):
    out = []
    at = 0
    while True:
        start = text.find(opener, at)
        if start < 0:
            out.append(text[at:])
            return "".join(out)
        end = text.find(closer, start + len(opener))
        if end < 0:
            out.append(text[at:])
            return "".join(out)
        out.append(text[at:start])
        at = end + len(closer)

def strip_js_comments(text):
    out = []
    state = "code"
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
            if c in "'\"`":
                state = c
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
        elif state == "line":
            if c == "\n":
                state = "code"
                out.append(c)
            at += 1
        else:
            if pair == "*/":
                state = "code"
                at += 2
                continue
            at += 1
    return "".join(out)

def tidy(text, protect_backquotes):
    lines = []
    inside = False
    for line in text.split("\n"):
        if inside:
            lines.append(line)
        else:
            stripped = line.strip()
            if stripped:
                lines.append(stripped)
        if protect_backquotes:
            inside ^= bool(len(re.findall(r"(?<!\\)`", line)) % 2)
    return "\n".join(lines)

def minify(text):
    out = []
    at = 0
    region = re.compile(r"<(script|style)\b[^>]*>", re.IGNORECASE)
    while True:
        opening = region.search(text, at)
        if not opening:
            out.append(tidy(re.sub(r"(?s)<!--.*?-->", "", text[at:]), False))
            return "\n".join(out)
        markup = re.sub(r"(?s)<!--.*?-->", "", text[at : opening.end()])
        out.append(tidy(markup, False))

        kind = opening.group(1).lower()
        closing = text.lower().find(f"</{kind}>", opening.end())
        if closing < 0:
            raise SystemExit(f"unclosed <{kind}>")
        body = text[opening.end() : closing]
        if kind == "script":
            out.append(tidy(strip_js_comments(body), True))
        else:
            out.append(tidy(strip_block_comments(body), False))
        at = closing

def main(argv):
    argv = list(argv)
    chromebook = "--chromebook" in argv[1:]
    apple = "--apple" in argv[1:]
    argv = [a for a in argv if a != "--apple"]
    argv = [a for a in argv if a != "--chromebook"]
    if len(argv) != 3:
        raise SystemExit(__doc__)
    source = open(argv[1], encoding="utf-8").read()
    result = minify(source) + "\n"
    head = []
    if chromebook or apple:
        head.append(INSTALLABLE)
    if chromebook:
        head.append(CHROMEBOOK)
    if apple:
        head.append(APPLE_HEAD)
        result += APPLE_TAIL
    if head:
        if result.count("</title>") != 1:
            raise SystemExit("--chromebook and --apple need exactly one </title> to insert after")
        result = result.replace("</title>", "</title>\n" + "\n".join(head))
    open(argv[2], "w", encoding="utf-8").write(result)
    before = len(source.encode("utf-8"))
    after = len(result.encode("utf-8"))
    print(f"{argv[2]}: {before:,} -> {after:,} bytes ({100 - 100 * after // before}% off)")

if __name__ == "__main__":
    main(sys.argv)
