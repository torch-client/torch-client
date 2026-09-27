use bevy::math::Vec3;

use crate::session::LevelEventEmit;
use crate::util::particles::{Emitter, rel, spawn_block_break};
use azalea::entity::particle::Particle as P;
use azalea::entity::particle::{BlockParticle, PowerParticle, SpellParticle};

#[derive(Clone, Copy)]
#[cfg_attr(not(feature = "audio"), allow(dead_code))]
enum Bus {
    Blocks,
    Hostile,
    Neutral,
}

#[derive(Clone, Copy)]
enum Pitch {
    Fixed(f32),
    Wobble,
    Slight,
}

#[rustfmt::skip]
const SOUNDS: &[(u32, &str, Bus, f32, Pitch)] = &[
    (1000, "block.dispenser.dispense", Bus::Blocks, 1.0, Pitch::Fixed(1.0)),
    (1001, "block.dispenser.fail", Bus::Blocks, 1.0, Pitch::Fixed(1.2)),
    (1002, "block.dispenser.launch", Bus::Blocks, 1.0, Pitch::Fixed(1.2)),
    (1004, "entity.firework_rocket.shoot", Bus::Neutral, 1.0, Pitch::Fixed(1.2)),
    (1015, "entity.ghast.warn", Bus::Hostile, 10.0, Pitch::Wobble),
    (1016, "entity.ghast.shoot", Bus::Hostile, 10.0, Pitch::Wobble),
    (1017, "entity.ender_dragon.shoot", Bus::Hostile, 10.0, Pitch::Wobble),
    (1018, "entity.blaze.shoot", Bus::Hostile, 2.0, Pitch::Wobble),
    (1019, "entity.zombie.attack_wooden_door", Bus::Hostile, 2.0, Pitch::Wobble),
    (1020, "entity.zombie.attack_iron_door", Bus::Hostile, 2.0, Pitch::Wobble),
    (1021, "entity.zombie.break_wooden_door", Bus::Hostile, 2.0, Pitch::Wobble),
    (1022, "entity.wither.break_block", Bus::Hostile, 2.0, Pitch::Wobble),
    (1024, "entity.wither.shoot", Bus::Hostile, 2.0, Pitch::Wobble),
    (1025, "entity.bat.takeoff", Bus::Neutral, 0.05, Pitch::Wobble),
    (1026, "entity.zombie.infect", Bus::Hostile, 2.0, Pitch::Wobble),
    (1027, "entity.zombie_villager.converted", Bus::Hostile, 2.0, Pitch::Wobble),
    (1029, "block.anvil.destroy", Bus::Blocks, 1.0, Pitch::Slight),
    (1030, "block.anvil.use", Bus::Blocks, 1.0, Pitch::Slight),
    (1031, "block.anvil.land", Bus::Blocks, 0.3, Pitch::Slight),
    (1033, "block.chorus_flower.grow", Bus::Blocks, 1.0, Pitch::Fixed(1.0)),
    (1034, "block.chorus_flower.death", Bus::Blocks, 1.0, Pitch::Fixed(1.0)),
    (1035, "block.brewing_stand.brew", Bus::Blocks, 1.0, Pitch::Fixed(1.0)),
    (1039, "entity.phantom.bite", Bus::Hostile, 0.3, Pitch::Slight),
    (1040, "entity.zombie.converted_to_drowned", Bus::Hostile, 2.0, Pitch::Wobble),
    (1041, "entity.husk.converted_to_zombie", Bus::Hostile, 2.0, Pitch::Wobble),
    (1042, "block.grindstone.use", Bus::Blocks, 1.0, Pitch::Slight),
    (1043, "item.book.page_turn", Bus::Blocks, 1.0, Pitch::Slight),
    (1044, "block.smithing_table.use", Bus::Blocks, 1.0, Pitch::Slight),
    (1045, "block.pointed_dripstone.land", Bus::Blocks, 2.0, Pitch::Slight),
    (1046, "block.pointed_dripstone.drip_lava_into_cauldron", Bus::Blocks, 2.0, Pitch::Slight),
    (1047, "block.pointed_dripstone.drip_water_into_cauldron", Bus::Blocks, 2.0, Pitch::Slight),
    (1048, "entity.skeleton.converted_to_stray", Bus::Hostile, 2.0, Pitch::Wobble),
    (1049, "block.crafter.craft", Bus::Blocks, 1.0, Pitch::Fixed(1.0)),
    (1050, "block.crafter.fail", Bus::Blocks, 1.0, Pitch::Fixed(1.0)),
];

const SHRIEKER_TOP_Y: f32 = 0.5;

pub(crate) fn spawn(event: &LevelEventEmit, ctx: &mut Emitter) {
    crate::log_debug!(
        "particles",
        "level event {} at {:?} data {} global {}",
        event.id,
        event.pos,
        event.data,
        event.global
    );
    ctx.cell = event.pos;
    if event.global {
        global(event, ctx);
        return;
    }
    if let Some(row) = SOUNDS.iter().find(|row| row.0 == event.id) {
        let pitch = match row.4 {
            Pitch::Fixed(p) => p,
            Pitch::Wobble => (ctx.f32() - ctx.f32()) * 0.2 + 1.0,
            Pitch::Slight => ctx.f32() * 0.1 + 0.9,
        };
        sound(event.pos, row.1, row.2, row.3, pitch);
        return;
    }
    effects(event, ctx);
}

fn global(event: &LevelEventEmit, ctx: &Emitter) {
    let (name, volume) = match event.id {
        1023 => ("entity.wither.spawn", 1.0),
        1028 => ("entity.ender_dragon.death", 5.0),
        1038 => ("block.end_portal.spawn", 1.0),
        _ => return,
    };
    let to_event = (center(event.pos) - ctx.eye).normalize_or_zero();
    let at = ctx.eye + to_event * 2.0;
    play(name, Bus::Hostile, [at.x, at.y, at.z], volume, 1.0);
}

fn effects(event: &LevelEventEmit, ctx: &mut Emitter) {
    let cell = event.pos;
    let data = event.data;
    match event.id {
        1009 => match data {
            0 => {
                let pitch = 2.6 + (ctx.f32() - ctx.f32()) * 0.8;
                sound(cell, "block.fire.extinguish", Bus::Blocks, 0.5, pitch);
            }
            1 => {
                let pitch = 1.6 + (ctx.f32() - ctx.f32()) * 0.4;
                sound(
                    cell,
                    "entity.generic.extinguish_fire",
                    Bus::Blocks,
                    0.7,
                    pitch,
                );
            }
            _ => {}
        },
        1032 => {
            let pitch = ctx.f32() * 0.4 + 0.8;
            play_local("block.portal.travel", Bus::Blocks, 0.25, pitch);
        }
        1051 => {
            let pitch = 0.4 / (ctx.f32() * 0.4 + 0.8);
            sound(cell, "entity.wind_charge.throw", Bus::Blocks, 0.5, pitch);
        }

        1500 => {
            let name = if data > 0 {
                "block.composter.fill_success"
            } else {
                "block.composter.fill"
            };
            sound(cell, name, Bus::Blocks, 1.0, 1.0);
            let top = 0.28125;
            for _ in 0..10 {
                let x = 0.1875 + 0.625 * ctx.f32();
                let y = top + ctx.f32() * (1.0 - top);
                let z = 0.1875 + 0.625 * ctx.f32();
                let vel = gaussian_vel(ctx, 0.02);
                ctx.emit(&P::Composter, rel(cell, [x, y, z]), vel, false);
            }
        }
        1501 => {
            let pitch = 2.6 + (ctx.f32() - ctx.f32()) * 0.8;
            sound(cell, "block.lava.extinguish", Bus::Blocks, 0.5, pitch);
            for _ in 0..8 {
                let x = ctx.f32();
                let z = ctx.f32();
                ctx.emit(&P::LargeSmoke, rel(cell, [x, 1.2, z]), Vec3::ZERO, false);
            }
        }
        1502 => {
            let pitch = 2.6 + (ctx.f32() - ctx.f32()) * 0.8;
            sound(
                cell,
                "block.redstone_torch.burnout",
                Bus::Blocks,
                0.5,
                pitch,
            );
            smoke_cloud(ctx, cell, 5);
        }
        1503 => {
            sound(cell, "block.end_portal_frame.fill", Bus::Blocks, 1.0, 1.0);
            smoke_cloud(ctx, cell, 16);
        }
        1504 => {
            let mut probe = cell;
            for _ in 0..11 {
                probe[1] += 1;
                let Some(above) = ctx.block(probe) else { break };
                if above
                    .as_block_kind()
                    .to_str()
                    .ends_with("pointed_dripstone")
                {
                    continue;
                }
                let fluid = azalea::block::fluid_state::FluidState::from(above);
                let drip = match fluid.kind {
                    azalea::block::fluid_state::FluidKind::Water => P::DrippingDripstoneWater,
                    azalea::block::fluid_state::FluidKind::Lava => P::DrippingDripstoneLava,
                    azalea::block::fluid_state::FluidKind::Empty => break,
                };
                ctx.emit(&drip, rel(cell, [0.5, 0.25, 0.5]), Vec3::ZERO, false);
                break;
            }
        }
        1505 => {
            sound(cell, "item.bone_meal.use", Bus::Blocks, 1.0, 1.0);
            spawn_in_block(ctx, cell, data.min(MAX_EVENT_COUNT), &P::HappyVillager);
        }

        2000 => shoot(ctx, cell, data, &P::Smoke),
        2001 | 3008 => {
            let state = u16::try_from(data)
                .ok()
                .and_then(|id| azalea::block::BlockState::try_from(id).ok());
            let Some(state) = state else { return };
            break_block(ctx, cell, state);
        }
        2002 | 2007 => {
            let kind = if event.id == 2007 {
                P::InstantEffect(SpellParticle::default())
            } else {
                P::Effect(SpellParticle::default())
            };
            for _ in 0..100 {
                let speed = ctx.rng.next_f64() * 4.0;
                let angle = ctx.rng.next_f64() * std::f64::consts::TAU;
                let vx = angle.cos() * speed;
                let vy = 0.01 + ctx.rng.next_f64() * 0.5;
                let vz = angle.sin() * speed;
                ctx.emit(
                    &kind,
                    rel(
                        cell,
                        [0.5 + (vx * 0.1) as f32, 0.3, 0.5 + (vz * 0.1) as f32],
                    ),
                    Vec3::new(vx as f32, vy as f32, vz as f32),
                    false,
                );
            }
            let pitch = ctx.f32() * 0.1 + 0.9;
            sound(cell, "entity.splash_potion.break", Bus::Neutral, 1.0, pitch);
        }
        2003 => {
            let mut angle = 0.0f64;
            while angle < std::f64::consts::TAU {
                let (sin, cos) = angle.sin_cos();
                let at = Vec3::new(
                    cell[0] as f32 + 0.5 + (cos * 5.0) as f32,
                    cell[1] as f32 - 0.4,
                    cell[2] as f32 + 0.5 + (sin * 5.0) as f32,
                );
                for speed in [-5.0f64, -7.0] {
                    ctx.emit(
                        &P::Portal,
                        at,
                        Vec3::new((cos * speed) as f32, 0.0, (sin * speed) as f32),
                        false,
                    );
                }
                angle += std::f64::consts::PI / 20.0;
            }
        }
        2004 => {
            for _ in 0..20 {
                let at = rel(
                    cell,
                    [
                        0.5 + (ctx.f32() - 0.5) * 2.0,
                        0.5 + (ctx.f32() - 0.5) * 2.0,
                        0.5 + (ctx.f32() - 0.5) * 2.0,
                    ],
                );
                ctx.emit(&P::Smoke, at, Vec3::ZERO, false);
                ctx.emit(&P::Flame, at, Vec3::ZERO, false);
            }
        }
        2006 => {
            for _ in 0..200 {
                let speed = ctx.f32() * 4.0;
                let angle = ctx.f32() * std::f32::consts::TAU;
                let vx = angle.cos() * speed;
                let vy = 0.01 + ctx.rng.next_f64() as f32 * 0.5;
                let vz = angle.sin() * speed;
                ctx.emit(
                    &P::DragonBreath(PowerParticle::default()),
                    rel(cell, [vx * 0.1, 0.3, vz * 0.1]),
                    Vec3::new(vx, vy, vz),
                    false,
                );
            }
            if data == 1 {
                let pitch = ctx.f32() * 0.1 + 0.9;
                sound(
                    cell,
                    "entity.dragon_fireball.explode",
                    Bus::Hostile,
                    1.0,
                    pitch,
                );
            }
        }
        2008 => {
            ctx.emit(&P::Explosion, rel(cell, [0.5, 0.5, 0.5]), Vec3::ZERO, false);
        }
        2009 => {
            for _ in 0..8 {
                let x = ctx.f32();
                let z = ctx.f32();
                ctx.emit(&P::Cloud, rel(cell, [x, 1.2, z]), Vec3::ZERO, false);
            }
        }
        2010 => shoot(ctx, cell, data, &P::WhiteSmoke),
        2011 | 2012 => spawn_in_block(ctx, cell, data.min(MAX_EVENT_COUNT), &P::HappyVillager),
        2013 => {
            let count = data.min(MAX_EVENT_COUNT);
            let smashed = P::DustPillar(BlockParticle {
                block_state: crate::util::particles::block_at(ctx.world, cell).unwrap_or_default(),
            });
            for _ in 0..count.div_ceil(3) {
                let x = ctx.rng.next_gaussian() as f32 / 2.0;
                let z = ctx.rng.next_gaussian() as f32 / 2.0;
                let vel = gaussian_vel(ctx, 0.2);
                ctx.emit(&smashed, rel(cell, [0.5 + x, 1.0, 0.5 + z]), vel, false);
            }
            for i in 0..count.div_ceil(2) {
                let i = i as f64;
                let x = 3.5 * i.cos() + ctx.rng.next_gaussian() / 2.0;
                let z = 3.5 * i.sin() + ctx.rng.next_gaussian() / 2.0;
                let vel = gaussian_vel(ctx, 0.05);
                ctx.emit(
                    &smashed,
                    rel(cell, [0.5 + x as f32, 1.0, 0.5 + z as f32]),
                    vel,
                    false,
                );
            }
        }

        3000 => {
            ctx.emit(
                &P::ExplosionEmitter,
                rel(cell, [0.5, 0.5, 0.5]),
                Vec3::ZERO,
                true,
            );
            let pitch = 1.0 + (ctx.f32() - ctx.f32()) * 0.2;
            sound(cell, "block.end_gateway.spawn", Bus::Blocks, 10.0, pitch);
        }
        3001 => {
            let pitch = 0.8 + ctx.f32() * 0.3;
            sound(cell, "entity.ender_dragon.growl", Bus::Hostile, 64.0, pitch);
        }
        3002 => match data {
            0..=2 => {
                let count = 10 + ctx.rng.next_int(10);
                for _ in 0..count {
                    along_axis(ctx, cell, data as usize, &P::ElectricSpark);
                }
            }
            _ => on_faces(ctx, cell, &P::ElectricSpark, 3, 3),
        },
        3003 => {
            on_faces(ctx, cell, &P::WaxOn, 3, 3);
            sound(cell, "item.honeycomb.wax_on", Bus::Blocks, 1.0, 1.0);
        }
        3004 => on_faces(ctx, cell, &P::WaxOff, 3, 3),
        3005 => on_faces(ctx, cell, &P::Scrape, 3, 3),
        3006 => sculk_charge(ctx, cell, data),
        3007 => {
            for i in 0..10 {
                let particle = P::Shriek(azalea::entity::particle::ShriekParticle { delay: i * 5 });
                ctx.emit(
                    &particle,
                    rel(cell, [0.5, SHRIEKER_TOP_Y, 0.5]),
                    Vec3::ZERO,
                    false,
                );
            }
            let pitch = 0.6 + ctx.f32() * 0.4;
            sound(cell, "block.sculk_shrieker.shriek", Bus::Blocks, 2.0, pitch);
        }
        3009 => on_faces(ctx, cell, &P::EggCrack, 3, 4),
        3011 | 3012 | 3021 => {
            if event.id != 3011 {
                let pitch = (ctx.f32() - ctx.f32()) * 0.2 + 1.0;
                let name = if event.id == 3012 {
                    "block.trial_spawner.spawn_mob"
                } else {
                    "block.trial_spawner.spawn_item"
                };
                sound(cell, name, Bus::Blocks, 1.0, pitch);
            }
            let flame = flame_particle(data);
            for _ in 0..20 {
                let at = rel(
                    cell,
                    [
                        0.5 + (ctx.f32() - 0.5) * 2.0,
                        0.5 + (ctx.f32() - 0.5) * 2.0,
                        0.5 + (ctx.f32() - 0.5) * 2.0,
                    ],
                );
                ctx.emit(&P::Smoke, at, Vec3::ZERO, false);
                ctx.emit(&flame, at, Vec3::ZERO, false);
            }
        }
        3013 | 3019 | 3020 => {
            let ominous = event.id != 3013;
            if event.id == 3020 {
                let volume = if data == 0 { 0.3 } else { 1.0 };
                let pitch = (ctx.f32() - ctx.f32()) * 0.2 + 1.0;
                sound(
                    cell,
                    "block.trial_spawner.ominous_activate",
                    Bus::Blocks,
                    volume,
                    pitch,
                );
            } else {
                let pitch = (ctx.f32() - ctx.f32()) * 0.2 + 1.0;
                sound(
                    cell,
                    "block.trial_spawner.detect_player",
                    Bus::Blocks,
                    1.0,
                    pitch,
                );
            }
            let players = if event.id == 3020 { 0 } else { data.min(10) };
            let kind = if ominous {
                P::TrialSpawnerDetectionOminous
            } else {
                P::TrialSpawnerDetection
            };
            for _ in 0..30 + players * 5 {
                let x = (2.0 * ctx.f32() - 1.0) * 0.65;
                let z = (2.0 * ctx.f32() - 1.0) * 0.65;
                let y = 0.1 + ctx.f32() * 0.8;
                ctx.emit(&kind, rel(cell, [0.5 + x, y, 0.5 + z]), Vec3::ZERO, false);
            }
        }
        3014 => {
            let pitch = (ctx.f32() - ctx.f32()) * 0.2 + 1.0;
            sound(
                cell,
                "block.trial_spawner.eject_item",
                Bus::Blocks,
                1.0,
                pitch,
            );
        }
        3015 | 3016 => {
            let flame = if data == 0 {
                P::SmallFlame
            } else {
                P::SoulFireFlame
            };
            let activating = event.id == 3015;
            for _ in 0..20 {
                if activating {
                    let at = rel(
                        cell,
                        [
                            0.1 + ctx.f32() * 0.8,
                            0.25 + ctx.f32() * 0.5,
                            0.1 + ctx.f32() * 0.8,
                        ],
                    );
                    ctx.emit(&P::Smoke, at, Vec3::ZERO, false);
                    ctx.emit(&flame, at, Vec3::ZERO, false);
                } else {
                    let at = rel(
                        cell,
                        [
                            0.4 + ctx.f32() * 0.2,
                            0.4 + ctx.f32() * 0.2,
                            0.4 + ctx.f32() * 0.2,
                        ],
                    );
                    let vel = gaussian_vel(ctx, 0.02);
                    ctx.emit(&flame, at, vel, false);
                }
            }
            let name = if activating {
                "block.vault.activate"
            } else {
                "block.vault.deactivate"
            };
            let pitch = (ctx.f32() - ctx.f32()) * 0.2 + 1.0;
            sound(cell, name, Bus::Blocks, 1.0, pitch);
        }
        3017 => {
            for _ in 0..20 {
                let at = rel(
                    cell,
                    [
                        0.4 + ctx.f32() * 0.2,
                        0.4 + ctx.f32() * 0.2,
                        0.4 + ctx.f32() * 0.2,
                    ],
                );
                let vel = gaussian_vel(ctx, 0.02);
                ctx.emit(
                    &P::SmallFlame,
                    at,
                    Vec3::new(vel.x, vel.y, vel.z * 0.25),
                    false,
                );
                ctx.emit(&P::Smoke, at, vel, false);
            }
            let pitch = (ctx.f32() - ctx.f32()) * 0.2 + 1.0;
            sound(cell, "block.vault.eject_item", Bus::Blocks, 1.0, pitch);
        }
        3018 => {
            for _ in 0..10 {
                let x = ctx.f32();
                let y = ctx.f32();
                let z = ctx.f32();
                let vel = gaussian_vel(ctx, 0.02);
                ctx.emit(&P::Poof, rel(cell, [x, y, z]), vel, false);
            }
            let pitch = (ctx.f32() - ctx.f32()) * 0.2 + 1.0;
            sound(cell, "block.cobweb.place", Bus::Blocks, 1.0, pitch);
        }
        _ => {}
    }
}

const MAX_EVENT_COUNT: u32 = 256;

fn shoot(ctx: &mut Emitter, cell: [i32; 3], data: u32, kind: &P) {
    let step = FACES[(data as usize) % FACES.len()];
    for _ in 0..10 {
        let pow = ctx.f32() * 0.2 + 0.01;
        let at = rel(
            cell,
            [
                step[0] * 0.61 + 0.5 + (ctx.f32() - 0.5) * step[2] * 0.5,
                step[1] * 0.61 + 0.5 + (ctx.f32() - 0.5) * step[1] * 0.5,
                step[2] * 0.61 + 0.5 + (ctx.f32() - 0.5) * step[0] * 0.5,
            ],
        );
        let vel = Vec3::new(
            step[0] * pow + ctx.rng.next_gaussian() as f32 * 0.01,
            step[1] * pow + ctx.rng.next_gaussian() as f32 * 0.01,
            step[2] * pow + ctx.rng.next_gaussian() as f32 * 0.01,
        );
        ctx.emit(kind, at, vel, false);
    }
}

const FACES: [[f32; 3]; 6] = [
    [0.0, -1.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, -1.0],
    [0.0, 0.0, 1.0],
    [-1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
];

fn on_faces(ctx: &mut Emitter, cell: [i32; 3], kind: &P, lo: u32, span: u32) {
    for step in FACES {
        let count = lo + ctx.rng.next_int(span);
        for _ in 0..count {
            on_face(ctx, cell, step, kind, 0.55, 0.5);
        }
    }
}

fn on_face(
    ctx: &mut Emitter,
    cell: [i32; 3],
    step: [f32; 3],
    kind: &P,
    step_factor: f32,
    speed: f32,
) {
    let mut at = [0.0f32; 3];
    let mut vel = [0.0f32; 3];
    for axis in 0..3 {
        if step[axis] == 0.0 {
            at[axis] = 0.5 + (ctx.f32() - 0.5);
            vel[axis] = (ctx.f32() - 0.5) * 2.0 * speed;
        } else {
            at[axis] = 0.5 + step[axis] * step_factor;
        }
    }
    ctx.emit(kind, rel(cell, at), Vec3::from_array(vel), false);
}

fn along_axis(ctx: &mut Emitter, cell: [i32; 3], axis: usize, kind: &P) {
    const RADIUS: f32 = 0.125;
    let mut at = [0.0f32; 3];
    let mut vel = [0.0f32; 3];
    for i in 0..3 {
        let spread = if i == axis { 0.5 } else { RADIUS };
        at[i] = 0.5 + (ctx.f32() * 2.0 - 1.0) * spread;
        if i == axis {
            vel[i] = ctx.f32() * 2.0 - 1.0;
        }
    }
    ctx.emit(kind, rel(cell, at), Vec3::from_array(vel), false);
}

fn spawn_in_block(ctx: &mut Emitter, cell: [i32; 3], count: u32, kind: &P) {
    for _ in 0..count {
        let vel = gaussian_vel(ctx, 0.02);
        let x = ctx.f32();
        let y = ctx.f32();
        let z = ctx.f32();
        ctx.emit(kind, rel(cell, [x, y, z]), vel, false);
    }
}

fn sculk_charge(ctx: &mut Emitter, cell: [i32; 3], data: u32) {
    let charge = (data >> 6).min(MAX_EVENT_COUNT);
    if charge == 0 {
        sound(cell, "block.sculk.charge", Bus::Blocks, 1.0, 1.0);
        let solid = ctx.solid(cell);
        let count = if solid { 40 } else { 20 };
        let spread = if solid { 0.45 } else { 0.25 };
        for _ in 0..count {
            let dir = Vec3::new(
                2.0 * ctx.f32() - 1.0,
                2.0 * ctx.f32() - 1.0,
                2.0 * ctx.f32() - 1.0,
            );
            ctx.emit(
                &P::SculkChargePop,
                rel(cell, [0.5, 0.5, 0.5]) + dir * spread,
                dir * 0.07,
                false,
            );
        }
        return;
    }

    if ctx.f32() < 0.3 + charge as f32 * 0.1 {
        let volume = 0.15 + 0.02 * charge as f32 * charge as f32 * ctx.f32();
        let pitch = 0.4 + 0.3 * charge as f32 * ctx.f32();
        sound(cell, "block.sculk.charge", Bus::Blocks, volume, pitch);
    }
    let faces = data & 63;
    for (i, step) in FACES.iter().enumerate() {
        if faces != 0 && faces & (1 << i) == 0 {
            continue;
        }
        let roll = if faces == 0 {
            if step[1] < 0.0 {
                std::f32::consts::PI
            } else {
                0.0
            }
        } else if step[1] > 0.0 {
            std::f32::consts::PI
        } else {
            0.0
        };
        let step_factor = if faces != 0 {
            0.35
        } else if step[1] != 0.0 {
            0.65
        } else {
            0.57
        };
        let particle = P::SculkCharge(azalea::entity::particle::SculkChargeParticle { roll });
        let repetitions = ctx.rng.next_int(charge + 1);
        for _ in 0..repetitions {
            on_face(ctx, cell, *step, &particle, step_factor, 0.005);
        }
    }
}

fn flame_particle(data: u32) -> P {
    if data == 1 {
        P::SoulFireFlame
    } else {
        P::Flame
    }
}

fn break_block(ctx: &mut Emitter, cell: [i32; 3], state: azalea::block::BlockState) {
    let spawn = crate::session::ParticleSpawn {
        pos: cell,
        state,
        mining: false,
        face: [0.0, 1.0, 0.0],
    };
    spawn_block_break(&spawn, ctx.status, Some(ctx.world), ctx.live, ctx.rng);

    #[cfg(feature = "audio")]
    {
        use azalea::block::BlockTrait;
        let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
        let group = crate::util::block_model::sound_group(block.id());
        sound(cell, group.break_event, Bus::Blocks, 1.0, 0.8);
    }
}

fn smoke_cloud(ctx: &mut Emitter, cell: [i32; 3], count: u32) {
    for _ in 0..count {
        let x = ctx.f32();
        let y = ctx.f32();
        let z = ctx.f32();
        ctx.emit(&P::Smoke, rel(cell, [x, y, z]), Vec3::ZERO, false);
    }
}

fn gaussian_vel(ctx: &mut Emitter, scale: f32) -> Vec3 {
    Vec3::new(
        ctx.rng.next_gaussian() as f32 * scale,
        ctx.rng.next_gaussian() as f32 * scale,
        ctx.rng.next_gaussian() as f32 * scale,
    )
}

fn center(cell: [i32; 3]) -> Vec3 {
    Vec3::new(
        cell[0] as f32 + 0.5,
        cell[1] as f32 + 0.5,
        cell[2] as f32 + 0.5,
    )
}

fn sound(cell: [i32; 3], event: &str, bus: Bus, volume: f32, pitch: f32) {
    let at = center(cell);
    play(event, bus, [at.x, at.y, at.z], volume, pitch);
}

#[cfg_attr(not(feature = "audio"), allow(unused_variables))]
fn play(event: &str, bus: Bus, at: [f32; 3], volume: f32, pitch: f32) {
    #[cfg(feature = "audio")]
    crate::audio::play_at(event, bus.category(), at, volume, pitch);
}

#[cfg_attr(not(feature = "audio"), allow(unused_variables))]
fn play_local(event: &str, bus: Bus, volume: f32, pitch: f32) {
    #[cfg(feature = "audio")]
    crate::audio::play(event, bus.category(), volume, pitch);
}

#[cfg(feature = "audio")]
impl Bus {
    fn category(self) -> crate::audio::SoundCategory {
        match self {
            Bus::Blocks => crate::audio::SoundCategory::Blocks,
            Bus::Hostile => crate::audio::SoundCategory::Hostile,
            Bus::Neutral => crate::audio::SoundCategory::Neutral,
        }
    }
}
