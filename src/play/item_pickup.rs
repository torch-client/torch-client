use std::collections::HashSet;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU16, Ordering};

use azalea_protocol::packets::game::c_take_item_entity::ClientboundTakeItemEntity;
use azalea_registry::builtin::EntityKind;

use crate::client::tracking::{anim_states, entity_anims, local_entity_id};
use crate::entities::EntityState;
use crate::session::{SessionState, SharedMutex, SideMutex};
use crate::util::mth::lerp_f64;

const LIFE_TIME: u32 = 3;

#[derive(Clone, Debug)]
pub struct ItemPickup {
    state: EntityState,
    replaces_source: bool,
    target_id: i32,
    life: u32,
    target: [f64; 3],
    target_old: [f64; 3],
}

impl ItemPickup {
    pub fn sample(&self, partial: f32) -> EntityState {
        let time = ((self.life as f32 + partial) / LIFE_TIME as f32).powi(2);
        let mut st = self.state.clone();
        st.pos = std::array::from_fn(|i| {
            let target = lerp_f64(partial, self.target_old[i], self.target[i]);
            lerp_f64(time, self.state.pos[i], target)
        });
        st
    }

    pub fn replaces(&self) -> Option<i32> {
        self.replaces_source.then_some(self.state.id)
    }
}

static TAKEN: OnceLock<SideMutex<HashSet<i32>>> = OnceLock::new();

pub(crate) fn taken() -> &'static SideMutex<HashSet<i32>> {
    TAKEN.get_or_init(Default::default)
}

fn ghost_id() -> i32 {
    static NEXT: AtomicU16 = AtomicU16::new(1);
    i32::MIN + i32::from(NEXT.fetch_add(1, Ordering::Relaxed))
}

pub(crate) fn take(shared: &SharedMutex, p: &ClientboundTakeItemEntity) {
    let id = p.item_id as i32;
    if taken().lock().unwrap().contains(&id) {
        return;
    }
    let Some(mut state) = entity_anims()
        .lock()
        .unwrap()
        .get(&id)
        .map(|a| a.sample(1.0))
    else {
        return;
    };
    let orb = state.kind == EntityKind::ExperienceOrb;
    #[cfg(feature = "audio")]
    {
        let (event, volume, spread, base) = if orb {
            ("entity.experience_orb.pickup", 0.1, 0.35, 0.9)
        } else {
            ("entity.item.pickup", 0.2, 1.4, 2.0)
        };
        let pitch = (crate::audio::random_f32() - crate::audio::random_f32()) * spread + base;
        crate::audio::play_at(
            event,
            crate::audio::SoundCategory::Players,
            state.pos,
            volume,
            pitch,
        );
    }
    let removed = match state.kind {
        EntityKind::Item => u32::from(state.extras.item.count) <= p.amount,
        _ => !orb,
    };
    if removed {
        taken().lock().unwrap().insert(id);
    } else {
        state.id = ghost_id();
    }
    state.frozen_light = Some(crate::entities::light_levels(&state));

    let mut s = shared.lock().unwrap();
    let session = &mut s.session;
    let (target_id, target) = match target_point(session, p.player_id.0) {
        Some(point) => (p.player_id.0, point),
        None => (local_entity_id(), local_point(session)),
    };
    session.item_pickups.push(ItemPickup {
        state,
        replaces_source: removed,
        target_id,
        life: 0,
        target,
        target_old: target,
    });
}

pub(crate) fn tick(session: &mut SessionState) {
    if session.item_pickups.is_empty() {
        return;
    }
    let mut pickups = std::mem::take(&mut session.item_pickups);
    pickups.retain_mut(|p| {
        p.life += 1;
        if p.life == LIFE_TIME {
            return false;
        }
        p.target_old = p.target;
        if let Some(point) = target_point(session, p.target_id) {
            p.target = point;
        }
        true
    });
    session.item_pickups = pickups;
}

fn target_point(session: &SessionState, id: i32) -> Option<[f64; 3]> {
    if id == local_entity_id() {
        return Some(local_point(session));
    }
    if let Some(anim) = entity_anims().lock().unwrap().get(&id) {
        return Some(midpoint(anim.position(1.0), anim.eye_height()));
    }
    let eye_height = session
        .other_players
        .iter()
        .find(|p| p.id == id)?
        .body
        .eye_height;
    let pos = anim_states().lock().unwrap().get(&id)?.position(1.0);
    Some(midpoint(pos, eye_height))
}

fn local_point(session: &SessionState) -> [f64; 3] {
    midpoint(session.player_pos, session.local_body.eye_height)
}

fn midpoint(pos: [f64; 3], eye_height: f32) -> [f64; 3] {
    [pos[0], pos[1] + f64::from(eye_height) / 2.0, pos[2]]
}
