use std::collections::HashMap;

use azalea::block::BlockTrait;

use crate::blocks::Blocks;
use crate::blocks::rand::ShapeOffset;
use crate::renderer::{BlockGeom, RenderedBlock, TintKind};

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

pub fn block_visual(state: azalea::block::BlockState) -> RenderedBlock {
    block_visual_in(state, &crate::blocks::registry())
}

pub fn block_visual_in(state: azalea::block::BlockState, blocks: &Blocks) -> RenderedBlock {
    let tex = |name: &str| blocks.sprites().tile(name).unwrap_or(0);
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
    } else if name == "water_cauldron" {
        TintKind::Water
    } else if matches!(name, "melon_stem" | "pumpkin_stem") {
        let age = props.get("age").and_then(|v| v.parse().ok()).unwrap_or(0);
        TintKind::Stem(age)
    } else if matches!(name, "attached_melon_stem" | "attached_pumpkin_stem") {
        TintKind::Stem(7)
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
            #[cfg(feature = "shader_support")]
            state: state.id(),
            translucent: false,
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
                lava: is_lava,
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
            #[cfg(feature = "shader_support")]
            state: state.id(),
            translucent: false,
        };
    }

    let baked = blocks.baked(state.id(), name, &props);
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
        #[cfg(feature = "shader_support")]
        state: state.id(),
        translucent: translucent_layer(name),
    }
}

fn translucent_layer(name: &str) -> bool {
    matches!(
        name,
        "ice" | "frosted_ice" | "slime_block" | "honey_block" | "tinted_glass" | "nether_portal"
    ) || name.ends_with("_stained_glass")
        || name.ends_with("_stained_glass_pane")
}

pub fn is_solid(state: azalea::block::BlockState) -> bool {
    use std::sync::atomic::{AtomicU8, Ordering::Relaxed};
    const UNKNOWN: u8 = 0;
    const SOLID: u8 = 1;
    const OPEN: u8 = 2;
    const STATES: usize = azalea::block::BlockState::MAX_STATE as usize + 1;
    static CACHE: [AtomicU8; STATES] = [const { AtomicU8::new(UNKNOWN) }; STATES];

    let Some(slot) = CACHE.get(state.id() as usize) else {
        return block_visual(state).is_solid;
    };
    match slot.load(Relaxed) {
        SOLID => true,
        OPEN => false,
        _ => {
            let solid = block_visual(state).is_solid;
            slot.store(if solid { SOLID } else { OPEN }, Relaxed);
            solid
        }
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

#[cfg(feature = "shader_support")]
pub fn powder_snow_at(x: f64, y: f64, z: f64) -> bool {
    let Some(world) = crate::client::tracking::current_world()
        .lock()
        .unwrap()
        .clone()
    else {
        return false;
    };
    let world = crate::client::tracking::read_lock(&world, "powder_snow_at");
    let pos =
        azalea_core::position::BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
    world.get_block_state(pos).is_some_and(|state| {
        let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
        block.id() == "powder_snow"
    })
}

pub fn solid_at(pos: bevy::math::DVec3) -> bool {
    let Some(world) = crate::client::tracking::current_world()
        .lock()
        .unwrap()
        .clone()
    else {
        return false;
    };
    let pos = azalea_core::position::BlockPos::new(
        pos.x.floor() as i32,
        pos.y.floor() as i32,
        pos.z.floor() as i32,
    );
    let state = crate::client::tracking::read_lock(&world, "solid_at").get_block_state(pos);
    state.is_some_and(is_solid)
}

pub fn eye_fluid_at(x: f64, y: f64, z: f64) -> Option<EyeFluid> {
    let world = crate::client::tracking::current_world()
        .lock()
        .unwrap()
        .clone()?;
    let world = crate::client::tracking::read_lock(&world, "eye_fluid_at");
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

    let height = if same_above { 1.0 } else { amount as f32 / 9.0 };
    let frac_y = (y - pos.y as f64) as f32;
    crate::log_debug!(
        "fog",
        "eye {pos:?} frac_y={frac_y:.3} name={} amount={amount} height={height:.3} same_above={same_above}",
        block.id()
    );
    (frac_y < height).then_some(kind)
}
