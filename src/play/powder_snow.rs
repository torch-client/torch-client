use azalea::entity::Physics;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::particle::Particle;
use azalea::physics::clip::box_traverse_blocks;
use azalea::prelude::*;
use azalea_core::position::BlockPos;
use azalea_registry::builtin::BlockKind;

use crate::session::{LevelParticle, ParticleEmit, SharedMutex, SideMutex};
use crate::util::javarandom::JavaRandom;

const MOMENTUM: f32 = 0.083333336;
const RISE: f32 = 0.05;

static RNG: SideMutex<Option<JavaRandom>> = SideMutex::new(None);

pub(crate) fn tick(bot: &Client, shared: &SharedMutex) {
    let (old, pos, aabb) = {
        let Ok(physics) = bot.component::<Physics>() else {
            return;
        };
        if !physics.is_in_powder_snow {
            return;
        }
        let Ok(pos) = bot.position() else { return };
        let Ok(dims) = bot.component::<EntityDimensions>() else {
            return;
        };
        (
            physics.old_position,
            pos,
            dims.make_bounding_box(pos).deflate_all(1.0e-5),
        )
    };
    if old.x == pos.x && old.z == pos.z {
        return;
    }
    let Ok(world) = bot.world() else { return };
    let world = world.read();
    let is_snow = |p: BlockPos| {
        BlockKind::from(world.get_block_state(p).unwrap_or_default()) == BlockKind::PowderSnow
    };
    if !is_snow(BlockPos::from(pos)) {
        return;
    }

    let mut rng = RNG.lock().unwrap();
    let rng = rng.get_or_insert_with(JavaRandom::time_seeded);
    let mut emits = Vec::new();
    for cell in box_traverse_blocks(old, pos, &aabb) {
        if !is_snow(cell) || !rng.next_bool() {
            continue;
        }
        let dx = (rng.next_f32() * 2.0 - 1.0) * MOMENTUM;
        let dz = (rng.next_f32() * 2.0 - 1.0) * MOMENTUM;
        let at = [pos.x, f64::from(cell.y + 1), pos.z];
        emits.push(LevelParticle::add(Particle::Snowflake, at, [dx, RISE, dz]));
    }
    drop(world);
    crate::play::emit::queue(shared, emits, ParticleEmit::Level);
}
