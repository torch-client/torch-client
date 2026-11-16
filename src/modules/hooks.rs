use azalea::Client;

use super::turn::{self, Shape};
use super::{
    aim_assist, auto_mace, auto_mine, auto_walk, crystal_aura, ore_miner, pathfinder, safe_anchor,
    scaffold, sneak,
};
use crate::session::{SharedMutex, SharedState};

pub fn shapes_look() -> bool {
    aim_assist::enabled()
        || auto_mine::enabled()
        || auto_mace::drives_camera()
        || safe_anchor::drives_camera()
        || pathfinder::drives_camera()
        || ore_miner::drives_camera()
}

pub fn drives_look() -> bool {
    auto_mine::enabled()
        || auto_mace::drives_camera()
        || safe_anchor::drives_camera()
        || pathfinder::drives_camera()
        || ore_miner::drives_camera()
        || aim_assist::pulls()
}

pub fn look_delta(s: &SharedState, yaw: f32, pitch: f32, dt: f32, hand: (f32, f32)) -> (f32, f32) {
    let d = if aim_assist::enabled() {
        aim_assist::nudge(s, yaw, pitch, dt, hand)
    } else {
        hand
    };
    let d = if auto_mine::enabled() {
        auto_mine::look_step(s, yaw, pitch, dt, d)
    } else {
        d
    };
    let aims = [
        (
            auto_mace::drives_camera(),
            s.session.auto_mace,
            Shape::Capped,
        ),
        (
            safe_anchor::drives_camera(),
            s.session.safe_anchor,
            Shape::Eased,
        ),
        (
            pathfinder::drives_camera(),
            s.session.route_look,
            Shape::Eased,
        ),
        (
            ore_miner::drives_camera(),
            s.session.ore_miner_look,
            Shape::Eased,
        ),
    ];
    aims.into_iter().fold(d, |d, (on, aim, shape)| match aim {
        Some(a) if on => turn::look_toward_by(s, yaw, pitch, dt, d, a.aim, a.turn, shape),
        _ => d,
    })
}

pub fn shapes_movement() -> bool {
    sneak::enabled() || auto_mine::enabled() || auto_walk::enabled()
}

pub fn move_flags(s: &SharedState, keys: u8) -> u8 {
    let mut flags = keys;
    if sneak::enabled() {
        flags |= sneak::flags(s.session.flying);
    }
    if s.session.auto_mine.is_some_and(|i| i.forward) {
        flags |= crate::session::MOVE_FORWARD;
    }
    if auto_walk::enabled() {
        flags |= auto_walk::flags(s);
    }
    flags
}

pub fn auto_jump(option: bool) -> bool {
    option || auto_walk::auto_jump()
}

pub fn script_flags(base: u8) -> u8 {
    #[cfg(lua_mods)]
    return (base | super::script::moves()) & !super::script::releases();
    #[cfg(not(lua_mods))]
    base
}

pub fn sent_look(
    bot: &Client,
    shared: &SharedMutex,
    yaw: f32,
    pitch: f32,
    flags: u8,
) -> (f32, f32, u8) {
    let (look, flags) = scaffold::steer(bot, shared, yaw, pitch, flags);
    if let Some(l) = look {
        return (l.yaw, l.pitch, flags);
    }
    if crystal_aura::enabled()
        && let Some(sent) = crystal_aura::steer(bot, yaw, pitch, flags)
    {
        return sent;
    }
    if safe_anchor::enabled()
        && let Some(sent) = safe_anchor::steer(bot, yaw, pitch, flags)
    {
        return sent;
    }
    if auto_mace::enabled() {
        auto_mace::steer(bot, yaw, pitch, flags)
    } else {
        (yaw, pitch, flags)
    }
}
