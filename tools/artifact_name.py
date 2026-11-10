#!/usr/bin/env python3
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UPDATER = os.path.join(ROOT, "src", "client", "self_update.rs")

def feature_table():
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1",
         "--manifest-path", os.path.join(ROOT, "Cargo.toml")],
        check=True, capture_output=True, text=True,
    ).stdout
    meta = json.loads(out)
    root = next(p for p in meta["packages"] if p["name"] == "torch-client")
    return root["features"]

def requested(args):
    defaults, every, asked = True, False, []
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--no-default-features":
            defaults = False
        elif a == "--all-features":
            every = True
        elif a in ("--features", "-F") and i + 1 < len(args):
            asked.append(args[i + 1])
            i += 1
        elif a.startswith("--features=") or a.startswith("-F="):
            asked.append(a.split("=", 1)[1])
        i += 1
    names = [n.split("/")[-1] for s in asked for n in s.replace(",", " ").split()]
    return defaults, every, names

def enabled(args):
    table = feature_table()
    defaults, every, names = requested(args)
    if every:
        return set(table)
    todo = names + (["default"] if defaults else [])
    on = set()
    while todo:
        f = todo.pop()
        if f in on or f not in table:
            continue
        on.add(f)
        todo.extend(table[f])
    return on

def name(platform, on):
    gui = "-clickgui" if "click_gui" in on else ""
    if platform == "linux":
        return "torch-client%s-linux-x86_64.zip" % gui
    if platform == "windows":
        return "torch-client%s-windows-x86_64.exe.zip" % gui
    if platform == "android":
        return "torch-client-android-arm64.apk"
    if platform == "web":
        return "torch-client-html%s-beta.html" % ("-mobile" if "mobile_ui" in on else "")
    sys.exit("unknown platform %r (linux, windows, android, web)" % platform)

def main():
    if len(sys.argv) < 3 or sys.argv[1] not in ("name", "has"):
        sys.exit("usage: artifact_name.py name PLATFORM [cargo args...]\n"
                 "       artifact_name.py has FEATURE [cargo args...]")
    mode, subject, args = sys.argv[1], sys.argv[2], sys.argv[3:]
    on = enabled(args)
    if mode == "has":
        sys.exit(0 if subject in on else 1)
    result = name(subject, on)
    if subject in ("linux", "windows") and os.path.exists(UPDATER):
        with open(UPDATER) as f:
            if '"%s"' % result not in f.read():
                sys.exit("%s is not one of the names self_update asks for; "
                         "change both together" % result)
    print(result)

if __name__ == "__main__":
    main()
