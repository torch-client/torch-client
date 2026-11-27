#![allow(dead_code)]

pub mod bucket;
pub mod stock;

use azalea::Client;
use azalea::block::{BlockState, BlockTrait};
use azalea::entity::inventory::Inventory;
use azalea::entity::{LookDirection, Physics};
use azalea::movement::{LastSentInput, LastSentLookDirection};
use azalea::physics::collision::BlockWithShape;
use azalea_core::direction::Direction;
use azalea_core::hit_result::BlockHitResult;
use azalea_core::position::{BlockPos, Vec3};
use azalea_inventory::ItemStack;
use azalea_protocol::common::movements::MoveFlags;
use azalea_protocol::packets::game::s_move_player_rot::ServerboundMovePlayerRot;
use azalea_registry::builtin::BlockKind;

use crate::modules::scaffold::aim::{WorldCells, face_click};
use crate::modules::sent::{self, Look, look_at};
use crate::play::interaction;
use crate::play::placement::{Placed, has_use_interaction, predict_placed};
use crate::session::SharedMutex;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Want {
    pub cell: BlockPos,
    pub state: Option<BlockState>,
}

#[derive(Clone, Debug)]
pub struct Click {
    pub hit: BlockHitResult,
    pub look: Look,
    pub sneak: bool,
}

pub fn same_placement(placed: BlockState, want: BlockState) -> bool {
    if placed == want {
        return true;
    }
    let kind = want.as_block_kind();
    if placed.as_block_kind() != kind {
        return false;
    }
    let placed = Box::<dyn BlockTrait>::from(placed);
    let want = Box::<dyn BlockTrait>::from(want);
    want.property_map().into_iter().all(|(name, value)| {
        neighbour_computed(kind, name, value) || placed.get_property(name) == Some(value)
    })
}

fn neighbour_computed(kind: BlockKind, name: &str, value: &str) -> bool {
    let id = kind.to_str().trim_start_matches("minecraft:");
    match name {
        "north" | "east" | "south" | "west" | "up" | "down" => !faces_are_placement(kind),
        "powered" => kind != BlockKind::Lever,
        "extended" => id.ends_with("piston"),
        "lit" => {
            matches!(
                id,
                "redstone_lamp" | "redstone_torch" | "redstone_wall_torch"
            ) || id.ends_with("copper_bulb")
        }
        "triggered" | "crafting" | "enabled" | "instrument" | "attached" | "disarmed"
        | "sculk_sensor_phase" => true,
        "age" => matches!(
            id,
            "kelp" | "weeping_vines" | "twisting_vines" | "cave_vines"
        ),
        "shape" => true,
        "type" => matches!(value, "single" | "left" | "right"),
        "snowy"
        | "in_wall"
        | "signal_fire"
        | "locked"
        | "thickness"
        | "tip"
        | "potent_sulfur_state" => true,
        "distance" => true,
        "bottom" => kind == BlockKind::Scaffolding,
        "power" => {
            kind == BlockKind::RedstoneWire
                || matches!(
                    id,
                    "daylight_detector" | "target" | "sculk_sensor" | "calibrated_sculk_sensor"
                )
                || id.ends_with("weighted_pressure_plate")
        }
        "waterlogged" => value == "false",
        _ => false,
    }
}

fn faces_are_placement(kind: BlockKind) -> bool {
    matches!(
        kind,
        BlockKind::GlowLichen | BlockKind::SculkVein | BlockKind::ResinClump | BlockKind::Vine
    )
}

fn use_cycle(kind: BlockKind, name: &str) -> Option<&'static [&'static str]> {
    let id = kind.to_str().trim_start_matches("minecraft:");
    let wooden = !id.contains("iron");
    match name {
        "delay" if kind == BlockKind::Repeater => Some(&["1", "2", "3", "4"]),
        "mode" if kind == BlockKind::Comparator => Some(&["compare", "subtract"]),
        "powered" if kind == BlockKind::Lever => Some(&["false", "true"]),
        "open"
            if wooden
                && (id.ends_with("_door")
                    || id.ends_with("_trapdoor")
                    || id.ends_with("fence_gate")) =>
        {
            Some(&["false", "true"])
        }
        "inverted" if kind == BlockKind::DaylightDetector => Some(&["false", "true"]),
        "note" if kind == BlockKind::NoteBlock => Some(&NOTES),
        _ => None,
    }
}

const NOTES: [&str; 25] = [
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
    "17", "18", "19", "20", "21", "22", "23", "24",
];

pub fn cycle_clicks(from: usize, to: usize, len: usize) -> usize {
    (to + len - from) % len
}

pub fn use_clicks(placed: BlockState, want: BlockState) -> Option<usize> {
    let kind = want.as_block_kind();
    if placed == want || placed.as_block_kind() != kind {
        return None;
    }
    let placed = Box::<dyn BlockTrait>::from(placed);
    let want = Box::<dyn BlockTrait>::from(want);
    let mut clicks = 0;
    for (name, value) in want.property_map() {
        let have = placed.get_property(name);
        if neighbour_computed(kind, name, value) || have == Some(value) {
            continue;
        }
        let cycle = use_cycle(kind, name)?;
        let from = cycle.iter().position(|v| Some(*v) == have)?;
        let to = cycle.iter().position(|v| *v == value)?;
        if clicks > 0 {
            return None;
        }
        clicks = cycle_clicks(from, to, cycle.len());
    }
    (clicks > 0).then_some(clicks)
}

const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

const ACROSS: [f64; 3] = [0.5, 0.15, 0.85];

fn face_axis(face: Direction) -> (usize, bool) {
    match face {
        Direction::Down => (1, false),
        Direction::Up => (1, true),
        Direction::North => (2, false),
        Direction::South => (2, true),
        Direction::West => (0, false),
        Direction::East => (0, true),
    }
}

fn face_plane(lo: [f64; 3], hi: [f64; 3], face: Direction) -> f64 {
    let (axis, positive) = face_axis(face);
    if positive { hi[axis] } else { lo[axis] }
}

const FINE: [f64; 5] = [0.5, 0.15, 0.85, 0.03, 0.97];

pub const AIR_PITCH: f32 = 70.0;

fn face_points(lo: [f64; 3], hi: [f64; 3], face: Direction) -> Vec<[f64; 3]> {
    face_points_at(lo, hi, face, &ACROSS)
}

fn face_points_at(lo: [f64; 3], hi: [f64; 3], face: Direction, across: &[f64]) -> Vec<[f64; 3]> {
    let (axis, _) = face_axis(face);
    let plane = face_plane(lo, hi, face);
    let (a, b) = match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    let mut out = Vec::with_capacity(across.len() * across.len());
    for fa in across {
        for fb in across {
            let mut p = [0.0; 3];
            p[axis] = plane;
            p[a] = lo[a] + (hi[a] - lo[a]) * fa;
            p[b] = lo[b] + (hi[b] - lo[b]) * fb;
            out.push(p);
        }
    }
    out
}

pub fn vertical_need(held: &ItemStack, want: BlockState) -> Option<bool> {
    let cell = BlockPos::new(0, 64, 0);
    let below = BlockPos::new(0, 63, 0);
    let hit = BlockHitResult {
        location: Vec3::new(0.5, 64.0, 0.5),
        miss: false,
        direction: Direction::Up,
        block_pos: below,
        inside: false,
        world_border: false,
    };
    let block_at = |p: BlockPos| {
        Some(if p == below {
            BlockState::from(BlockKind::Stone)
        } else {
            BlockState::AIR
        })
    };
    let gives = |pitch: f32| {
        [0.0f32, 90.0, 180.0, 270.0].iter().any(|&yaw| {
            predict_placed(held, &hit, yaw, pitch, false, -64, 384, block_at)
                .is_some_and(|p| p.pos == cell && placement_fits(p.state, want))
        })
    };
    match (gives(-89.0), gives(89.0)) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

fn distance(a: [f64; 3], b: Vec3) -> f64 {
    ((b.x - a[0]).powi(2) + (b.y - a[1]).powi(2) + (b.z - a[2]).powi(2)).sqrt()
}

pub fn placement_fits(placed: BlockState, want: BlockState) -> bool {
    let want = dry(want);
    same_placement(placed, want) || use_clicks(placed, want).is_some()
}

pub fn dry(state: BlockState) -> BlockState {
    let mut block = Box::<dyn BlockTrait>::from(state);
    if block.get_property("waterlogged") != Some("true")
        || block.set_property("waterlogged", "false").is_err()
    {
        return state;
    }
    block.as_block_state()
}

fn meets(want: &Want, pos: BlockPos, placed: BlockState) -> bool {
    pos == want.cell && want.state.is_none_or(|s| placement_fits(placed, s))
}

pub fn candidates(
    world: &azalea_world::World,
    eye: [f64; 3],
    reach: f64,
    held: &ItemStack,
    want: &Want,
    visible: bool,
) -> Vec<Click> {
    let min_y = world.chunks.min_y();
    let height = world.chunks.height();
    let predict = |hit: &BlockHitResult, look: Look, sneak: bool| {
        predict_placed(
            held,
            hit,
            look.yaw,
            look.pitch,
            sneak,
            min_y,
            height,
            |at| world.get_block_state(at),
        )
        .map(|p| (p.pos, p.state))
    };
    let fine = want.state.is_some_and(|s| vertical_need(held, s).is_some());
    candidates_with(world, eye, reach, want, visible, fine, &predict)
}

#[allow(clippy::too_many_arguments)]
fn candidates_with(
    world: &azalea_world::World,
    eye: [f64; 3],
    reach: f64,
    want: &Want,
    visible: bool,
    fine: bool,
    predict: &dyn Fn(&BlockHitResult, Look, bool) -> Option<(BlockPos, BlockState)>,
) -> Vec<Click> {
    let tracer = WorldCells {
        world,
        item: BlockKind::Air,
    };

    let neighbours = DIRECTIONS.map(|d| (want.cell + d.normal(), d.opposite()));
    let inside = DIRECTIONS.map(|d| (want.cell, d));

    let mut found: Vec<(f64, Click)> = Vec::new();
    for (support, face) in neighbours.into_iter().chain(inside) {
        let Some(state) = world.get_block_state(support) else {
            continue;
        };
        let shape = state.outline_shape(support);
        if shape.is_empty() {
            continue;
        }
        let b = shape.bounds();
        let base = [
            f64::from(support.x),
            f64::from(support.y),
            f64::from(support.z),
        ];
        let lo = [base[0] + b.min.x, base[1] + b.min.y, base[2] + b.min.z];
        let hi = [base[0] + b.max.x, base[1] + b.max.y, base[2] + b.max.z];
        if visible {
            let (axis, positive) = face_axis(face);
            let plane = face_plane(lo, hi, face);
            if (positive && eye[axis] <= plane) || (!positive && eye[axis] >= plane) {
                continue;
            }
        }
        let sneak = has_use_interaction(state);
        let across: &[f64] = if fine { &FINE } else { &ACROSS };
        for p in face_points_at(lo, hi, face, across) {
            if distance(eye, Vec3::new(p[0], p[1], p[2])) > reach {
                continue;
            }
            let look = look_at(eye, p);
            let raw = if visible {
                match tracer.trace(eye, look, reach) {
                    Some(h) if h.block_pos == support && h.direction == face => h,
                    _ => continue,
                }
            } else {
                BlockHitResult {
                    location: Vec3::new(p[0], p[1], p[2]),
                    miss: false,
                    direction: face,
                    block_pos: support,
                    inside: false,
                    world_border: false,
                }
            };
            let hit = face_click(&raw);
            let d = distance(eye, hit.location);
            if d > reach {
                continue;
            }
            if predict(&hit, look, sneak).is_some_and(|(pos, state)| meets(want, pos, state)) {
                found.push((d, Click { hit, look, sneak }));
            }
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, c)| c).collect()
}

pub fn solve(
    world: &azalea_world::World,
    eye: [f64; 3],
    reach: f64,
    held: &ItemStack,
    want: &Want,
    visible: bool,
) -> Option<Click> {
    candidates(world, eye, reach, held, want, visible)
        .into_iter()
        .next()
}

pub fn air_candidates(
    world: &azalea_world::World,
    eye: [f64; 3],
    reach: f64,
    held: &ItemStack,
    want: &Want,
) -> Vec<Click> {
    let min_y = world.chunks.min_y();
    let height = world.chunks.height();
    let lo = [
        f64::from(want.cell.x),
        f64::from(want.cell.y),
        f64::from(want.cell.z),
    ];
    let hi = [lo[0] + 1.0, lo[1] + 1.0, lo[2] + 1.0];
    let vertical = want.state.is_some_and(|s| vertical_need(held, s).is_some());
    let mut found: Vec<(f64, Click)> = Vec::new();
    for face in DIRECTIONS {
        for p in face_points(lo, hi, face) {
            let location = Vec3::new(p[0], p[1], p[2]);
            let d = distance(eye, location);
            if d > reach {
                continue;
            }
            let toward = look_at(eye, p);
            let hit = BlockHitResult {
                location,
                miss: false,
                direction: face,
                block_pos: want.cell,
                inside: false,
                world_border: false,
            };
            let looks = [
                toward,
                Look {
                    yaw: toward.yaw,
                    pitch: -AIR_PITCH,
                },
                Look {
                    yaw: toward.yaw,
                    pitch: AIR_PITCH,
                },
            ];
            let n = if vertical { 3 } else { 1 };
            for look in looks.into_iter().take(n) {
                let predicted = predict_placed(
                    held,
                    &hit,
                    look.yaw,
                    look.pitch,
                    false,
                    min_y,
                    height,
                    |at| world.get_block_state(at),
                );
                if predicted.is_some_and(|p| meets(want, p.pos, p.state)) {
                    found.push((
                        d,
                        Click {
                            hit: hit.clone(),
                            look,
                            sneak: false,
                        },
                    ));
                    break;
                }
            }
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, c)| c).collect()
}

pub fn fire_at(bot: &Client, shared: &SharedMutex, want: &Want, hit: &BlockHitResult) -> Fired {
    let Some((_, look)) = sent::sent_view(bot) else {
        return Fired::NotAimed;
    };
    let secondary_use = bot.component::<LastSentInput>().is_ok_and(|i| i.0.shift);
    let Some(held) = held_item(bot) else {
        return Fired::Declined;
    };
    let Ok(world) = bot.world() else {
        return Fired::NotAimed;
    };
    let predicted = {
        let world = world.read();
        predict_placed(
            &held,
            hit,
            look.yaw,
            look.pitch,
            secondary_use,
            world.chunks.min_y(),
            world.chunks.height(),
            |at| world.get_block_state(at),
        )
    };
    let placed = match predicted {
        Some(p) if meets(want, p.pos, p.state) => p,
        Some(_) => return Fired::NotAimed,
        None => return Fired::Declined,
    };
    if send(bot, shared, hit, look, secondary_use, &placed) {
        Fired::Placed
    } else {
        Fired::Declined
    }
}

fn send(
    bot: &Client,
    shared: &SharedMutex,
    hit: &BlockHitResult,
    look: Look,
    secondary_use: bool,
    placed: &Placed,
) -> bool {
    if placed.partner.is_some() {
        interaction::module_use_on(bot, shared, hit, crate::session::InteractionHand::Main);
        true
    } else {
        interaction::module_place(bot, shared, hit, (look.pitch, look.yaw), secondary_use)
    }
}

pub fn use_block(bot: &Client, shared: &SharedMutex, hit: &BlockHitResult) {
    interaction::module_use_on(bot, shared, hit, crate::session::InteractionHand::Main);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fired {
    Placed,
    NotAimed,
    Declined,
}

pub fn fire(bot: &Client, shared: &SharedMutex, want: &Want, reach: f64) -> Fired {
    let Some((eye, look)) = sent::sent_view(bot) else {
        return Fired::NotAimed;
    };
    let secondary_use = bot.component::<LastSentInput>().is_ok_and(|i| i.0.shift);
    let Some(held) = held_item(bot) else {
        return Fired::Declined;
    };
    let Ok(world) = bot.world() else {
        return Fired::NotAimed;
    };
    let hit = {
        let world = world.read();
        let tracer = WorldCells {
            world: &world,
            item: BlockKind::Air,
        };
        let Some(hit) = tracer.trace(eye, look, reach) else {
            return Fired::NotAimed;
        };
        let hit = face_click(&hit);
        let predicted = predict_placed(
            &held,
            &hit,
            look.yaw,
            look.pitch,
            secondary_use,
            world.chunks.min_y(),
            world.chunks.height(),
            |at| world.get_block_state(at),
        );
        match predicted {
            Some(p) if meets(want, p.pos, p.state) => (hit, p),
            Some(_) => return Fired::NotAimed,
            None => {
                let onto = hit.block_pos == want.cell
                    || hit.block_pos + hit.direction.normal() == want.cell;
                return if onto {
                    Fired::Declined
                } else {
                    Fired::NotAimed
                };
            }
        }
    };
    let (hit, placed) = hit;
    if send(bot, shared, &hit, look, secondary_use, &placed) {
        Fired::Placed
    } else {
        Fired::Declined
    }
}

pub fn fire_fast(bot: &Client, shared: &SharedMutex, click: &Click) -> bool {
    let secondary_use = bot.component::<LastSentInput>().is_ok_and(|i| i.0.shift);
    if click.sneak && !secondary_use {
        return false;
    }
    let Some(held) = held_item(bot) else {
        return false;
    };
    let Ok(world) = bot.world() else {
        return false;
    };
    let placed = {
        let world = world.read();
        predict_placed(
            &held,
            &click.hit,
            click.look.yaw,
            click.look.pitch,
            secondary_use,
            world.chunks.min_y(),
            world.chunks.height(),
            |at| world.get_block_state(at),
        )
    };
    let Some(placed) = placed else {
        return false;
    };
    let Some(flags) = move_flags(bot) else {
        return false;
    };
    crate::client::outbox::send(
        bot,
        ServerboundMovePlayerRot {
            look_direction: LookDirection::new(click.look.yaw, click.look.pitch),
            flags,
        },
    );
    send(bot, shared, &click.hit, click.look, secondary_use, &placed)
}

pub fn restore_look(bot: &Client) {
    let look = {
        let Ok(last) = bot.component::<LastSentLookDirection>() else {
            return;
        };
        LookDirection::new(last.y_rot, last.x_rot)
    };
    let Some(flags) = move_flags(bot) else {
        return;
    };
    crate::client::outbox::send(
        bot,
        ServerboundMovePlayerRot {
            look_direction: look,
            flags,
        },
    );
}

fn held_item(bot: &Client) -> Option<ItemStack> {
    bot.component::<Inventory>()
        .ok()
        .map(|inv| inv.held_item().clone())
}

fn move_flags(bot: &Client) -> Option<MoveFlags> {
    let physics = bot.component::<Physics>().ok()?;
    Some(MoveFlags {
        on_ground: physics.on_ground(),
        horizontal_collision: physics.horizontal_collision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(kind: BlockKind, props: &[(&str, &str)]) -> BlockState {
        let mut block = Box::<dyn BlockTrait>::from(BlockState::from(kind));
        for (name, value) in props {
            block.set_property(name, value).unwrap();
        }
        block.as_block_state()
    }

    #[test]
    fn a_stair_corner_and_fence_sides_are_the_neighbours() {
        let want = with(
            BlockKind::OakStairs,
            &[("facing", "east"), ("shape", "straight")],
        );
        let placed = with(
            BlockKind::OakStairs,
            &[("facing", "east"), ("shape", "inner_left")],
        );
        assert!(same_placement(placed, want));
        let turned = with(BlockKind::OakStairs, &[("facing", "west")]);
        assert!(!same_placement(turned, want));

        let fence = with(BlockKind::OakFence, &[("north", "true")]);
        assert!(same_placement(BlockState::from(BlockKind::OakFence), fence));
    }

    #[test]
    fn a_slab_half_counts_and_a_chest_pairing_does_not() {
        let top = with(BlockKind::OakSlab, &[("type", "top")]);
        let bottom = with(BlockKind::OakSlab, &[("type", "bottom")]);
        assert!(!same_placement(bottom, top));

        let left = with(BlockKind::Chest, &[("type", "left")]);
        let single = with(BlockKind::Chest, &[("type", "single")]);
        assert!(same_placement(left, single));
    }

    #[test]
    fn power_is_the_world_s_but_a_lever_is_the_player_s() {
        let lever_on = with(BlockKind::Lever, &[("powered", "true")]);
        let lever_off = with(BlockKind::Lever, &[("powered", "false")]);
        assert!(!same_placement(lever_off, lever_on));
        let repeater_on = with(BlockKind::Repeater, &[("powered", "true")]);
        let repeater_off = with(BlockKind::Repeater, &[("powered", "false")]);
        assert!(same_placement(repeater_off, repeater_on));
        let d1 = with(BlockKind::Repeater, &[("delay", "1")]);
        let d3 = with(BlockKind::Repeater, &[("delay", "3")]);
        assert!(!same_placement(d1, d3));
        assert_eq!(use_clicks(d1, d3), Some(2));
        assert_eq!(use_clicks(d3, d1), Some(2));
        assert_eq!(use_clicks(lever_off, lever_on), Some(1));
        let lamp_on = with(BlockKind::RedstoneLamp, &[("lit", "true")]);
        assert!(same_placement(
            BlockState::from(BlockKind::RedstoneLamp),
            lamp_on
        ));
        let furnace_lit = with(BlockKind::Furnace, &[("lit", "true")]);
        assert!(!same_placement(
            BlockState::from(BlockKind::Furnace),
            furnace_lit
        ));
        let east = with(BlockKind::Repeater, &[("facing", "east"), ("delay", "2")]);
        assert_eq!(use_clicks(d1, east), None);
    }

    #[test]
    fn multiface_faces_are_the_placement_and_fence_faces_are_not() {
        let lichen_north = with(BlockKind::GlowLichen, &[("north", "true")]);
        let lichen_south = with(BlockKind::GlowLichen, &[("south", "true")]);
        assert!(!same_placement(lichen_south, lichen_north));
        let fence = with(BlockKind::OakFence, &[("north", "true")]);
        assert!(same_placement(BlockState::from(BlockKind::OakFence), fence));
    }

    #[test]
    fn note_and_delay_cycles_count_forward() {
        assert_eq!(cycle_clicks(0, 24, 25), 24);
        assert_eq!(cycle_clicks(24, 0, 25), 1);
        assert_eq!(cycle_clicks(3, 0, 4), 1);
        assert_eq!(cycle_clicks(2, 2, 4), 0);
    }

    #[test]
    fn water_counts_one_way() {
        let dry = with(BlockKind::OakSlab, &[("waterlogged", "false")]);
        let wet = with(BlockKind::OakSlab, &[("waterlogged", "true")]);
        assert!(same_placement(wet, dry));
        assert!(!same_placement(dry, wet));
    }

    #[test]
    fn another_block_never_counts() {
        assert!(!same_placement(
            BlockState::from(BlockKind::Stone),
            BlockState::from(BlockKind::Dirt)
        ));
        assert!(same_placement(
            BlockState::from(BlockKind::Stone),
            BlockState::from(BlockKind::Stone)
        ));
    }

    #[test]
    fn samples_lie_on_the_face_inside_its_edges() {
        let (lo, hi) = ([2.0, 64.0, -3.0], [3.0, 64.5, -2.0]);
        let up = face_points(lo, hi, Direction::Up);
        assert_eq!(up[0], [2.5, 64.5, -2.5]);
        for p in up {
            assert_eq!(p[1], 64.5);
            assert!(
                p[0] > lo[0] && p[0] < hi[0] && p[2] > lo[2] && p[2] < hi[2],
                "{p:?}"
            );
        }
        let west = face_points(lo, hi, Direction::West);
        assert!(west.iter().all(|p| p[0] == 2.0));
        assert!(west.iter().any(|p| p[1] < 64.25) && west.iter().any(|p| p[1] > 64.25));
    }

    #[test]
    fn which_wants_need_a_look_up_or_down() {
        use azalea_registry::builtin::ItemKind;
        let item = |k: ItemKind| ItemStack::new(k, 1);
        assert_eq!(
            vertical_need(
                &item(ItemKind::Observer),
                with(BlockKind::Observer, &[("facing", "up")])
            ),
            Some(true)
        );
        assert_eq!(
            vertical_need(
                &item(ItemKind::Dispenser),
                with(BlockKind::Dispenser, &[("facing", "down")])
            ),
            Some(true)
        );
        assert_eq!(
            vertical_need(
                &item(ItemKind::Dispenser),
                with(BlockKind::Dispenser, &[("facing", "up")])
            ),
            Some(false)
        );
        assert_eq!(
            vertical_need(&item(ItemKind::Stone), BlockState::from(BlockKind::Stone)),
            None
        );
        assert_eq!(
            vertical_need(
                &item(ItemKind::Furnace),
                BlockState::from(BlockKind::Furnace)
            ),
            None
        );
    }

    #[test]
    fn a_look_from_below_needs_the_point_higher_than_it_is_far() {
        let up_enough = |eye: [f64; 3], p: [f64; 3]| look_at(eye, p).pitch < -50.0;
        let eye = [0.3, 0.0, 0.5];
        assert!(up_enough(eye, [0.0, 0.38, 0.5]));
        assert!(!up_enough(eye, [0.0, 0.2, 0.5]));
        assert!(!up_enough([0.8, 0.0, 0.5], [0.0, 0.38, 0.5]));
        let low = [0.3, -0.35, 0.5];
        assert!(up_enough(low, [0.0, 0.1, 0.5]));
        let fine_top = 0.97;
        assert!(up_enough(low, [0.0, fine_top - 0.62, 0.5]));
    }
}
