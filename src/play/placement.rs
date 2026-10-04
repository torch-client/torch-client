use std::{
    collections::HashSet,
    str::FromStr,
    sync::{LazyLock, OnceLock},
};

use crate::blocks::facing::{clockwise, name as direction_name};
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
use azalea_registry::{
    Registry,
    builtin::{BlockEntityKind, BlockKind, ItemKind},
    tags::blocks::{MAINTAINS_FARMLAND, REPLACEABLE, SUPPORTS_BAMBOO},
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
    let ItemStack::Present(item) = held else {
        return None;
    };

    let block = BlockKind::from_str(item.kind.to_str()).ok()?;
    if block == BlockKind::Air {
        return None;
    }

    let clicked_state = block_at(hit.block_pos)?;

    let (pos, replacing_clicked) = if replaceable(clicked_state, block)? {
        (hit.block_pos, true)
    } else {
        if has_use_interaction(clicked_state) {
            return None;
        }
        (hit.block_pos + hit.direction.normal(), false)
    };

    if pos.y < min_y || pos.y >= min_y + height as i32 {
        return None;
    }

    let replaced_state = block_at(pos)?;
    if !replacing_clicked && !replaceable(replaced_state, block)? {
        return None;
    }

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

    if state.is_collision_shape_empty() && attachment == Attachment::None {
        return None;
    }
    if accepts(block, "half", "upper") || accepts(block, "part", "head") {
        return None;
    }

    Some((pos, state))
}

pub fn is_unobstructed(state: BlockState, pos: BlockPos, entities: &[Aabb]) -> bool {
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
    let fluid = match item.kind {
        ItemKind::WaterBucket => BlockKind::Water,
        ItemKind::LavaBucket => BlockKind::Lava,
        _ => return None,
    };

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

pub fn predict_bucket_fill(
    held: &ItemStack,
    hit: &BlockHitResult,
    block_at: impl Fn(BlockPos) -> Option<BlockState>,
) -> Option<(BlockPos, bool)> {
    let ItemStack::Present(item) = held else {
        return None;
    };
    if item.kind != ItemKind::Bucket {
        return None;
    }

    let clicked_state = block_at(hit.block_pos)?;
    let is_lava = match clicked_state.as_block_kind() {
        BlockKind::Water => false,
        BlockKind::Lava => true,
        _ => return None,
    };
    let block = Box::<dyn BlockTrait>::from(clicked_state);
    if block.get_property("level") != Some("0") {
        return None;
    }
    Some((hit.block_pos, is_lava))
}

fn fluid_replaceable(state: BlockState) -> bool {
    let kind = state.as_block_kind();
    kind == BlockKind::Water || kind == BlockKind::Lava || REPLACEABLE.contains(&kind)
}

fn replaceable(state: BlockState, item_block: BlockKind) -> Option<bool> {
    let kind = state.as_block_kind();
    if kind == item_block && !state.is_collision_shape_full() {
        return None;
    }
    Some(REPLACEABLE.contains(&kind))
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

#[cold]
fn block_has_use_interaction(state: BlockState) -> bool {
    let block = Box::<dyn BlockTrait>::from(state);
    if BLOCK_ENTITY_IDS.contains(&format!("minecraft:{}", block.id()).as_str())
        || MENU_BLOCK_IDS.contains(&block.id())
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

    if let Some(substituted) = substituted_placement(block, pos, replaced_state, block_at) {
        return substituted;
    }

    let _ = placed.set_property("axis", axis_name(face));

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
        let _ = placed.set_property("waterlogged", bool_str(is_water(replaced_state)));
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
        _ if accepts(block, "rotation", "0") && accepts(block, "powered", "true") => &["powered"],
        _ => &[],
    };
    if !signalled.is_empty() {
        let value = bool_str(has_neighbor_signal(pos, block_at));
        for name in signalled {
            let _ = placed.set_property(name, value);
        }
    }

    if block == BlockKind::Crafter {
        let _ = placed.set_property("orientation", crafter_orientation(yaw, pitch));
    }

    let _ = placed.set_property("rotation", rotation_segment(yaw));

    let replaced_water = is_water(replaced_state);
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

fn rotation_segment(yaw: f32) -> &'static str {
    const NAMES: [&str; 16] = [
        "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
    ];
    NAMES[(((yaw * (16.0 / 360.0)).round() as i32) & 15) as usize]
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
        let _ = placed.set_property("waterlogged", bool_str(is_water(replaced_state)));
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
    let _ = placed.set_property("waterlogged", bool_str(is_water(replaced_state)));
    Some(placed.as_block_state())
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
    let _ = placed.set_property("waterlogged", bool_str(is_water(replaced_state)));
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
    None,
}

fn attachment(block: BlockKind) -> Attachment {
    if accepts(block, "face", "ceiling") {
        Attachment::FaceAttached
    } else if accepts(block, "attachment", "single_wall") {
        Attachment::Bell
    } else if accepts(block, "hanging", "true") {
        Attachment::Lantern
    } else {
        Attachment::None
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
        || id == "amethyst_cluster"
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
        if !can_attach(pos, connected.opposite(), block_at) {
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
    let yaw = (yaw as f64).to_radians();
    let pitch = (pitch as f64).to_radians();
    Vec3 {
        x: -yaw.sin() * pitch.cos(),
        y: -pitch.sin(),
        z: yaw.cos() * pitch.cos(),
    }
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
    fn a_slab_clicked_with_its_own_item_is_not_predicted() {
        assert!(
            place(
                ItemKind::StoneSlab,
                Direction::Up,
                1.0,
                0.,
                BlockKind::StoneSlab
            )
            .is_none()
        );
        assert!(place(ItemKind::Snow, Direction::Up, 1.0, 0., BlockKind::Snow).is_none());
        assert!(place(ItemKind::Stone, Direction::Up, 1.0, 0., BlockKind::Stone).is_some());
    }

    #[test]
    fn blocks_that_need_support_are_not_predicted() {
        assert!(place(ItemKind::Torch, Direction::Up, 1.0, 0., BlockKind::Stone).is_none());
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
        assert!(place(ItemKind::Rail, Direction::Up, 1.0, 0., BlockKind::Stone).is_none());
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
        let (pos, is_lava) = predict_bucket_fill(
            &ItemStack::new(ItemKind::Bucket, 1),
            &hit(Direction::Up, 0.5),
            |pos| {
                Some(if pos == TARGET {
                    BlockState::from(BlockKind::Water)
                } else {
                    BlockState::AIR
                })
            },
        )
        .expect("a water source is picked up");
        assert_eq!(pos, TARGET);
        assert!(!is_lava);
    }

    #[test]
    fn an_empty_bucket_declines_flowing_water() {
        let mut flowing = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::Water));
        let _ = flowing.set_property("level", "1");
        let flowing = flowing.as_block_state();
        assert!(
            predict_bucket_fill(
                &ItemStack::new(ItemKind::Bucket, 1),
                &hit(Direction::Up, 0.5),
                |pos| {
                    Some(if pos == TARGET {
                        flowing
                    } else {
                        BlockState::AIR
                    })
                }
            )
            .is_none()
        );
    }

    #[test]
    fn a_full_bucket_is_not_fill_predicted() {
        assert!(
            predict_bucket_fill(
                &ItemStack::new(ItemKind::WaterBucket, 1),
                &hit(Direction::Up, 0.5),
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
}
