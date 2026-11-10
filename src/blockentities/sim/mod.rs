pub mod beacon;
pub mod bell;
pub mod book;
pub mod bounds;
pub mod conduit;
pub mod gateway;
pub mod items;
pub mod piston;
pub mod pot;
pub mod potent_sulfur;
pub mod shulker;
pub mod spawner;
pub mod vault;

use azalea::block::BlockState;
use azalea::entity::particle::Particle;
use azalea_core::position::BlockPos;
use azalea_registry::builtin::BlockEntityKind;

use super::feed::{BlockEntityData, BlockStateInfo};
use crate::session::{LevelParticle, ParticleEmit, SharedMutex};
use crate::util::javarandom::JavaRandom;

#[derive(Clone, Copy, Debug)]
pub struct PlayerSeen {
    pub feet: [f64; 3],
    pub uuid: u128,
    pub height: f32,
}

pub struct TickCtx<'a> {
    pub world: &'a azalea_world::World,
    pub pos: [i32; 3],
    pub state: &'a BlockStateInfo,
    pub data: &'a BlockEntityData,
    pub game_time: u64,
    pub players: &'a [PlayerSeen],
}

impl TickCtx<'_> {
    pub fn block(&self, pos: [i32; 3]) -> Option<BlockState> {
        self.world
            .get_block_state(BlockPos::new(pos[0], pos[1], pos[2]))
    }

    pub fn nearest_player(&self, at: [f64; 3], range: f64) -> Option<[f64; 3]> {
        self.players
            .iter()
            .map(|p| (p.feet, dist_sq(p.feet, at)))
            .filter(|(_, d)| range < 0.0 || *d < range * range)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(p, _)| p)
    }

    pub fn player_within(&self, at: [f64; 3], range: f64) -> bool {
        self.players
            .iter()
            .any(|p| dist_sq(p.feet, at) < range * range)
    }

    pub fn player_by_uuid(&self, uuid: u128) -> Option<&PlayerSeen> {
        self.players.iter().find(|p| p.uuid == uuid)
    }

    pub fn random<R>(&self, f: impl FnOnce(&mut JavaRandom) -> R) -> R {
        let mut random = RANDOM.lock();
        f(random.get_or_insert_with(JavaRandom::time_seeded))
    }

    pub fn add_particle(&self, particle: Particle, at: [f64; 3], velocity: [f64; 3]) {
        let range = crate::play::emit::EMIT_RANGE;
        if !crate::util::particles::overrides_limiter(&particle)
            && self
                .players
                .first()
                .is_some_and(|me| dist_sq(me.feet, at) > range * range)
        {
            return;
        }
        PENDING
            .lock()
            .push(LevelParticle::add(particle, at, velocity.map(|v| v as f32)));
    }

    pub fn add_always_visible_particle(
        &self,
        particle: Particle,
        at: [f64; 3],
        velocity: [f64; 3],
    ) {
        let mut emit = LevelParticle::add(particle, at, velocity.map(|v| v as f32));
        emit.override_limiter = true;
        emit.always_show = true;
        PENDING.lock().push(emit);
    }

    pub fn center(&self) -> [f64; 3] {
        [
            self.pos[0] as f64 + 0.5,
            self.pos[1] as f64 + 0.5,
            self.pos[2] as f64 + 0.5,
        ]
    }
}

static RANDOM: parking_lot::Mutex<Option<JavaRandom>> = parking_lot::Mutex::new(None);

static PENDING: parking_lot::Mutex<Vec<LevelParticle>> = parking_lot::Mutex::new(Vec::new());

pub fn flush(shared: &SharedMutex) {
    let batch = {
        let mut pending = PENDING.lock();
        if pending.is_empty() {
            return;
        }
        std::mem::take(&mut *pending)
    };
    crate::play::emit::queue(shared, batch, ParticleEmit::Level);
}

fn idle_sound(ctx: &TickCtx, event: &str) {
    let roll = ctx.random(|r| {
        (r.next_f32() <= 0.02).then(|| (r.next_f32() * 0.25 + 0.75, r.next_f32() + 0.5))
    });
    if let Some((volume, pitch)) = roll {
        crate::util::level_events::play(
            event,
            crate::util::level_events::Bus::Blocks,
            ctx.center(),
            volume,
            pitch,
        );
    }
}

fn center_offset_random(ctx: &TickCtx, offset: f32) -> [f64; 3] {
    let c = ctx.center();
    ctx.random(|r| {
        let mut out = c;
        for axis in &mut out {
            *axis += f64::from((r.next_f32() - 0.5) * offset);
        }
        out
    })
}

fn next_double(r: &mut JavaRandom, min: f64, max: f64) -> f64 {
    min + r.next_f64() * (max - min)
}

fn dist_sq(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Anim {
    #[default]
    None,
    Lid(Lid),
    Skull(Skull),
    Shulker(shulker::Shulker),
    Bell(bell::Bell),
    Pot(pot::Wobble),
    Book(book::Book),
    Conduit(conduit::Conduit),
    Spawner(spawner::Spin),
    Vault(vault::Vault),
    Gateway(gateway::Gateway),
    Beacon(beacon::Beacon),
    Piston(piston::Progress),
    PotentSulfur(potent_sulfur::Sulfur),
    Campfire,
}

impl Anim {
    pub fn new(kind: BlockEntityKind, data: &BlockEntityData) -> Anim {
        let mut anim = match kind {
            BlockEntityKind::Chest
            | BlockEntityKind::TrappedChest
            | BlockEntityKind::EnderChest => Anim::Lid(Lid::default()),
            BlockEntityKind::Skull => Anim::Skull(Skull::default()),
            BlockEntityKind::ShulkerBox => Anim::Shulker(Default::default()),
            BlockEntityKind::Bell => Anim::Bell(Default::default()),
            BlockEntityKind::DecoratedPot => Anim::Pot(Default::default()),
            BlockEntityKind::EnchantingTable => Anim::Book(Default::default()),
            BlockEntityKind::Conduit => Anim::Conduit(Default::default()),
            BlockEntityKind::MobSpawner | BlockEntityKind::TrialSpawner => {
                Anim::Spawner(Default::default())
            }
            BlockEntityKind::Vault => Anim::Vault(Default::default()),
            BlockEntityKind::EndGateway => Anim::Gateway(Default::default()),
            BlockEntityKind::Beacon => Anim::Beacon(Default::default()),
            BlockEntityKind::Piston => Anim::Piston(Default::default()),
            BlockEntityKind::PotentSulfur => Anim::PotentSulfur(Default::default()),
            BlockEntityKind::Campfire => Anim::Campfire,
            _ => Anim::None,
        };
        anim.on_data(data);
        anim
    }

    pub fn on_data(&mut self, data: &BlockEntityData) {
        match self {
            Anim::Spawner(spin) => spin.on_data(data),
            Anim::Gateway(gateway) => gateway.on_data(data),
            Anim::Piston(progress) => progress.on_data(data),
            _ => {}
        }
    }

    pub fn tick(&mut self, ctx: &TickCtx, private: &mut Private) {
        match self {
            Anim::None => {}
            Anim::Lid(lid) => lid.tick(),
            Anim::Skull(skull) => skull.tick(ctx.state.powered),
            Anim::Shulker(shulker) => shulker.tick(),
            Anim::Bell(bell) => bell.tick(),
            Anim::Pot(_) => {}
            Anim::Book(book) => book.tick(ctx),
            Anim::Conduit(conduit) => {
                if !matches!(private, Private::Conduit(_)) {
                    *private = Private::Conduit(Vec::new());
                }
                if let Private::Conduit(effect_blocks) = private {
                    conduit.tick(ctx, effect_blocks);
                }
            }
            Anim::Spawner(spin) => spin.tick(ctx),
            Anim::Vault(vault) => vault.tick(ctx),
            Anim::Gateway(gateway) => gateway.tick(),
            Anim::Beacon(beacon) => {
                if !matches!(private, Private::Beacon(_)) {
                    *private = Private::Beacon(Default::default());
                }
                if let Private::Beacon(scan) = private {
                    beacon.tick(ctx, scan);
                }
            }
            Anim::Piston(progress) => progress.tick(),
            Anim::PotentSulfur(sulfur) => sulfur.tick(ctx),
            Anim::Campfire => items::campfire_particle_tick(ctx),
        }
    }

    pub fn event(&mut self, action: u8, param: u8, game_time: u64) {
        match self {
            Anim::Lid(lid) if action == 1 => lid.should_be_open = param > 0,
            Anim::Shulker(shulker) => shulker.event(action, param),
            Anim::Bell(bell) => bell.event(action, param),
            Anim::Pot(wobble) => wobble.event(action, param, game_time),
            Anim::Gateway(gateway) => gateway.event(action),
            Anim::Spawner(spin) => spin.event(action),
            Anim::PotentSulfur(sulfur) => sulfur.event(game_time),
            _ => {}
        }
    }
}

#[derive(Default)]
pub enum Private {
    #[default]
    None,
    Beacon(beacon::Scan),
    Conduit(Vec<[i32; 3]>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lid {
    pub should_be_open: bool,
    pub openness: f32,
    pub o_openness: f32,
}

impl Lid {
    fn tick(&mut self) {
        self.o_openness = self.openness;
        if !self.should_be_open && self.openness > 0.0 {
            self.openness = (self.openness - 0.1).max(0.0);
        } else if self.should_be_open && self.openness < 1.0 {
            self.openness = (self.openness + 0.1).min(1.0);
        }
    }

    pub fn at(&self, partial: f32) -> f32 {
        self.o_openness + (self.openness - self.o_openness) * partial
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Skull {
    pub tick: u32,
    pub animating: bool,
}

impl Skull {
    fn tick(&mut self, powered: bool) {
        self.animating = powered;
        if powered {
            self.tick = self.tick.wrapping_add(1);
        }
    }

    pub fn at(&self, partial: f32) -> f32 {
        if self.animating {
            self.tick as f32 + partial
        } else {
            self.tick as f32
        }
    }
}

#[cfg(test)]
pub(crate) mod test_nbt {
    use simdnbt::Mutf8String;
    use simdnbt::owned::{Nbt, NbtCompound, NbtList, NbtTag};

    pub fn string(value: &str) -> NbtTag {
        NbtTag::String(Mutf8String::from(value))
    }

    pub fn strings(values: &[&str]) -> NbtTag {
        NbtTag::List(NbtList::String(
            values.iter().copied().map(Mutf8String::from).collect(),
        ))
    }

    pub fn compound(values: Vec<(&str, NbtTag)>) -> NbtCompound {
        NbtCompound::from_values(
            values
                .into_iter()
                .map(|(key, tag)| (Mutf8String::from(key), tag))
                .collect(),
        )
    }

    pub fn root(values: Vec<(&str, NbtTag)>) -> Nbt {
        Nbt::new(Mutf8String::from(""), compound(values))
    }

    pub fn item(id: &str, count: i32) -> Vec<(&str, NbtTag)> {
        vec![("id", string(id)), ("count", NbtTag::Int(count))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lid_takes_ten_ticks_to_open() {
        let mut lid = Lid {
            should_be_open: true,
            ..Default::default()
        };
        for _ in 0..10 {
            lid.tick();
        }
        assert!((lid.openness - 1.0).abs() < 1e-6);
        lid.tick();
        assert_eq!(lid.openness, 1.0);
        lid.should_be_open = false;
        for _ in 0..10 {
            lid.tick();
        }
        assert_eq!(lid.openness, 0.0);
    }
}
