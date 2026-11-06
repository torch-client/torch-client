#!/usr/bin/env python3
import collections
import glob
import gzip
import json
import lzma
import os
import re
import struct
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HERE = os.path.dirname(os.path.abspath(__file__))

FORBIDDEN = [0x00, 0x0D, 0x3C]
CP1252_HIGH = "\u20ac\x81\u201a\u0192\u201e\u2026\u2020\u2021\u02c6\u2030\u0160\u2039\u0152\x8d\u017d\x8f" \
              "\x90\u2018\u2019\u201c\u201d\u2022\u2013\u2014\u02dc\u2122\u0161\u203a\u0153\x9d\u017e\u0178"

def newest_site():
    sites = glob.glob(os.path.join(ROOT, "web-output", "*", "site", "index.html"))
    if not sites:
        sys.exit("no web-output/*/site/index.html; run tools/build_web.sh first")
    return os.path.dirname(max(sites, key=os.path.getmtime))

def read(path, mode="r"):
    with open(path, mode) as f:
        return f.read()

def js_string(text):
    return json.dumps(text).replace("</", "<\\/").replace("\u2028", "\\u2028").replace("\u2029", "\\u2029")

def compress(glue, wasm):
    g = glue.encode("utf-8")
    raw = struct.pack("<I", len(g)) + g + wasm
    filters = [{"id": lzma.FILTER_LZMA1, "preset": 9 | lzma.PRESET_EXTREME, "dict_size": 64 << 20,
                "lc": 4, "lp": 0, "pb": 0, "nice_len": 273, "mf": lzma.MF_BT4, "depth": 0}]
    packed = bytearray(lzma.compress(raw, format=lzma.FORMAT_ALONE, filters=filters))
    packed[5:13] = struct.pack("<Q", len(raw))
    return bytes(packed)

def escape(packed):
    counts = collections.Counter(packed)
    esc = min((b for b in range(1, 256) if b not in FORBIDDEN and not 0x41 <= b <= 0x44),
              key=lambda b: counts[b])
    table = FORBIDDEN + [esc]
    body = packed.replace(bytes([esc]), bytes([esc, 0x41 + 3]))
    for k, b in enumerate(FORBIDDEN):
        body = body.replace(bytes([b]), bytes([esc, 0x41 + k]))
    return body, esc, table

def ascii_page(page):
    out = []
    for i, part in enumerate(re.split(r"(<script\b[^>]*>.*?</script>)", page, flags=re.S)):
        if i % 2:
            out.append("".join(c if ord(c) < 128 else "\\u%04x" % ord(c) for c in part))
        else:
            out.append("".join(c if ord(c) < 128 else "&#%d;" % ord(c) for c in part))
    return "".join(out)

BOOT = r"""<script>
window.__ready = (async () => {
  const fill = document.getElementById("fill"), pct = document.getElementById("pct");
  document.getElementById("label").textContent = "unpacking the client...";
  const el = document.getElementById("packed");
  const text = el.textContent;
  el.remove();
  if (text.indexOf("\ufffd") >= 0) {
    throw new Error("this page was read as UTF-8; open it from disk or serve it without a charset=utf-8 header");
  }
  const packed = new Uint8Array(%(size)d);
  const hi = %(high)s, inv = new Uint8Array(0x2123), tab = %(table)s;
  for (let b = 128; b < 160; b++) inv[b] = b;
  for (let b = 0; b < 256; b++) inv[b < 128 || b >= 160 ? b : hi.charCodeAt(b - 128)] = b;
  let j = 0;
  for (let i = 0; i < text.length; i++) {
    let b = inv[text.charCodeAt(i)];
    if (b === %(esc)d) b = tab[inv[text.charCodeAt(++i)] - 65];
    packed[j++] = b;
  }
  if (j !== packed.length) throw new Error(`unpacked ${j} of ${packed.length} bytes`);

  const src = %(decoder)s + `
self.onmessage = (e) => {
  const out = lzmaDecode(new Uint8Array(e.data), (a, b) => postMessage([a, b]));
  postMessage(out, [out.buffer]);
};`;
  const raw = await new Promise((resolve, reject) => {
    const w = new Worker(URL.createObjectURL(new Blob([src], { type: "text/javascript" })));
    w.onmessage = (e) => {
      if (Array.isArray(e.data)) {
        const p = Math.round((e.data[0] / e.data[1]) * 100);
        fill.style.width = `${p}%%`;
        pct.textContent = `${p}%%`;
      } else {
        w.terminate();
        resolve(e.data);
      }
    };
    w.onerror = (e) => { w.terminate(); reject(new Error(e.message || "the unpack worker failed")); };
    w.postMessage(packed.buffer, [packed.buffer]);
  });
  const glueLen = raw[0] | (raw[1] << 8) | (raw[2] << 16) | (raw[3] << 24);
  const glue = new TextDecoder().decode(raw.subarray(4, 4 + glueLen));
  const bytes = raw.subarray(4 + glueLen);
  const WASM_NAME = %(wasm_name)s;

  const blobUrl = (t) => URL.createObjectURL(new Blob([t], { type: "text/javascript" }));
  window.__glueUrl = blobUrl(glue);
  const workerUrl = %(worker)s;
  const workerBlob = workerUrl === null ? null : blobUrl(workerUrl.split(%(glue_ref)s).join(window.__glueUrl));

  const realFetch = window.fetch.bind(window);
  window.fetch = (input, init) => {
    const url = typeof input === "string" ? input : input.url;
    if (url.endsWith(WASM_NAME)) {
      return Promise.resolve(new Response(bytes.slice().buffer, {
        status: 200,
        headers: { "content-length": String(bytes.length), "content-type": "application/wasm" },
      }));
    }
    return realFetch(input, init);
  };

  const RealWorker = window.Worker;
  if (workerBlob !== null) {
    window.Worker = class extends RealWorker {
      constructor(url, options) {
        super(String(url) === %(worker_ref)s ? workerBlob : url, options);
      }
    };
  }
})();
window.__ready.catch((e) => {
  document.getElementById("label").textContent = `failed to unpack:\n\n${e}`;
});
</script>
"""

def main():
    site = sys.argv[1] if len(sys.argv) > 1 else newest_site()
    out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(os.path.dirname(os.path.abspath(site)), "torch-client-single.html")
    print("packing %s" % site)

    page = read(os.path.join(site, "index.html"))

    wasm_paths = sorted(glob.glob(os.path.join(site, "torch-client_bg*.wasm.gz")))
    if not wasm_paths:
        sys.exit("no torch-client_bg*.wasm.gz in " + site)
    wasm_path = wasm_paths[-1]
    wasm_name = os.path.basename(wasm_path)

    m = re.search(r"""from\s*["']\./(torch-client[^"']*\.js)["']""", page)
    if not m:
        sys.exit("page has no static import of the glue")
    glue_name = m.group(1)
    glue = read(os.path.join(site, glue_name))

    inlined = {}

    def inline_body(path):
        if path in inlined:
            return ""
        body = read(os.path.join(site, path))
        inlined[path] = re.findall(r"^export\s+(?:async\s+)?(?:function\*?|const|let|var|class)\s+([A-Za-z_$][\w$]*)",
                                   body, flags=re.M)
        return re.sub(r"^export\s+", "", body, flags=re.M) + "\n"

    def inline_snippet(match):
        return inline_body(match.group(2))

    def inline_namespace(match):
        text = inline_body(match.group(2))
        names = ", ".join(inlined[match.group(2)])
        return text + "const %s = { %s };\n" % (match.group(1), names)

    glue, _ = re.subn(r"^import\s*\{([^}]*)\}\s*from\s*['\"](\./snippets/[^'\"]+)['\"];?\s*\n",
                      inline_snippet, glue, flags=re.M)
    glue, _ = re.subn(r"^import\s*\*\s*as\s+(\w+)\s+from\s*['\"](\./snippets/[^'\"]+)['\"];?\s*\n",
                      inline_namespace, glue, flags=re.M)
    if re.search(r"^import\s", glue, flags=re.M):
        sys.exit("glue still has an import after inlining snippets")

    w = re.search(r"""globalThis\.torchWorker\s*=\s*["']\./(worker[^"']*\.js)["']""", page)
    if not w:
        sys.exit("page has no torchWorker line")
    worker_name = w.group(1)
    worker_path = os.path.join(site, worker_name)
    worker_src = read(worker_path) if os.path.exists(worker_path) else None

    imp = re.search(r"""import\s+init\s*,\s*\{([^}]*)\}\s*from\s*["']\./""" + re.escape(glue_name) + r"""["']\s*;?""", page)
    if not imp:
        sys.exit("could not find the page's `import init, {...}` line")
    names = imp.group(1).strip()
    page = page.replace(
        imp.group(0),
        "await window.__ready;\nconst { default: init, %s } = await import(window.__glueUrl);" % names,
        1,
    )
    page = page.replace('import("./%s")' % glue_name, "import(window.__glueUrl)")
    if "./" + glue_name in page:
        sys.exit("page still names ./" + glue_name)

    if '<meta charset="utf-8">' not in page:
        sys.exit('page has no <meta charset="utf-8"> to replace')
    page = page.replace('<meta charset="utf-8">', '<meta charset="windows-1252">', 1)

    wasm = gzip.decompress(read(wasm_path, "rb"))
    packed = compress(glue, wasm)
    body, esc, table = escape(packed)
    print("wasm %.1f MB, lzma %.1f MB, escaped %.1f MB" % (len(wasm) / 1e6, len(packed) / 1e6, len(body) / 1e6))

    boot = BOOT % {
        "size": len(packed),
        "high": js_string(CP1252_HIGH),
        "table": json.dumps(table),
        "esc": esc,
        "decoder": js_string(read(os.path.join(HERE, "lzma_dec.js"))),
        "wasm_name": js_string(wasm_name),
        "worker": js_string(worker_src) if worker_src is not None else "null",
        "worker_ref": js_string("./" + worker_name),
        "glue_ref": js_string("./" + glue_name),
    }

    marker = '<script type="module">'
    if marker not in page:
        sys.exit("page has no module script")
    data_marker = "\0DATA\0"
    page = page.replace(marker, '<script type="x" id="packed">' + data_marker + "</script>\n" + boot + marker, 1)
    pre, post = ascii_page(page).split(data_marker)

    with open(out, "wb") as f:
        f.write(pre.encode("ascii") + body + post.encode("ascii"))
    print("wrote %s (%.1f MB)" % (out, os.path.getsize(out) / 1e6))

if __name__ == "__main__":
    main()
