use std::collections::HashMap;

use azalea::block::BlockTrait;

use crate::TEXTURE_MAP;
use crate::blocks::rand::ShapeOffset;
use crate::renderer::{BlockGeom, RenderedBlock, TintKind};

fn tex(name: &str) -> u32 {
    TEXTURE_MAP
        .get()
        .and_then(|m| m.get(name).copied())
        .unwrap_or(0)
}

fn bool_prop(props: &HashMap<&str, &str>, key: &str) -> bool {
    props.get(key).map(|v| *v == "true").unwrap_or(false)
}

const IMPLICIT_WATER: &[&str] = &["kelp", "kelp_plant", "seagrass", "tall_seagrass"];

#[cfg(feature = "builtin_shaders")]
const NON_SWAYING_CUTOUTS: &[&str] = &[
    "cauldron",
    "water_cauldron",
    "lava_cauldron",
    "powder_snow_cauldron",
    "hopper",
    "comparator",
    "repeater",
    "redstone_wire",
    "lever",
    "ladder",
    "rail",
    "powered_rail",
    "detector_rail",
    "activator_rail",
    "tripwire",
    "tripwire_hook",
    "iron_bars",
    "glass_pane",
    "fire",
    "soul_fire",
    "resin_clump",
    "sculk_vein",
    "glow_lichen",
    "frogspawn",
    "sugar_cane",
];

const SKIP_SAME: &[&str] = &[
    "glass",
    "tinted_glass",
    "ice",
    "blue_ice",
    "frosted_ice",
    "honey_block",
    "slime_block",
    "powder_snow",
];

pub const LEAVES_CULL_GROUP: u32 = 1;

fn cull_group(name: &str, block: &dyn BlockTrait) -> u32 {
    if name.ends_with("_leaves") {
        return LEAVES_CULL_GROUP;
    }
    if SKIP_SAME.contains(&name)
        || name.ends_with("_stained_glass")
        || name.ends_with("copper_grate")
    {
        return block.as_block_kind() as u32 + 2;
    }
    0
}

pub fn emissive_rendering(name: &str, props: &HashMap<&str, &str>) -> bool {
    match name {
        "magma_block" => true,
        "sculk_sensor" | "calibrated_sculk_sensor" => {
            props.get("sculk_sensor_phase").copied() == Some("active")
        }
        _ => false,
    }
}

#[cfg(feature = "audio")]
pub struct SoundGroup {
    pub break_event: &'static str,
    pub place_event: &'static str,
    pub step_event: &'static str,
}

#[cfg(feature = "audio")]
macro_rules! sound_group {
    ($name:literal) => {
        SoundGroup {
            break_event: concat!("block.", $name, ".break"),
            place_event: concat!("block.", $name, ".place"),
            step_event: concat!("block.", $name, ".step"),
        }
    };
}

#[cfg(feature = "audio")]
static WOOD: SoundGroup = sound_group!("wood");
#[cfg(feature = "audio")]
static SAND: SoundGroup = sound_group!("sand");
#[cfg(feature = "audio")]
static GRAVEL: SoundGroup = sound_group!("gravel");
#[cfg(feature = "audio")]
static GRASS: SoundGroup = sound_group!("grass");
#[cfg(feature = "audio")]
static GLASS: SoundGroup = sound_group!("glass");
#[cfg(feature = "audio")]
static METAL: SoundGroup = sound_group!("metal");
#[cfg(feature = "audio")]
static WOOL: SoundGroup = sound_group!("wool");
#[cfg(feature = "audio")]
static STONE: SoundGroup = sound_group!("stone");

#[cfg(feature = "audio")]
pub fn sound_group(name: &str) -> &'static SoundGroup {
    if name.ends_with("_planks")
        || name.contains("_log")
        || name.contains("_wood")
        || name.ends_with("_fence")
        || name.ends_with("_fence_gate")
        || name.ends_with("_door")
        || name.ends_with("_trapdoor")
        || name.ends_with("_button")
        || name.ends_with("_pressure_plate")
        || name.contains("bamboo")
        || name.contains("chest")
    {
        &WOOD
    } else if name == "sand" || name == "red_sand" || name.contains("sandstone") {
        &SAND
    } else if name == "gravel" {
        &GRAVEL
    } else if name.contains("grass") || name.contains("dirt") || name.contains("farmland") {
        &GRASS
    } else if name.contains("glass") {
        &GLASS
    } else if name.contains("iron")
        || name.contains("copper")
        || name.contains("gold")
        || name.contains("anvil")
        || name.contains("chain")
    {
        &METAL
    } else if name.ends_with("_wool") || name.ends_with("_carpet") {
        &WOOL
    } else {
        &STONE
    }
}

#[cfg(all(test, feature = "audio"))]
mod sound_group_tests {
    use super::*;

    #[test]
    fn a_family_names_its_three_events() {
        assert_eq!(sound_group("oak_planks").place_event, "block.wood.place");
        assert_eq!(sound_group("oak_planks").break_event, "block.wood.break");
        assert_eq!(sound_group("oak_planks").step_event, "block.wood.step");
    }

    #[test]
    fn an_unrecognised_block_falls_back_to_stone() {
        assert_eq!(sound_group("obsidian").break_event, "block.stone.break");
        assert_eq!(sound_group("").break_event, "block.stone.break");
    }
}

pub fn block_visual(state: azalea::block::BlockState) -> RenderedBlock {
    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    let name = block.id();
    let props = block.property_map();
    let cull_group = cull_group(name, block.as_ref());
    let tint_kind = if crate::util::biome_color::GRASS_TINTED_BLOCKS.contains(&name) {
        TintKind::Grass
    } else if name == "vine" {
        TintKind::Foliage
    } else if name == "leaf_litter" {
        TintKind::DryFoliage
    } else if name == "redstone_wire" {
        let power: u8 = props.get("power").and_then(|v| v.parse().ok()).unwrap_or(0);
        TintKind::Redstone(power)
    } else if matches!(
        name,
        "spruce_leaves"
            | "birch_leaves"
            | "pale_oak_leaves"
            | "cherry_leaves"
            | "azalea_leaves"
            | "flowering_azalea_leaves"
            | "lily_pad"
    ) {
        TintKind::None
    } else if name.ends_with("_leaves") {
        TintKind::Foliage
    } else {
        TintKind::None
    };

    #[cfg(feature = "builtin_shaders")]
    let sways = |ambient_occlusion: bool| -> bool {
        !ambient_occlusion && !NON_SWAYING_CUTOUTS.contains(&name) && !name.ends_with("_pane")
    };

    if matches!(name, "air" | "cave_air" | "void_air") {
        return RenderedBlock {
            geom: BlockGeom::Model(Default::default()),
            is_solid: false,
            waterlogged: false,
            full_cube: false,
            emission: 0,
            emissive: false,
            cull_group,
            tint_kind,
            offset: ShapeOffset::None,
            #[cfg(feature = "builtin_shaders")]
            sways: false,
        };
    }

    if matches!(name, "water" | "lava" | "bubble_column") {
        let is_lava = name == "lava";
        let still = tex(if is_lava { "lava_still" } else { "water_still" });
        let flow = tex(if is_lava { "lava_flow" } else { "water_flow" });
        let level: u32 = props.get("level").and_then(|v| v.parse().ok()).unwrap_or(0);
        let amount: u8 = if name == "bubble_column" || level == 0 || level >= 8 {
            8
        } else {
            (8 - level) as u8
        };
        let light = crate::lighting::props::of(state);
        return RenderedBlock {
            geom: BlockGeom::Fluid {
                amount,
                still,
                flow,
            },
            is_solid: false,
            waterlogged: false,
            full_cube: false,
            emission: light.emission,
            emissive: false,
            cull_group,
            tint_kind: TintKind::None,
            offset: ShapeOffset::None,
            #[cfg(feature = "builtin_shaders")]
            sways: false,
        };
    }

    let baked = crate::blocks::registry().baked(state.id(), name, &props);
    let is_solid = baked.is_solid;
    let full_cube = baked.full_cube;
    #[cfg(feature = "builtin_shaders")]
    let sways = sways(baked.ambient_occlusion);
    let light = crate::lighting::props::of(state);
    RenderedBlock {
        geom: BlockGeom::Model(baked),
        is_solid,
        full_cube,
        emission: light.emission,
        emissive: emissive_rendering(name, &props),
        waterlogged: !is_solid
            && (IMPLICIT_WATER.contains(&name) || bool_prop(&props, "waterlogged")),
        cull_group,
        tint_kind,
        offset: ShapeOffset::for_block(name),
        #[cfg(feature = "builtin_shaders")]
        sways,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EyeFluid {
    Water,
    Lava,
}

fn fluid_at(block: &dyn BlockTrait) -> Option<(EyeFluid, u8)> {
    let name = block.id();
    let level_amount = || {
        let level: u32 = block
            .get_property("level")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if level == 0 || level >= 8 {
            8
        } else {
            (8 - level) as u8
        }
    };
    match name {
        "water" | "bubble_column" => Some((
            EyeFluid::Water,
            if name == "bubble_column" {
                8
            } else {
                level_amount()
            },
        )),
        "lava" => Some((EyeFluid::Lava, level_amount())),
        "kelp" | "kelp_plant" | "seagrass" | "tall_seagrass" => Some((EyeFluid::Water, 8)),
        _ if block.get_property("waterlogged") == Some("true") => Some((EyeFluid::Water, 8)),
        _ => None,
    }
}

pub fn eye_fluid_at(x: f64, y: f64, z: f64) -> Option<EyeFluid> {
    let world = crate::client::tracking::current_world()
        .lock()
        .unwrap()
        .clone()?;
    let world = world.read();
    let pos =
        azalea_core::position::BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
    let state = world.get_block_state(pos)?;
    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    let (kind, amount) = fluid_at(block.as_ref())?;

    let same_above = world
        .get_block_state(pos.up(1))
        .and_then(|s| {
            let above: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(s);
            fluid_at(above.as_ref())
        })
        .is_some_and(|(above_kind, _)| above_kind == kind);

    let height = if amount == 8 || same_above {
        1.0
    } else {
        amount as f32 / 9.0
    };
    let frac_y = (y - pos.y as f64) as f32;
    crate::log_debug!(
        "fog",
        "eye {pos:?} frac_y={frac_y:.3} name={} amount={amount} height={height:.3} same_above={same_above}",
        block.id()
    );
    (frac_y < height).then_some(kind)
}
