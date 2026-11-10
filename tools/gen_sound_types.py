#!/usr/bin/env python3

import argparse
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

_ap = argparse.ArgumentParser(description="Generate src/blocks/sound_types.rs.")
_ap.add_argument("--version", default="26.2",
                 help="reference/minecraft-<version> tree to read (default 26.2)")
MC = ROOT / "reference" / f"minecraft-{_ap.parse_args().version}" / "net/minecraft"
BLOCK = MC / "world/level/block"
PROPERTIES = BLOCK / "state/properties"
KINDS = ROOT / "vendor/azalea/azalea-registry/src/builtin.rs"
OUT = ROOT / "src/blocks/sound_types.rs"

DEFAULT = "STONE"
COLORS = ("white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray",
          "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black")
WEATHER_PREFIXES = ("", "exposed_", "weathered_", "oxidized_", "waxed_", "waxed_exposed_",
                    "waxed_weathered_", "waxed_oxidized_")
SETTERS = {
    "DoorBlock": ("BlockSetType", "sound", ("door_open", "door_close")),
    "TrapDoorBlock": ("BlockSetType", "sound", ("trapdoor_open", "trapdoor_close")),
    "ButtonBlock": ("BlockSetType", "sound", ("button_on", "button_off")),
    "BasePressurePlateBlock": ("BlockSetType", "sound", ("plate_on", "plate_off")),
    "FenceGateBlock": ("WoodType", "sound", ("gate_open", "gate_close")),
    "StandingSignBlock": ("WoodType", "sound", None),
    "WallSignBlock": ("WoodType", "sound", None),
    "CeilingHangingSignBlock": ("WoodType", "hanging", None),
    "WallHangingSignBlock": ("WoodType", "hanging", None),
}
SET_FIELDS = ("sound", "door_close", "door_open", "trapdoor_close", "trapdoor_open",
              "plate_off", "plate_on", "button_off", "button_on")
WOOD_FIELDS = ("sound", "hanging", "gate_close", "gate_open")

def die(message):
    sys.exit("gen_sound_types: " + message)

def statements(body):
    out, depth, start = [], 0, 0
    for i, c in enumerate(body):
        if c in "({":
            depth += 1
        elif c in ")}":
            depth -= 1
        elif c == ";" and depth == 0:
            out.append(body[start:i].strip())
            start = i + 1
    return out

def args(text, open_paren):
    out, depth, start = [], 0, open_paren + 1
    for i in range(open_paren, len(text)):
        c = text[i]
        if c in "({":
            depth += 1
        elif c in ")}":
            depth -= 1
            if depth == 0:
                out.append(text[start:i].strip())
                return out
        elif c == "," and depth == 1:
            out.append(text[start:i].strip())
            start = i + 1
    die("unclosed call: " + text[:80])

def sound_events():
    src = (MC / "sounds/SoundEvents.java").read_text()
    return dict(re.findall(r"SoundEvent (\w+) = register\(\"([^\"]+)\"\)", src))

def sound_types(events):
    src = (BLOCK / "SoundType.java").read_text()
    if "SoundEvent breakSound, final SoundEvent stepSound, final SoundEvent placeSound, " \
       "final SoundEvent hitSound, final SoundEvent fallSound" not in src:
        die("SoundType constructor order changed")
    names = re.findall(r"public static final SoundType (\w+);", src)
    rows = {}
    for name, volume, pitch, rest in re.findall(
            r"^\s+(\w+) = new SoundType\(([\d.]+)F, ([\d.]+)F, (.*)\);$", src, re.M):
        keys = [a.removeprefix("SoundEvents.") for a in rest.split(", ")]
        if len(keys) != 5 or any(k not in events for k in keys):
            die("SoundType.%s names an event not in SoundEvents: %s" % (name, keys))
        rows[name] = (volume, pitch, [events[k] for k in keys])
    if sorted(rows) != sorted(names):
        die("SoundType constants without a row: %s" % (set(names) ^ set(rows)))
    return {n: rows[n] for n in names}

def type_rows(cls, fields, skip):
    src = (PROPERTIES / (cls + ".java")).read_text()
    this = src.index("this(name")
    defaults = args(src, src.index("(", this))
    rows = {}
    for m in re.finditer(r"^\s+(\w+) = register\(new %s\(" % cls, src, re.M):
        given = args(src, src.index("(", m.end() - 1))
        full = given if len(given) > 2 else defaults
        rows[m.group(1)] = dict(zip(fields, full[skip:]))
    return rows

def blocks_type_tables(events):
    def event(value):
        key = value.removeprefix("SoundEvents.")
        if key not in events:
            die("unknown event " + value)
        return events[key]

    def clean(rows):
        out = {}
        for name, row in rows.items():
            out[name] = {k: v.removeprefix("SoundType.") if k in ("sound", "hanging") else event(v)
                         for k, v in row.items()}
        return out

    sets = clean(type_rows("BlockSetType", SET_FIELDS, 5))
    woods = clean(type_rows("WoodType", WOOD_FIELDS, 2))
    return {"BlockSetType": sets, "WoodType": woods}

def setter_of(cls, cache={}):
    if cls in SETTERS:
        return cls
    if cls not in cache:
        path = BLOCK / (cls + ".java")
        m = re.search(r"class %s(?:<[^{]*?>)? extends (\w+)" % cls,
                      path.read_text()) if path.exists() else None
        cache[cls] = setter_of(m.group(1)) if m else None
    return cache[cls]

def helpers(src):
    found = {}
    for m in re.finditer(r"(?:private|public) static [\w.<>?]+ (\w+)\(([^)]*)\) \{", src):
        depth, i = 1, m.end()
        while depth:
            depth += {"{": 1, "}": -1}.get(src[i], 0)
            i += 1
        body = src[m.end():i]
        if "BlockBehaviour.Properties.of" not in body:
            continue
        if "Copy(" in body:
            continue
        sounds = re.findall(r"\.sound\((?:SoundType\.)?(\w+)\)", body)
        params = [p.split()[-1] for p in m.group(2).split(",") if p.strip()]
        if sounds and sounds[-1] in params:
            found[m.group(1)] = params.index(sounds[-1])
        else:
            found[m.group(1)] = sounds[-1] if sounds else None
    return found

def block_statements(src):
    fields = re.findall(r"public static final Block (\w+ = .*);$", src, re.M)
    static = src[src.index("   static {"):]
    return fields + statements(static[static.index("{") + 1:])

def resolve(src, types, tables):
    ids, colors, copper = {}, {}, {}
    for holder in ("BlockIds", "BlockItemIds"):
        ref = (MC / "references" / (holder + ".java")).read_text()
        for const, name in re.findall(
                r"\b([A-Z][A-Z0-9_]*) = (?:BlockItemId\.)?create\(\"([a-z0-9_]+)\"", ref):
            ids[holder + "." + const] = name
        for const, base in re.findall(r"\b([A-Z][A-Z0-9_]*) = createSimpleColored\(\"(\w+)\"\)", ref):
            colors[holder + "." + const] = base
        for const, base in re.findall(r"\b([A-Z][A-Z0-9_]*) = createSimpleCopper\(\"(\w+)\"\)", ref):
            copper[holder + "." + const] = [base] * 4
        special = re.search(r"COPPER_BLOCK_SPECIAL_NAMES = new WeatheringCopperCollection\.ByState"
                            r"\(\"(\w+)\", \"(\w+)\", \"(\w+)\", \"(\w+)\"\)", ref)
        if special:
            copper[holder + ".COPPER_BLOCK"] = list(special.groups())
    helper = helpers(src)
    by_const, sound, pairs, rule = {}, {}, {}, Counter()
    registered = 0

    def add(const, name, value, how):
        if value not in types:
            die("%s resolved to unknown SoundType.%s" % (name, value))
        if name in sound:
            die("registered twice: " + name)
        sound[name] = value
        by_const.setdefault(const, value)
        rule[how] += 1

    for st in block_statements(src):
        m = re.match(r"(\w+) = ([\w.]+)\((?:(?:BlockIds|BlockItemIds)\.\w+)", st)
        if not m:
            if re.match(r"[A-Z_0-9]+ = ", st):
                die("unparsed statement: " + st[:100])
            continue
        const = m.group(1)
        ref = re.match(r"\w+ = [\w.]+\(((?:BlockIds|BlockItemIds)\.\w+)", st).group(1)
        if m.group(2) == "ColorCollection.registerBlocks":
            if ref not in colors:
                die("no colour collection for " + ref)
            names = [c + "_" + colors[ref] for c in COLORS]
        elif m.group(2) == "WeatheringCopperCollection.registerBlocks":
            if ref not in copper:
                die("no copper collection for " + ref)
            names = [pre + ref_name for pre, ref_name in zip(
                WEATHER_PREFIXES, copper[ref] * 2)]
        else:
            if ref not in ids:
                die("no block id for " + ref)
            names = [ids[ref]]
        registered += len(names)

        classes = re.findall(r"new (\w+Block\w*)\(", st) + re.findall(r"(\w+Block\w*)::new", st)
        setters = {setter_of(c) for c in classes} - {None}
        if len(setters) > 1:
            die("two setter classes in " + const)
        if setters:
            setter = setters.pop()
            cls, field, events = SETTERS[setter]
            found = set(re.findall(r"\b(BlockSetType|WoodType)\.(\w+)", st))
            if len(found) != 1 or next(iter(found))[0] != cls:
                die("%s: %s needs one %s, found %s" % (const, setter, cls, found))
            row = tables[cls][next(iter(found))[1]]
            for name in names:
                add(const, name, row[field], "constructor")
                if events:
                    pairs[name] = (row[events[0]], row[events[1]])
            continue

        direct = re.findall(r"\.sound\(SoundType\.(\w+)\)", st)
        if st.count(".sound(") != len(direct):
            die("non-constant .sound in " + const)
        if direct:
            for name in names:
                add(const, name, direct[-1], "direct")
            continue

        used = [h for h in helper if re.search(r"\b%s\(" % h, st)]
        if len(used) > 1:
            die("two helpers in %s: %s" % (const, used))
        if used:
            value = helper[used[0]]
            if isinstance(value, int):
                call_at = re.search(r"\b%s\(" % used[0], st).end() - 1
                arg = args(st, call_at)[value]
                if not arg.startswith("SoundType."):
                    die("%s: helper argument %s" % (const, arg))
                for name in names:
                    add(const, name, arg.removeprefix("SoundType."), "helper argument")
            elif value:
                for name in names:
                    add(const, name, value, "helper constant")
            else:
                for name in names:
                    add(const, name, DEFAULT, "default")
            continue

        copies = re.findall(r"Properties\.of(?:Full|Legacy)Copy\((?:\(BlockBehaviour\))?([\w.]+?)"
                            r"(?:\.weathering\(\)\.pick\(\w+\))?\)", st)
        stair = re.match(r"\w+ = register(?:Legacy)?(?:Stair|Slab|Wall)\([\w.]+, (\w+)\)", st)
        if "Copy(" in st and len(copies) != 1:
            die("unreadable copy in " + const)
        source = copies[0] if copies else stair.group(1) if stair else None
        if source:
            source = source.removeprefix("Blocks.")
            if source not in by_const:
                die("%s copies %s, not yet resolved" % (const, source))
            for name in names:
                add(const, name, by_const[source], "copy")
            continue

        for name in names:
            add(const, name, DEFAULT, "default")

    sound["decorated_pot"] = "DECORATED_POT"
    rule["default"] -= 1
    rule["override"] += 1
    return sound, pairs, rule, registered

def block_kinds():
    src = KINDS.read_text()
    start = src.index("enum BlockKind {")
    body = src[start:src.index("\n}", start)]
    return re.findall(r"^\s+(\w+) => \"([a-z0-9_]+)\",$", body, re.M)

HEADER = '''\
//! Generated by `tools/gen_sound_types.py` from SoundType.java, SoundEvents.java,
//! Blocks.java, BlockSetType.java and WoodType.java. Do not edit by hand.
//!
//! `BlockBehaviour.getSoundType` as a table, through `BlockState.as_block_kind`.

use azalea::block::BlockState;
use azalea::block::properties::Cracked;
use azalea_registry::builtin::BlockKind;

/// `SoundType`, with the `sounds.json` event names.
pub struct SoundType {
    pub volume: f32,
    pub pitch: f32,
    pub break_event: &'static str,
    pub step_event: &'static str,
    pub place_event: &'static str,
    pub hit_event: &'static str,
    pub fall_event: &'static str,
}

/// The two events of a door, trapdoor or fence gate (`on` opens, `off` closes),
/// a button or a pressure plate (click on, click off). From `BlockSetType`, and
/// from `WoodType` for a fence gate.
pub struct BlockSetSounds {
    pub on: &'static str,
    pub off: &'static str,
}

/// `BlockState.getSoundType`.
pub fn of(state: BlockState) -> &'static SoundType {
    let kind = state.as_block_kind();
    // `DecoratedPotBlock.getSoundType`, the only per-state override.
    if kind == BlockKind::DecoratedPot && state.property::<Cracked>() == Some(true) {
        return &TYPES[DECORATED_POT_CRACKED];
    }
    &TYPES[BY_KIND[kind as usize] as usize]
}

/// The toggle events of `state`, or `None` for a block without any.
pub fn block_set(state: BlockState) -> Option<&'static BlockSetSounds> {
    SETS.get(SET_BY_KIND[state.as_block_kind() as usize] as usize)
}

'''

def main():
    events = sound_events()
    types = sound_types(events)
    tables = blocks_type_tables(events)
    sound, pairs, rule, registered = resolve((BLOCK / "Blocks.java").read_text(), types, tables)

    kinds = block_kinds()
    names = [n for _, n in kinds]
    if set(names) != set(sound):
        die("BlockKind without a row: %s\nrow without a BlockKind: %s"
            % (sorted(set(names) - set(sound)), sorted(set(sound) - set(names))))
    if len(names) != registered or len(names) != len(set(names)):
        die("%d kinds, %d registrations" % (len(names), registered))

    order = list(types)
    if len(order) > 255:
        die("SoundType index no longer fits u8")
    sets = sorted(set(pairs.values()))
    if len(sets) >= 255:
        die("set index no longer fits u8")

    lines = [HEADER]
    lines.append("const DECORATED_POT_CRACKED: usize = %d;\n" % order.index("DECORATED_POT_CRACKED"))
    lines.append("/// A row of [`SET_BY_KIND`] for a block outside [`SETS`].\nconst NONE: u8 = 255;\n\n")
    lines.append(
        "const _: () = assert!(BlockKind::is_valid_id(%d) && !BlockKind::is_valid_id(%d));\n\n"
        % (len(kinds) - 1, len(kinds)))

    lines.append("#[rustfmt::skip]\nstatic TYPES: [SoundType; %d] = [\n" % len(order))
    for name in order:
        volume, pitch, e = types[name]
        lines.append("    // %s\n" % name)
        lines.append('    SoundType { volume: %s, pitch: %s, break_event: "%s", step_event: "%s", '
                     'place_event: "%s", hit_event: "%s", fall_event: "%s" },\n'
                     % (volume, pitch, *e))
    lines.append("];\n\n")

    lines.append("/// Index into [`TYPES`], in `BlockKind` order.\n")
    lines.append("#[rustfmt::skip]\nstatic BY_KIND: [u8; %d] = [\n" % len(kinds))
    for _, name in kinds:
        lines.append("    %d, // %s: %s\n" % (order.index(sound[name]), name, sound[name]))
    lines.append("];\n\n")

    lines.append("#[rustfmt::skip]\nstatic SETS: [BlockSetSounds; %d] = [\n" % len(sets))
    for on, off in sets:
        lines.append('    BlockSetSounds { on: "%s", off: "%s" },\n' % (on, off))
    lines.append("];\n\n")

    lines.append("/// Index into [`SETS`], in `BlockKind` order.\n")
    lines.append("#[rustfmt::skip]\nstatic SET_BY_KIND: [u8; %d] = [\n" % len(kinds))
    cells = [str(sets.index(pairs[n])) if n in pairs else "NONE" for _, n in kinds]
    for i in range(0, len(cells), 16):
        lines.append("    " + ", ".join(cells[i:i + 16]) + ",\n")
    lines.append("];\n\n")

    lines.append('''\
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_door_sounds_like_its_block_set() {
        let door = BlockState::from(BlockKind::CherryDoor);
        assert_eq!(of(door).break_event, "block.cherry_wood.break");
        assert_eq!(block_set(door).unwrap().on, "block.cherry_wood_door.open");
        assert!(block_set(BlockState::from(BlockKind::Stone)).is_none());
    }
}
''')
    OUT.write_text("".join(lines))

    for how, count in rule.most_common():
        print("%-16s %d" % (how, count), file=sys.stderr)
    print("%-16s %d, %d SoundTypes, %d event pairs on %d blocks"
          % ("rows", len(kinds), len(order), len(sets), len(pairs)), file=sys.stderr)

main()
