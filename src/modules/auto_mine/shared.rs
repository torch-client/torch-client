use std::collections::HashMap;

use azalea::Client;
use azalea::auto_tool::best_tool_in_hotbar_for_block;
use azalea::block::fluid_state::{FluidKind, FluidState};
use azalea::block::{BlockState, BlockTrait};
use azalea::entity::inventory::Inventory;
use azalea::inventory::LastSentSelectedHotbarSlot;
use azalea::physics::clip::{BlockShapeType, ClipContext, FluidPickType, clip};
use azalea::physics::collision::BlockWithShape;
use azalea_core::position::{BlockPos, Vec3};
use azalea_inventory::ItemStack;

use crate::modules::roll;
use crate::session::{Gamemode, SharedMutex};

pub(in crate::modules) const STALL_TICKS: i64 = 60;

pub(in crate::modules) const MAX_STALLS: u32 = 5;

pub(in crate::modules) const TRAVEL_TICKS: i64 = 600;
pub(in crate::modules) const TRAVEL_PROGRESS: i32 = 4;

pub(in crate::modules) const REACH: f64 = 4.2;

pub(in crate::modules) const EYE: f64 = 1.62;

pub(in crate::modules) const JITTER: f64 = 0.22;

pub(in crate::modules) fn gate(shared: &SharedMutex, in_gui: bool) -> Option<f32> {
    let st = shared.lock().unwrap();
    let blocked = !st.in_world
        || (st.screen_open && !st.pause_screen && !in_gui)
        || st.session.dead
        || st.session.gamemode == Gamemode::Spectator;
    (!blocked).then_some(st.camera_yaw)
}

pub(in crate::modules) fn low_health(
    shared: &SharedMutex,
    on: bool,
    limit: f32,
    label: &str,
) -> bool {
    if !on {
        return false;
    }
    let mut st = shared.lock().unwrap();
    if !st.session.health_known || st.session.dead || st.session_ended || st.session.health >= limit
    {
        return false;
    }
    let text = format!(
        "{label}: health {:.0} fell below {:.0}",
        st.session.health, limit
    );
    st.end_session(
        "Disconnected".to_string(),
        Some(crate::text::parse_formatted(&text)),
    );
    st.disconnect_requested = true;
    true
}

pub(in crate::modules) fn present(bot: &Client, pos: BlockPos) -> bool {
    state_at(bot, pos).is_some_and(|st| !st.is_air() && !is_fluid(st))
}

pub(in crate::modules) fn state_at(bot: &Client, pos: BlockPos) -> Option<BlockState> {
    bot.world().ok().and_then(|w| w.read().get_block_state(pos))
}

pub(in crate::modules) fn is_fluid(state: BlockState) -> bool {
    FluidState::from(state).kind != FluidKind::Empty
}

pub(in crate::modules) fn solid(state: BlockState) -> bool {
    !state.is_air() && !is_fluid(state)
}

pub(in crate::modules) fn exposes_fluid(bot: &Client, pos: BlockPos) -> bool {
    let Ok(instance) = bot.world() else {
        return false;
    };
    let world = instance.read();
    [
        BlockPos::new(pos.x + 1, pos.y, pos.z),
        BlockPos::new(pos.x - 1, pos.y, pos.z),
        BlockPos::new(pos.x, pos.y + 1, pos.z),
        BlockPos::new(pos.x, pos.y - 1, pos.z),
        BlockPos::new(pos.x, pos.y, pos.z + 1),
        BlockPos::new(pos.x, pos.y, pos.z - 1),
    ]
    .iter()
    .any(|p| world.get_block_state(*p).is_some_and(is_fluid))
}

pub(in crate::modules) fn breakable(cache: &mut HashMap<u32, bool>, state: BlockState) -> bool {
    *cache.entry(state.id() as u32).or_insert_with(|| {
        let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
        block.behavior().destroy_time >= 0.0
    })
}

pub(in crate::modules) fn aim_at(
    chunks: &azalea_world::ChunkStorage,
    eye: [f64; 3],
    t: BlockPos,
    state: BlockState,
) -> Option<[f64; 3]> {
    let shape = state.outline_shape(t);
    if shape.is_empty() {
        return None;
    }
    let b = shape.bounds();
    let base = [f64::from(t.x), f64::from(t.y), f64::from(t.z)];
    let lo = [base[0] + b.min.x, base[1] + b.min.y, base[2] + b.min.z];
    let hi = [base[0] + b.max.x, base[1] + b.max.y, base[2] + b.max.z];
    let mut faces: Vec<(usize, [f64; 3])> = Vec::with_capacity(3);
    for axis in 0..3 {
        let plane = if eye[axis] < lo[axis] {
            lo[axis]
        } else if eye[axis] > hi[axis] {
            hi[axis]
        } else {
            continue;
        };
        let mut c = [
            (lo[0] + hi[0]) * 0.5,
            (lo[1] + hi[1]) * 0.5,
            (lo[2] + hi[2]) * 0.5,
        ];
        c[axis] = plane;
        faces.push((axis, c));
    }
    let d2 = |p: &[f64; 3]| (0..3).map(|i| (p[i] - eye[i]).powi(2)).sum::<f64>();
    faces.sort_by(|a, b| d2(&a.1).total_cmp(&d2(&b.1)));
    for (axis, centre) in faces {
        let mut jittered = centre;
        for i in (0..3).filter(|&i| i != axis) {
            let half = JITTER.min((hi[i] - lo[i]) * 0.4);
            jittered[i] += f64::from(roll(0.0, 2.0 * half as f32)) - half;
        }
        for p in [jittered, centre] {
            if sees(chunks, eye, t, p) {
                return Some(p);
            }
        }
    }
    None
}

pub(in crate::modules) fn sees(
    chunks: &azalea_world::ChunkStorage,
    eye: [f64; 3],
    t: BlockPos,
    p: [f64; 3],
) -> bool {
    let len = (0..3).map(|i| (p[i] - eye[i]).powi(2)).sum::<f64>().sqrt();
    if len > REACH {
        return false;
    }
    let len = len.max(1e-6);
    let dir = [
        (p[0] - eye[0]) / len,
        (p[1] - eye[1]) / len,
        (p[2] - eye[2]) / len,
    ];
    let hit = clip(
        chunks,
        ClipContext {
            from: Vec3::new(eye[0], eye[1], eye[2]),
            to: Vec3::new(
                p[0] + dir[0] * 0.05,
                p[1] + dir[1] * 0.05,
                p[2] + dir[2] * 0.05,
            ),
            block_shape_type: BlockShapeType::Outline,
            fluid_pick_type: FluidPickType::None,
            held_item: azalea_registry::builtin::ItemKind::Air,
            entity: None,
        },
    );
    !hit.miss && hit.block_pos == t
}

pub(in crate::modules) fn best_tool(bot: &Client, state: BlockState) -> Option<u8> {
    let inv = bot.component::<Inventory>().ok()?;
    Some(best_tool_in_hotbar_for_block(state, &inv.inventory_menu).index as u8)
}

pub(in crate::modules) fn harvests(bot: &Client, state: BlockState, slot: Option<u8>) -> bool {
    let (Some(slot), Ok(inv)) = (slot, bot.component::<Inventory>()) else {
        return true;
    };
    let menu = &inv.inventory_menu;
    let index = *menu.hotbar_slots_range().start() + usize::from(slot);
    let Some(item) = menu.slot(index) else {
        return true;
    };
    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    azalea::entity::mining::has_correct_tool_for_drops(&*block, item)
}

pub(in crate::modules) fn select_tool(bot: &Client, slot: Option<u8>) -> bool {
    match slot {
        Some(slot) => {
            crate::play::interaction::select_hotbar_slot(bot, slot);
            bot.component::<LastSentSelectedHotbarSlot>()
                .is_ok_and(|s| s.slot == slot)
        }
        None => true,
    }
}

pub(in crate::modules) fn inventory_full(bot: &Client) -> bool {
    let Ok(inv) = bot.component::<Inventory>() else {
        return false;
    };
    let menu = &inv.inventory_menu;
    menu.player_slots_without_hotbar_range()
        .chain(menu.hotbar_slots_range())
        .all(|i| match menu.slot(i) {
            Some(ItemStack::Present(d)) => d.count >= azalea_inventory::bundle::max_stack_size(d),
            _ => false,
        })
}
