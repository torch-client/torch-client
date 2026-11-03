use std::collections::HashMap;

use azalea::block::{BlockState, BlockTrait};
use azalea::physics::collision::BlockWithShape;

use crate::renderer::{BlockGeom, RenderedBlock, TintKind};
use crate::util::block_model::block_visual_in;

use super::point;

pub const FULLY_TRANSPARENT: u8 = 0;
pub const FULLY_OPAQUE: u8 = 16;
pub const MAX_FLUID_AMOUNT: u8 = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Material {
    Unknown = 0,
    Leaves = 1,
    Stone = 2,
    Wood = 3,
    Metal = 4,
    Dirt = 5,
    Lava = 6,
    Deepslate = 7,
    Snow = 8,
    Sand = 9,
    Terracotta = 10,
    NetherStone = 11,
    Water = 12,
    Grass = 13,
    Air = 14,
    Illuminated = 15,
}

#[derive(Clone, Copy, Debug)]
pub struct StateInfo {
    pub is_air: bool,
    pub is_solid: bool,
    pub is_liquid: bool,
    pub opacity: u8,
    pub material: Material,
    pub is_ice: bool,
    pub fluid_amount: u8,
    pub waterlogged: bool,
    pub ignored: bool,
    pub cave_ignored: bool,
    pub water_surface_replacement: bool,
    pub water_subsurface_replacement: bool,
    pub is_snow_layer: bool,
}

const IGNORED: &[&str] = &[
    "barrier",
    "structure_void",
    "light",
    "tripwire",
    "brown_mushroom",
];
const WATER_SUBSURFACE: &[&str] = &["kelp", "kelp_plant", "tall_seagrass", "seagrass"];
const WATER_SURFACE: &[&str] = &["lily_pad"];

const FLOWERS: &[&str] = &[
    "dandelion",
    "poppy",
    "blue_orchid",
    "allium",
    "azure_bluet",
    "red_tulip",
    "orange_tulip",
    "white_tulip",
    "pink_tulip",
    "oxeye_daisy",
    "cornflower",
    "lily_of_the_valley",
    "wither_rose",
    "torchflower",
    "open_eyeblossom",
    "closed_eyeblossom",
];

pub use crate::renderer::tile_colors::TileColors;
use crate::renderer::tile_colors::{ColorMode, tile_of_uv};

#[derive(Clone, Copy)]
struct BaseColor {
    argb: u32,
    tint: TintKind,
}

pub struct BlockTable {
    blocks: std::sync::Arc<crate::blocks::Blocks>,
    states: Vec<Option<(StateInfo, BaseColor)>>,
    colors: crate::lighting::FastMap<u32, u32>,
    water: Option<u16>,
    air: Option<[u16; 3]>,
}

impl BlockTable {
    pub fn new() -> Self {
        Self {
            blocks: crate::blocks::registry(),
            states: Vec::new(),
            colors: Default::default(),
            water: None,
            air: None,
        }
    }

    pub fn air_states(&mut self) -> [u16; 3] {
        if let Some(a) = self.air {
            return a;
        }
        let mut found = [0u16; 3];
        let mut n = 0;
        for id in 0..=u16::MAX {
            let Ok(state) = BlockState::try_from(id as u32) else {
                break;
            };
            if matches!(
                crate::blocks::block_name(state),
                "air" | "cave_air" | "void_air"
            ) {
                found[n] = id;
                n += 1;
                if n == 3 {
                    break;
                }
            }
        }
        self.air = Some(found);
        found
    }

    pub fn refresh(&mut self) -> bool {
        let blocks = crate::blocks::registry();
        if blocks.generation() == self.blocks.generation() {
            return false;
        }
        self.blocks = blocks;
        self.states.clear();
        self.colors.clear();
        true
    }

    fn resolve(&mut self, state: u16) -> &(StateInfo, BaseColor) {
        let i = state as usize;
        if i >= self.states.len() {
            self.states.resize(i + 1, None);
        }
        if self.states[i].is_none() {
            let resolved = resolve_state(state, &self.blocks);
            self.states[i] = Some(resolved);
        }
        self.states[i].as_ref().expect("resolved above")
    }

    pub fn info(&mut self, state: u16) -> StateInfo {
        self.resolve(state).0
    }

    pub fn color(&mut self, id: u32) -> u32 {
        if let Some(&c) = self.colors.get(&id) {
            return c;
        }
        let base = self.resolve(point::id_state(id)).1;
        let c = apply_tint(base, point::id_biome(id));
        self.colors.insert(id, c);
        c
    }

    pub fn water_state(&mut self) -> u16 {
        if let Some(w) = self.water {
            return w;
        }
        let mut found = 0u16;
        for id in 0..=u16::MAX {
            let Ok(state) = BlockState::try_from(id as u32) else {
                break;
            };
            let block = Box::<dyn BlockTrait>::from(state);
            if block.id() == "water" && block.property_map().get("level").is_some_and(|l| *l == "0")
            {
                found = id;
                break;
            }
        }
        self.water = Some(found);
        found
    }
}

impl Default for BlockTable {
    fn default() -> Self {
        Self::new()
    }
}

fn apply_tint(base: BaseColor, biome: u8) -> u32 {
    use crate::util::biome_color as bc;
    let ratio = match base.tint {
        TintKind::None => return base.argb,
        TintKind::Grass => bc::grass_ratio(biome),
        TintKind::Foliage => bc::foliage_ratio(biome),
        TintKind::DryFoliage => bc::dry_foliage_ratio(biome),
        TintKind::Water => bc::water_ratio(biome),
        TintKind::Redstone(power) => bc::redstone_ratio(power),
        TintKind::Stem(age) => bc::stem_ratio(age),
    };
    let ch = |shift: u32, k: f32| {
        ((((base.argb >> shift) & 0xFF) as f32 * k)
            .round()
            .clamp(0.0, 255.0)) as u32
    };
    (base.argb & 0xFF00_0000) | ch(16, ratio[0]) << 16 | ch(8, ratio[1]) << 8 | ch(0, ratio[2])
}

fn resolve_state(id: u16, blocks: &crate::blocks::Blocks) -> (StateInfo, BaseColor) {
    let Ok(state) = BlockState::try_from(id as u32) else {
        return (
            air_info(),
            BaseColor {
                argb: 0,
                tint: TintKind::None,
            },
        );
    };
    let block = Box::<dyn BlockTrait>::from(state);
    let name = block.id();
    let props = block.property_map();
    let is_air = matches!(name, "air" | "cave_air" | "void_air");
    if is_air {
        return (
            air_info(),
            BaseColor {
                argb: 0,
                tint: TintKind::None,
            },
        );
    }
    let rb = block_visual_in(state, blocks);

    let is_fluid_block = matches!(rb.geom, BlockGeom::Fluid { .. });
    let waterlogged = rb.waterlogged && !is_fluid_block && name != "bubble_column";
    let is_liquid = is_fluid_block || rb.waterlogged;
    let is_solid = !state.is_collision_shape_empty();
    let can_occlude = rb.is_solid;
    let skylight_down = crate::lighting::props::of(state).dampening == 0;
    let opacity = if is_liquid && !can_occlude {
        FULLY_TRANSPARENT + 1
    } else if skylight_down && !can_occlude {
        FULLY_TRANSPARENT
    } else {
        FULLY_OPAQUE
    };
    let fluid_amount = match rb.geom {
        BlockGeom::Fluid { amount, .. } => amount,
        _ if rb.waterlogged => MAX_FLUID_AMOUNT,
        _ => 0,
    };
    let is_snow_layer = name == "snow"
        && props
            .get("layers")
            .and_then(|l| l.parse::<u8>().ok())
            .is_some_and(|l| (1..=3).contains(&l));
    let info = StateInfo {
        is_air: false,
        is_solid,
        is_liquid,
        opacity,
        material: material_of(name, is_liquid, rb.emission),
        is_ice: name == "ice",
        fluid_amount,
        waterlogged,
        ignored: IGNORED.contains(&name),
        cave_ignored: false,
        water_surface_replacement: WATER_SURFACE.contains(&name),
        water_subsurface_replacement: WATER_SUBSURFACE.contains(&name),
        is_snow_layer,
    };
    (info, base_color(name, &props, &rb, blocks))
}

fn air_info() -> StateInfo {
    StateInfo {
        is_air: true,
        is_solid: false,
        is_liquid: false,
        opacity: FULLY_TRANSPARENT,
        material: Material::Air,
        is_ice: false,
        fluid_amount: 0,
        waterlogged: false,
        ignored: true,
        cave_ignored: true,
        water_surface_replacement: false,
        water_subsurface_replacement: false,
        is_snow_layer: false,
    }
}

fn material_of(name: &str, is_liquid: bool, emission: u8) -> Material {
    let has = |s: &str| name.contains(s);
    if name.ends_with("_leaves")
        || has("bamboo")
        || has("cactus")
        || has("chorus_flower")
        || has("mushroom")
    {
        return Material::Leaves;
    }
    if name == "lava" {
        return Material::Lava;
    }
    if is_liquid || name == "water" {
        return Material::Water;
    }
    let wood = [
        "planks",
        "_log",
        "_wood",
        "_stem",
        "_hyphae",
        "bookshelf",
        "_door",
        "_fence",
        "barrel",
        "chest",
    ];
    if wood.iter().any(|w| has(w)) || has("root") {
        return Material::Wood;
    }
    let metal = [
        "iron_block",
        "gold_block",
        "copper",
        "anvil",
        "chain",
        "lantern",
        "netherite_block",
        "hopper",
        "cauldron",
    ];
    if metal.iter().any(|m| has(m)) {
        return Material::Metal;
    }
    if has("grass_block") || has("grass_slab") {
        return Material::Grass;
    }
    if has("dirt") || has("gravel") || has("mud") || has("podzol") || has("mycelium") {
        return Material::Dirt;
    }
    if has("deepslate") {
        return Material::Deepslate;
    }
    if matches!(name, "netherrack" | "basalt" | "blackstone") {
        return Material::NetherStone;
    }
    if has("snow") {
        return Material::Snow;
    }
    if has("sand") {
        return Material::Sand;
    }
    if has("terracotta") {
        return Material::Terracotta;
    }
    if has("stone") || has("ore") {
        return Material::Stone;
    }
    if emission > 0 {
        return Material::Illuminated;
    }
    Material::Unknown
}

fn base_color(
    name: &str,
    props: &HashMap<&'static str, &'static str>,
    rb: &RenderedBlock,
    blocks: &crate::blocks::Blocks,
) -> BaseColor {
    let sprites = blocks.sprites();
    let colors = sprites.tile_colors();
    let rows = sprites.rows().max(1);
    let mode = if name.ends_with("_leaves") {
        ColorMode::Leaves
    } else if FLOWERS.contains(&name) {
        ColorMode::Flower
    } else if name.contains("glass") {
        ColorMode::Glass
    } else {
        ColorMode::Default
    };

    if let BlockGeom::Fluid { still, lava, .. } = rb.geom {
        let tint = if lava {
            TintKind::None
        } else {
            TintKind::Water
        };
        return BaseColor {
            argb: colors.get(still, mode),
            tint,
        };
    }

    let BlockGeom::Model(baked) = &rb.geom else {
        return BaseColor {
            argb: 0,
            tint: TintKind::None,
        };
    };
    let pillar = props.contains_key("axis") && !matches!(name, "nether_portal" | "creaking_heart");
    use crate::items::model::{DOWN, EAST, NORTH, SOUTH, UP, WEST};
    let order: [Option<usize>; 7] = [
        Some(UP),
        None,
        Some(NORTH),
        Some(EAST),
        Some(WEST),
        Some(SOUTH),
        Some(DOWN),
    ];
    let first_model = |slot: &crate::blocks::bake::BakedSlot| slot.choices.first().cloned();
    let models: Vec<_> = baked.parts.iter().filter_map(first_model).collect();
    for dir in order {
        if pillar && dir == Some(UP) {
            continue;
        }
        let quad = models
            .iter()
            .flat_map(|m| m.quads.iter())
            .find(|q| q.cull.map(|c| c as usize) == dir);
        if let Some(q) = quad {
            let tile = tile_of_uv(&q.uv, rows);
            if tile == 0 {
                break;
            }
            let tint = if q.tint >= 0 {
                rb.tint_kind
            } else {
                TintKind::None
            };
            return BaseColor {
                argb: colors.get(tile, mode),
                tint,
            };
        }
    }
    if let Some(uv) = models.iter().find_map(|m| m.particle_uv) {
        let centre = [[(uv[0] + uv[2]) / 2.0, (uv[1] + uv[3]) / 2.0]; 4];
        let tile = tile_of_uv(&centre, rows);
        if tile != 0 {
            return BaseColor {
                argb: colors.get(tile, mode),
                tint: TintKind::None,
            };
        }
    }
    BaseColor {
        argb: 0,
        tint: TintKind::None,
    }
}
