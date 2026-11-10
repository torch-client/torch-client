#!/usr/bin/env python3
"""Generate the per-version protocol tables in `src/protocol/data/`.

    python3 protogen.py packets [--source PATH] [--version NAME] [--output PATH]
    python3 protogen.py remap --from NAME --to NAME [--output PATH]
    python3 protogen.py metadata --from NAME --to NAME [--output PATH]
    mkdir -p /tmp/gen && cd /tmp/gen
    java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar          --reports --output /tmp/gen
    cp /tmp/gen/reports/{registries,blocks}.json        <crate>/reference/minecraft-<version>/generated/reports/
    python3 protogen.py packets [--source PATH] [--version NAME] [--output PATH]
    --version 26.2
    --source  <crate>/reference/minecraft-<version>
    --output  <crate>/src/protocol/data/packets-<version>.json
    protocol number   <source>/version.json  ("protocol_version")
    packet names      <source>/net/minecraft/network/protocol/**/*PacketTypes.java
    packet ids        <source>/net/minecraft/network/protocol/<phase>/<Phase>Protocols.java
    `SERVERBOUND_SET_CREATIVE_MODE_SLOT`. The modifier is a runtime
    `CodecModifier`, not a condition, so the packet still takes exactly one id.
    clientbound game id 0. The id belongs to the **delimiter**, which is the
    packet that goes on the wire; `ClientboundBundlePacket` is a container that
    never does. Naming the id after the first argument would put `bundle` where
    `bundle_delimiter` belongs, which azalea's own generated table
    (`azalea-protocol/src/packets/game/mod.rs`) and vanilla both disagree with.
    registry ids     <source>/generated/reports/registries.json
    block state ids  <source>/generated/reports/blocks.json
    serializer ids   <source>/net/minecraft/network/syncher/EntityDataSerializers.java
    protocol number   <source>/version.json  ("protocol_version")
    synched fields    every `SynchedEntityData.defineId(X.class, S)` (Mojang
                      names) or `DataTracker.registerData(X.class, S)` (Yarn
                      names) under <source>/net/minecraft
    entity types      the `EntityType<X> NAME;` fields of `EntityType.java`
                      (Mojang) or `EntityTypes.java` (Yarn)
    python3 tools/protogen.py metadata --from 1.21.11 --to 26.1.1
    python3 tools/protogen.py metadata --from 26.1.1 --to 26.2
"""

import argparse
import difflib
import json
import os
import re
import sys
from pathlib import Path

CRATE = Path(__file__).resolve().parent.parent

PHASES = [
    ("handshake", "handshake", "HandshakeProtocols"),
    ("status", "status", "StatusProtocols"),
    ("login", "login", "LoginProtocols"),
    ("configuration", "configuration", "ConfigurationProtocols"),
    ("game", "game", "GameProtocols"),
]

DECL = re.compile(
    r"PacketType<\s*([A-Za-z0-9_.]+)\s*>\s+([A-Z0-9_]+)\s*=\s*"
    r"create(Clientbound|Serverbound)\(\s*\"([a-z0-9_/.]+)\"\s*\)"
)
TEMPLATE = re.compile(r"ProtocolInfoBuilder\.([A-Za-z]+)\(\s*ConnectionProtocol\.")

class Fail(Exception):
    pass

def read(path):
    if not path.is_file():
        raise Fail(f"missing {path}")
    return path.read_text(encoding="utf-8")

def packet_names(protocol_dir):
    by_constant = {}
    by_class = {}
    files = sorted(protocol_dir.glob("*/*PacketTypes.java"))
    if not files:
        raise Fail(f"no *PacketTypes.java under {protocol_dir}")
    for path in files:
        owner = path.stem
        for packet_class, constant, _flow, resource in DECL.findall(read(path)):
            key = f"{owner}.{constant}"
            if key in by_constant:
                raise Fail(f"{key} declared twice")
            by_constant[key] = resource
            by_class.setdefault(packet_class, resource)
    return by_constant, by_class

def split_args(text, start):
    depth = 0
    args = []
    current = start + 1
    i = current
    while i < len(text):
        c = text[i]
        if c in "([{":
            depth += 1
        elif c in ")]}":
            if depth == 0:
                args.append(text[current:i].strip())
                return args, i + 1
            depth -= 1
        elif c == "," and depth == 0:
            args.append(text[current:i].strip())
            current = i + 1
        elif c == '"':
            i = text.index('"', i + 1)
        i += 1
    raise Fail("unbalanced parentheses in a protocol builder chain")

def template_bodies(source):
    out = []
    for match in TEMPLATE.finditer(source):
        method = match.group(1)
        if "erverbound" in method:
            direction = "serverbound"
        elif "lientbound" in method:
            direction = "clientbound"
        else:
            raise Fail(f"cannot tell the direction of ProtocolInfoBuilder.{method}")
        args, _ = split_args(source, match.end() - 1)
        if len(args) != 2:
            raise Fail(f"ProtocolInfoBuilder.{method} took {len(args)} arguments, expected 2")
        out.append((direction, args[1]))
    return out

def scan(body, by_constant, by_class, where):
    names = []
    for match in re.finditer(r"\.(addPacket|withBundlePacket)\(", body):
        call = match.group(1)
        args, _ = split_args(body, match.end() - 1)
        if call == "addPacket":
            if len(args) not in (2, 3):
                raise Fail(f"{where}: addPacket took {len(args)} arguments")
            key = args[0]
            if key not in by_constant:
                raise Fail(f"{where}: unknown packet type {key}")
            names.append(by_constant[key])
        else:
            if len(args) != 3:
                raise Fail(f"{where}: withBundlePacket took {len(args)} arguments")
            delimiter = re.fullmatch(r"new\s+([A-Za-z0-9_]+)\(\s*\)", args[2])
            if not delimiter:
                raise Fail(f"{where}: cannot read a delimiter out of {args[2]!r}")
            packet_class = delimiter.group(1)
            if packet_class not in by_class:
                raise Fail(f"{where}: no packet type declares {packet_class}")
            names.append(by_class[packet_class])
    return names

CLIENT_REGISTRIES = [
    "attribute",
    "block",
    "block_entity_type",
    "data_component_type",
    "entity_type",
    "game_event",
    "item",
    "menu",
    "mob_effect",
    "particle_type",
    "potion",
    "sound_event",
    "villager_profession",
    "villager_type",
]

SERIALIZERS = "net/minecraft/network/syncher/EntityDataSerializers.java"

def reports(source, name):
    path = source / "generated" / "reports" / f"{name}.json"
    if not path.is_file():
        raise Fail(
            f"missing {path}; run the server jar's data generator, see this script's header"
        )
    return json.loads(read(path))

def registry_ids(source, registry):
    key = f"minecraft:{registry}"
    data = reports(source, "registries")
    if key not in data:
        raise Fail(f"{source} has no {key} registry")
    return {name: e["protocol_id"] for name, e in data[key]["entries"].items()}

def block_state_ids(source):
    out = {}
    for block, info in reports(source, "blocks").items():
        for state in info["states"]:
            props = tuple(sorted(state.get("properties", {}).items()))
            out[(block, props)] = state["id"]
    return out

def serializer_ids(source):
    text = read(source / SERIALIZERS)
    names = re.findall(r"registerSerializer\(([A-Z0-9_]+)\)", text)
    if not names:
        raise Fail(f"no registerSerializer calls in {source / SERIALIZERS}")
    return {name: i for i, name in enumerate(names)}

def runs(source, target, what):
    by_id = {}
    for name, id in source.items():
        if id in by_id:
            raise Fail(f"{what}: id {id} is claimed by two entries")
        by_id[id] = name

    out = []
    previous = ()
    taken = {}
    for id in range(len(by_id)):
        name = by_id.get(id)
        if name is None:
            raise Fail(f"{what}: the source version has a hole at id {id}")
        landing = target.get(name)
        if landing is not None:
            if landing in taken:
                raise Fail(f"{what}: ids {taken[landing]} and {id} both land on {landing}")
            taken[landing] = id
        offset = None if landing is None else landing - id
        if (offset,) != previous:
            out.append([id, offset])
            previous = (offset,)
    return out

def version_meta(source, version):
    info = json.loads(read(source / "version.json"))
    if info.get("id") != version:
        raise Fail(f"{source}/version.json is {info.get('id')!r}, asked for {version!r}")
    return {"version": version, "protocol": info["protocol_version"]}

def generate_remap(old_source, old_version, new_source, new_version, out):
    old_meta = version_meta(old_source, old_version)
    new_meta = version_meta(new_source, new_version)
    if old_meta["protocol"] == new_meta["protocol"]:
        raise Fail("both versions speak the same protocol; there is nothing to remap")

    sources = [(r, registry_ids) for r in CLIENT_REGISTRIES]
    sources.append(("block_state", block_state_ids))
    sources.append(("entity_data_serializer", serializer_ids))

    table = {"from": old_meta, "to": new_meta, "forward": {}, "back": {}}
    identity = []
    for name, reader in sources:
        if reader is registry_ids:
            old_ids, new_ids = reader(old_source, name), reader(new_source, name)
        else:
            old_ids, new_ids = reader(old_source), reader(new_source)
        forward = runs(old_ids, new_ids, name)
        back = runs(new_ids, old_ids, f"{name} (reversed)")
        table["forward"][name] = {"count": len(old_ids), "runs": forward}
        table["back"][name] = {"count": len(new_ids), "runs": back}
        if forward == [[0, 0]] and len(old_ids) == len(new_ids):
            identity.append(name)

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(table, indent=2) + "\n", encoding="utf-8")
    print(
        f"protogen: {old_meta['version']} ({old_meta['protocol']})"
        f" -> {new_meta['version']} ({new_meta['protocol']}) -> {out}"
    )
    for name, _ in sources:
        forward = table["forward"][name]
        note = "identity" if name in identity else f"{len(forward['runs'])} runs"
        holes = sum(1 for _, offset in table["back"][name]["runs"] if offset is None)
        holes = f", {holes} back with no counterpart" if holes else ""
        print(f"  {name:<24} {forward['count']:>6} ids  {note}{holes}")

DEFINE = re.compile(
    r"(?:DataTracker\.registerData|SynchedEntityData\.defineId)"
    r"\(\s*(\w+)\.class\s*,\s*([\w.]+)\s*\)"
)
CLASS = re.compile(
    r"^\s*(?:public |protected |private )?(?:static )?"
    r"(?:abstract |final |sealed |non-sealed )*(?:class|record)\s+(\w+)"
    r"(?:<[^{]*?>)?(?:\s+extends\s+([\w.]+))?",
    re.M,
)
PACKAGE = re.compile(r"^package\s+([\w.]+);", re.M)
IMPORT = re.compile(r"^import\s+([\w.]+);", re.M)
ENTITY_TYPE = re.compile(r"public static final EntityType<([\w.]+)>\s+(\w+);")

SERIALIZER_NAMES = {
    "INTEGER": "INT",
    "TEXT_COMPONENT": "COMPONENT",
    "OPTIONAL_TEXT_COMPONENT": "OPTIONAL_COMPONENT",
    "ROTATION": "ROTATIONS",
    "FACING": "DIRECTION",
    "LAZY_ENTITY_REFERENCE": "OPTIONAL_LIVING_ENTITY_REFERENCE",
    "NBT_COMPONENT": "COMPOUND_TAG",
    "ENTITY_POSE": "POSE",
    "OPTIONAL_INT": "OPTIONAL_UNSIGNED_INT",
    "PARTICLE_LIST": "PARTICLES",
    "ARM": "HUMANOID_ARM",
    "PROFILE": "RESOLVABLE_PROFILE",
    "OXIDATION_LEVEL": "WEATHERING_COPPER_STATE",
    "QUATERNION_F": "QUATERNION",
    "VECTOR_3F": "VECTOR3",
}

def class_index(root):
    raw = {}
    by_simple = {}
    fields = {}
    for path in sorted(root.rglob("*.java")):
        source = read(path)
        package = PACKAGE.search(source)
        package = package.group(1) if package else ""
        imports = {i.rsplit(".", 1)[1]: i for i in IMPORT.findall(source)}
        top = path.stem
        for match in CLASS.finditer(source):
            name, extends = match.group(1), match.group(2)
            full = package + "." + (name if name == top else f"{top}${name}")
            raw[full] = (package, imports, extends)
            by_simple.setdefault(name, []).append(full)
        found = {}
        for owner, serializer in DEFINE.findall(source):
            serializer = serializer.rsplit(".", 1)[-1]
            found.setdefault(owner, []).append(SERIALIZER_NAMES.get(serializer, serializer))
        for owner, serializers in found.items():
            key = package + "." + (owner if owner == top else f"{top}${owner}")
            raw.setdefault(key, (package, imports, None))
            fields[key] = serializers

    def resolve(package, imports, extends):
        if extends is None:
            return None
        extends = extends.rsplit(".", 1)[-1]
        if extends in imports:
            return imports[extends]
        if f"{package}.{extends}" in raw:
            return f"{package}.{extends}"
        candidates = by_simple.get(extends, [])
        same = [c for c in candidates if c.startswith(package + ".")]
        if same:
            return same[0]
        return candidates[0] if len(candidates) == 1 else None

    return {k: resolve(*v) for k, v in raw.items()}, fields

def entity_chains(root):
    parent, fields = class_index(root)
    text = ""
    package = ""
    for path in sorted(root.rglob("entity")):
        for name in ("EntityType.java", "EntityTypes.java"):
            if (path / name).is_file():
                body = read(path / name)
                text += body
                package = PACKAGE.search(body).group(1)
    if not text:
        raise Fail(f"no EntityType.java or EntityTypes.java under {root}")
    imports = {i.rsplit(".", 1)[1]: i for i in IMPORT.findall(text)}
    out = {}
    for cls, field in ENTITY_TYPE.findall(text):
        outer, _, inner = cls.partition(".")
        full = imports.get(outer) or f"{package}.{outer}"
        if inner:
            full += "$" + inner
        if full not in parent:
            raise Fail(f"entity type {field}: class {full} not found under {root}")
        chain = []
        while full:
            chain.append((full.rsplit(".", 1)[1], fields.get(full, [])))
            full = parent.get(full)
        chain.reverse()
        out[field.lower()] = chain
    return out

def field_map(old, new):
    if len(old) == len(new):
        out, old_at, new_at = {}, 0, 0
        for (_, olds), (_, news) in zip(old, new):
            for k, serializer in enumerate(olds):
                same = k < len(news) and serializer == news[k]
                out[old_at + k] = new_at + k if same else None
            old_at += len(olds)
            new_at += len(news)
        return out, False
    old_flat = [s for _, ss in old for s in ss]
    new_flat = [s for _, ss in new for s in ss]
    out = {i: None for i in range(len(old_flat))}
    matcher = difflib.SequenceMatcher(None, old_flat, new_flat, autojunk=False)
    for a, b, n in matcher.get_matching_blocks():
        for k in range(n):
            out[a + k] = b + k
    return out, True

def generate_metadata(old_source, old_version, new_source, new_version, out):
    old_meta = version_meta(old_source, old_version)
    new_meta = version_meta(new_source, new_version)
    old_chains = entity_chains(old_source / "net" / "minecraft")
    new_chains = entity_chains(new_source / "net" / "minecraft")

    types = {}
    for name in sorted(set(old_chains) & set(new_chains)):
        moved, guessed = field_map(old_chains[name], new_chains[name])
        table = runs(
            {i: i for i in moved},
            {i: n for i, n in moved.items() if n is not None},
            f"metadata {name}",
        )
        if table[:1] == [[0, 0]]:
            table = table[1:]
        if not table:
            continue
        types[name] = table
        if guessed:
            print(f"{name}: aligned by serializer, check: {table}", file=sys.stderr)
        if any(offset is None for _, offset in table):
            print(f"{name}: a field has no new index: {table}", file=sys.stderr)

    lines = [f"{json.dumps(name)}: {json.dumps(table)}" for name, table in types.items()]
    body = ",\n".join("  " + line for line in lines)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(
        "{\n"
        f' "from": {json.dumps(old_meta)},\n'
        f' "to": {json.dumps(new_meta)},\n'
        ' "entity_types": {\n'
        f"{body}\n"
        " }\n}\n",
        encoding="utf-8",
    )
    print(
        f"protogen: {old_meta['version']} ({old_meta['protocol']})"
        f" -> {new_meta['version']} ({new_meta['protocol']}), {len(types)} entity types -> {out}"
    )

def generate(source, version, out):
    meta = json.loads(read(source / "version.json"))
    protocol = meta.get("protocol_version")
    if not isinstance(protocol, int):
        raise Fail(f"{source}/version.json has no integer protocol_version")
    if meta.get("id") != version:
        raise Fail(f"{source}/version.json is {meta.get('id')!r}, asked for {version!r}")

    protocol_dir = source / "net" / "minecraft" / "network" / "protocol"
    by_constant, by_class = packet_names(protocol_dir)

    table = {"version": version, "protocol": protocol}
    for key, directory, stem in PHASES:
        text = read(protocol_dir / directory / f"{stem}.java")
        phase = {"serverbound": [], "clientbound": []}
        seen = set()
        for direction, body in template_bodies(text):
            if direction in seen:
                raise Fail(f"{stem}: two {direction} templates")
            seen.add(direction)
            phase[direction] = scan(body, by_constant, by_class, f"{stem}/{direction}")
        table[key] = phase

    for key, _, _ in PHASES:
        for direction, names in table[key].items():
            if len(set(names)) != len(names):
                raise Fail(f"{key}/{direction} registers a packet twice")
    if not table["game"]["clientbound"]:
        raise Fail("no clientbound game packets")
    if table["game"]["clientbound"][0] != "bundle_delimiter":
        raise Fail(
            "game clientbound id 0 is "
            f"{table['game']['clientbound'][0]}, expected bundle_delimiter"
        )

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(table, indent=2) + "\n", encoding="utf-8")
    total = sum(len(d) for k, _, _ in PHASES for d in table[k].values())
    print(f"protogen: {version} (protocol {protocol}), {total} packets -> {out}")
    for key, _, _ in PHASES:
        counts = {d: len(n) for d, n in table[key].items()}
        print(f"  {key:<14} {counts['serverbound']:>3} serverbound  {counts['clientbound']:>3} clientbound")

def main():
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    modes = ap.add_subparsers(dest="mode", required=True)

    packets = modes.add_parser("packets", help="packet-id tables from decompiled source")
    packets.add_argument("--version", default="26.2")
    packets.add_argument("--source")
    packets.add_argument("--output")

    remap = modes.add_parser("remap", help="one version's ids into another's")
    remap.add_argument("--from", dest="old", required=True)
    remap.add_argument("--to", dest="new", required=True)
    remap.add_argument("--source-from", dest="old_source")
    remap.add_argument("--source-to", dest="new_source")
    remap.add_argument("--output")

    metadata = modes.add_parser("metadata", help="entity metadata index moves between versions")
    metadata.add_argument("--from", dest="old", required=True)
    metadata.add_argument("--to", dest="new", required=True)
    metadata.add_argument("--source-from", dest="old_source")
    metadata.add_argument("--source-to", dest="new_source")
    metadata.add_argument("--output")

    args = ap.parse_args()
    tree = lambda version: CRATE / "reference" / f"minecraft-{version}"
    data = CRATE / "src" / "protocol" / "data"

    try:
        if args.mode == "packets":
            source = Path(args.source) if args.source else tree(args.version)
            out = Path(args.output) if args.output else data / f"packets-{args.version}.json"
            generate(source, args.version, out)
        else:
            old_source = Path(args.old_source) if args.old_source else tree(args.old)
            new_source = Path(args.new_source) if args.new_source else tree(args.new)
            if args.output:
                out = Path(args.output)
            else:
                numbers = [
                    json.loads((s / "version.json").read_text())["protocol_version"]
                    for s in (old_source, new_source)
                ]
                out = data / f"{args.mode}-{numbers[0]}-{numbers[1]}.json"
            if args.mode == "remap":
                generate_remap(old_source, args.old, new_source, args.new, out)
            else:
                generate_metadata(old_source, args.old, new_source, args.new, out)
    except Fail as e:
        print(f"protogen: {e}", file=sys.stderr)
        return 1
    return 0

if __name__ == "__main__":
    sys.exit(main())
