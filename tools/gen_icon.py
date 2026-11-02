import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
RES = ROOT / "android" / "res"
OUT = ROOT / "windows"
WEB = ROOT / "web" / "chromebook"
WEB_SIZES = [192, 512]
NS = "{http://schemas.android.com/apk/res/android}"
VIEWPORT = 108
SAFE_MIN = 18
SAFE_SIZE = 72
SCALE = 16
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]
PATH_RE = re.compile(r"^M(\d+),(\d+)h(\d+)v(\d+)h-\d+z$")

def background():
    colors = ET.parse(RES / "values" / "colors.xml").getroot()
    for color in colors.iter("color"):
        if color.get("name") == "ic_launcher_background":
            return color.text.strip()
    sys.exit("ic_launcher_background not found in colors.xml")

def rects():
    vector = ET.parse(RES / "drawable" / "ic_launcher_foreground.xml").getroot()
    for path in vector.iter("path"):
        data = path.get(NS + "pathData")
        match = PATH_RE.match(data)
        if not match:
            sys.exit(f"unsupported pathData {data!r}")
        x, y, w, h = map(int, match.groups())
        yield path.get(NS + "fillColor"), x, y, w, h

def render_full():
    full = Image.new("RGBA", (VIEWPORT, VIEWPORT), background())
    draw = ImageDraw.Draw(full)
    for fill, x, y, w, h in rects():
        draw.rectangle([x, y, x + w - 1, y + h - 1], fill=fill)
    return full

def render():
    full = render_full()
    safe = full.crop((SAFE_MIN, SAFE_MIN, SAFE_MIN + SAFE_SIZE, SAFE_MIN + SAFE_SIZE))
    return safe.resize((SAFE_SIZE * SCALE, SAFE_SIZE * SCALE), Image.NEAREST)

def main():
    big = render()
    OUT.mkdir(exist_ok=True)
    big.resize((256, 256), Image.LANCZOS).save(OUT / "icon.png")
    big.save(OUT / "icon.ico", sizes=[(s, s) for s in ICO_SIZES])
    print(f"wrote {OUT / 'icon.png'} and {OUT / 'icon.ico'}")

    WEB.mkdir(parents=True, exist_ok=True)
    for size in WEB_SIZES:
        big.resize((size, size), Image.LANCZOS).save(WEB / f"icon-{size}.png")
    render_full().resize((512, 512), Image.NEAREST).save(WEB / "icon-maskable-512.png")
    print(f"wrote the web icons into {WEB}")

if __name__ == "__main__":
    main()
