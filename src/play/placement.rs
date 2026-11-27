mod attach;
mod circuit;
mod pairs;

use std::{
    collections::HashSet,
    str::FromStr,
    sync::{LazyLock, OnceLock},
};

use crate::blocks::facing::{
    clockwise, counter_clockwise, from_name as direction_from_name, name as direction_name,
};
use crate::play::redstone::has_neighbor_signal;
use azalea::block::{BlockState, BlockTrait};
use azalea::physics::collision::{
    BlockWithShape,
    sturdy::{can_support_center, is_face_sturdy, is_solid},
};
use azalea_core::{
    aabb::Aabb,
    direction::Direction,
    hit_result::BlockHitResult,
    position::{BlockPos, Vec3},
};
use azalea_inventory::ItemStack;
use azalea_registry::tags::blocks::{CANDLES, PRESSURE_PLATES, RAILS, WOOL_CARPETS};
use azalea_registry::{
    Registry,
    builtin::{BlockEntityKind, BlockKind, ItemKind},
    tags::blocks::{
        COPPER_CHESTS, MAINTAINS_FARMLAND, PREVENTS_NEARBY_LEAF_DECAY, REPLACEABLE, SNOW,
        SUPPORTS_BAMBOO, WALLS,
    },
};

const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

const INTERACTION_PROPERTIES: [&str; 11] = [
    "open",
    "powered",
    "lit",
    "level",
    "note",
    "has_record",
    "has_book",
    "bites",
    "charges",
    "candles",
    "occupied",
];

static BLOCK_ENTITY_IDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    (0..)
        .map_while(BlockEntityKind::from_u32)
        .map(|kind| kind.to_str())
        .collect()
});

const MENU_BLOCK_IDS: [&str; 9] = [
    "crafting_table",
    "cartography_table",
    "loom",
    "stonecutter",
    "grindstone",
    "smithing_table",
    "anvil",
    "chipped_anvil",
    "damaged_anvil",
];

#[allow(clippy::too_many_arguments)]
pub fn predict(
    held: &ItemStack,
    hit: &BlockHitResult,
    yaw: f32,
    pitch: f32,
    secondary_use: bool,
    min_y: i32,
    height: u32,
    block_at: impl Fn(BlockPos) -> Option<BlockState>,
) -> Option<(BlockPos, BlockState)> {
    let placed = predict_placed(
        held,
        hit,
        yaw,
        pitch,
        secondary_use,
        min_y,
        height,
        block_at,
    )?;
    placed
        .partner
        .is_none()
        .then_some((placed.pos, placed.state))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub pos: BlockPos,
    pub state: BlockState,
    pub partner: Option<(BlockPos, BlockState)>,
}

#[allow(clippy::too_many_arguments)]
pub fn predict_placed(
    held: &ItemStack,
    hit: &BlockHitResult,
    yaw: f32,
    pitch: f32,
    secondary_use: bool,
    min_y: i32,
    height: u32,
    block_at: impl Fn(BlockPos) -> Option<BlockState>,
) -> Option<Placed> {
    let ItemStack::Present(item) = held else {
        return None;
    };

    let item_id = item.known_kind()?.to_str().trim_start_matches("minecraft:");
    let block = crate::items::placed::placed_block(item_id)?;
    if block.to_str().trim_start_matches("minecraft:") != item_id
        && !matches!(
            block,
            BlockKind::RedstoneWire | BlockKind::Tripwire | BlockKind::Cocoa
        )
    {
        return None;
    }

    let clicked_state = block_at(hit.block_pos)?;

    let (pos, replacing_clicked) = if replaceable_by(clicked_state, block, hit, true)? {
        (hit.block_pos, true)
    } else {
        if !secondary_use && has_use_interaction(clicked_state) {
            return None;
        }
        (hit.block_pos + hit.direction.normal(), false)
    };

    if pos.y < min_y || pos.y >= min_y + height as i32 {
        return None;
    }

    let replaced_state = block_at(pos)?;
    if !replacing_clicked && !replaceable_by(replaced_state, block, hit, false)? {
        return None;
    }

    let (pos, replaced_state, replacing_clicked) = if block == BlockKind::Scaffolding {
        let moved =
            scaffolding_placement_pos(pos, replaced_state, hit, yaw, secondary_use, &block_at)?;
        if moved.y < min_y || moved.y >= min_y + height as i32 {
            return None;
        }
        (moved, block_at(moved)?, replacing_clicked || moved != pos)
    } else {
        (pos, replaced_state, replacing_clicked)
    };

    let attachment = attachment(block);
    let state = state_for_placement(
        block,
        attachment,
        hit,
        yaw,
        pitch,
        replaced_state,
        pos,
        replacing_clicked,
        secondary_use,
        &block_at,
    )?;

    if BlockState::from(block).is_collision_shape_empty() && attachment == Attachment::None {
        return None;
    }
    if !survives(state, pos, &block_at) {
        return None;
    }

    let partner = match attachment {
        Attachment::Door | Attachment::Bed => {
            let (other, other_state) = pairs::partner(state, pos)?;
            if other.y < min_y || other.y >= min_y + height as i32 {
                return None;
            }
            Some((other, other_state))
        }
        _ if accepts(block, "half", "upper") || accepts(block, "part", "head") => return None,
        _ => None,
    };

    Some(Placed {
        pos,
        state,
        partner,
    })
}

fn survives(
    state: BlockState,
    pos: BlockPos,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> bool {
    let kind = state.as_block_kind();
    let id = kind.to_str();
    let below = || block_at(pos.down(1)).unwrap_or(BlockState::AIR);
    if id.ends_with("amethyst_bud") || id.ends_with("amethyst_cluster") {
        let Some(facing) = Box::<dyn BlockTrait>::from(state)
            .get_property("facing")
            .and_then(direction_from_name)
        else {
            return false;
        };
        return block_at(pos + facing.opposite().normal())
            .is_some_and(|behind| is_face_sturdy(behind, facing));
    }
    if CANDLES.contains(&kind) {
        return can_support_center(below(), Direction::Up);
    }
    if WOOL_CARPETS.contains(&kind) || kind == BlockKind::MossCarpet {
        return !matches!(
            below().as_block_kind(),
            BlockKind::Air | BlockKind::CaveAir | BlockKind::VoidAir
        );
    }
    true
}

pub fn is_unobstructed(state: BlockState, pos: BlockPos, entities: &[Aabb]) -> bool {
    if state.as_block_kind() == BlockKind::Scaffolding {
        return true;
    }
    let offset = pos.to_vec3_floored();
    state.collision_shape(pos).to_aabbs().iter().all(|part| {
        let part = part.move_relative(offset);
        entities.iter().all(|entity| !part.intersects_aabb(entity))
    })
}

pub fn predict_bucket(
    held: &ItemStack,
    hit: &BlockHitResult,
    min_y: i32,
    height: u32,
    block_at: impl Fn(BlockPos) -> Option<BlockState>,
) -> Option<(BlockPos, BlockState)> {
    let ItemStack::Present(item) = held else {
        return None;
    };
    let fluid = bucket_fluid(item.kind)?;

    let clicked_state = block_at(hit.block_pos)?;
    let container =
        fluid == BlockKind::Water && accepts(clicked_state.as_block_kind(), "waterlogged", "true");
    let pos = if container {
        hit.block_pos
    } else {
        hit.block_pos + hit.direction.normal()
    };

    if pos.y < min_y || pos.y >= min_y + height as i32 {
        return None;
    }

    if container {
        let mut placed = Box::<dyn BlockTrait>::from(clicked_state);
        if placed.get_property("waterlogged") == Some("true") {
            return None;
        }
        let _ = placed.set_property("waterlogged", "true");
        return Some((pos, placed.as_block_state()));
    }

    let replaced_state = block_at(pos)?;
    if !fluid_replaceable(replaced_state) {
        return None;
    }
    Some((pos, BlockState::from(fluid)))
}

pub fn bucket_fluid(kind: ItemKind) -> Option<BlockKind> {
    use ItemKind as I;
    match kind {
        I::WaterBucket
        | I::PufferfishBucket
        | I::SalmonBucket
        | I::CodBucket
        | I::TropicalFishBucket
        | I::AxolotlBucket
        | I::TadpoleBucket => Some(BlockKind::Water),
        I::LavaBucket => Some(BlockKind::Lava),
        _ => None,
    }
}

pub fn predict_bucket_fill(
    held: &ItemStack,
    hit: &BlockHitResult,
    creative: bool,
    block_at: impl Fn(BlockPos) -> Option<BlockState>,
) -> Option<(BlockPos, BlockState, ItemKind)> {
    let ItemStack::Present(item) = held else {
        return None;
    };
    if item.kind != ItemKind::Bucket {
        return None;
    }

    let clicked_state = block_at(hit.block_pos)?;
    let kind = clicked_state.as_block_kind();
    let mut block = Box::<dyn BlockTrait>::from(clicked_state);
    let (new, filled) = match kind {
        BlockKind::Water | BlockKind::Lava => {
            if block.get_property("level") != Some("0") {
                return None;
            }
            let lava = kind == BlockKind::Lava;
            (
                BlockState::AIR,
                if lava {
                    ItemKind::LavaBucket
                } else {
                    ItemKind::WaterBucket
                },
            )
        }
        BlockKind::BubbleColumn => (BlockState::AIR, ItemKind::WaterBucket),
        BlockKind::PowderSnow => (BlockState::AIR, ItemKind::PowderSnowBucket),
        BlockKind::Barrier if !creative => return None,
        _ => {
            if block.get_property("waterlogged") != Some("true") {
                return None;
            }
            let _ = block.set_property("waterlogged", "false");
            (block.as_block_state(), ItemKind::WaterBucket)
        }
    };
    Some((hit.block_pos, new, filled))
}

fn fluid_replaceable(state: BlockState) -> bool {
    let kind = state.as_block_kind();
    kind == BlockKind::Water || kind == BlockKind::Lava || REPLACEABLE.contains(&kind)
}

pub(crate) fn replaceable(state: BlockState, item_block: BlockKind) -> Option<bool> {
    let kind = state.as_block_kind();
    if kind == BlockKind::Scaffolding {
        return Some(item_block == BlockKind::Scaffolding);
    }
    if kind == BlockKind::Snow {
        if item_block == BlockKind::Snow {
            return None;
        }
        return Some(property_digit(state, "layers") == Some(1));
    }
    if kind == item_block {
        return if stacks_with_own_item(kind) {
            None
        } else {
            Some(false)
        };
    }
    Some(REPLACEABLE.contains(&kind))
}

fn stacks_with_own_item(kind: BlockKind) -> bool {
    accepts(kind, "type", "double")
        || kind.to_str().ends_with("candle")
        || matches!(
            kind,
            BlockKind::SeaPickle
                | BlockKind::TurtleEgg
                | BlockKind::PinkPetals
                | BlockKind::Wildflowers
                | BlockKind::LeafLitter
                | BlockKind::Vine
                | BlockKind::GlowLichen
                | BlockKind::ResinClump
                | BlockKind::SculkVein
        )
}

fn replaceable_by(
    state: BlockState,
    item_block: BlockKind,
    hit: &BlockHitResult,
    replacing_clicked: bool,
) -> Option<bool> {
    if state.as_block_kind() != item_block || !accepts(item_block, "type", "double") {
        return replaceable(state, item_block);
    }
    let slab_type = Box::<dyn BlockTrait>::from(state).get_property("type");
    if slab_type == Some("double") {
        return Some(false);
    }
    if !replacing_clicked {
        return Some(true);
    }
    let above = hit.location.y - hit.block_pos.y as f64 > 0.5;
    let horizontal = !matches!(hit.direction, Direction::Up | Direction::Down);
    Some(if slab_type == Some("bottom") {
        hit.direction == Direction::Up || (above && horizontal)
    } else {
        hit.direction == Direction::Down || (!above && horizontal)
    })
}

pub(crate) fn has_use_interaction(state: BlockState) -> bool {
    let kind = state.as_block_kind();
    match USE_INTERACTION.get(kind.to_u32() as usize) {
        Some(slot) => *slot.get_or_init(|| block_has_use_interaction(state)),
        None => block_has_use_interaction(state),
    }
}

static USE_INTERACTION: LazyLock<Box<[OnceLock<bool>]>> = LazyLock::new(|| {
    (0u32..)
        .take_while(|id| BlockKind::is_valid_id(*id))
        .map(|_| OnceLock::new())
        .collect()
});

fn ignores_clicks(id: &str) -> bool {
    matches!(
        id,
        "redstone_lamp"
            | "powered_rail"
            | "activator_rail"
            | "detector_rail"
            | "tripwire"
            | "tripwire_hook"
            | "observer"
            | "redstone_torch"
            | "redstone_wall_torch"
            | "conduit"
            | "sculk_sensor"
            | "calibrated_sculk_sensor"
            | "sculk_shrieker"
            | "iron_door"
            | "iron_trapdoor"
    ) || id.ends_with("_pressure_plate")
        || id.ends_with("copper_bulb")
        || id.ends_with("lightning_rod")
}

pub(crate) fn is_game_master_block(block: BlockKind) -> bool {
    matches!(
        block,
        BlockKind::CommandBlock
            | BlockKind::ChainCommandBlock
            | BlockKind::RepeatingCommandBlock
            | BlockKind::StructureBlock
            | BlockKind::Jigsaw
            | BlockKind::TestBlock
            | BlockKind::TestInstanceBlock
    )
}

#[cold]
fn block_has_use_interaction(state: BlockState) -> bool {
    let block = Box::<dyn BlockTrait>::from(state);
    let id = block.id();
    if ignores_clicks(id) {
        return false;
    }
    if is_game_master_block(state.as_block_kind()) {
        return true;
    }
    if BLOCK_ENTITY_IDS.contains(&format!("minecraft:{id}").as_str())
        || MENU_BLOCK_IDS.contains(&id)
        || id.ends_with("shulker_box")
        || id.ends_with("copper_chest")
        || id == "bee_nest"
    {
        return true;
    }
    INTERACTION_PROPERTIES
        .iter()
        .any(|name| block.get_property(name).is_some())
}

#[allow(clippy::too_many_arguments)]
fn state_for_placement(
    block: BlockKind,
    attachment: Attachment,
    hit: &BlockHitResult,
    yaw: f32,
    pitch: f32,
    replaced_state: BlockState,
    pos: BlockPos,
    replacing_clicked: bool,
    secondary_use: bool,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> Option<BlockState> {
    let mut placed = Box::<dyn BlockTrait>::from(BlockState::from(block));
    let face = hit.direction;

    if replaced_state.as_block_kind() == block && accepts(block, "type", "double") {
        let mut doubled = Box::<dyn BlockTrait>::from(replaced_state);
        let _ = doubled.set_property("type", "double");
        let _ = doubled.set_property("waterlogged", "false");
        return Some(doubled.as_block_state());
    }

    if let Some(substituted) = substituted_placement(block, pos, replaced_state, block_at) {
        return substituted;
    }

    let _ = placed.set_property("axis", axis_name(face));

    let ctx = PlaceContext {
        hit,
        yaw,
        pitch,
        pos,
        replaced_state,
        replacing_clicked,
        secondary_use,
        block_at,
    };
    match attachment {
        Attachment::FaceAttached => {
            return face_attached_placement(
                placed,
                hit,
                yaw,
                pitch,
                pos,
                replacing_clicked,
                block_at,
            );
        }
        Attachment::Wire => return circuit::wire_placement(&ctx),
        Attachment::Diode => return circuit::diode_placement(block, &ctx),
        Attachment::Rail => return circuit::rail_placement(block, &ctx),
        Attachment::PressurePlate => return circuit::pressure_plate_placement(block, &ctx),
        Attachment::Tripwire => return circuit::tripwire_placement(&ctx),
        Attachment::TripwireHook => return circuit::tripwire_hook_placement(&ctx),
        Attachment::Wall(wall, mount) => return attach::standing_or_wall(block, wall, mount, &ctx),
        Attachment::Cocoa => return attach::cocoa_placement(&ctx),
        Attachment::Multiface => return attach::multiface_placement(block, &ctx),
        Attachment::Vine => return attach::vine_placement(&ctx),
        Attachment::Snow => return attach::snow_placement(&ctx),
        Attachment::Door => return pairs::door_placement(block, &ctx),
        Attachment::Bed => return pairs::bed_placement(block, &ctx),
        Attachment::Bell => return bell_placement(placed, face, yaw, pos, block_at),
        Attachment::Lantern => {
            return lantern_placement(
                placed,
                yaw,
                pitch,
                face,
                pos,
                replacing_clicked,
                replaced_state,
                block_at,
            );
        }
        Attachment::None => {}
    }

    match block {
        BlockKind::Scaffolding => {
            return scaffolding_placement(placed, pos, replaced_state, block_at);
        }
        BlockKind::PointedDripstone => {
            return pointed_dripstone_placement(
                placed,
                pitch,
                yaw,
                pos,
                replaced_state,
                secondary_use,
                block_at,
            );
        }
        BlockKind::BrownMushroomBlock | BlockKind::RedMushroomBlock | BlockKind::MushroomStem => {
            return huge_mushroom_placement(placed, block, pos, block_at);
        }
        BlockKind::Ladder => {
            return ladder_placement(
                placed,
                hit,
                yaw,
                pitch,
                pos,
                replacing_clicked,
                replaced_state,
                block_at,
            );
        }
        _ if matches!(block, BlockKind::Chest | BlockKind::TrappedChest)
            || COPPER_CHESTS.contains(&block) =>
        {
            return chest_placement(
                placed,
                block,
                face,
                yaw,
                pos,
                secondary_use,
                replaced_state,
                block_at,
            );
        }
        _ if accepts(block, "persistent", "true") => {
            return leaves_placement(placed, pos, replaced_state, block_at);
        }
        _ => {}
    }

    if accepts(block, "half", "top") && accepts(block, "open", "true") {
        let (facing, half) =
            if !replacing_clicked && !matches!(face, Direction::Up | Direction::Down) {
                let half = if hit.location.y - pos.y as f64 > 0.5 {
                    "top"
                } else {
                    "bottom"
                };
                (face, half)
            } else {
                let half = if face == Direction::Up {
                    "bottom"
                } else {
                    "top"
                };
                (player_facing(yaw).opposite(), half)
            };
        let _ = placed.set_property("facing", direction_name(facing));
        let _ = placed.set_property("half", half);
        if has_neighbor_signal(pos, block_at) {
            let _ = placed.set_property("open", "true");
            let _ = placed.set_property("powered", "true");
        }
        let _ = placed.set_property("waterlogged", bool_str(is_water_source(replaced_state)));
        return Some(placed.as_block_state());
    }

    let facing = exceptional_facing(block, hit, yaw, pitch, pos, block_at).unwrap_or_else(|| {
        if accepts(block, "facing", "up") {
            Direction::nearest(look_vector(yaw, pitch)).opposite()
        } else {
            player_facing(yaw).opposite()
        }
    });
    let _ = placed.set_property("facing", direction_name(facing));

    let top_half = top_half(face, hit.location, hit.block_pos);
    let _ = placed.set_property("half", if top_half { "top" } else { "bottom" });
    if accepts(block, "type", "double") {
        let _ = placed.set_property("type", if top_half { "top" } else { "bottom" });
    }

    let signalled: &[&str] = match block {
        BlockKind::RedstoneLamp => &["lit"],
        BlockKind::Crafter => &["triggered"],
        _ if block.to_str().ends_with("fence_gate") => &["open", "powered"],
        _ if block.to_str().ends_with("_shelf") => &["powered"],
        _ => &[],
    };
    if !signalled.is_empty() {
        let value = bool_str(has_neighbor_signal(pos, block_at));
        for name in signalled {
            let _ = placed.set_property(name, value);
        }
    }

    if block.to_str().ends_with("fence_gate") {
        let (a, b) = match player_facing(yaw) {
            Direction::North | Direction::South => (Direction::West, Direction::East),
            _ => (Direction::North, Direction::South),
        };
        let in_wall = [a, b].into_iter().any(|side| {
            block_at(pos + side.normal())
                .is_some_and(|state| WALLS.contains(&state.as_block_kind()))
        });
        let _ = placed.set_property("in_wall", bool_str(in_wall));
    }

    if accepts(block, "snowy", "true") {
        let above = block_at(pos + Direction::Up.normal()).unwrap_or(BlockState::AIR);
        let _ = placed.set_property("snowy", bool_str(SNOW.contains(&above.as_block_kind())));
    }

    if matches!(block, BlockKind::Campfire | BlockKind::SoulCampfire) {
        let below = block_at(pos + Direction::Down.normal()).unwrap_or(BlockState::AIR);
        let _ = placed.set_property(
            "signal_fire",
            bool_str(below.as_block_kind() == BlockKind::HayBlock),
        );
    }

    if block == BlockKind::Crafter {
        let _ = placed.set_property("orientation", crafter_orientation(yaw, pitch));
    }

    let id = block.to_str();
    let full_water_rule =
        block == BlockKind::Conduit || (id.contains("coral") && !id.ends_with("coral_block"));
    let replaced_water = if full_water_rule {
        is_full_water(replaced_state)
    } else {
        is_water_source(replaced_state)
    };
    let _ = placed.set_property("waterlogged", bool_str(replaced_water));

    if replaced_water {
        let _ = placed.set_property("lit", "false");
    }

    Some(placed.as_block_state())
}

fn crafter_orientation(yaw: f32, pitch: f32) -> &'static str {
    let front = Direction::nearest(look_vector(yaw, pitch)).opposite();
    match front {
        Direction::Down => match player_facing(yaw).opposite() {
            Direction::North => "down_north",
            Direction::South => "down_south",
            Direction::West => "down_west",
            _ => "down_east",
        },
        Direction::Up => match player_facing(yaw) {
            Direction::North => "up_north",
            Direction::South => "up_south",
            Direction::West => "up_west",
            _ => "up_east",
        },
        Direction::North => "north_up",
        Direction::South => "south_up",
        Direction::West => "west_up",
        Direction::East => "east_up",
    }
}

fn rotation_segment(degrees: f32) -> &'static str {
    let scaled = degrees * (16.0f32 / 360.0f32);
    segment_name(((scaled as f64) + 0.5).floor() as i32)
}

fn segment_name(segment: i32) -> &'static str {
    const NAMES: [&str; 16] = [
        "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
    ];
    NAMES[(segment & 15) as usize]
}

#[allow(clippy::too_many_arguments)]
fn lantern_placement(
    mut placed: Box<dyn BlockTrait>,
    yaw: f32,
    pitch: f32,
    clicked_face: Direction,
    pos: BlockPos,
    replacing_clicked: bool,
    replaced_state: BlockState,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> Option<BlockState> {
    for direction in nearest_looking_directions(yaw, pitch, clicked_face, replacing_clicked) {
        if !matches!(direction, Direction::Up | Direction::Down) {
            continue;
        }
        let hanging = direction == Direction::Up;
        let support = if hanging {
            Direction::Up
        } else {
            Direction::Down
        };
        let neighbour = pos + support.normal();
        if !block_at(neighbour).is_some_and(|state| can_support_center(state, support.opposite())) {
            continue;
        }
        let _ = placed.set_property("hanging", bool_str(hanging));
        let _ = placed.set_property("waterlogged", bool_str(is_water_source(replaced_state)));
        return Some(placed.as_block_state());
    }
    None
}

fn scaffolding_placement(
    mut placed: Box<dyn BlockTrait>,
    pos: BlockPos,
    replaced_state: BlockState,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> Option<BlockState> {
    let distance = scaffolding_distance(pos, block_at);
    let below_is_scaffolding = block_at(pos + Direction::Down.normal())
        .is_some_and(|state| state.as_block_kind() == BlockKind::Scaffolding);
    let bottom = distance > 0 && !below_is_scaffolding;

    let _ = placed.set_property("distance", DIGITS[distance as usize]);
    let _ = placed.set_property("bottom", bool_str(bottom));
    let _ = placed.set_property("waterlogged", bool_str(is_water_source(replaced_state)));
    Some(placed.as_block_state())
}

fn scaffolding_placement_pos(
    pos: BlockPos,
    replaced_state: BlockState,
    hit: &BlockHitResult,
    yaw: f32,
    secondary_use: bool,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> Option<BlockPos> {
    if replaced_state.as_block_kind() != BlockKind::Scaffolding {
        return (scaffolding_distance(pos, block_at) != 7).then_some(pos);
    }
    let direction = if secondary_use {
        if hit.inside {
            hit.direction.opposite()
        } else {
            hit.direction
        }
    } else if hit.direction == Direction::Up {
        player_facing(yaw)
    } else {
        Direction::Up
    };
    let horizontal = !matches!(direction, Direction::Up | Direction::Down);
    let mut steps = 0;
    let mut at = pos + direction.normal();
    while steps < 7 {
        let state = block_at(at)?;
        if state.as_block_kind() != BlockKind::Scaffolding {
            return replaceable(state, BlockKind::Scaffolding)
                .unwrap_or(false)
                .then_some(at);
        }
        at = at + direction.normal();
        if horizontal {
            steps += 1;
        }
    }
    None
}

fn scaffolding_distance(pos: BlockPos, block_at: &dyn Fn(BlockPos) -> Option<BlockState>) -> u32 {
    let below_pos = pos + Direction::Down.normal();
    let below = block_at(below_pos).unwrap_or(BlockState::AIR);
    let mut distance = 7;
    if below.as_block_kind() == BlockKind::Scaffolding {
        distance = property_digit(below, "distance").unwrap_or(7);
    } else if is_face_sturdy(below, Direction::Up) {
        return 0;
    }

    for direction in [
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ] {
        let Some(neighbour) = block_at(pos + direction.normal()) else {
            continue;
        };
        if neighbour.as_block_kind() == BlockKind::Scaffolding {
            let neighbour_distance = property_digit(neighbour, "distance").unwrap_or(7);
            distance = distance.min(neighbour_distance + 1);
            if distance == 1 {
                break;
            }
        }
    }
    distance.min(7)
}

fn pointed_dripstone_placement(
    mut placed: Box<dyn BlockTrait>,
    pitch: f32,
    yaw: f32,
    pos: BlockPos,
    replaced_state: BlockState,
    secondary_use: bool,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> Option<BlockState> {
    let looking = if look_vector(yaw, pitch).y > 0. {
        Direction::Up
    } else {
        Direction::Down
    };
    let default_tip = looking.opposite();

    let tip = if dripstone_placement_valid(pos, default_tip, block_at) {
        default_tip
    } else if dripstone_placement_valid(pos, default_tip.opposite(), block_at) {
        default_tip.opposite()
    } else {
        return None;
    };

    let thickness = dripstone_thickness(pos, tip, !secondary_use, block_at);
    let _ = placed.set_property("vertical_direction", direction_name(tip));
    let _ = placed.set_property("thickness", thickness);
    let _ = placed.set_property("waterlogged", bool_str(is_water_source(replaced_state)));
    Some(placed.as_block_state())
}

fn dripstone_placement_valid(
    pos: BlockPos,
    tip: Direction,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> bool {
    let behind_pos = pos + tip.opposite().normal();
    let Some(behind) = block_at(behind_pos) else {
        return false;
    };
    is_face_sturdy(behind, tip) || is_dripstone_facing(behind, tip)
}

fn dripstone_thickness(
    pos: BlockPos,
    tip: Direction,
    merge_opposing_tips: bool,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> &'static str {
    let base = tip.opposite();
    let in_front = block_at(pos + tip.normal()).unwrap_or(BlockState::AIR);

    if is_dripstone_facing(in_front, base) {
        let in_front_thickness = dripstone_thickness_of(in_front);
        return if !merge_opposing_tips && in_front_thickness != Some("tip_merge") {
            "tip"
        } else {
            "tip_merge"
        };
    }
    if !is_dripstone_facing(in_front, tip) {
        return "tip";
    }
    match dripstone_thickness_of(in_front) {
        Some("tip") | Some("tip_merge") => "frustum",
        _ => {
            let behind = block_at(pos + base.normal()).unwrap_or(BlockState::AIR);
            if is_dripstone_facing(behind, tip) {
                "middle"
            } else {
                "base"
            }
        }
    }
}

fn is_dripstone_facing(state: BlockState, tip: Direction) -> bool {
    state.as_block_kind() == BlockKind::PointedDripstone
        && Box::<dyn BlockTrait>::from(state).get_property("vertical_direction")
            == Some(direction_name(tip))
}

fn dripstone_thickness_of(state: BlockState) -> Option<&'static str> {
    (state.as_block_kind() == BlockKind::PointedDripstone)
        .then(|| Box::<dyn BlockTrait>::from(state).get_property("thickness"))
        .flatten()
}

fn huge_mushroom_placement(
    mut placed: Box<dyn BlockTrait>,
    block: BlockKind,
    pos: BlockPos,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> Option<BlockState> {
    for direction in DIRECTIONS {
        let same = block_at(pos + direction.normal())
            .is_some_and(|neighbour| neighbour.as_block_kind() == block);
        let _ = placed.set_property(direction_name(direction), bool_str(!same));
    }
    Some(placed.as_block_state())
}

#[allow(clippy::too_many_arguments)]
fn ladder_placement(
    mut placed: Box<dyn BlockTrait>,
    hit: &BlockHitResult,
    yaw: f32,
    pitch: f32,
    pos: BlockPos,
    replacing_clicked: bool,
    replaced_state: BlockState,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> Option<BlockState> {
    if !replacing_clicked {
        let behind = block_at(pos + hit.direction.opposite().normal());
        let chained = behind.is_some_and(|state| {
            state.as_block_kind() == BlockKind::Ladder
                && Box::<dyn BlockTrait>::from(state).get_property("facing")
                    == Some(direction_name(hit.direction))
        });
        if chained {
            return None;
        }
    }
    for direction in nearest_looking_directions(yaw, pitch, hit.direction, replacing_clicked) {
        if matches!(direction, Direction::Up | Direction::Down) {
            continue;
        }
        if !can_attach(pos, direction, block_at) {
            continue;
        }
        let _ = placed.set_property("facing", direction_name(direction.opposite()));
        let _ = placed.set_property("waterlogged", bool_str(is_water_source(replaced_state)));
        return Some(placed.as_block_state());
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn chest_placement(
    mut placed: Box<dyn BlockTrait>,
    block: BlockKind,
    face: Direction,
    yaw: f32,
    pos: BlockPos,
    secondary_use: bool,
    replaced_state: BlockState,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> Option<BlockState> {
    let copper = COPPER_CHESTS.contains(&block);
    let partner_facing = |direction: Direction| -> Option<Direction> {
        let state = block_at(pos + direction.normal())?;
        let kind = state.as_block_kind();
        let connects = if copper {
            COPPER_CHESTS.contains(&kind)
        } else {
            kind == block
        };
        if !connects {
            return None;
        }
        let partner = Box::<dyn BlockTrait>::from(state);
        if partner.get_property("type") != Some("single") {
            return None;
        }
        direction_from_name(partner.get_property("facing")?)
    };

    let mut facing = player_facing(yaw).opposite();
    let mut chest_type = "single";
    if !matches!(face, Direction::Up | Direction::Down) && secondary_use {
        if let Some(neighbour) = partner_facing(face.opposite()) {
            if neighbour != face && neighbour != face.opposite() {
                facing = neighbour;
                chest_type = if counter_clockwise(facing) == face.opposite() {
                    "right"
                } else {
                    "left"
                };
            }
        }
    }
    if chest_type == "single" && !secondary_use {
        if partner_facing(clockwise(facing)) == Some(facing) {
            chest_type = "left";
        } else if partner_facing(counter_clockwise(facing)) == Some(facing) {
            chest_type = "right";
        }
    }
    if copper && chest_type != "single" {
        let connected = if chest_type == "left" {
            clockwise(facing)
        } else {
            counter_clockwise(facing)
        };
        if block_at(pos + connected.normal()).map(|state| state.as_block_kind()) != Some(block) {
            return None;
        }
    }

    let _ = placed.set_property("facing", direction_name(facing));
    let _ = placed.set_property("type", chest_type);
    let _ = placed.set_property("waterlogged", bool_str(is_water_source(replaced_state)));
    Some(placed.as_block_state())
}

fn leaves_placement(
    mut placed: Box<dyn BlockTrait>,
    pos: BlockPos,
    replaced_state: BlockState,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> Option<BlockState> {
    let distance = DIRECTIONS
        .iter()
        .map(|direction| {
            let neighbour = block_at(pos + direction.normal()).unwrap_or(BlockState::AIR);
            leaf_distance_at(neighbour) + 1
        })
        .fold(7, u32::min);
    let _ = placed.set_property("persistent", "true");
    let _ = placed.set_property("waterlogged", bool_str(is_water_source(replaced_state)));
    let _ = placed.set_property("distance", DIGITS[distance as usize]);
    Some(placed.as_block_state())
}

fn leaf_distance_at(state: BlockState) -> u32 {
    let kind = state.as_block_kind();
    if PREVENTS_NEARBY_LEAF_DECAY.contains(&kind) {
        0
    } else if accepts(kind, "persistent", "true") {
        property_digit(state, "distance").unwrap_or(7)
    } else {
        7
    }
}

const DIGITS: [&str; 8] = ["0", "1", "2", "3", "4", "5", "6", "7"];

fn property_digit(state: BlockState, name: &str) -> Option<u32> {
    Box::<dyn BlockTrait>::from(state)
        .get_property(name)
        .and_then(|value| value.parse().ok())
}

fn substituted_placement(
    block: BlockKind,
    pos: BlockPos,
    replaced_state: BlockState,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> Option<Option<BlockState>> {
    let id = block.to_str();

    if let Some(colour) = id.strip_suffix("_concrete_powder") {
        let solidifies = is_water(replaced_state)
            || DIRECTIONS.iter().any(|&direction| {
                block_at(pos + direction.normal()).is_some_and(|neighbour| {
                    is_water(neighbour) && !is_face_sturdy(neighbour, direction.opposite())
                })
            });
        if !solidifies {
            return None;
        }
        return Some(
            BlockKind::from_str(&format!("{colour}_concrete"))
                .ok()
                .map(BlockState::from),
        );
    }

    match block {
        BlockKind::Farmland => {
            let above = block_at(pos + Direction::Up.normal())?;
            let crushed = is_solid(above) && !MAINTAINS_FARMLAND.contains(&above.as_block_kind());
            crushed.then(|| Some(BlockState::from(BlockKind::Dirt)))
        }
        BlockKind::DirtPath => {
            let above = block_at(pos + Direction::Up.normal())?;
            let crushed =
                is_solid(above) && !above.as_block_kind().to_str().ends_with("fence_gate");
            crushed.then(|| Some(BlockState::from(BlockKind::Dirt)))
        }
        BlockKind::Bamboo => {
            if is_water(replaced_state) || is_lava(replaced_state) {
                return Some(None);
            }
            let below = block_at(pos + Direction::Down.normal())?;
            if !SUPPORTS_BAMBOO.contains(&below.as_block_kind()) {
                return Some(None);
            }
            let mut stalk = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::Bamboo));
            match below.as_block_kind() {
                BlockKind::BambooSapling => {
                    let _ = stalk.set_property("age", "0");
                }
                BlockKind::Bamboo => {
                    let grown = Box::<dyn BlockTrait>::from(below).get_property("age") != Some("0");
                    let _ = stalk.set_property("age", if grown { "1" } else { "0" });
                }
                _ => {
                    let above = block_at(pos + Direction::Up.normal())?;
                    if above.as_block_kind() != BlockKind::Bamboo {
                        return Some(Some(BlockState::from(BlockKind::BambooSapling)));
                    }
                    let age = Box::<dyn BlockTrait>::from(above)
                        .get_property("age")
                        .unwrap_or("0");
                    let _ = stalk.set_property("age", age);
                }
            }
            Some(Some(stalk.as_block_state()))
        }
        _ => None,
    }
}

fn bool_str(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn is_water(state: BlockState) -> bool {
    matches!(
        azalea::block::fluid_state::FluidState::from(state).kind,
        azalea::block::fluid_state::FluidKind::Water
    )
}

fn is_water_source(state: BlockState) -> bool {
    let fluid = azalea::block::fluid_state::FluidState::from(state);
    fluid.kind == azalea::block::fluid_state::FluidKind::Water && fluid.is_source()
}

fn is_full_water(state: BlockState) -> bool {
    let fluid = azalea::block::fluid_state::FluidState::from(state);
    fluid.kind == azalea::block::fluid_state::FluidKind::Water && fluid.amount == 8
}

fn is_lava(state: BlockState) -> bool {
    matches!(
        azalea::block::fluid_state::FluidState::from(state).kind,
        azalea::block::fluid_state::FluidKind::Lava
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Attachment {
    FaceAttached,
    Bell,
    Lantern,
    Wire,
    Diode,
    Rail,
    PressurePlate,
    Tripwire,
    TripwireHook,
    Wall(BlockKind, attach::Mount),
    Cocoa,
    Multiface,
    Vine,
    Snow,
    Door,
    Bed,
    None,
}

fn attachment(block: BlockKind) -> Attachment {
    if accepts(block, "face", "ceiling") {
        Attachment::FaceAttached
    } else if accepts(block, "attachment", "single_wall") {
        Attachment::Bell
    } else if accepts(block, "hanging", "true") && block.to_str().ends_with("lantern") {
        Attachment::Lantern
    } else if let Some((wall, mount)) = attach::wall_variant(block) {
        Attachment::Wall(wall, mount)
    } else {
        match block {
            BlockKind::RedstoneWire => Attachment::Wire,
            BlockKind::Repeater | BlockKind::Comparator => Attachment::Diode,
            BlockKind::Tripwire => Attachment::Tripwire,
            BlockKind::TripwireHook => Attachment::TripwireHook,
            BlockKind::Cocoa => Attachment::Cocoa,
            BlockKind::GlowLichen | BlockKind::SculkVein | BlockKind::ResinClump => {
                Attachment::Multiface
            }
            BlockKind::Vine => Attachment::Vine,
            BlockKind::Snow => Attachment::Snow,
            _ if RAILS.contains(&block) => Attachment::Rail,
            _ if PRESSURE_PLATES.contains(&block) => Attachment::PressurePlate,
            _ if accepts(block, "hinge", "left") => Attachment::Door,
            _ if accepts(block, "part", "head") => Attachment::Bed,
            _ => Attachment::None,
        }
    }
}

struct PlaceContext<'a> {
    hit: &'a BlockHitResult,
    yaw: f32,
    pitch: f32,
    pos: BlockPos,
    replaced_state: BlockState,
    replacing_clicked: bool,
    secondary_use: bool,
    block_at: &'a dyn Fn(BlockPos) -> Option<BlockState>,
}

impl PlaceContext<'_> {
    fn at(&self, at: BlockPos) -> BlockState {
        (self.block_at)(at).unwrap_or(BlockState::AIR)
    }

    fn horizontal(&self) -> Direction {
        player_facing(self.yaw)
    }

    fn looking(&self) -> [Direction; 6] {
        nearest_looking_directions(
            self.yaw,
            self.pitch,
            self.hit.direction,
            self.replacing_clicked,
        )
    }

    fn first_horizontal(&self, mut holds: impl FnMut(Direction) -> bool) -> Option<Direction> {
        self.looking()
            .into_iter()
            .filter(|d| !matches!(d, Direction::Up | Direction::Down))
            .find(|&d| holds(d))
    }

    fn waterlogged(&self) -> &'static str {
        bool_str(is_water_source(self.replaced_state))
    }
}

fn exceptional_facing(
    block: BlockKind,
    hit: &BlockHitResult,
    yaw: f32,
    pitch: f32,
    pos: BlockPos,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> Option<Direction> {
    let id = block.to_str();

    if id.ends_with("shulker_box")
        || id.ends_with("lightning_rod")
        || id.ends_with("amethyst_bud")
        || id.ends_with("amethyst_cluster")
    {
        return Some(hit.direction);
    }

    if accepts(block, "shape", "inner_left")
        || id.ends_with("fence_gate")
        || matches!(
            block,
            BlockKind::Campfire
                | BlockKind::SoulCampfire
                | BlockKind::DecoratedPot
                | BlockKind::CalibratedSculkSensor
        )
    {
        return Some(player_facing(yaw));
    }

    match block {
        BlockKind::Anvil | BlockKind::ChippedAnvil | BlockKind::DamagedAnvil => {
            Some(clockwise(player_facing(yaw)))
        }
        BlockKind::Observer => Some(Direction::nearest(look_vector(yaw, pitch))),
        BlockKind::Hopper => Some(match hit.direction.opposite() {
            Direction::Up | Direction::Down => Direction::Down,
            horizontal => horizontal,
        }),
        BlockKind::EndRod => {
            let clicked = hit.direction;
            let behind = pos + clicked.opposite().normal();
            let chains = block_at(behind).is_some_and(|state| {
                state.as_block_kind() == BlockKind::EndRod
                    && Box::<dyn BlockTrait>::from(state).get_property("facing")
                        == Some(direction_name(clicked))
            });
            Some(if chains { clicked.opposite() } else { clicked })
        }
        BlockKind::BigDripleaf => block_at(pos + Direction::Down.normal())
            .filter(|below| {
                matches!(
                    below.as_block_kind(),
                    BlockKind::BigDripleaf | BlockKind::BigDripleafStem
                )
            })
            .and_then(|below| {
                Box::<dyn BlockTrait>::from(below)
                    .get_property("facing")
                    .and_then(direction_from_name)
            }),
        _ => None,
    }
}

fn face_attached_placement(
    mut placed: Box<dyn BlockTrait>,
    hit: &BlockHitResult,
    yaw: f32,
    pitch: f32,
    pos: BlockPos,
    replacing_clicked: bool,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> Option<BlockState> {
    let anywhere = placed.as_block_state().as_block_kind() == BlockKind::Grindstone;
    for direction in nearest_looking_directions(yaw, pitch, hit.direction, replacing_clicked) {
        let (face, facing) = match direction {
            Direction::Up => ("ceiling", player_facing(yaw)),
            Direction::Down => ("floor", player_facing(yaw)),
            horizontal => ("wall", horizontal.opposite()),
        };
        let connected = match face {
            "ceiling" => Direction::Down,
            "floor" => Direction::Up,
            _ => facing,
        };
        if !anywhere && !can_attach(pos, connected.opposite(), block_at) {
            continue;
        }
        let _ = placed.set_property("face", face);
        let _ = placed.set_property("facing", direction_name(facing));
        return Some(placed.as_block_state());
    }
    None
}

fn bell_placement(
    mut placed: Box<dyn BlockTrait>,
    face: Direction,
    yaw: f32,
    pos: BlockPos,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> Option<BlockState> {
    let apply = |placed: &mut Box<dyn BlockTrait>, attachment: &str, facing: Direction| {
        let _ = placed.set_property("attachment", attachment);
        let _ = placed.set_property("facing", direction_name(facing));
    };

    if matches!(face, Direction::Up | Direction::Down) {
        let attachment = if face == Direction::Down {
            "ceiling"
        } else {
            "floor"
        };
        apply(&mut placed, attachment, player_facing(yaw));
        return bell_survives(pos, attachment, player_facing(yaw), block_at)
            .then(|| placed.as_block_state());
    }

    let facing = face.opposite();
    let across = matches!(face, Direction::West | Direction::East);
    let (a, b) = if across {
        (Direction::West, Direction::East)
    } else {
        (Direction::North, Direction::South)
    };
    let double_attached = can_attach(pos, a, block_at) && can_attach(pos, b, block_at);
    let attachment = if double_attached {
        "double_wall"
    } else {
        "single_wall"
    };
    apply(&mut placed, attachment, facing);
    if bell_survives(pos, attachment, facing, block_at) {
        return Some(placed.as_block_state());
    }

    let attachment = if can_attach(pos, Direction::Down, block_at) {
        "floor"
    } else {
        "ceiling"
    };
    apply(&mut placed, attachment, facing);
    bell_survives(pos, attachment, facing, block_at).then(|| placed.as_block_state())
}

fn bell_survives(
    pos: BlockPos,
    attachment: &str,
    facing: Direction,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> bool {
    let connected = match attachment {
        "floor" => Direction::Up,
        "ceiling" => Direction::Down,
        _ => facing.opposite(),
    };
    let connection_dir = connected.opposite();
    if connection_dir == Direction::Up {
        let above = pos + Direction::Up.normal();
        return block_at(above).is_some_and(|state| can_support_center(state, Direction::Down));
    }
    can_attach(pos, connection_dir, block_at)
}

fn can_attach(
    pos: BlockPos,
    direction: Direction,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> bool {
    let relative = pos + direction.normal();
    block_at(relative).is_some_and(|state| is_face_sturdy(state, direction.opposite()))
}

fn ordered_by_nearest(yaw: f32, pitch: f32) -> [Direction; 6] {
    let pitch_rad = (pitch as f64).to_radians();
    let yaw_rad = -(yaw as f64).to_radians();
    let (pitch_sin, pitch_cos) = (pitch_rad.sin(), pitch_rad.cos());
    let (yaw_sin, yaw_cos) = (yaw_rad.sin(), yaw_rad.cos());

    let x_pos = yaw_sin > 0.;
    let y_pos = pitch_sin < 0.;
    let z_pos = yaw_cos > 0.;
    let x_yaw = if x_pos { yaw_sin } else { -yaw_sin };
    let y_mag = if y_pos { -pitch_sin } else { pitch_sin };
    let z_yaw = if z_pos { yaw_cos } else { -yaw_cos };
    let x_mag = x_yaw * pitch_cos;
    let z_mag = z_yaw * pitch_cos;

    let axis_x = if x_pos {
        Direction::East
    } else {
        Direction::West
    };
    let axis_y = if y_pos {
        Direction::Up
    } else {
        Direction::Down
    };
    let axis_z = if z_pos {
        Direction::South
    } else {
        Direction::North
    };

    let (first, second, third) = if x_yaw > z_yaw {
        if y_mag > x_mag {
            (axis_y, axis_x, axis_z)
        } else if z_mag > y_mag {
            (axis_x, axis_z, axis_y)
        } else {
            (axis_x, axis_y, axis_z)
        }
    } else if y_mag > z_mag {
        (axis_y, axis_z, axis_x)
    } else if x_mag > y_mag {
        (axis_z, axis_x, axis_y)
    } else {
        (axis_z, axis_y, axis_x)
    };
    [
        first,
        second,
        third,
        third.opposite(),
        second.opposite(),
        first.opposite(),
    ]
}

fn nearest_looking_directions(
    yaw: f32,
    pitch: f32,
    clicked_face: Direction,
    replacing_clicked: bool,
) -> [Direction; 6] {
    let mut directions = ordered_by_nearest(yaw, pitch);
    if replacing_clicked {
        return directions;
    }
    let first = clicked_face.opposite();
    if let Some(index) = directions.iter().position(|&d| d == first) {
        directions[..=index].rotate_right(1);
    }
    directions
}

fn accepts(block: BlockKind, name: &str, value: &str) -> bool {
    Box::<dyn BlockTrait>::from(BlockState::from(block))
        .set_property(name, value)
        .is_ok()
}

pub fn toggled_open(state: BlockState) -> Option<BlockState> {
    let mut block = Box::<dyn BlockTrait>::from(state);
    let name = block.id();
    if name == "iron_door"
        || name == "iron_trapdoor"
        || !(name.ends_with("_door") || name.ends_with("_trapdoor"))
    {
        return None;
    }
    let flipped = match block.property_map().get("open").copied()? {
        "true" => "false",
        _ => "true",
    };
    block.set_property("open", flipped).ok()?;
    Some(block.as_block_state())
}

pub fn door_other_half(
    old: BlockState,
    new: BlockState,
    pos: BlockPos,
    block_at: impl Fn(BlockPos) -> Option<BlockState>,
) -> Option<(BlockPos, BlockState, BlockState)> {
    let block = Box::<dyn BlockTrait>::from(old);
    let (partner_pos, partner_half) = match block.property_map().get("half").copied()? {
        "lower" => (pos.up(1), "upper"),
        "upper" => (pos.down(1), "lower"),
        _ => return None,
    };
    let old_partner = block_at(partner_pos)?;
    let found = Box::<dyn BlockTrait>::from(old_partner);
    if found.id() != block.id() || found.property_map().get("half").copied() != Some(partner_half) {
        return None;
    }
    let mut adopted = Box::<dyn BlockTrait>::from(new);
    adopted.set_property("half", partner_half).ok()?;
    Some((partner_pos, old_partner, adopted.as_block_state()))
}

fn top_half(face: Direction, location: Vec3, pos: BlockPos) -> bool {
    match face {
        Direction::Up => false,
        Direction::Down => true,
        _ => location.y - pos.y as f64 > 0.5,
    }
}

fn look_vector(yaw: f32, pitch: f32) -> Vec3 {
    let [x, y, z] = crate::modules::sent::view_vector(crate::modules::sent::Look { yaw, pitch });
    Vec3::new(x, y, z)
}

fn player_facing(yaw: f32) -> Direction {
    match (yaw as f64 / 90.0 + 0.5).floor() as i64 & 3 {
        0 => Direction::South,
        1 => Direction::West,
        2 => Direction::North,
        _ => Direction::East,
    }
}

fn axis_name(face: Direction) -> &'static str {
    match face {
        Direction::Up | Direction::Down => "y",
        Direction::North | Direction::South => "z",
        Direction::East | Direction::West => "x",
    }
}

#[cfg(test)]
mod tests {
    use azalea_registry::builtin::ItemKind;

    use super::*;

    const MIN_Y: i32 = -64;
    const HEIGHT: u32 = 384;
    const TARGET: BlockPos = BlockPos { x: 0, y: 0, z: 0 };

    fn hit(face: Direction, frac_y: f64) -> BlockHitResult {
        BlockHitResult {
            location: Vec3 {
                x: 0.5,
                y: frac_y,
                z: 0.5,
            },
            direction: face,
            block_pos: TARGET,
            inside: false,
            world_border: false,
            miss: false,
        }
    }

    fn world(clicked: BlockKind) -> impl Fn(BlockPos) -> Option<BlockState> {
        move |pos| {
            Some(if pos == TARGET {
                BlockState::from(clicked)
            } else {
                BlockState::AIR
            })
        }
    }

    #[test]
    fn game_master_blocks_answer_a_click() {
        for block in [
            BlockKind::CommandBlock,
            BlockKind::ChainCommandBlock,
            BlockKind::RepeatingCommandBlock,
            BlockKind::StructureBlock,
            BlockKind::Jigsaw,
            BlockKind::TestBlock,
            BlockKind::TestInstanceBlock,
        ] {
            assert!(has_use_interaction(BlockState::from(block)), "{block:?}");
            assert!(
                place(ItemKind::Stone, Direction::Up, 1.0, 0.0, block).is_none(),
                "{block:?}"
            );
        }
    }

    fn place(
        item: ItemKind,
        face: Direction,
        frac_y: f64,
        yaw: f32,
        clicked: BlockKind,
    ) -> Option<(BlockPos, BlockState)> {
        predict(
            &ItemStack::new(item, 1),
            &hit(face, frac_y),
            yaw,
            0.,
            false,
            MIN_Y,
            HEIGHT,
            world(clicked),
        )
    }

    fn property(state: BlockState, name: &str) -> Option<&'static str> {
        Box::<dyn BlockTrait>::from(state).get_property(name)
    }

    #[test]
    fn a_log_takes_the_axis_of_the_clicked_face() {
        for (face, axis) in [
            (Direction::Up, "y"),
            (Direction::Down, "y"),
            (Direction::North, "z"),
            (Direction::South, "z"),
            (Direction::East, "x"),
            (Direction::West, "x"),
        ] {
            let (pos, state) = place(ItemKind::OakLog, face, 0.5, 0., BlockKind::Stone)
                .expect("a log against stone is predictable");
            assert_eq!(pos, TARGET + face.normal(), "placed on the wrong side");
            assert_eq!(property(state, "axis"), Some(axis), "face {face:?}");
        }
    }

    #[test]
    fn a_slab_takes_the_half_that_was_clicked() {
        let (_, state) = place(
            ItemKind::StoneSlab,
            Direction::North,
            0.75,
            0.,
            BlockKind::Stone,
        )
        .expect("a slab against stone is predictable");
        assert_eq!(property(state, "type"), Some("top"));

        let (_, state) = place(
            ItemKind::StoneSlab,
            Direction::North,
            0.25,
            0.,
            BlockKind::Stone,
        )
        .expect("a slab against stone is predictable");
        assert_eq!(property(state, "type"), Some("bottom"));

        let (_, state) = place(
            ItemKind::StoneSlab,
            Direction::Up,
            0.9,
            0.,
            BlockKind::Stone,
        )
        .expect("a slab against stone is predictable");
        assert_eq!(property(state, "type"), Some("bottom"));
    }

    #[test]
    fn a_stair_faces_the_way_the_player_looks() {
        let (_, state) = place(
            ItemKind::StoneStairs,
            Direction::Up,
            1.0,
            0.,
            BlockKind::Stone,
        )
        .expect("a stair against stone is predictable");
        assert_eq!(property(state, "facing"), Some("south"));
        assert_eq!(property(state, "half"), Some("bottom"));

        let (_, state) = place(
            ItemKind::StoneStairs,
            Direction::Up,
            1.0,
            180.,
            BlockKind::Stone,
        )
        .expect("a stair against stone is predictable");
        assert_eq!(property(state, "facing"), Some("north"));
    }

    #[test]
    fn a_furnace_faces_back_at_the_player() {
        let (_, state) = place(ItemKind::Furnace, Direction::Up, 1.0, 0., BlockKind::Stone)
            .expect("a furnace against stone is predictable");
        assert_eq!(property(state, "facing"), Some("north"));
    }

    #[test]
    fn an_anvil_faces_clockwise_of_the_player() {
        let (_, state) = place(ItemKind::Anvil, Direction::Up, 1.0, 0., BlockKind::Stone)
            .expect("an anvil against stone is predictable");
        assert_eq!(property(state, "facing"), Some("west"));
    }

    #[test]
    fn a_campfire_faces_the_same_way_as_the_player() {
        let (_, state) = place(ItemKind::Campfire, Direction::Up, 1.0, 0., BlockKind::Stone)
            .expect("a campfire against stone is predictable");
        assert_eq!(property(state, "facing"), Some("south"));
    }

    #[test]
    fn an_observer_faces_along_the_look_direction() {
        let (_, state) = predict(
            &ItemStack::new(ItemKind::Observer, 1),
            &hit(Direction::Up, 1.0),
            0.,
            90.,
            false,
            MIN_Y,
            HEIGHT,
            world(BlockKind::Stone),
        )
        .expect("an observer against stone is predictable");
        assert_eq!(property(state, "facing"), Some("down"));
    }

    #[test]
    fn a_hopper_points_down_off_a_top_face_and_sideways_off_a_wall() {
        let (_, state) = place(ItemKind::Hopper, Direction::Up, 1.0, 0., BlockKind::Stone)
            .expect("a hopper against stone is predictable");
        assert_eq!(property(state, "facing"), Some("down"));

        let (_, state) = place(
            ItemKind::Hopper,
            Direction::North,
            0.5,
            0.,
            BlockKind::Stone,
        )
        .expect("a hopper against stone is predictable");
        assert_eq!(property(state, "facing"), Some("south"));
    }

    #[test]
    fn the_clicked_face_family_sits_on_the_face_that_was_clicked() {
        for item in [
            ItemKind::ShulkerBox,
            ItemKind::LightningRod,
            ItemKind::AmethystCluster,
        ] {
            let (_, state) = place(item, Direction::Up, 1.0, 0., BlockKind::Stone)
                .unwrap_or_else(|| panic!("{item:?} against stone is predictable"));
            assert_eq!(property(state, "facing"), Some("up"), "{item:?}");
        }
    }

    #[test]
    fn a_fence_gate_faces_the_same_way_as_the_player() {
        let (_, state) = place(
            ItemKind::OakFenceGate,
            Direction::Up,
            1.0,
            0.,
            BlockKind::Stone,
        )
        .expect("a fence gate against stone is predictable");
        assert_eq!(property(state, "facing"), Some("south"));
    }

    #[test]
    fn a_lever_mounts_on_the_face_it_was_clicked_against() {
        for (face, expected_face, expected_facing) in [
            (Direction::Up, "floor", "south"),
            (Direction::Down, "ceiling", "south"),
            (Direction::North, "wall", "north"),
            (Direction::East, "wall", "east"),
        ] {
            let (_, state) = place(ItemKind::Lever, face, 0.5, 0., BlockKind::Stone)
                .unwrap_or_else(|| panic!("a lever on {face:?} of stone is predictable"));
            assert_eq!(property(state, "face"), Some(expected_face), "{face:?}");
            assert_eq!(property(state, "facing"), Some(expected_facing), "{face:?}");
        }
    }

    #[test]
    fn a_lever_with_nothing_to_hold_it_is_not_predicted() {
        assert!(
            place(
                ItemKind::Lever,
                Direction::Up,
                1.0,
                0.,
                BlockKind::ShortGrass
            )
            .is_none()
        );
    }

    #[test]
    fn buttons_and_grindstones_attach_the_same_way_as_levers() {
        for item in [ItemKind::StoneButton, ItemKind::Grindstone] {
            let (_, state) = place(item, Direction::Up, 1.0, 0., BlockKind::Stone)
                .unwrap_or_else(|| panic!("{item:?} against stone is predictable"));
            assert_eq!(property(state, "face"), Some("floor"), "{item:?}");
        }
    }

    #[test]
    fn a_bell_takes_its_attachment_from_the_clicked_face() {
        let (_, state) = place(ItemKind::Bell, Direction::Up, 1.0, 0., BlockKind::Stone)
            .expect("a bell on top of stone is predictable");
        assert_eq!(property(state, "attachment"), Some("floor"));
        assert_eq!(property(state, "facing"), Some("south"));

        let (_, state) = place(ItemKind::Bell, Direction::North, 0.5, 0., BlockKind::Stone)
            .expect("a bell on the side of stone is predictable");
        assert_eq!(property(state, "attachment"), Some("single_wall"));
        assert_eq!(property(state, "facing"), Some("south"));
    }

    #[test]
    fn a_lantern_hangs_from_a_ceiling_and_stands_on_a_floor() {
        let (_, state) = place(ItemKind::Lantern, Direction::Up, 1.0, 0., BlockKind::Stone)
            .expect("a lantern on top of stone is predictable");
        assert_eq!(property(state, "hanging"), Some("false"));

        let (_, state) = place(
            ItemKind::Lantern,
            Direction::Down,
            0.0,
            0.,
            BlockKind::Stone,
        )
        .expect("a lantern under stone is predictable");
        assert_eq!(property(state, "hanging"), Some("true"));
    }

    #[test]
    fn a_skull_takes_a_rotation_segment_from_the_yaw() {
        for (yaw, segment) in [(0., "0"), (90., "4"), (180., "8"), (-90., "12")] {
            let (_, state) = place(
                ItemKind::SkeletonSkull,
                Direction::Up,
                1.0,
                yaw,
                BlockKind::Stone,
            )
            .expect("a skull on top of stone is predictable");
            assert_eq!(property(state, "rotation"), Some(segment), "yaw {yaw}");
        }
    }

    #[test]
    fn a_crafter_takes_a_front_and_a_top() {
        let (_, state) = predict(
            &ItemStack::new(ItemKind::Crafter, 1),
            &hit(Direction::Up, 1.0),
            0.,
            90.,
            false,
            MIN_Y,
            HEIGHT,
            world(BlockKind::Stone),
        )
        .expect("a crafter against stone is predictable");
        assert_eq!(property(state, "orientation"), Some("up_south"));

        let (_, state) = place(ItemKind::Crafter, Direction::Up, 1.0, 0., BlockKind::Stone)
            .expect("a crafter against stone is predictable");
        assert_eq!(property(state, "orientation"), Some("north_up"));
    }

    #[test]
    fn scaffolding_measures_its_distance_from_the_ground() {
        let (_, state) = place(
            ItemKind::Scaffolding,
            Direction::Up,
            1.0,
            0.,
            BlockKind::Stone,
        )
        .expect("scaffolding on stone is predictable");
        assert_eq!(property(state, "distance"), Some("0"));
        assert_eq!(property(state, "bottom"), Some("false"));
    }

    fn scaffolding(distance: usize) -> BlockState {
        let mut s = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::Scaffolding));
        let _ = s.set_property("distance", DIGITS[distance]);
        let _ = s.set_property("bottom", "false");
        s.as_block_state()
    }

    fn cells(list: Vec<(BlockPos, BlockState)>) -> impl Fn(BlockPos) -> Option<BlockState> {
        move |pos| {
            Some(
                list.iter()
                    .find(|(p, _)| *p == pos)
                    .map_or(BlockState::AIR, |(_, s)| *s),
            )
        }
    }

    fn at(x: i32, y: i32, z: i32) -> BlockPos {
        BlockPos { x, y, z }
    }

    fn place_scaffolding(
        face: Direction,
        yaw: f32,
        sneak: bool,
        list: Vec<(BlockPos, BlockState)>,
    ) -> Option<(BlockPos, BlockState)> {
        predict(
            &ItemStack::new(ItemKind::Scaffolding, 1),
            &hit(face, 0.5),
            yaw,
            0.,
            sneak,
            MIN_Y,
            HEIGHT,
            cells(list),
        )
    }

    #[test]
    fn scaffolding_clicked_on_its_side_builds_up() {
        let world = vec![
            (at(0, -1, 0), BlockState::from(BlockKind::Stone)),
            (TARGET, scaffolding(0)),
        ];
        let (pos, state) =
            place_scaffolding(Direction::North, 0., false, world).expect("a tower is predictable");
        assert_eq!(pos, at(0, 1, 0));
        assert_eq!(property(state, "distance"), Some("0"));
        assert_eq!(property(state, "bottom"), Some("false"));
    }

    #[test]
    fn scaffolding_walks_up_a_stack() {
        let world = vec![
            (at(0, -1, 0), BlockState::from(BlockKind::Stone)),
            (TARGET, scaffolding(0)),
            (at(0, 1, 0), scaffolding(0)),
            (at(0, 2, 0), scaffolding(0)),
        ];
        let (pos, _) =
            place_scaffolding(Direction::East, 0., false, world).expect("a tower is predictable");
        assert_eq!(pos, at(0, 3, 0));
    }

    #[test]
    fn scaffolding_clicked_on_top_extends_along_the_look() {
        let world = vec![(TARGET, scaffolding(0))];
        let (pos, state) =
            place_scaffolding(Direction::Up, 0., false, world).expect("a bridge is predictable");
        assert_eq!(pos, at(0, 0, 1));
        assert_eq!(property(state, "distance"), Some("1"));
        assert_eq!(property(state, "bottom"), Some("true"));
    }

    #[test]
    fn sneaking_extends_scaffolding_out_of_the_clicked_face() {
        let world = vec![(TARGET, scaffolding(0))];
        let (pos, _) = place_scaffolding(Direction::North, 0., true, world).expect("predictable");
        assert_eq!(pos, at(0, 0, -1));
    }

    #[test]
    fn scaffolding_extends_at_most_seven_along() {
        let run = |n: i32| {
            let mut world = vec![(TARGET, scaffolding(0))];
            world.extend((1..=n).map(|z| (at(0, 0, z), scaffolding(1))));
            place_scaffolding(Direction::Up, 0., false, world)
        };
        assert_eq!(run(6).map(|(pos, _)| pos), Some(at(0, 0, 7)));
        assert_eq!(run(7), None);
    }

    #[test]
    fn unsupported_scaffolding_is_not_placed() {
        assert_eq!(
            place(
                ItemKind::Scaffolding,
                Direction::North,
                0.5,
                0.,
                BlockKind::Stone
            ),
            None
        );
    }

    #[test]
    fn scaffolding_is_never_obstructed() {
        let player = [Aabb {
            min: Vec3 {
                x: 0.2,
                y: 0.0,
                z: 0.2,
            },
            max: Vec3 {
                x: 0.8,
                y: 1.8,
                z: 0.8,
            },
        }];
        assert!(is_unobstructed(scaffolding(0), TARGET, &player));
        assert!(!is_unobstructed(
            BlockState::from(BlockKind::Stone),
            TARGET,
            &player
        ));
    }

    #[test]
    fn only_a_single_snow_layer_is_replaced() {
        let snow = |layers: &str| {
            let mut s = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::Snow));
            let _ = s.set_property("layers", layers);
            s.as_block_state()
        };
        for (layers, expected) in [("1", TARGET), ("2", at(0, 1, 0))] {
            let (pos, _) = predict(
                &ItemStack::new(ItemKind::Stone, 1),
                &hit(Direction::Up, 1.0),
                0.,
                0.,
                false,
                MIN_Y,
                HEIGHT,
                cells(vec![(TARGET, snow(layers))]),
            )
            .expect("stone against snow is predictable");
            assert_eq!(pos, expected, "{layers} layers");
        }
    }

    #[test]
    fn scaffolding_waterlogs_in_source_water_only() {
        let water = |level: &str| {
            let mut s = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::Water));
            let _ = s.set_property("level", level);
            s.as_block_state()
        };
        for (level, logged) in [("0", "true"), ("1", "false")] {
            let world = vec![
                (TARGET, BlockState::from(BlockKind::Stone)),
                (at(0, 1, 0), water(level)),
            ];
            let (_, state) =
                place_scaffolding(Direction::Up, 0., false, world).expect("predictable in water");
            assert_eq!(
                property(state, "waterlogged"),
                Some(logged),
                "water level {level}"
            );
        }
    }

    #[test]
    fn dripstone_hangs_from_a_ceiling() {
        let (_, state) = place(
            ItemKind::PointedDripstone,
            Direction::Down,
            0.0,
            0.,
            BlockKind::Stone,
        )
        .expect("dripstone under stone is predictable");
        assert_eq!(property(state, "vertical_direction"), Some("down"));
        assert_eq!(property(state, "thickness"), Some("tip"));
    }

    #[test]
    fn dripstone_stands_on_a_floor() {
        let (_, state) = place(
            ItemKind::PointedDripstone,
            Direction::Up,
            1.0,
            0.,
            BlockKind::Stone,
        )
        .expect("dripstone on stone is predictable");
        assert_eq!(property(state, "vertical_direction"), Some("up"));
    }

    #[test]
    fn a_lone_mushroom_block_is_skin_on_every_face() {
        let (_, state) = place(
            ItemKind::RedMushroomBlock,
            Direction::Up,
            1.0,
            0.,
            BlockKind::Stone,
        )
        .expect("a mushroom block on stone is predictable");
        for face in ["up", "down", "north", "south", "east", "west"] {
            assert_eq!(property(state, face), Some("true"), "{face}");
        }
    }

    #[test]
    fn concrete_powder_in_water_places_concrete() {
        let (_, state) = place(
            ItemKind::WhiteConcretePowder,
            Direction::Up,
            1.0,
            0.,
            BlockKind::Water,
        )
        .expect("powder into water is predictable");
        assert_eq!(state.as_block_kind(), BlockKind::WhiteConcrete);
    }

    #[test]
    fn concrete_powder_in_the_dry_stays_powder() {
        let (_, state) = place(
            ItemKind::WhiteConcretePowder,
            Direction::Up,
            1.0,
            0.,
            BlockKind::Stone,
        )
        .expect("powder on stone is predictable");
        assert_eq!(state.as_block_kind(), BlockKind::WhiteConcretePowder);
    }

    #[test]
    fn planting_bamboo_gives_a_sapling() {
        let (_, state) = place(ItemKind::Bamboo, Direction::Up, 1.0, 0., BlockKind::Dirt)
            .expect("bamboo on dirt is predictable");
        assert_eq!(state.as_block_kind(), BlockKind::BambooSapling);
    }

    #[test]
    fn bamboo_on_stone_is_not_predicted() {
        assert!(place(ItemKind::Bamboo, Direction::Up, 1.0, 0., BlockKind::Stone).is_none());
    }

    #[test]
    fn a_trapdoor_clicked_on_a_side_hangs_on_that_side() {
        let (_, state) = place(
            ItemKind::OakTrapdoor,
            Direction::North,
            0.2,
            0.,
            BlockKind::Stone,
        )
        .expect("a trapdoor on the side of stone is predictable");
        assert_eq!(property(state, "facing"), Some("north"));
        assert_eq!(property(state, "half"), Some("bottom"));
    }

    #[test]
    fn a_dispenser_faces_back_along_the_look_direction() {
        let (_, state) = predict(
            &ItemStack::new(ItemKind::Dispenser, 1),
            &hit(Direction::Up, 1.0),
            0.,
            90.,
            false,
            MIN_Y,
            HEIGHT,
            world(BlockKind::Stone),
        )
        .expect("a dispenser against stone is predictable");
        assert_eq!(property(state, "facing"), Some("up"));
    }

    #[test]
    fn a_slab_placed_into_water_is_waterlogged() {
        let (pos, state) = place(
            ItemKind::StoneSlab,
            Direction::Up,
            1.0,
            0.,
            BlockKind::Water,
        )
        .expect("a slab into water is predictable");
        assert_eq!(pos, TARGET, "a replaceable block is replaced in place");
        assert_eq!(property(state, "waterlogged"), Some("true"));
    }

    #[test]
    fn a_full_block_placed_into_water_still_predicts() {
        let (pos, state) = place(ItemKind::Stone, Direction::Up, 1.0, 0., BlockKind::Water)
            .expect("stone into water is predictable");
        assert_eq!(pos, TARGET);
        assert_eq!(state, BlockState::from(BlockKind::Stone));
    }

    #[test]
    fn a_non_block_item_is_not_predicted() {
        assert!(place(ItemKind::Apple, Direction::Up, 1.0, 0., BlockKind::Stone).is_none());
        assert!(
            place(
                ItemKind::DiamondSword,
                Direction::Up,
                1.0,
                0.,
                BlockKind::Stone
            )
            .is_none()
        );
        assert!(
            place(
                ItemKind::WaterBucket,
                Direction::Up,
                1.0,
                0.,
                BlockKind::Stone
            )
            .is_none()
        );
        assert!(
            place(
                ItemKind::CowSpawnEgg,
                Direction::Up,
                1.0,
                0.,
                BlockKind::Stone
            )
            .is_none()
        );
        assert!(place(ItemKind::Air, Direction::Up, 1.0, 0., BlockKind::Stone).is_none());
    }

    #[test]
    fn a_click_on_an_interactive_block_is_not_predicted() {
        assert!(place(ItemKind::Stone, Direction::Up, 1.0, 0., BlockKind::Chest).is_none());
        assert!(place(ItemKind::Stone, Direction::Up, 1.0, 0., BlockKind::OakDoor).is_none());
        assert!(place(ItemKind::Stone, Direction::Up, 1.0, 0., BlockKind::Lever).is_none());
    }

    #[test]
    fn a_menu_block_with_no_signal_of_its_own_is_not_predicted() {
        assert!(
            place(
                ItemKind::Stone,
                Direction::Up,
                1.0,
                0.,
                BlockKind::CraftingTable
            )
            .is_none()
        );
        assert!(place(ItemKind::Stone, Direction::Up, 1.0, 0., BlockKind::Anvil).is_none());
    }

    #[test]
    fn stacking_blocks_clicked_with_their_own_item_are_not_predicted() {
        assert!(place(ItemKind::Snow, Direction::Up, 1.0, 0., BlockKind::Snow).is_none());
        assert!(place(ItemKind::Candle, Direction::Up, 1.0, 0., BlockKind::Candle).is_none());
        assert!(
            place(
                ItemKind::SeaPickle,
                Direction::Up,
                1.0,
                0.,
                BlockKind::SeaPickle
            )
            .is_none()
        );
        assert!(
            place(
                ItemKind::TurtleEgg,
                Direction::Up,
                1.0,
                0.,
                BlockKind::TurtleEgg
            )
            .is_none()
        );
        assert!(place(ItemKind::Stone, Direction::Up, 1.0, 0., BlockKind::Stone).is_some());
    }

    #[test]
    fn a_stair_clicked_with_its_own_item_goes_beside_it() {
        let (pos, state) = place(
            ItemKind::StoneStairs,
            Direction::Up,
            1.0,
            0.,
            BlockKind::StoneStairs,
        )
        .expect("a stair on a stair is predictable");
        assert_eq!(pos, at(0, 1, 0));
        assert_eq!(state.as_block_kind(), BlockKind::StoneStairs);
    }

    fn place_in(
        item: ItemKind,
        face: Direction,
        frac_y: f64,
        yaw: f32,
        sneak: bool,
        list: Vec<(BlockPos, BlockState)>,
    ) -> Option<(BlockPos, BlockState)> {
        predict(
            &ItemStack::new(item, 1),
            &hit(face, frac_y),
            yaw,
            0.,
            sneak,
            MIN_Y,
            HEIGHT,
            cells(list),
        )
    }

    #[test]
    fn a_bottom_slab_clicked_on_its_open_half_becomes_double() {
        let wet = with(
            BlockKind::StoneSlab,
            &[("type", "bottom"), ("waterlogged", "true")],
        );
        for (face, frac_y) in [(Direction::Up, 1.0), (Direction::North, 0.75)] {
            let (pos, state) = place_in(
                ItemKind::StoneSlab,
                face,
                frac_y,
                0.,
                false,
                vec![(TARGET, wet)],
            )
            .expect("a slab onto a slab is predictable");
            assert_eq!(pos, TARGET, "{face:?}");
            assert_eq!(
                state,
                with(
                    BlockKind::StoneSlab,
                    &[("type", "double"), ("waterlogged", "false")]
                ),
                "{face:?}"
            );
        }
    }

    #[test]
    fn a_bottom_slab_clicked_low_on_its_side_is_not_doubled() {
        let bottom = with(BlockKind::StoneSlab, &[("type", "bottom")]);
        let (pos, state) = place_in(
            ItemKind::StoneSlab,
            Direction::North,
            0.25,
            0.,
            false,
            vec![(TARGET, bottom)],
        )
        .expect("a slab beside a slab is predictable");
        assert_eq!(pos, at(0, 0, -1));
        assert_eq!(property(state, "type"), Some("bottom"));
    }

    #[test]
    fn a_top_slab_doubles_from_below_only() {
        let top = with(BlockKind::StoneSlab, &[("type", "top")]);
        let (pos, state) = place_in(
            ItemKind::StoneSlab,
            Direction::Down,
            0.0,
            0.,
            false,
            vec![(TARGET, top)],
        )
        .expect("a slab under a top slab is predictable");
        assert_eq!(pos, TARGET);
        assert_eq!(property(state, "type"), Some("double"));

        let (pos, state) = place_in(
            ItemKind::StoneSlab,
            Direction::Up,
            1.0,
            0.,
            false,
            vec![(TARGET, top)],
        )
        .expect("a slab on a top slab is predictable");
        assert_eq!(pos, at(0, 1, 0));
        assert_eq!(property(state, "type"), Some("bottom"));
    }

    #[test]
    fn a_double_or_different_slab_is_not_merged() {
        let double = with(BlockKind::StoneSlab, &[("type", "double")]);
        let (pos, _) = place_in(
            ItemKind::StoneSlab,
            Direction::Up,
            1.0,
            0.,
            false,
            vec![(TARGET, double)],
        )
        .expect("a slab on a double slab is predictable");
        assert_eq!(pos, at(0, 1, 0));

        let (pos, state) = place(
            ItemKind::StoneSlab,
            Direction::Up,
            1.0,
            0.,
            BlockKind::OakSlab,
        )
        .expect("a stone slab on an oak slab is predictable");
        assert_eq!(pos, at(0, 1, 0));
        assert_eq!(state.as_block_kind(), BlockKind::StoneSlab);
    }

    #[test]
    fn a_slab_in_the_neighbouring_cell_is_doubled() {
        let world = vec![
            (TARGET, BlockState::from(BlockKind::Stone)),
            (
                at(0, 0, -1),
                with(BlockKind::StoneSlab, &[("type", "bottom")]),
            ),
        ];
        let (pos, state) = place_in(
            ItemKind::StoneSlab,
            Direction::North,
            0.25,
            0.,
            false,
            world,
        )
        .expect("a slab into a slab's cell is predictable");
        assert_eq!(pos, at(0, 0, -1));
        assert_eq!(property(state, "type"), Some("double"));
    }

    #[test]
    fn sneaking_places_against_an_interactive_block() {
        for clicked in [
            BlockKind::Chest,
            BlockKind::CraftingTable,
            BlockKind::OakDoor,
        ] {
            let (pos, state) = predict(
                &ItemStack::new(ItemKind::Stone, 1),
                &hit(Direction::Up, 1.0),
                0.,
                0.,
                true,
                MIN_Y,
                HEIGHT,
                world(clicked),
            )
            .unwrap_or_else(|| panic!("stone on a sneak-clicked {clicked:?} is predictable"));
            assert_eq!(pos, at(0, 1, 0), "{clicked:?}");
            assert_eq!(state, BlockState::from(BlockKind::Stone), "{clicked:?}");
        }
    }

    #[test]
    fn placed_leaves_are_persistent_with_a_distance() {
        let leaves = |neighbour: Option<BlockState>| {
            let mut world = vec![(TARGET, BlockState::from(BlockKind::Stone))];
            world.extend(neighbour.map(|state| (at(1, 1, 0), state)));
            let (pos, state) = place_in(ItemKind::OakLeaves, Direction::Up, 1.0, 0., false, world)
                .expect("leaves on stone are predictable");
            assert_eq!(pos, at(0, 1, 0));
            assert_eq!(property(state, "persistent"), Some("true"));
            assert_eq!(property(state, "waterlogged"), Some("false"));
            property(state, "distance")
        };
        assert_eq!(leaves(None), Some("7"));
        assert_eq!(leaves(Some(BlockState::from(BlockKind::OakLog))), Some("1"));
        assert_eq!(
            leaves(Some(with(BlockKind::BirchLeaves, &[("distance", "3")]))),
            Some("4")
        );
        assert_eq!(leaves(Some(scaffolding(0))), Some("7"));
    }

    #[test]
    fn a_ladder_hangs_on_the_block_it_was_clicked_against() {
        let (pos, state) = place(
            ItemKind::Ladder,
            Direction::North,
            0.5,
            90.,
            BlockKind::Stone,
        )
        .expect("a ladder on the side of stone is predictable");
        assert_eq!(pos, at(0, 0, -1));
        assert_eq!(property(state, "facing"), Some("north"));
    }

    #[test]
    fn a_ladder_with_no_wall_is_not_predicted() {
        assert!(place(ItemKind::Ladder, Direction::Up, 1.0, 0., BlockKind::Stone).is_none());
    }

    fn single_chest() -> BlockState {
        with(BlockKind::Chest, &[("facing", "north"), ("type", "single")])
    }

    #[test]
    fn sneaking_against_a_chest_side_pairs_with_it() {
        for (face, pos, chest_type) in [
            (Direction::West, at(-1, 0, 0), "left"),
            (Direction::East, at(1, 0, 0), "right"),
        ] {
            let (placed_at, state) = place_in(
                ItemKind::Chest,
                face,
                0.5,
                90.,
                true,
                vec![(TARGET, single_chest())],
            )
            .unwrap_or_else(|| panic!("a chest beside a chest on {face:?} is predictable"));
            assert_eq!(placed_at, pos, "{face:?}");
            assert_eq!(property(state, "facing"), Some("north"), "{face:?}");
            assert_eq!(property(state, "type"), Some(chest_type), "{face:?}");
        }
    }

    #[test]
    fn a_chest_beside_a_matching_chest_pairs_unless_sneaking() {
        let world = || {
            vec![
                (TARGET, BlockState::from(BlockKind::Stone)),
                (at(1, 1, 0), single_chest()),
            ]
        };
        let (_, state) = place_in(ItemKind::Chest, Direction::Up, 1.0, 0., false, world())
            .expect("a chest on stone is predictable");
        assert_eq!(property(state, "facing"), Some("north"));
        assert_eq!(property(state, "type"), Some("left"));

        let (_, state) = place_in(ItemKind::Chest, Direction::Up, 1.0, 0., true, world())
            .expect("a chest on stone is predictable");
        assert_eq!(property(state, "type"), Some("single"));
    }

    #[test]
    fn grass_under_snow_is_snowy() {
        for (above, snowy) in [
            (BlockState::from(BlockKind::Snow), "true"),
            (BlockState::AIR, "false"),
        ] {
            let world = vec![
                (TARGET, BlockState::from(BlockKind::Stone)),
                (at(0, 1, -1), above),
            ];
            let (pos, state) = place_in(
                ItemKind::GrassBlock,
                Direction::North,
                0.5,
                0.,
                false,
                world,
            )
            .expect("grass beside stone is predictable");
            assert_eq!(pos, at(0, 0, -1));
            assert_eq!(property(state, "snowy"), Some(snowy));
        }
    }

    #[test]
    fn a_fence_gate_beside_a_wall_is_in_wall() {
        let world = vec![
            (TARGET, BlockState::from(BlockKind::Stone)),
            (at(1, 1, 0), BlockState::from(BlockKind::CobblestoneWall)),
        ];
        let (_, state) = place_in(ItemKind::OakFenceGate, Direction::Up, 1.0, 0., false, world)
            .expect("a fence gate on stone is predictable");
        assert_eq!(property(state, "in_wall"), Some("true"));
    }

    #[test]
    fn a_campfire_on_hay_is_a_signal_fire() {
        let (_, state) = place(
            ItemKind::Campfire,
            Direction::Up,
            1.0,
            0.,
            BlockKind::HayBlock,
        )
        .expect("a campfire on hay is predictable");
        assert_eq!(property(state, "signal_fire"), Some("true"));
    }

    #[test]
    fn a_big_dripleaf_continues_the_stem_below() {
        let stem = with(BlockKind::BigDripleafStem, &[("facing", "east")]);
        let block_at = cells(vec![(TARGET, stem)]);
        let facing = exceptional_facing(
            BlockKind::BigDripleaf,
            &hit(Direction::Up, 1.0),
            0.,
            0.,
            at(0, 1, 0),
            &block_at,
        );
        assert_eq!(facing, Some(Direction::East));
        let alone = exceptional_facing(
            BlockKind::BigDripleaf,
            &hit(Direction::Up, 1.0),
            0.,
            0.,
            at(0, 5, 0),
            &block_at,
        );
        assert_eq!(alone, None);
    }

    #[test]
    fn blocks_that_need_support_are_not_predicted() {
        assert!(
            place(
                ItemKind::OakSapling,
                Direction::Up,
                1.0,
                0.,
                BlockKind::Stone
            )
            .is_none()
        );
        assert!(
            place(
                ItemKind::WheatSeeds,
                Direction::Up,
                1.0,
                0.,
                BlockKind::Farmland
            )
            .is_none()
        );
    }

    #[test]
    fn two_block_placements_are_not_predicted() {
        assert!(place(ItemKind::OakDoor, Direction::Up, 1.0, 0., BlockKind::Stone).is_none());
        assert!(place(ItemKind::RedBed, Direction::Up, 1.0, 0., BlockKind::Stone).is_none());
    }

    fn player_at(x: f64, y: f64, z: f64) -> Aabb {
        Aabb {
            min: Vec3 {
                x: x - 0.3,
                y,
                z: z - 0.3,
            },
            max: Vec3 {
                x: x + 0.3,
                y: y + 1.8,
                z: z + 0.3,
            },
        }
    }

    #[test]
    fn a_block_in_the_players_own_cell_is_obstructed() {
        let stone = BlockState::from(BlockKind::Stone);
        let feet = BlockPos { x: 0, y: 1, z: 0 };
        let head = BlockPos { x: 0, y: 2, z: 0 };
        let player = [player_at(0.5, 1.0, 0.5)];
        assert!(!is_unobstructed(stone, feet, &player));
        assert!(!is_unobstructed(stone, head, &player));
    }

    #[test]
    fn a_block_touching_the_player_is_not_obstructed() {
        let stone = BlockState::from(BlockKind::Stone);
        let player = [player_at(0.5, 1.0, 0.5)];
        assert!(is_unobstructed(
            stone,
            BlockPos { x: 0, y: 0, z: 0 },
            &player
        ));
        assert!(is_unobstructed(
            stone,
            BlockPos { x: 0, y: 3, z: 0 },
            &player
        ));
        assert!(is_unobstructed(
            stone,
            BlockPos { x: 1, y: 1, z: 0 },
            &player
        ));
    }

    #[test]
    fn a_bottom_slab_under_a_player_standing_on_it_is_not_obstructed() {
        let slab = BlockState::from(BlockKind::StoneSlab);
        let player = [player_at(0.5, 1.5, 0.5)];
        assert!(is_unobstructed(
            slab,
            BlockPos { x: 0, y: 1, z: 0 },
            &player
        ));
        let stone = BlockState::from(BlockKind::Stone);
        assert!(!is_unobstructed(
            stone,
            BlockPos { x: 0, y: 1, z: 0 },
            &player
        ));
    }

    #[test]
    fn a_placement_outside_the_world_is_not_predicted() {
        let top = BlockPos {
            x: 0,
            y: MIN_Y + HEIGHT as i32 - 1,
            z: 0,
        };
        let hit = BlockHitResult {
            location: Vec3 {
                x: 0.5,
                y: top.y as f64 + 1.,
                z: 0.5,
            },
            direction: Direction::Up,
            block_pos: top,
            inside: false,
            world_border: false,
            miss: false,
        };
        assert!(
            predict(
                &ItemStack::new(ItemKind::Stone, 1),
                &hit,
                0.,
                0.,
                false,
                MIN_Y,
                HEIGHT,
                |pos| Some(if pos == top {
                    BlockState::from(BlockKind::Stone)
                } else {
                    BlockState::AIR
                }),
            )
            .is_none()
        );
    }

    #[test]
    fn an_unloaded_block_is_not_predicted() {
        assert!(
            predict(
                &ItemStack::new(ItemKind::Stone, 1),
                &hit(Direction::Up, 1.0),
                0.,
                0.,
                false,
                MIN_Y,
                HEIGHT,
                |_| None,
            )
            .is_none()
        );
    }

    fn bucket(
        item: ItemKind,
        face: Direction,
        clicked: BlockKind,
    ) -> Option<(BlockPos, BlockState)> {
        predict_bucket(
            &ItemStack::new(item, 1),
            &hit(face, 0.5),
            MIN_Y,
            HEIGHT,
            world(clicked),
        )
    }

    #[test]
    fn a_water_bucket_places_a_source_next_to_a_plain_block() {
        let (pos, state) = bucket(ItemKind::WaterBucket, Direction::Up, BlockKind::Stone)
            .expect("water into air is predictable");
        assert_eq!(pos, TARGET + Direction::Up.normal());
        assert_eq!(state, BlockState::from(BlockKind::Water));
    }

    #[test]
    fn a_lava_bucket_places_a_source_next_to_a_plain_block() {
        let (pos, state) = bucket(ItemKind::LavaBucket, Direction::Up, BlockKind::Stone)
            .expect("lava into air is predictable");
        assert_eq!(pos, TARGET + Direction::Up.normal());
        assert_eq!(state, BlockState::from(BlockKind::Lava));
    }

    #[test]
    fn a_water_bucket_waterlogs_a_container_block_in_place() {
        let (pos, state) = bucket(ItemKind::WaterBucket, Direction::Up, BlockKind::OakFence)
            .expect("water into a fence is predictable");
        assert_eq!(pos, TARGET, "the clicked block itself is waterlogged");
        assert_eq!(property(state, "waterlogged"), Some("true"));
    }

    #[test]
    fn a_lava_bucket_never_waterlogs() {
        let (pos, _) = bucket(ItemKind::LavaBucket, Direction::Up, BlockKind::OakFence)
            .expect("lava next to a fence is predictable");
        assert_eq!(pos, TARGET + Direction::Up.normal());
    }

    #[test]
    fn a_bucket_with_no_replaceable_target_is_not_predicted() {
        assert!(
            predict_bucket(
                &ItemStack::new(ItemKind::WaterBucket, 1),
                &hit(Direction::Up, 0.5),
                MIN_Y,
                HEIGHT,
                |_| Some(BlockState::from(BlockKind::Stone)),
            )
            .is_none()
        );
    }

    #[test]
    fn a_non_bucket_item_is_not_bucket_predicted() {
        assert!(bucket(ItemKind::Stone, Direction::Up, BlockKind::Stone).is_none());
        assert!(bucket(ItemKind::Bucket, Direction::Up, BlockKind::Stone).is_none());
    }

    #[test]
    fn a_water_bucket_places_a_source_into_flowing_water() {
        let neighbor = TARGET + Direction::Up.normal();
        let (pos, state) = predict_bucket(
            &ItemStack::new(ItemKind::WaterBucket, 1),
            &hit(Direction::Up, 0.5),
            MIN_Y,
            HEIGHT,
            |pos| {
                Some(if pos == TARGET || pos == neighbor {
                    BlockState::from(BlockKind::Water)
                } else {
                    BlockState::AIR
                })
            },
        )
        .expect("water into flowing water is predictable");
        assert_eq!(pos, neighbor);
        assert_eq!(state, BlockState::from(BlockKind::Water));
    }

    #[test]
    fn a_lava_bucket_places_a_source_into_existing_lava() {
        let neighbor = TARGET + Direction::Up.normal();
        let (pos, state) = predict_bucket(
            &ItemStack::new(ItemKind::LavaBucket, 1),
            &hit(Direction::Up, 0.5),
            MIN_Y,
            HEIGHT,
            |pos| {
                Some(if pos == TARGET || pos == neighbor {
                    BlockState::from(BlockKind::Lava)
                } else {
                    BlockState::AIR
                })
            },
        )
        .expect("lava into lava is predictable");
        assert_eq!(pos, neighbor);
        assert_eq!(state, BlockState::from(BlockKind::Lava));
    }

    #[test]
    fn an_empty_bucket_picks_up_a_water_source() {
        let (pos, new, filled) =
            fill(BlockState::from(BlockKind::Water)).expect("a water source is picked up");
        assert_eq!(pos, TARGET);
        assert_eq!(new, BlockState::AIR);
        assert_eq!(filled, ItemKind::WaterBucket);
    }

    fn fill(state: BlockState) -> Option<(BlockPos, BlockState, ItemKind)> {
        predict_bucket_fill(
            &ItemStack::new(ItemKind::Bucket, 1),
            &hit(Direction::Up, 0.5),
            false,
            |pos| {
                Some(if pos == TARGET {
                    state
                } else {
                    BlockState::AIR
                })
            },
        )
    }

    #[test]
    fn an_empty_bucket_picks_up_powder_snow() {
        let (_, new, filled) =
            fill(BlockState::from(BlockKind::PowderSnow)).expect("powder snow is picked up");
        assert_eq!(new, BlockState::AIR);
        assert_eq!(filled, ItemKind::PowderSnowBucket);
    }

    #[test]
    fn an_empty_bucket_drains_a_waterlogged_block() {
        let dry = with(BlockKind::OakSlab, &[("waterlogged", "false")]);
        let wet = with(BlockKind::OakSlab, &[("waterlogged", "true")]);
        let (_, new, filled) = fill(wet).expect("a waterlogged slab is drained");
        assert_eq!(new, dry);
        assert_eq!(filled, ItemKind::WaterBucket);
        assert!(fill(dry).is_none());
    }

    #[test]
    fn an_empty_bucket_declines_flowing_water() {
        let mut flowing = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::Water));
        let _ = flowing.set_property("level", "1");
        let flowing = flowing.as_block_state();
        assert!(fill(flowing).is_none());
    }

    #[test]
    fn a_full_bucket_is_not_fill_predicted() {
        assert!(
            predict_bucket_fill(
                &ItemStack::new(ItemKind::WaterBucket, 1),
                &hit(Direction::Up, 0.5),
                false,
                |_| Some(BlockState::from(BlockKind::Water)),
            )
            .is_none()
        );
    }

    fn with(kind: BlockKind, pairs: &[(&str, &str)]) -> BlockState {
        let mut block = Box::<dyn BlockTrait>::from(BlockState::from(kind));
        for (name, value) in pairs {
            block
                .set_property(name, value)
                .expect("the test names a property this block has");
        }
        block.as_block_state()
    }

    #[test]
    fn a_door_click_opens_both_halves() {
        let lower = with(BlockKind::OakDoor, &[("half", "lower"), ("open", "false")]);
        let upper = with(BlockKind::OakDoor, &[("half", "upper"), ("open", "false")]);
        let opened = toggled_open(lower).expect("a wooden door opens by hand");
        assert_eq!(
            opened,
            with(BlockKind::OakDoor, &[("half", "lower"), ("open", "true")])
        );

        let (pos, old, new) = door_other_half(lower, opened, TARGET, |p| {
            Some(if p == TARGET.up(1) {
                upper
            } else {
                BlockState::AIR
            })
        })
        .expect("the block above is this door's upper half");
        assert_eq!(pos, TARGET.up(1));
        assert_eq!(old, upper);
        assert_eq!(
            new,
            with(BlockKind::OakDoor, &[("half", "upper"), ("open", "true")])
        );
    }

    #[test]
    fn a_click_on_the_upper_half_carries_to_the_lower() {
        let upper = with(BlockKind::OakDoor, &[("half", "upper"), ("open", "true")]);
        let lower = with(BlockKind::OakDoor, &[("half", "lower"), ("open", "true")]);
        let closed = toggled_open(upper).expect("a wooden door closes by hand");
        let (pos, _, new) = door_other_half(upper, closed, TARGET, |p| {
            Some(if p == TARGET.down(1) {
                lower
            } else {
                BlockState::AIR
            })
        })
        .expect("the block below is this door's lower half");
        assert_eq!(pos, TARGET.down(1));
        assert_eq!(
            new,
            with(BlockKind::OakDoor, &[("half", "lower"), ("open", "false")])
        );
    }

    #[test]
    fn an_iron_door_does_not_open_by_hand() {
        assert!(toggled_open(BlockState::from(BlockKind::IronDoor)).is_none());
        assert!(toggled_open(BlockState::from(BlockKind::IronTrapdoor)).is_none());
    }

    #[test]
    fn a_trapdoor_toggles_alone() {
        let shut = with(BlockKind::OakTrapdoor, &[("open", "false")]);
        let opened = toggled_open(shut).expect("a wooden trapdoor opens by hand");
        assert!(door_other_half(shut, opened, TARGET, |_| Some(BlockState::AIR)).is_none());
    }

    #[test]
    fn a_lone_door_half_has_no_partner() {
        let lower = with(BlockKind::OakDoor, &[("half", "lower"), ("open", "false")]);
        let opened = toggled_open(lower).expect("a wooden door opens by hand");
        assert!(door_other_half(lower, opened, TARGET, |_| Some(BlockState::AIR)).is_none());
        let other = with(
            BlockKind::SpruceDoor,
            &[("half", "upper"), ("open", "false")],
        );
        assert!(door_other_half(lower, opened, TARGET, |_| Some(other)).is_none());
    }

    fn look_place(
        item: ItemKind,
        hit: &BlockHitResult,
        yaw: f32,
        pitch: f32,
        sneak: bool,
        list: Vec<(BlockPos, BlockState)>,
    ) -> Option<Placed> {
        predict_placed(
            &ItemStack::new(item, 1),
            hit,
            yaw,
            pitch,
            sneak,
            MIN_Y,
            HEIGHT,
            cells(list),
        )
    }

    fn on_stone(mut list: Vec<(BlockPos, BlockState)>) -> Vec<(BlockPos, BlockState)> {
        list.push((TARGET, BlockState::from(BlockKind::Stone)));
        list
    }

    #[test]
    fn an_observer_placed_looking_north_faces_north() {
        let placed = look_place(
            ItemKind::Observer,
            &hit(Direction::Up, 1.0),
            180.,
            0.,
            false,
            on_stone(vec![]),
        )
        .expect("an observer on stone is predictable");
        assert_eq!(property(placed.state, "facing"), Some("north"));
    }

    #[test]
    fn a_piston_faces_the_player() {
        for item in [ItemKind::Piston, ItemKind::StickyPiston] {
            let placed = look_place(
                item,
                &hit(Direction::Up, 1.0),
                180.,
                0.,
                false,
                on_stone(vec![]),
            )
            .unwrap_or_else(|| panic!("{item:?} on stone is predictable"));
            assert_eq!(property(placed.state, "facing"), Some("south"), "{item:?}");
            assert_eq!(
                property(placed.state, "extended"),
                Some("false"),
                "{item:?}"
            );
            let placed = look_place(
                item,
                &hit(Direction::Down, 0.0),
                0.,
                -90.,
                false,
                on_stone(vec![]),
            )
            .unwrap_or_else(|| panic!("{item:?} under stone is predictable"));
            assert_eq!(property(placed.state, "facing"), Some("down"), "{item:?}");
        }
    }

    #[test]
    fn a_dropper_faces_back_along_the_look() {
        let placed = look_place(
            ItemKind::Dropper,
            &hit(Direction::Up, 1.0),
            90.,
            0.,
            false,
            on_stone(vec![]),
        )
        .expect("a dropper on stone is predictable");
        assert_eq!(property(placed.state, "facing"), Some("east"));
    }

    #[test]
    fn a_hopper_clicked_from_below_points_down() {
        let (_, state) = place(ItemKind::Hopper, Direction::Down, 0.0, 0., BlockKind::Stone)
            .expect("a hopper under stone is predictable");
        assert_eq!(property(state, "facing"), Some("down"));
    }

    #[test]
    fn a_diode_faces_opposite_the_player() {
        for item in [ItemKind::Repeater, ItemKind::Comparator] {
            let (pos, state) = place(item, Direction::Up, 1.0, 0., BlockKind::Stone)
                .unwrap_or_else(|| panic!("{item:?} on stone is predictable"));
            assert_eq!(pos, at(0, 1, 0));
            assert_eq!(property(state, "facing"), Some("north"), "{item:?}");
        }
    }

    #[test]
    fn a_diode_needs_a_rigid_floor() {
        assert!(
            place(
                ItemKind::Repeater,
                Direction::North,
                0.5,
                0.,
                BlockKind::Stone
            )
            .is_none()
        );
        assert!(
            place(
                ItemKind::Comparator,
                Direction::Up,
                1.0,
                0.,
                BlockKind::StoneSlab
            )
            .is_none()
        );
        let on_hopper = look_place(
            ItemKind::Repeater,
            &hit(Direction::Up, 1.0),
            0.,
            0.,
            true,
            vec![(TARGET, BlockState::from(BlockKind::Hopper))],
        );
        assert!(on_hopper.is_some(), "a hopper's top is rigid");
    }

    #[test]
    fn a_repeater_beside_a_powered_repeater_pointing_in_is_locked() {
        let side = with(
            BlockKind::Repeater,
            &[("facing", "east"), ("powered", "true")],
        );
        let placed = look_place(
            ItemKind::Repeater,
            &hit(Direction::Up, 1.0),
            0.,
            0.,
            false,
            on_stone(vec![(at(1, 1, 0), side)]),
        )
        .expect("a repeater on stone is predictable");
        assert_eq!(property(placed.state, "locked"), Some("true"));

        let away = with(
            BlockKind::Repeater,
            &[("facing", "west"), ("powered", "true")],
        );
        let placed = look_place(
            ItemKind::Repeater,
            &hit(Direction::Up, 1.0),
            0.,
            0.,
            false,
            on_stone(vec![(at(1, 1, 0), away)]),
        )
        .expect("a repeater on stone is predictable");
        assert_eq!(property(placed.state, "locked"), Some("false"));
    }

    #[test]
    fn redstone_wire_is_a_cross_alone_and_a_line_beside_wire() {
        let (pos, state) = place(ItemKind::Redstone, Direction::Up, 1.0, 0., BlockKind::Stone)
            .expect("redstone on stone is predictable");
        assert_eq!(pos, at(0, 1, 0));
        assert_eq!(state.as_block_kind(), BlockKind::RedstoneWire);
        for side in ["north", "east", "south", "west"] {
            assert_eq!(property(state, side), Some("side"), "{side}");
        }

        let wire = BlockState::from(BlockKind::RedstoneWire);
        let placed = look_place(
            ItemKind::Redstone,
            &hit(Direction::Up, 1.0),
            0.,
            0.,
            false,
            on_stone(vec![(at(1, 1, 0), wire)]),
        )
        .expect("redstone on stone is predictable");
        assert_eq!(property(placed.state, "east"), Some("side"));
        assert_eq!(property(placed.state, "west"), Some("side"));
        assert_eq!(property(placed.state, "north"), Some("none"));
        assert_eq!(property(placed.state, "south"), Some("none"));
    }

    #[test]
    fn redstone_wire_needs_a_sturdy_top_or_a_hopper() {
        assert!(
            place(
                ItemKind::Redstone,
                Direction::Up,
                1.0,
                0.,
                BlockKind::StoneSlab
            )
            .is_none()
        );
        assert!(
            place(
                ItemKind::Redstone,
                Direction::North,
                0.5,
                0.,
                BlockKind::Stone
            )
            .is_none()
        );
        let on_hopper = look_place(
            ItemKind::Redstone,
            &hit(Direction::Up, 1.0),
            0.,
            0.,
            true,
            vec![(TARGET, BlockState::from(BlockKind::Hopper))],
        );
        assert!(on_hopper.is_some());
    }

    #[test]
    fn a_torch_stands_on_a_top_face_and_hangs_on_a_side() {
        for (item, standing, wall) in [
            (ItemKind::Torch, BlockKind::Torch, BlockKind::WallTorch),
            (
                ItemKind::SoulTorch,
                BlockKind::SoulTorch,
                BlockKind::SoulWallTorch,
            ),
            (
                ItemKind::CopperTorch,
                BlockKind::CopperTorch,
                BlockKind::CopperWallTorch,
            ),
            (
                ItemKind::RedstoneTorch,
                BlockKind::RedstoneTorch,
                BlockKind::RedstoneWallTorch,
            ),
        ] {
            let (pos, state) = place(item, Direction::Up, 1.0, 0., BlockKind::Stone)
                .unwrap_or_else(|| panic!("{item:?} on stone is predictable"));
            assert_eq!(pos, at(0, 1, 0));
            assert_eq!(state.as_block_kind(), standing, "{item:?}");

            let (pos, state) = place(item, Direction::North, 0.5, 0., BlockKind::Stone)
                .unwrap_or_else(|| panic!("{item:?} on the side of stone is predictable"));
            assert_eq!(pos, at(0, 0, -1));
            assert_eq!(state.as_block_kind(), wall, "{item:?}");
            assert_eq!(property(state, "facing"), Some("north"), "{item:?}");
        }
        let (_, state) = place(
            ItemKind::RedstoneTorch,
            Direction::North,
            0.5,
            0.,
            BlockKind::Stone,
        )
        .expect("a redstone torch on the side of stone is predictable");
        assert_eq!(property(state, "lit"), Some("true"));
    }

    #[test]
    fn a_torch_on_a_fence_side_is_refused() {
        assert!(
            place(
                ItemKind::Torch,
                Direction::North,
                0.5,
                0.,
                BlockKind::OakFence
            )
            .is_none()
        );
        assert!(place(ItemKind::Torch, Direction::Up, 1.0, 0., BlockKind::OakFence).is_some());
    }

    #[test]
    fn a_grindstone_survives_anywhere() {
        let (_, state) = place(
            ItemKind::Grindstone,
            Direction::Up,
            1.0,
            0.,
            BlockKind::ShortGrass,
        )
        .expect("a grindstone needs no support");
        assert_eq!(property(state, "face"), Some("wall"));
        assert_eq!(property(state, "facing"), Some("north"));
    }

    fn top_hit_at(x: f64) -> BlockHitResult {
        BlockHitResult {
            location: Vec3 { x, y: 1.0, z: 0.5 },
            direction: Direction::Up,
            block_pos: TARGET,
            inside: false,
            world_border: false,
            miss: false,
        }
    }

    #[test]
    fn a_door_hinge_follows_the_click_when_nothing_is_beside_it() {
        let placed = look_place(
            ItemKind::OakDoor,
            &top_hit_at(0.5),
            0.,
            0.,
            false,
            on_stone(vec![]),
        )
        .expect("a door on stone is predictable");
        assert_eq!(placed.pos, at(0, 1, 0));
        assert_eq!(property(placed.state, "facing"), Some("south"));
        assert_eq!(property(placed.state, "half"), Some("lower"));
        assert_eq!(property(placed.state, "hinge"), Some("left"));
        let (upper_pos, upper) = placed.partner.expect("a door has an upper half");
        assert_eq!(upper_pos, at(0, 2, 0));
        assert_eq!(property(upper, "half"), Some("upper"));
        assert_eq!(property(upper, "hinge"), Some("left"));

        let placed = look_place(
            ItemKind::OakDoor,
            &top_hit_at(0.25),
            0.,
            0.,
            false,
            on_stone(vec![]),
        )
        .expect("a door on stone is predictable");
        assert_eq!(property(placed.state, "hinge"), Some("right"));
    }

    #[test]
    fn a_door_hinge_goes_to_the_wall_or_away_from_a_door() {
        let wall = vec![(at(-1, 1, 0), BlockState::from(BlockKind::Stone))];
        let placed = look_place(
            ItemKind::OakDoor,
            &top_hit_at(0.5),
            0.,
            0.,
            false,
            on_stone(wall),
        )
        .expect("a door beside stone is predictable");
        assert_eq!(property(placed.state, "hinge"), Some("right"));

        let door = with(BlockKind::OakDoor, &[("half", "lower")]);
        let placed = look_place(
            ItemKind::OakDoor,
            &top_hit_at(0.5),
            0.,
            0.,
            false,
            on_stone(vec![(at(1, 1, 0), door)]),
        )
        .expect("a door beside a door is predictable");
        assert_eq!(property(placed.state, "hinge"), Some("right"));
    }

    #[test]
    fn a_door_without_room_or_floor_is_refused() {
        let side = hit(Direction::North, 0.5);
        assert!(look_place(ItemKind::OakDoor, &side, 0., 0., false, on_stone(vec![])).is_none());
        let roof = vec![(at(0, 2, 0), BlockState::from(BlockKind::Stone))];
        assert!(
            look_place(
                ItemKind::OakDoor,
                &top_hit_at(0.5),
                0.,
                0.,
                false,
                on_stone(roof)
            )
            .is_none()
        );
    }

    #[test]
    fn a_bed_places_its_head_ahead_of_the_foot() {
        let placed = look_place(
            ItemKind::RedBed,
            &hit(Direction::Up, 1.0),
            0.,
            0.,
            false,
            on_stone(vec![]),
        )
        .expect("a bed on stone is predictable");
        assert_eq!(placed.pos, at(0, 1, 0));
        assert_eq!(property(placed.state, "facing"), Some("south"));
        assert_eq!(property(placed.state, "part"), Some("foot"));
        let (head_pos, head) = placed.partner.expect("a bed has a head");
        assert_eq!(head_pos, at(0, 1, 1));
        assert_eq!(property(head, "part"), Some("head"));

        let blocked = vec![(at(0, 1, 1), BlockState::from(BlockKind::Stone))];
        assert!(
            look_place(
                ItemKind::RedBed,
                &hit(Direction::Up, 1.0),
                0.,
                0.,
                false,
                on_stone(blocked)
            )
            .is_none()
        );
    }

    #[test]
    fn trapdoor_halves_follow_the_click() {
        let (_, state) = place(
            ItemKind::OakTrapdoor,
            Direction::North,
            0.8,
            0.,
            BlockKind::Stone,
        )
        .expect("a trapdoor on the side of stone is predictable");
        assert_eq!(property(state, "half"), Some("top"));
        assert_eq!(property(state, "facing"), Some("north"));

        let (_, state) = place(
            ItemKind::OakTrapdoor,
            Direction::Up,
            1.0,
            0.,
            BlockKind::Stone,
        )
        .expect("a trapdoor on stone is predictable");
        assert_eq!(property(state, "half"), Some("bottom"));
        assert_eq!(property(state, "facing"), Some("north"));

        let (_, state) = place(
            ItemKind::OakTrapdoor,
            Direction::Down,
            0.0,
            0.,
            BlockKind::Stone,
        )
        .expect("a trapdoor under stone is predictable");
        assert_eq!(property(state, "half"), Some("top"));
    }

    #[test]
    fn a_sign_takes_a_rotation_segment_from_the_yaw() {
        for (yaw, segment) in [
            (0., "8"),
            (11., "8"),
            (12., "9"),
            (-11., "8"),
            (-12., "7"),
            (90., "12"),
            (-90., "4"),
        ] {
            let (_, state) = place(ItemKind::OakSign, Direction::Up, 1.0, yaw, BlockKind::Stone)
                .unwrap_or_else(|| panic!("a sign on stone at yaw {yaw} is predictable"));
            assert_eq!(state.as_block_kind(), BlockKind::OakSign);
            assert_eq!(property(state, "rotation"), Some(segment), "yaw {yaw}");
        }
    }

    #[test]
    fn rotation_segments_wrap_below_zero_and_past_a_turn() {
        assert_eq!(rotation_segment(-12.), "15");
        assert_eq!(rotation_segment(-11.), "0");
        assert_eq!(rotation_segment(348.), "15");
    }

    #[test]
    fn signs_banners_and_skulls_hang_on_a_side_face() {
        for (item, wall) in [
            (ItemKind::OakSign, BlockKind::OakWallSign),
            (ItemKind::WhiteBanner, BlockKind::WhiteWallBanner),
            (ItemKind::SkeletonSkull, BlockKind::SkeletonWallSkull),
            (ItemKind::PlayerHead, BlockKind::PlayerWallHead),
            (ItemKind::TubeCoralFan, BlockKind::TubeCoralWallFan),
        ] {
            let (pos, state) = place(item, Direction::North, 0.5, 0., BlockKind::Stone)
                .unwrap_or_else(|| panic!("{item:?} on the side of stone is predictable"));
            assert_eq!(pos, at(0, 0, -1));
            assert_eq!(state.as_block_kind(), wall, "{item:?}");
            assert_eq!(property(state, "facing"), Some("north"), "{item:?}");
        }
    }

    #[test]
    fn a_sign_on_a_carpet_is_refused() {
        assert!(
            place(
                ItemKind::OakSign,
                Direction::Up,
                1.0,
                0.,
                BlockKind::WhiteCarpet
            )
            .is_none()
        );
    }

    #[test]
    fn a_hanging_sign_under_stone_hangs_flat() {
        let (pos, state) = place(
            ItemKind::OakHangingSign,
            Direction::Down,
            0.0,
            0.,
            BlockKind::Stone,
        )
        .expect("a hanging sign under stone is predictable");
        assert_eq!(pos, at(0, -1, 0));
        assert_eq!(state.as_block_kind(), BlockKind::OakHangingSign);
        assert_eq!(property(state, "attached"), Some("false"));
        assert_eq!(property(state, "rotation"), Some("8"));
    }

    #[test]
    fn a_wall_hanging_sign_turns_across_the_clicked_face() {
        let (_, state) = place(
            ItemKind::OakHangingSign,
            Direction::North,
            0.5,
            0.,
            BlockKind::Stone,
        )
        .expect("a hanging sign on the side of stone is predictable");
        assert_eq!(state.as_block_kind(), BlockKind::OakWallHangingSign);
        assert_eq!(property(state, "facing"), Some("east"));
    }

    #[test]
    fn cocoa_faces_its_log() {
        let (pos, state) = place(
            ItemKind::CocoaBeans,
            Direction::North,
            0.5,
            0.,
            BlockKind::JungleLog,
        )
        .expect("cocoa on a jungle log is predictable");
        assert_eq!(pos, at(0, 0, -1));
        assert_eq!(state.as_block_kind(), BlockKind::Cocoa);
        assert_eq!(property(state, "facing"), Some("south"));
        assert!(
            place(
                ItemKind::CocoaBeans,
                Direction::North,
                0.5,
                0.,
                BlockKind::Stone
            )
            .is_none()
        );
    }

    #[test]
    fn a_rail_runs_along_the_player_and_needs_a_floor() {
        let (_, state) = place(ItemKind::Rail, Direction::Up, 1.0, 0., BlockKind::Stone)
            .expect("a rail on stone is predictable");
        assert_eq!(property(state, "shape"), Some("north_south"));
        let (_, state) = place(
            ItemKind::PoweredRail,
            Direction::Up,
            1.0,
            90.,
            BlockKind::Stone,
        )
        .expect("a powered rail on stone is predictable");
        assert_eq!(property(state, "shape"), Some("east_west"));
        assert!(
            place(
                ItemKind::DetectorRail,
                Direction::Up,
                1.0,
                0.,
                BlockKind::StoneSlab
            )
            .is_none()
        );
        let on_hopper = look_place(
            ItemKind::ActivatorRail,
            &hit(Direction::Up, 1.0),
            0.,
            0.,
            true,
            vec![(TARGET, BlockState::from(BlockKind::Hopper))],
        );
        assert!(on_hopper.is_some());
    }

    #[test]
    fn a_pressure_plate_needs_a_floor() {
        assert!(
            place(
                ItemKind::StonePressurePlate,
                Direction::Up,
                1.0,
                0.,
                BlockKind::OakFence
            )
            .is_some()
        );
        assert!(
            place(
                ItemKind::StonePressurePlate,
                Direction::Up,
                1.0,
                0.,
                BlockKind::ShortGrass
            )
            .is_none()
        );
    }

    #[test]
    fn tripwire_and_hooks() {
        let wire = BlockState::from(BlockKind::Tripwire);
        let placed = look_place(
            ItemKind::String,
            &hit(Direction::Up, 1.0),
            0.,
            0.,
            false,
            on_stone(vec![(at(1, 1, 0), wire)]),
        )
        .expect("string on stone is predictable");
        assert_eq!(placed.state.as_block_kind(), BlockKind::Tripwire);
        assert_eq!(property(placed.state, "east"), Some("true"));
        assert_eq!(property(placed.state, "west"), Some("false"));

        let (_, state) = place(
            ItemKind::TripwireHook,
            Direction::North,
            0.5,
            0.,
            BlockKind::Stone,
        )
        .expect("a hook on the side of stone is predictable");
        assert_eq!(property(state, "facing"), Some("north"));
        assert!(
            place(
                ItemKind::TripwireHook,
                Direction::Up,
                1.0,
                0.,
                BlockKind::Stone
            )
            .is_none()
        );
    }

    #[test]
    fn an_amethyst_bud_needs_a_sturdy_face() {
        assert!(
            place(
                ItemKind::AmethystCluster,
                Direction::North,
                0.25,
                0.,
                BlockKind::StoneSlab
            )
            .is_none()
        );
        assert!(
            place(
                ItemKind::SmallAmethystBud,
                Direction::North,
                0.5,
                0.,
                BlockKind::Stone
            )
            .is_some()
        );
    }

    #[test]
    fn a_snow_layer_needs_a_full_top_and_not_ice() {
        let (_, state) = place(ItemKind::Snow, Direction::Up, 1.0, 0., BlockKind::Stone)
            .expect("snow on stone is predictable");
        assert_eq!(property(state, "layers"), Some("1"));
        assert!(place(ItemKind::Snow, Direction::Up, 1.0, 0., BlockKind::Ice).is_none());
    }

    #[test]
    fn lichen_and_vines_take_the_face_towards_the_click() {
        for item in [ItemKind::GlowLichen, ItemKind::Vine, ItemKind::SculkVein] {
            let (pos, state) = place(item, Direction::North, 0.5, 0., BlockKind::Stone)
                .unwrap_or_else(|| panic!("{item:?} on the side of stone is predictable"));
            assert_eq!(pos, at(0, 0, -1));
            assert_eq!(property(state, "south"), Some("true"), "{item:?}");
            assert_eq!(property(state, "north"), Some("false"), "{item:?}");
        }
    }

    #[test]
    fn candles_and_carpets_need_something_below() {
        assert!(
            place(
                ItemKind::Candle,
                Direction::Up,
                1.0,
                0.,
                BlockKind::ShortGrass
            )
            .is_none()
        );
        assert!(
            place(
                ItemKind::WhiteCarpet,
                Direction::Up,
                1.0,
                0.,
                BlockKind::ShortGrass
            )
            .is_none()
        );
        assert!(place(ItemKind::Candle, Direction::Up, 1.0, 0., BlockKind::Stone).is_some());
    }
}
