#!/usr/bin/env python3
"""Generate Rust item metadata and creative-tab tables from vanilla Minecraft data.

    python3 gen_items.py [--assets PATH] [--source PATH] [--azalea PATH] [--output PATH]
    --assets <crate>/reference/minecraft-26.1.1/assets/minecraft
    --source <crate>/reference/minecraft-26.1.1/net/minecraft
    --azalea <crate>/vendor/azalea
    --output <crate>/src/generated_items.rs
    item id list      <assets>/items/*.json
    display names     <assets>/lang/en_us.json
    is_block          <assets>/blockstates/<id>.json
    max_stack         <azalea>/azalea-inventory/src/default_components/generated.rs
    max_damage        same
    rarity            <source>/world/item/Items.java  (.rarity(Rarity.X))
    rarity colours    <source>/world/item/Rarity.java + <source>/ChatFormatting.java
    creative tabs     <source>/world/item/CreativeModeTabs.java
"""

import argparse
import json
import os
import re
import sys
from pathlib import Path
from typing import Dict, List, Optional, Tuple

def load_item_ids(assets: Path) -> List[str]:
    items_dir = assets / 'items'
    ids = sorted(p.stem for p in items_dir.glob('*.json'))
    if not ids:
        print(f"Error: no item JSONs under {items_dir}", file=sys.stderr)
        sys.exit(1)
    return ids

def load_block_ids(assets: Path) -> set:
    return {p.stem for p in (assets / 'blockstates').glob('*.json')}

def title_case(item_id: str) -> str:
    return ' '.join(w.capitalize() for w in item_id.split('_'))

def load_names(assets: Path, item_ids: List[str]) -> Tuple[Dict[str, str], List[str]]:
    with open(assets / 'lang' / 'en_us.json') as f:
        lang = json.load(f)
    names: Dict[str, str] = {}
    fallbacks: List[str] = []
    for item_id in item_ids:
        name = lang.get(f'item.minecraft.{item_id}') or lang.get(f'block.minecraft.{item_id}')
        if name is None:
            name = title_case(item_id)
            fallbacks.append(item_id)
        names[item_id] = name
    return names, fallbacks

def camel_to_snake(name: str) -> str:
    out = []
    for i, ch in enumerate(name):
        if ch.isupper() and i > 0:
            out.append('_')
        elif ch.isdigit() and i > 0 and not name[i - 1].isdigit():
            out.append('_')
        out.append(ch.lower())
    return ''.join(out)

def load_item_kind_order(azalea: Path) -> List[str]:
    text = (azalea / 'azalea-registry' / 'src' / 'builtin.rs').read_text()
    start = text.index('enum ItemKind {')
    end = text.index('\n}', start)
    body = text[start:end]
    return re.findall(r'^\s*\w+ => "([a-z0-9_]+)",', body, re.M)

def load_stack_and_damage(azalea: Path, order: List[str]) -> Tuple[Dict[str, int], Dict[str, int]]:
    path = azalea / 'azalea-inventory' / 'src' / 'default_components' / 'generated.rs'
    text = path.read_text()

    m = re.search(r'static MAX_STACK_SIZE_VALUES: \[i32; (\d+)\] = \[([^\]]*)\];', text)
    if not m:
        print(f"Error: MAX_STACK_SIZE_VALUES not found in {path}", file=sys.stderr)
        sys.exit(1)
    values = [int(v) for v in m.group(2).split(',') if v.strip()]
    if len(values) != len(order):
        print(f"Error: MAX_STACK_SIZE_VALUES has {len(values)} entries but ItemKind "
              f"has {len(order)}", file=sys.stderr)
        sys.exit(1)
    max_stack = dict(zip(order, values))

    max_damage: Dict[str, int] = {}
    block = _rust_match_block(text, 'impl DefaultableComponent for MaxDamage {')
    for variant, value in re.findall(r'ItemKind::(\w+) => (\d+),', block):
        max_damage[camel_to_snake(variant)] = int(value)
    return max_stack, max_damage

def _rust_match_block(text: str, header: str) -> str:
    start = text.index(header)
    depth = 0
    for i in range(start + len(header) - 1, len(text)):
        if text[i] == '{':
            depth += 1
        elif text[i] == '}':
            depth -= 1
            if depth == 0:
                return text[start:i]
    return text[start:]

def load_azalea_rarity(azalea: Path) -> Dict[str, str]:
    path = azalea / 'azalea-inventory' / 'src' / 'default_components' / 'generated.rs'
    block = _rust_match_block(path.read_text(), 'impl DefaultableComponent for Rarity {')
    out = {}
    for variant, value in re.findall(r'ItemKind::(\w+) => Rarity::(\w+),', block):
        out[camel_to_snake(variant)] = value.lower()
    return out

def load_constant_ids(source: Path) -> Tuple[Dict[str, str], Dict[str, str]]:
    blocks_src = (source / 'world' / 'level' / 'block' / 'Blocks.java').read_text()
    blocks: Dict[str, str] = {}
    for const, item_id in re.findall(
            r'^\s{6}([A-Z][A-Z0-9_]*) = register\w*\("([a-z0-9_]+)"', blocks_src, re.M):
        blocks[const] = item_id

    items_src = (source / 'world' / 'item' / 'Items.java').read_text()
    items: Dict[str, str] = {}
    for const, item_id in re.findall(
            r'^\s{6}([A-Z][A-Z0-9_]*) = registerItem\("([a-z0-9_]+)"', items_src, re.M):
        items[const] = item_id
    for const, block_const in re.findall(
            r'^\s{6}([A-Z][A-Z0-9_]*) = registerBlock\(Blocks\.([A-Z][A-Z0-9_]*)', items_src, re.M):
        if block_const in blocks:
            items[const] = blocks[block_const]
    return blocks, items

RARITIES = ['common', 'uncommon', 'rare', 'epic']

def load_rarity_colors(source: Path) -> Dict[str, int]:
    rarity_src = (source / 'world' / 'item' / 'Rarity.java').read_text()
    fmt_of = {}
    for name, fmt in re.findall(r'(COMMON|UNCOMMON|RARE|EPIC)\(\d+, "\w+", ChatFormatting\.(\w+)\)',
                                rarity_src):
        fmt_of[name.lower()] = fmt

    fmt_src = (source / 'ChatFormatting.java').read_text()
    rgb_of = {}
    for fmt, rgb in re.findall(r'^\s*(\w+)\("\w+", \'.\', \d+, (\d+)\),', fmt_src, re.M):
        rgb_of[fmt] = int(rgb)

    colors = {}
    for rarity in RARITIES:
        fmt = fmt_of.get(rarity)
        if fmt is None or fmt not in rgb_of:
            print(f"Error: no ChatFormatting colour for rarity {rarity}", file=sys.stderr)
            sys.exit(1)
        colors[rarity] = rgb_of[fmt]
    return colors

def load_rarities(source: Path, item_ids: set,
                  items_map: Dict[str, str]) -> Tuple[Dict[str, str], List[str]]:
    text = (source / 'world' / 'item' / 'Items.java').read_text()
    rarities: Dict[str, str] = {}
    unresolved: List[str] = []

    current: Optional[str] = None
    for line in text.splitlines():
        m = re.match(r'^\s{6}([A-Z][A-Z0-9_]*) = ', line)
        if m:
            const = m.group(1)
            current = items_map.get(const, const.lower())
        r = re.search(r'\.rarity\(Rarity\.(COMMON|UNCOMMON|RARE|EPIC)\)', line)
        if r and current is not None:
            if current in item_ids:
                rarities[current] = r.group(1).lower()
            else:
                unresolved.append(current)
    return rarities, unresolved

SPECIAL_TABS = {'hotbar', 'search', 'inventory', 'op_blocks'}

WEATHER_PREFIX = {
    'unaffected': '',
    'exposed': 'exposed_',
    'weathered': 'weathered_',
    'oxidized': 'oxidized_',
    'waxed': 'waxed_',
    'waxedExposed': 'waxed_exposed_',
    'waxedWeathered': 'waxed_weathered_',
    'waxedOxidized': 'waxed_oxidized_',
}
WEATHER_ORDER = ['unaffected', 'exposed', 'weathered', 'oxidized',
                 'waxed', 'waxedExposed', 'waxedWeathered', 'waxedOxidized']

HELPER_BASE_ITEM = {
    'generateFireworksAllDurations': 'firework_rocket',
    'generateSuspiciousStews': 'suspicious_stew',
    'generateOminousBottles': 'ominous_bottle',
    'generateEnchantmentBookTypesOnlyMaxLevel': 'enchanted_book',
    'generateEnchantmentBookTypesAllLevels': 'enchanted_book',
    'generatePresetPaintings': 'painting',
}
HELPER_ITEM_ARG = {'generatePotionEffectTypes', 'generateInstrumentTypes'}

class Tab:
    def __init__(self, tab_id: str, title_key: str, icon: str, row: str, column: int):
        self.id = tab_id
        self.title_key = title_key
        self.icon = icon
        self.row = row
        self.column = column
        self.items: List[str] = []

    def add(self, item_id: str):
        if item_id not in self.items:
            self.items.append(item_id)

def parse_creative_tabs(source: Path, item_ids: set, blocks_map: Dict[str, str],
                        items_map: Dict[str, str]) -> Tuple[List[Tab], List[str]]:
    path = source / 'world' / 'item' / 'CreativeModeTabs.java'
    lines = path.read_text().splitlines()
    warnings: List[str] = []

    def resolve(holder: str, const: str) -> str:
        table = blocks_map if holder == 'Blocks' else items_map
        return table.get(const, const.lower())

    key_of_const = dict(re.findall(
        r'ResourceKey<CreativeModeTab> (\w+) = createKey\("(\w+)"\);', '\n'.join(lines)))

    header_re = re.compile(
        r'Registry\.register\(registry, \(ResourceKey\)(\w+), '
        r'CreativeModeTab\.builder\(CreativeModeTab\.Row\.(TOP|BOTTOM), (\d+)\)'
        r'\.title\(Component\.translatable\("([\w.]+)"\)\)\.icon\(')

    tabs: List[Tab] = []
    current: Optional[Tab] = None
    pending_weathering: Optional[str] = None

    for raw in lines:
        line = raw.strip()

        m = header_re.search(line)
        if m:
            const, row, column, title_key = m.groups()
            tab_id = key_of_const.get(const)
            if tab_id is None:
                warnings.append(f"unknown tab constant {const}")
                current = None
                continue
            current = Tab(tab_id, title_key, '', row.lower(), int(column))
            tabs.append(current)
            continue

        if current is None:
            continue

        if not current.icon:
            m = re.search(r'return new ItemStack\((Items|Blocks)\.([A-Z][A-Z0-9_]*)\);', line)
            if m:
                current.icon = resolve(*m.groups())
                continue

        m = re.match(r'(?:WeatheringCopperItems )?var\d+ = Items\.([A-Z][A-Z0-9_]*);$', line)
        if m:
            pending_weathering = m.group(1).lower()
            continue
        if re.match(r'var\d+\.forEach\(', line):
            if pending_weathering is None:
                warnings.append(f"forEach with no pending WeatheringCopperItems in {current.id}")
            else:
                for state in WEATHER_ORDER:
                    current.add(WEATHER_PREFIX[state] + pending_weathering)
                pending_weathering = None
            continue

        m = re.search(r'\.accept\(\(ItemLike\)(?:Items|Blocks)\.([A-Z][A-Z0-9_]*)\.(\w+)\(\)\)', line)
        if m:
            base, state = m.groups()
            if state not in WEATHER_PREFIX:
                warnings.append(f"unknown weathering accessor {base}.{state}() in {current.id}")
            else:
                current.add(WEATHER_PREFIX[state] + base.lower())
            continue

        m = re.search(r'\.accept\(\(ItemLike\)(Items|Blocks)\.([A-Z][A-Z0-9_]*)\)', line)
        if m:
            current.add(resolve(*m.groups()))
            continue

        if 'Raid.getOminousBannerInstance' in line:
            current.add('white_banner')
            continue

        m = re.search(r'\w+\.set\w+OnStack\(new ItemStack\((Items|Blocks)\.([A-Z][A-Z0-9_]*)\)', line)
        if m:
            current.add(resolve(*m.groups()))
            continue

        m = re.match(r'(generate\w+)\(', line)
        if m:
            helper = m.group(1)
            if helper in HELPER_BASE_ITEM:
                current.add(HELPER_BASE_ITEM[helper])
            elif helper in HELPER_ITEM_ARG:
                a = re.search(r'(Items|Blocks)\.([A-Z][A-Z0-9_]*)', line)
                if a:
                    current.add(resolve(*a.groups()))
                else:
                    warnings.append(f"no Items argument for {helper} in {current.id}")
            else:
                warnings.append(f"unhandled helper {helper} in {current.id}")
            continue

    for tab in tabs:
        if not tab.icon:
            warnings.append(f"tab {tab.id} has no icon")
        for item_id in tab.items:
            if item_id not in item_ids:
                warnings.append(f"tab {tab.id} references unknown item {item_id}")
        tab.items = [i for i in tab.items if i in item_ids]

    return tabs, warnings

def rust_str(s: str) -> str:
    return '"' + s.replace('\\', '\\\\').replace('"', '\\"') + '"'

def generate_rust(defs: List[dict], tabs: List[Tab], titles: Dict[str, str],
                  colors: Dict[str, int]) -> str:
    out: List[str] = []
    out.append("// @generated by tools/gen_items.py — do not edit\n")
    out.append("//\n")
    out.append("// Item metadata and creative-tab contents extracted from the vanilla\n")
    out.append("// assets and Java source under reference/minecraft-26.1.1/.\n")
    out.append("// Regenerate with `python3 tools/gen_items.py`.\n")
    out.append("#![allow(dead_code)]\n\n")

    out.append("#[derive(Clone, Copy, PartialEq, Eq, Debug)]\n")
    out.append("pub enum Rarity {\n")
    for r in RARITIES:
        out.append(f"    {r.capitalize()},\n")
    out.append("}\n\n")

    out.append("impl Rarity {\n")
    out.append("    /// 0xRRGGBB, from the ChatFormatting each rarity names.\n")
    out.append("    pub fn color(self) -> u32 {\n")
    out.append("        match self {\n")
    for r in RARITIES:
        out.append(f"            Rarity::{r.capitalize()} => 0x{colors[r]:06X},\n")
    out.append("        }\n")
    out.append("    }\n")
    out.append("}\n\n")

    out.append("pub struct ItemDef {\n")
    out.append("    /// Registry id without namespace, e.g. \"diamond_sword\".\n")
    out.append("    pub id: &'static str,\n")
    out.append("    /// Localised display name from en_us.json, e.g. \"Diamond Sword\".\n")
    out.append("    pub name: &'static str,\n")
    out.append("    pub max_stack: u8,\n")
    out.append("    /// 0 = not damageable.\n")
    out.append("    pub max_damage: u16,\n")
    out.append("    pub rarity: Rarity,\n")
    out.append("    /// True if the item places a block (has a matching blockstate).\n")
    out.append("    pub is_block: bool,\n")
    out.append("}\n\n")

    out.append("/// Sorted by `id`.\n")
    out.append("pub static ITEMS: &[ItemDef] = &[\n")
    for d in defs:
        out.append(
            "    ItemDef { id: %s, name: %s, max_stack: %d, max_damage: %d, "
            "rarity: Rarity::%s, is_block: %s },\n"
            % (rust_str(d['id']), rust_str(d['name']), d['max_stack'], d['max_damage'],
               d['rarity'].capitalize(), 'true' if d['is_block'] else 'false'))
    out.append("];\n\n")

    out.append("pub fn item(id: &str) -> Option<&'static ItemDef> {\n")
    out.append("    ITEMS\n")
    out.append("        .binary_search_by_key(&id, |d| d.id)\n")
    out.append("        .ok()\n")
    out.append("        .map(|i| &ITEMS[i])\n")
    out.append("}\n\n")

    out.append("#[derive(Clone, Copy, PartialEq, Eq, Debug)]\n")
    out.append("pub enum TabRow {\n")
    out.append("    Top,\n")
    out.append("    Bottom,\n")
    out.append("}\n\n")

    out.append("pub struct CreativeTab {\n")
    out.append("    /// e.g. \"building_blocks\"\n")
    out.append("    pub id: &'static str,\n")
    out.append("    /// Localised tab title from en_us.json, e.g. \"Building Blocks\".\n")
    out.append("    pub title: &'static str,\n")
    out.append("    /// Item id of the tab icon.\n")
    out.append("    pub icon: &'static str,\n")
    out.append("    pub row: TabRow,\n")
    out.append("    /// 0-based column within the row.\n")
    out.append("    pub column: u8,\n")
    out.append("    /// Item ids in vanilla creative order.\n")
    out.append("    pub items: &'static [&'static str],\n")
    out.append("}\n\n")

    for tab in tabs:
        out.append(f"static TAB_{tab.id.upper()}: &[&str] = &[\n")
        for item_id in tab.items:
            out.append(f"    {rust_str(item_id)},\n")
        out.append("];\n\n")

    out.append("/// In vanilla display order. Does NOT include the special `hotbar`,\n")
    out.append("/// `inventory`, `search` or `op_blocks` tabs; the screen adds those itself.\n")
    out.append("pub static CREATIVE_TABS: &[CreativeTab] = &[\n")
    for tab in tabs:
        out.append("    CreativeTab {\n")
        out.append(f"        id: {rust_str(tab.id)},\n")
        out.append(f"        title: {rust_str(titles[tab.id])},\n")
        out.append(f"        icon: {rust_str(tab.icon)},\n")
        out.append(f"        row: TabRow::{'Top' if tab.row == 'top' else 'Bottom'},\n")
        out.append(f"        column: {tab.column},\n")
        out.append(f"        items: TAB_{tab.id.upper()},\n")
        out.append("    },\n")
    out.append("];\n")

    out.append(TESTS % (len(defs), len(tabs)))
    return ''.join(out)

TESTS = """
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_sorted_and_complete() {
        assert_eq!(ITEMS.len(), %d);
        for pair in ITEMS.windows(2) {
            assert!(pair[0].id < pair[1].id, "unsorted at {}", pair[0].id);
        }
    }

    #[test]
    fn known_items() {
        let sword = item("diamond_sword").unwrap();
        assert_eq!(sword.max_stack, 1);
        assert!(sword.max_damage > 0);
        assert_eq!(sword.name, "Diamond Sword");
        assert!(!sword.is_block);

        let stone = item("stone").unwrap();
        assert_eq!(stone.max_stack, 64);
        assert_eq!(stone.max_damage, 0);
        assert!(stone.is_block);

        // Vanilla 26.1.1 gives plain golden apples Common rarity; only the
        // enchanted variant carries `.rarity(Rarity.RARE)` in Items.java.
        assert_eq!(item("golden_apple").unwrap().rarity, Rarity::Common);
        assert_eq!(item("enchanted_golden_apple").unwrap().rarity, Rarity::Rare);
        assert_eq!(item("totem_of_undying").unwrap().rarity, Rarity::Uncommon);
        assert_eq!(item("dragon_egg").unwrap().rarity, Rarity::Epic);

        assert!(item("not_a_real_item").is_none());
    }

    #[test]
    fn rarity_colors() {
        assert_eq!(Rarity::Common.color(), 0xFFFFFF);
        assert_eq!(Rarity::Uncommon.color(), 0xFFFF55);
        assert_eq!(Rarity::Rare.color(), 0x55FFFF);
        assert_eq!(Rarity::Epic.color(), 0xFF55FF);
    }

    #[test]
    fn tabs_well_formed() {
        assert_eq!(CREATIVE_TABS.len(), %d);
        let building = CREATIVE_TABS
            .iter()
            .find(|t| t.id == "building_blocks")
            .unwrap();
        assert_eq!(building.title, "Building Blocks");
        assert_eq!(building.row, TabRow::Top);
        assert_eq!(building.column, 0);
        assert_eq!(
            &building.items[..4],
            &["oak_log", "oak_wood", "stripped_oak_log", "stripped_oak_wood"]
        );

        for tab in CREATIVE_TABS {
            assert!(item(tab.icon).is_some(), "bad icon {}", tab.icon);
            assert!(!tab.items.is_empty(), "empty tab {}", tab.id);
            for id in tab.items {
                assert!(item(id).is_some(), "tab {} has unknown item {}", tab.id, id);
            }
        }
    }
}
"""

CRATE_ROOT = Path(__file__).resolve().parent.parent
ASSETS_ROOT = Path(os.environ.get(
    'MINECRAFT_ASSETS',
    CRATE_ROOT / 'reference' / 'minecraft-26.1.1' / 'assets' / 'minecraft'))
SOURCE_ROOT = CRATE_ROOT / 'reference' / 'minecraft-26.1.1' / 'net' / 'minecraft'

EXPECTED_UNCOVERED = {
    'air', 'barrier', 'chain_command_block', 'command_block', 'command_block_minecart',
    'debug_stick', 'ender_dragon_spawn_egg', 'filled_map', 'jigsaw', 'knowledge_book',
    'light', 'petrified_oak_slab', 'repeating_command_block', 'structure_block',
    'structure_void', 'test_block', 'test_instance_block', 'wither_spawn_egg',
    'written_book',
}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--assets', default=str(ASSETS_ROOT),
                        help='Path to the assets/minecraft namespace root')
    parser.add_argument('--source', default=str(SOURCE_ROOT),
                        help='Path to the net/minecraft Java source root')
    parser.add_argument('--azalea', default=str(CRATE_ROOT / 'vendor' / 'azalea'),
                        help='Path to the vendored azalea checkout')
    parser.add_argument('--output', default=str(CRATE_ROOT / 'src' / 'generated_items.rs'),
                        help='Output Rust file path')
    args = parser.parse_args()

    assets = Path(args.assets)
    source = Path(args.source)
    azalea = Path(args.azalea)
    output_path = Path(args.output)

    for path, what in ((assets, 'assets'), (source, 'Java source'), (azalea, 'azalea')):
        if not path.exists():
            print(f"Error: {what} directory not found: {path}", file=sys.stderr)
            sys.exit(1)

    item_ids = load_item_ids(assets)
    id_set = set(item_ids)
    block_ids = load_block_ids(assets)
    names, name_fallbacks = load_names(assets, item_ids)

    kind_order = load_item_kind_order(azalea)
    max_stack, max_damage = load_stack_and_damage(azalea, kind_order)
    missing_stack = [i for i in item_ids if i not in max_stack]

    blocks_map, items_map = load_constant_ids(source)
    rarities, unresolved_rarity = load_rarities(source, id_set, items_map)
    colors = load_rarity_colors(source)
    azalea_rarity = load_azalea_rarity(azalea)
    rarity_mismatches = sorted(
        i for i in item_ids
        if rarities.get(i, 'common') != azalea_rarity.get(i, 'common'))

    defs = []
    for item_id in item_ids:
        defs.append({
            'id': item_id,
            'name': names[item_id],
            'max_stack': max_stack.get(item_id, 64),
            'max_damage': max_damage.get(item_id, 0),
            'rarity': rarities.get(item_id, 'common'),
            'is_block': item_id in block_ids,
        })

    all_tabs, warnings = parse_creative_tabs(source, id_set, blocks_map, items_map)
    tabs = [t for t in all_tabs if t.id not in SPECIAL_TABS]

    with open(assets / 'lang' / 'en_us.json') as f:
        lang = json.load(f)
    titles = {}
    for tab in tabs:
        title = lang.get(tab.title_key)
        if title is None:
            warnings.append(f"no translation for {tab.title_key}")
            title = tab.id.replace('_', ' ').title()
        titles[tab.id] = title

    covered = set()
    for tab in tabs:
        covered.update(tab.items)
    uncovered = sorted(id_set - covered)

    code = generate_rust(defs, tabs, titles, colors)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, 'w') as f:
        f.write(code)

    print('=' * 70)
    print('ITEM METADATA GENERATION REPORT')
    print('=' * 70)
    print(f"\nItems:              {len(defs)}")
    print(f"Creative tabs:      {len(tabs)} (of {len(all_tabs)} parsed; "
          f"{len(all_tabs) - len(tabs)} special tabs dropped)")
    print(f"Name fallbacks:     {len(name_fallbacks)}")
    if name_fallbacks:
        for i in name_fallbacks:
            print(f"  • {i}")
    print(f"max_stack source:   {azalea}/azalea-inventory/.../generated.rs "
          f"(missing: {len(missing_stack)})")
    if missing_stack:
        for i in missing_stack:
            print(f"  • {i}")
    print(f"Rarity overrides:   {len(rarities)} from Items.java "
          f"({len(unresolved_rarity)} constants not registry ids)")
    if unresolved_rarity:
        for i in sorted(set(unresolved_rarity)):
            print(f"  • {i}")
    print(f"Rarity vs azalea:   {len(rarity_mismatches)} mismatches")
    for i in rarity_mismatches:
        print(f"  • {i}: Items.java={rarities.get(i, 'common')} "
              f"azalea={azalea_rarity.get(i, 'common')}")

    print(f"\nTab contents:")
    for tab in tabs:
        print(f"  {tab.id:<20} row={tab.row:<6} col={tab.column} "
              f"icon={tab.icon:<20} items={len(tab.items)}")

    print(f"\nCoverage: {len(covered)}/{len(item_ids)} item ids appear in at least one tab")
    print(f"Uncovered ({len(uncovered)}):")
    for i in uncovered:
        print(f"  • {i}")
    unexpected = sorted(set(uncovered) - EXPECTED_UNCOVERED)
    if unexpected:
        print(f"\nUNEXPECTED uncovered ids ({len(unexpected)}) — parser may be incomplete:")
        for i in unexpected:
            print(f"  • {i}")

    if warnings:
        print(f"\nWarnings ({len(warnings)}):")
        for w in warnings:
            print(f"  • {w}")

    size = output_path.stat().st_size
    lines = code.count('\n')
    print(f"\nOutput: {output_path}")
    print(f"        {size} bytes, {lines} lines")

if __name__ == '__main__':
    main()
