use azalea::block::BlockTrait;
use azalea::block::fluid_state::{FluidKind, FluidState};
use azalea::entity::particle::Particle;
use simdnbt::owned::Nbt;

use super::TickCtx;

const VALID_BLOCKS: [&str; 4] = [
    "prismarine",
    "prismarine_bricks",
    "sea_lantern",
    "dark_prismarine",
];
const MIN_ACTIVE_SIZE: usize = 16;
const MIN_KILL_SIZE: usize = 42;
const ROTATION_SPEED: f32 = -0.0375;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Data {}

pub fn decode(_tag: &Nbt) -> Data {
    Data::default()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Conduit {
    pub tick_count: u32,
    pub active_rotation: u32,
    pub active: bool,
    pub hunting: bool,
}

impl Conduit {
    pub fn tick(&mut self, ctx: &TickCtx, effect_blocks: &mut Vec<[i32; 3]>) {
        self.tick_count = self.tick_count.wrapping_add(1);
        if ctx.game_time % 40 == 0 {
            self.active = update_shape(ctx, effect_blocks);
            self.hunting = effect_blocks.len() >= MIN_KILL_SIZE;
        }
        animation_tick(ctx, effect_blocks, self.tick_count);
        if self.active {
            self.active_rotation = self.active_rotation.wrapping_add(1);
        }
    }

    pub fn active_rotation(&self, partial: f32) -> f32 {
        (self.active_rotation as f32 + partial) * ROTATION_SPEED
    }

    pub fn animation_phase(&self) -> u32 {
        self.tick_count / 66 % 3
    }
}

fn update_shape(ctx: &TickCtx, effect_blocks: &mut Vec<[i32; 3]>) -> bool {
    effect_blocks.clear();
    let [x, y, z] = ctx.pos;
    for (ox, oy, oz) in cube(1) {
        let water = ctx
            .block([x + ox, y + oy, z + oz])
            .is_some_and(|state| FluidState::from(state).kind == FluidKind::Water);
        if !water {
            return false;
        }
    }
    effect_blocks.extend(
        cube(2)
            .filter(|&(ox, oy, oz)| {
                let (ax, ay, az) = (ox.abs(), oy.abs(), oz.abs());
                (ax > 1 || ay > 1 || az > 1)
                    && (ox == 0 && (ay == 2 || az == 2)
                        || oy == 0 && (ax == 2 || az == 2)
                        || oz == 0 && (ax == 2 || ay == 2))
            })
            .filter(|&(ox, oy, oz)| {
                ctx.block([x + ox, y + oy, z + oz]).is_some_and(|state| {
                    let block = Box::<dyn BlockTrait>::from(state);
                    VALID_BLOCKS.contains(&block.id())
                })
            })
            .map(|(ox, oy, oz)| [ox, oy, oz]),
    );
    effect_blocks.len() >= MIN_ACTIVE_SIZE
}

fn animation_tick(ctx: &TickCtx, effect_blocks: &[[i32; 3]], tick_count: u32) {
    if effect_blocks.is_empty() {
        return;
    }
    let hh = f64::from(((tick_count as f32 + 35.0) * 0.1).sin() / 2.0 + 0.5);
    let hh = (hh * hh + hh) * 0.300_000_011_920_928_96;
    let [x, y, z] = ctx.pos.map(f64::from);
    let end = [x + 0.5, y + 1.5 + hh, z + 0.5];
    let deltas = ctx.random(|r| {
        effect_blocks
            .iter()
            .filter_map(|delta| {
                (r.next_int(50) == 0).then(|| {
                    let dx = -0.5 + r.next_f32() + delta[0] as f32;
                    let dy = -2.0 + r.next_f32() + delta[1] as f32;
                    let dz = -0.5 + r.next_f32() + delta[2] as f32;
                    [f64::from(dx), f64::from(dy), f64::from(dz)]
                })
            })
            .collect::<Vec<_>>()
    });
    for velocity in deltas {
        ctx.add_particle(Particle::Nautilus, end, velocity);
    }
}

fn cube(radius: i32) -> impl Iterator<Item = (i32, i32, i32)> {
    (-radius..=radius).flat_map(move |x| {
        (-radius..=radius).flat_map(move |y| (-radius..=radius).map(move |z| (x, y, z)))
    })
}
