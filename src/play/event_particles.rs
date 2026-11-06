use std::str::FromStr;

use azalea::block::BlockState;
use azalea::ecs::entity::Entity;
use azalea::ecs::prelude::{MessageReader, Query};
use azalea::ecs::query::Has;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::indexing::EntityIdIndex;
use azalea::entity::inventory::Inventory;
use azalea::entity::metadata::{AbstractLiving, AbstractThrownItemProjectileItemStack};
use azalea::entity::particle::{BlockParticle, ItemParticle, Particle};
use azalea::entity::{EntityKindComponent, LocalEntity, LookDirection, Physics, Position};
use azalea::local_player::WorldHolder;
use azalea::packet::game::ReceiveGamePacketEvent;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::position::BlockPos;
use azalea_inventory::ItemStack;
use azalea_inventory::components::EquipmentSlot;
use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_registry::builtin::{BlockKind, EntityKind, ItemKind};

use crate::client::tracking::{
    EntityAttributes, Equipment, attributes, entity_anims, equipment, local_entity_id, velocities,
};
use crate::play::emit;
use crate::play::entity_feed::entity_event::JUMP;
use crate::session::{ItemUseEmit, LevelParticle, ParticleEmit, SideMutex};
use crate::util::javarandom::JavaRandom;
use crate::util::mth::DEG_TO_RAD;

const DEATH: u8 = 3;
const TAMING_FAILED: u8 = 6;
const TAMING_SUCCEEDED: u8 = 7;
const LOVE_HEARTS: u8 = 12;
const VILLAGER_ANGRY: u8 = 13;
const VILLAGER_HAPPY: u8 = 14;
const WITCH_HAT_MAGIC: u8 = 15;
const IN_LOVE_HEARTS: u8 = 18;
const SILVERFISH_MERGE_ANIM: u8 = 20;
const DOLPHIN_LOOKING_FOR_TREASURE: u8 = 38;
const TRUSTING_FAILED: u8 = 40;
const TRUSTING_SUCCEEDED: u8 = 41;
const VILLAGER_SWEAT: u8 = 42;
const FOX_EAT: u8 = 45;
const TELEPORT: u8 = 46;
const MAINHAND_BREAK: u8 = 47;
const OFFHAND_BREAK: u8 = 48;
const HEAD_BREAK: u8 = 49;
const CHEST_BREAK: u8 = 50;
const LEGS_BREAK: u8 = 51;
const FEET_BREAK: u8 = 52;
const HONEY_SLIDE: u8 = 53;
const HONEY_JUMP: u8 = 54;
const POOF: u8 = 60;
const BODY_BREAK: u8 = 65;
const DROWN_PARTICLES: u8 = 67;
const SADDLE_BREAK: u8 = 68;
const RAVAGER_ROARED: u8 = 69;

const TAMING_PARTICLES: u32 = 7;
const ALLAY_HEARTS: u32 = 3;
const VILLAGER_PARTICLES: u32 = 5;
const DOLPHIN_PARTICLES: u32 = 7;
const SPREAD: f64 = 0.02;
const DOLPHIN_SPREAD: f64 = 0.01;
const TAMING_LIFT: f64 = 0.5;
const VILLAGER_LIFT: f64 = 1.0;
const DOLPHIN_LIFT: f64 = 0.2;
const WITCH_BOUND: u32 = 35;
const WITCH_MIN: u32 = 10;
const WITCH_SPREAD: f64 = 0.12999999523162842;
const WITCH_LIFT: f64 = 0.5;
const POOF_PARTICLES: u32 = 20;
const POOF_LEAD: f64 = 10.0;
const TELEPORT_PARTICLES: u32 = 128;
const TELEPORT_SPEED: f32 = 0.2;
const BREAK_PARTICLES: u32 = 5;
const HONEY_SLIDE_PARTICLES: u32 = 5;
const HONEY_JUMP_PARTICLES: u32 = 10;
const DROWN_BUBBLES: u32 = 8;
const ROAR_PARTICLES: u32 = 40;
const ROAR_SPEED: f64 = 0.2;
const THROWN_PARTICLES: u32 = 8;
const EGG_SPEED: f64 = 0.08;
const FOX_PARTICLES: u32 = 8;
const ON_POS_LEGACY: f32 = 0.2;
const SPRINT_LIFT: f64 = 0.1;
const SPRINT_RISE: f64 = 1.5;
const SPRINT_DRAG: f64 = -4.0;

const QUEUE_SHARE: usize = crate::session::MAX_PENDING_PARTICLE_EMITS - 64;

static RNG: SideMutex<Option<JavaRandom>> = SideMutex::new(None);

#[derive(Clone, Debug)]
struct Body {
    kind: EntityKind,
    living: bool,
    pos: [f64; 3],
    old: [f64; 3],
    width: f64,
    height: f64,
    eye_height: f64,
    x_rot: f32,
    y_rot: f32,
    velocity: [f64; 3],
    item: Option<&'static str>,
    ground: Option<BlockState>,
}

impl Body {
    fn is_mob(&self) -> bool {
        self.living
            && !matches!(
                self.kind,
                EntityKind::Player | EntityKind::ArmorStand | EntityKind::Mannequin
            )
    }

    fn random_point(&self, rng: &mut JavaRandom) -> [f64; 3] {
        emit::random_point(rng, self.pos, self.width, self.height, 1.0)
    }
}

struct Pending {
    event: u8,
    id: i32,
    entity: Option<Entity>,
    receiver: Entity,
}

type EcsBody = (
    &'static Position,
    Option<&'static Physics>,
    Option<&'static EntityDimensions>,
    &'static EntityKindComponent,
    Option<&'static LookDirection>,
    Has<AbstractLiving>,
    Has<LocalEntity>,
    Option<&'static AbstractThrownItemProjectileItemStack>,
);

pub(crate) fn emit_event_particles(
    mut events: MessageReader<ReceiveGamePacketEvent>,
    indexes: Query<&EntityIdIndex>,
    entities: Query<EcsBody>,
    inventories: Query<&Inventory>,
    holders: Query<&WorldHolder>,
) {
    let pending: Vec<Pending> = events
        .read()
        .filter_map(|event| {
            let ClientboundGamePacket::EntityEvent(p) = event.packet.as_ref() else {
                return None;
            };
            handled(p.event_id).then(|| Pending {
                event: p.event_id,
                id: p.entity_id.0,
                entity: find_entity(event, p.entity_id, &indexes),
                receiver: event.entity,
            })
        })
        .collect();
    if pending.is_empty() {
        return;
    }
    let Some(shared) = crate::SHARED.get() else {
        return;
    };
    let local = local_entity_id();

    let mut bodies = snapshot_bodies(&pending);
    {
        let attributes = attributes().lock().unwrap();
        for (body, p) in bodies.iter_mut().zip(&pending) {
            if body.is_none() {
                *body = p
                    .entity
                    .and_then(|e| entities.get(e).ok())
                    .map(|q| ecs_body(q, p.id, &attributes));
            }
        }
    }
    fill_items(&mut bodies, &pending, local, &inventories);
    fill_velocities(&mut bodies, &pending, local);
    fill_ground(&mut bodies, &pending, &holders);

    let origin = pending
        .first()
        .and_then(|p| entities.get(p.receiver).ok())
        .map(|q| [q.0.x, q.0.y, q.0.z]);

    let mut emits = Vec::new();
    let mut ends = Vec::with_capacity(pending.len());
    {
        let mut rng = RNG.lock().unwrap();
        let rng = rng.get_or_insert_with(JavaRandom::time_seeded);
        for (body, p) in bodies.iter().zip(&pending) {
            let Some(body) = body else { continue };
            if !overrides_limiter(p.event)
                && origin
                    .is_some_and(|o| distance_sq(o, body.pos) > emit::EMIT_RANGE * emit::EMIT_RANGE)
            {
                continue;
            }
            spawn(p.event, body, rng, &mut emits);
            ends.push(emits.len());
        }
    }
    if emits.is_empty() {
        return;
    }
    let mut s = shared.lock().unwrap();
    let queue = &mut s.session.particle_emits;
    let room = QUEUE_SHARE.saturating_sub(queue.len());
    let fit = ends
        .iter()
        .copied()
        .take_while(|&end| end <= room)
        .last()
        .unwrap_or(0);
    emits.truncate(fit);
    crate::session::push_particle_emits(queue, emits);
}

fn handled(event: u8) -> bool {
    matches!(
        event,
        JUMP | DEATH
            | TAMING_FAILED
            | TAMING_SUCCEEDED
            | LOVE_HEARTS
            | VILLAGER_ANGRY
            | VILLAGER_HAPPY
            | WITCH_HAT_MAGIC
            | IN_LOVE_HEARTS
            | SILVERFISH_MERGE_ANIM
            | DOLPHIN_LOOKING_FOR_TREASURE
            | TRUSTING_FAILED
            | TRUSTING_SUCCEEDED
            | VILLAGER_SWEAT
            | FOX_EAT
            | TELEPORT
            | MAINHAND_BREAK
            ..=FEET_BREAK
                | HONEY_SLIDE
                | HONEY_JUMP
                | POOF
                | BODY_BREAK
                | DROWN_PARTICLES
                | SADDLE_BREAK
                | RAVAGER_ROARED
    )
}

fn overrides_limiter(event: u8) -> bool {
    matches!(event, SILVERFISH_MERGE_ANIM | POOF | RAVAGER_ROARED)
}

fn distance_sq(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]) * (a[i] - b[i])).sum()
}

fn find_entity(
    event: &ReceiveGamePacketEvent,
    id: MinecraftEntityId,
    indexes: &Query<&EntityIdIndex>,
) -> Option<Entity> {
    if id.0 == local_entity_id() {
        return Some(event.entity);
    }
    indexes
        .get(event.entity)
        .ok()
        .and_then(|i| i.get_by_minecraft_entity(id))
}

fn snapshot_bodies(pending: &[Pending]) -> Vec<Option<Body>> {
    let anims = entity_anims().lock().unwrap();
    pending
        .iter()
        .map(|p| {
            let anim = anims.get(&p.id)?;
            let (y_rot, x_rot) = anim.look(1.0);
            let thrown = anim.shared().item.id;
            Some(Body {
                kind: anim.kind,
                living: anim.is_living(),
                pos: anim.position(1.0),
                old: anim.position(0.0),
                width: f64::from(anim.bounding_box_width()),
                height: f64::from(anim.bounding_box_height()),
                eye_height: f64::from(anim.eye_height()),
                x_rot,
                y_rot,
                velocity: [0.0; 3],
                item: (!thrown.is_empty()).then_some(thrown),
                ground: None,
            })
        })
        .collect()
}

fn ecs_body(
    (pos, physics, dims, kind, look, living, is_local, thrown): (
        &Position,
        Option<&Physics>,
        Option<&EntityDimensions>,
        &EntityKindComponent,
        Option<&LookDirection>,
        bool,
        bool,
        Option<&AbstractThrownItemProjectileItemStack>,
    ),
    id: i32,
    attributes: &bevy::platform::collections::HashMap<i32, EntityAttributes>,
) -> Body {
    let scale = emit::box_scale(attributes, id, is_local);
    let dims = dims
        .cloned()
        .unwrap_or_else(|| EntityDimensions::from(kind.0));
    let old = physics.map_or(**pos, |p| p.old_position);
    let velocity = physics
        .filter(|_| is_local)
        .map_or([0.0; 3], |p| [p.velocity.x, p.velocity.y, p.velocity.z]);
    Body {
        kind: kind.0,
        living,
        pos: [pos.x, pos.y, pos.z],
        old: [old.x, old.y, old.z],
        width: f64::from(dims.width * scale),
        height: f64::from(dims.height * scale),
        eye_height: f64::from(dims.eye_height * scale),
        x_rot: look.map_or(0.0, |l| l.x_rot()),
        y_rot: look.map_or(0.0, |l| l.y_rot()),
        velocity,
        item: thrown.and_then(|s| crate::client::tick::item_id(&s.0)),
        ground: None,
    }
}

fn break_slot(event: u8) -> Option<EquipmentSlot> {
    Some(match event {
        MAINHAND_BREAK => EquipmentSlot::Mainhand,
        OFFHAND_BREAK => EquipmentSlot::Offhand,
        HEAD_BREAK => EquipmentSlot::Head,
        CHEST_BREAK => EquipmentSlot::Chest,
        LEGS_BREAK => EquipmentSlot::Legs,
        FEET_BREAK => EquipmentSlot::Feet,
        BODY_BREAK => EquipmentSlot::Body,
        SADDLE_BREAK => EquipmentSlot::Saddle,
        _ => return None,
    })
}

fn worn(on: &Equipment, slot: EquipmentSlot) -> Option<&'static str> {
    let held = |item: &crate::renderer::HeldItem| (!item.is_empty()).then_some(item.id);
    match slot {
        EquipmentSlot::Mainhand => held(&on.main_hand),
        EquipmentSlot::Offhand => held(&on.off_hand),
        EquipmentSlot::Head => held(&on.head),
        EquipmentSlot::Chest => on.chestplate,
        EquipmentSlot::Legs => on.leggings,
        EquipmentSlot::Feet => on.boots,
        EquipmentSlot::Body => on.body_armor,
        EquipmentSlot::Saddle => on.saddle,
    }
}

fn fill_items(
    bodies: &mut [Option<Body>],
    pending: &[Pending],
    local: i32,
    inventories: &Query<&Inventory>,
) {
    let slot_of = |event: u8| {
        if event == FOX_EAT {
            Some(EquipmentSlot::Mainhand)
        } else {
            break_slot(event)
        }
    };
    if !pending.iter().any(|p| slot_of(p.event).is_some()) {
        return;
    }
    let table = equipment().lock().unwrap();
    for (body, p) in bodies.iter_mut().zip(pending) {
        let (Some(body), Some(slot)) = (body.as_mut(), slot_of(p.event)) else {
            continue;
        };
        body.item = if p.id == local {
            inventories
                .get(p.receiver)
                .ok()
                .and_then(|inventory| inventory.get_equipment(slot))
                .and_then(crate::client::tick::item_id)
        } else {
            table.get(&p.id).and_then(|w| worn(w, slot))
        };
    }
}

fn fill_velocities(bodies: &mut [Option<Body>], pending: &[Pending], local: i32) {
    let reads = |event: u8| matches!(event, JUMP | DROWN_PARTICLES);
    if !pending.iter().any(|p| p.id != local && reads(p.event)) {
        return;
    }
    let table = velocities().lock().unwrap();
    for (body, p) in bodies.iter_mut().zip(pending) {
        if let Some(body) = body.as_mut()
            && p.id != local
            && reads(p.event)
        {
            body.velocity = table.get(&p.id).map_or([0.0; 3], |(v, _)| *v);
        }
    }
}

fn fill_ground(bodies: &mut [Option<Body>], pending: &[Pending], holders: &Query<&WorldHolder>) {
    for (body, p) in bodies.iter_mut().zip(pending) {
        let Some(body) = body.as_mut() else { continue };
        if p.event != JUMP || body.kind != EntityKind::Rabbit {
            continue;
        }
        let Ok(holder) = holders.get(p.receiver) else {
            continue;
        };
        body.ground = holder
            .shared
            .read()
            .get_block_state(on_pos_legacy(body.pos));
    }
}

fn on_pos_legacy(pos: [f64; 3]) -> BlockPos {
    BlockPos::new(
        pos[0].floor() as i32,
        (pos[1] - f64::from(ON_POS_LEGACY)).floor() as i32,
        pos[2].floor() as i32,
    )
}

fn invisible(state: BlockState) -> bool {
    use BlockKind as B;
    crate::blocks::state_is_air(state)
        || matches!(
            state.as_block_kind(),
            B::Barrier
                | B::BubbleColumn
                | B::EndGateway
                | B::EndPortal
                | B::Lava
                | B::Light
                | B::MovingPiston
                | B::StructureVoid
                | B::Water
        )
}

fn is_tamable(kind: EntityKind) -> bool {
    use EntityKind as K;
    matches!(
        kind,
        K::Cat | K::Parrot | K::Wolf | K::Nautilus | K::ZombieNautilus
    )
}

fn is_horse(kind: EntityKind) -> bool {
    use EntityKind as K;
    matches!(
        kind,
        K::Camel
            | K::CamelHusk
            | K::Horse
            | K::SkeletonHorse
            | K::ZombieHorse
            | K::Donkey
            | K::Llama
            | K::TraderLlama
            | K::Mule
    )
}

fn is_animal(kind: EntityKind) -> bool {
    use EntityKind as K;
    is_tamable(kind)
        || is_horse(kind)
        || matches!(
            kind,
            K::Armadillo
                | K::Axolotl
                | K::Bee
                | K::Chicken
                | K::Cow
                | K::Fox
                | K::Frog
                | K::Goat
                | K::HappyGhast
                | K::Hoglin
                | K::Mooshroom
                | K::Ocelot
                | K::Panda
                | K::Pig
                | K::PolarBear
                | K::Rabbit
                | K::Sheep
                | K::Sniffer
                | K::Strider
                | K::Turtle
        )
}

fn add(out: &mut Vec<ParticleEmit>, particle: Particle, pos: [f64; 3], velocity: [f64; 3]) {
    out.push(ParticleEmit::Level(LevelParticle::add(
        particle,
        pos,
        velocity.map(|v| v as f32),
    )));
}

fn around(
    out: &mut Vec<ParticleEmit>,
    rng: &mut JavaRandom,
    body: &Body,
    particle: Particle,
    count: u32,
    spread: f64,
    lift: f64,
) {
    for _ in 0..count {
        let velocity = [0; 3].map(|_| rng.next_gaussian() * spread);
        let [x, y, z] = body.random_point(rng);
        add(out, particle.clone(), [x, y + lift, z], velocity);
    }
}

fn item_particle(id: &str) -> Option<Particle> {
    let kind = ItemKind::from_str(id).ok()?;
    Some(Particle::Item(ItemParticle {
        item: ItemStack::new(kind, 1),
    }))
}

fn x_rot(v: [f64; 3], radians: f32) -> [f64; 3] {
    let (sin, cos) = (f64::from(radians.sin()), f64::from(radians.cos()));
    [v[0], v[1] * cos + v[2] * sin, v[2] * cos - v[1] * sin]
}

fn y_rot(v: [f64; 3], radians: f32) -> [f64; 3] {
    let (sin, cos) = (f64::from(radians.sin()), f64::from(radians.cos()));
    [v[0] * cos + v[2] * sin, v[1], v[2] * cos - v[0] * sin]
}

fn look_angle(x_rot: f32, y_rot: f32) -> [f64; 3] {
    let (x, y) = (x_rot * DEG_TO_RAD, -y_rot * DEG_TO_RAD);
    let (x_sin, x_cos) = x.sin_cos();
    let (y_sin, y_cos) = y.sin_cos();
    [
        f64::from(y_sin * x_cos),
        f64::from(-x_sin),
        f64::from(y_cos * x_cos),
    ]
}

fn spawn(event: u8, body: &Body, rng: &mut JavaRandom, out: &mut Vec<ParticleEmit>) {
    use EntityKind as K;
    let kind = body.kind;
    match event {
        JUMP if kind == K::Rabbit => {
            let Some(state) = body.ground.filter(|&s| !invisible(s)) else {
                return;
            };
            let x = body.pos[0] + (rng.next_f64() - 0.5) * body.width;
            let z = body.pos[2] + (rng.next_f64() - 0.5) * body.width;
            let [vx, _, vz] = body.velocity;
            add(
                out,
                Particle::Block(BlockParticle { block_state: state }),
                [x, body.pos[1] + SPRINT_LIFT, z],
                [vx * SPRINT_DRAG, SPRINT_RISE, vz * SPRINT_DRAG],
            );
        }
        DEATH if kind == K::Snowball => {
            let Some(particle) = item_particle(body.item.unwrap_or("snowball")) else {
                return;
            };
            for _ in 0..THROWN_PARTICLES {
                add(out, particle.clone(), body.pos, [0.0; 3]);
            }
        }
        DEATH if kind == K::Egg => {
            let Some(particle) = item_particle(body.item.unwrap_or("egg")) else {
                return;
            };
            for _ in 0..THROWN_PARTICLES {
                let velocity = [0; 3].map(|_| (f64::from(rng.next_f32()) - 0.5) * EGG_SPEED);
                add(out, particle.clone(), body.pos, velocity);
            }
        }
        TAMING_FAILED | TAMING_SUCCEEDED if is_tamable(kind) || is_horse(kind) => {
            let particle = if event == TAMING_SUCCEEDED {
                Particle::Heart
            } else {
                Particle::Smoke
            };
            around(
                out,
                rng,
                body,
                particle,
                TAMING_PARTICLES,
                SPREAD,
                TAMING_LIFT,
            );
        }
        TRUSTING_FAILED | TRUSTING_SUCCEEDED if kind == K::Ocelot => {
            let particle = if event == TRUSTING_SUCCEEDED {
                Particle::Heart
            } else {
                Particle::Smoke
            };
            around(
                out,
                rng,
                body,
                particle,
                TAMING_PARTICLES,
                SPREAD,
                TAMING_LIFT,
            );
        }
        LOVE_HEARTS | VILLAGER_ANGRY | VILLAGER_HAPPY | VILLAGER_SWEAT if kind == K::Villager => {
            let particle = match event {
                LOVE_HEARTS => Particle::Heart,
                VILLAGER_ANGRY => Particle::AngryVillager,
                VILLAGER_HAPPY => Particle::HappyVillager,
                _ => Particle::Splash,
            };
            around(
                out,
                rng,
                body,
                particle,
                VILLAGER_PARTICLES,
                SPREAD,
                VILLAGER_LIFT,
            );
        }
        IN_LOVE_HEARTS if is_animal(kind) => {
            around(
                out,
                rng,
                body,
                Particle::Heart,
                TAMING_PARTICLES,
                SPREAD,
                TAMING_LIFT,
            );
        }
        IN_LOVE_HEARTS if kind == K::Allay => {
            around(
                out,
                rng,
                body,
                Particle::Heart,
                ALLAY_HEARTS,
                SPREAD,
                TAMING_LIFT,
            );
        }
        DOLPHIN_LOOKING_FOR_TREASURE if kind == K::Dolphin => {
            around(
                out,
                rng,
                body,
                Particle::HappyVillager,
                DOLPHIN_PARTICLES,
                DOLPHIN_SPREAD,
                DOLPHIN_LIFT,
            );
        }
        WITCH_HAT_MAGIC if kind == K::Witch => {
            let top = body.pos[1] + body.height + WITCH_LIFT;
            let mut i = 0;
            while i < rng.next_int(WITCH_BOUND) + WITCH_MIN {
                let x = body.pos[0] + rng.next_gaussian() * WITCH_SPREAD;
                let y = top + rng.next_gaussian() * WITCH_SPREAD;
                let z = body.pos[2] + rng.next_gaussian() * WITCH_SPREAD;
                add(out, Particle::Witch, [x, y, z], [0.0; 3]);
                i += 1;
            }
        }
        SILVERFISH_MERGE_ANIM if body.is_mob() => poof(out, rng, body),
        POOF if body.living => poof(out, rng, body),
        FOX_EAT if kind == K::Fox => {
            let Some(particle) = body.item.and_then(item_particle) else {
                return;
            };
            let look = look_angle(body.x_rot, body.y_rot);
            let at = [
                body.pos[0] + look[0] / 2.0,
                body.pos[1],
                body.pos[2] + look[2] / 2.0,
            ];
            for _ in 0..FOX_PARTICLES {
                let d = [
                    (f64::from(rng.next_f32()) - 0.5) * 0.1,
                    f64::from(rng.next_f32()) * 0.1 + 0.1,
                    0.0,
                ];
                let d = y_rot(x_rot(d, -body.x_rot * DEG_TO_RAD), -body.y_rot * DEG_TO_RAD);
                add(out, particle.clone(), at, [d[0], d[1] + 0.05, d[2]]);
            }
        }
        TELEPORT if body.living => {
            let last = f64::from(TELEPORT_PARTICLES - 1);
            for i in 0..TELEPORT_PARTICLES {
                let d = f64::from(i) / last;
                let velocity = [0; 3].map(|_| f64::from((rng.next_f32() - 0.5) * TELEPORT_SPEED));
                let lerp = |axis: usize| body.old[axis] + d * (body.pos[axis] - body.old[axis]);
                let x = lerp(0) + (rng.next_f64() - 0.5) * body.width * 2.0;
                let y = lerp(1) + rng.next_f64() * body.height;
                let z = lerp(2) + (rng.next_f64() - 0.5) * body.width * 2.0;
                add(out, Particle::Portal, [x, y, z], velocity);
            }
        }
        MAINHAND_BREAK..=FEET_BREAK | BODY_BREAK | SADDLE_BREAK if body.living => {
            let Some(item) = body.item else { return };
            out.push(ParticleEmit::ItemUse(ItemUseEmit {
                item,
                eye: [body.pos[0], body.pos[1] + body.eye_height, body.pos[2]],
                x_rot: body.x_rot,
                y_rot: body.y_rot,
                count: BREAK_PARTICLES,
            }));
        }
        HONEY_SLIDE if !matches!(kind, K::Snowball | K::Egg | K::SpawnerMinecart) => {
            honey(out, body, HONEY_SLIDE_PARTICLES);
        }
        HONEY_JUMP if body.living => honey(out, body, HONEY_JUMP_PARTICLES),
        DROWN_PARTICLES if body.living => {
            for _ in 0..DROWN_BUBBLES {
                let offset = [0; 3].map(|_| rng.next_f64() - rng.next_f64());
                let at = [0, 1, 2].map(|i| body.pos[i] + offset[i]);
                add(out, Particle::Bubble, at, body.velocity);
            }
        }
        RAVAGER_ROARED if kind == K::Ravager => {
            let center = [body.pos[0], body.pos[1] + body.height / 2.0, body.pos[2]];
            for _ in 0..ROAR_PARTICLES {
                let velocity = [0; 3].map(|_| rng.next_gaussian() * ROAR_SPEED);
                add(out, Particle::Poof, center, velocity);
            }
        }
        _ => {}
    }
}

fn poof(out: &mut Vec<ParticleEmit>, rng: &mut JavaRandom, body: &Body) {
    for _ in 0..POOF_PARTICLES {
        let [xa, ya, za] = [0; 3].map(|_| rng.next_gaussian() * SPREAD);
        let [x, y, z] = body.random_point(rng);
        add(
            out,
            Particle::Poof,
            [x - xa * POOF_LEAD, y - ya * POOF_LEAD, z - za * POOF_LEAD],
            [xa, ya, za],
        );
    }
}

fn honey(out: &mut Vec<ParticleEmit>, body: &Body, count: u32) {
    let particle = Particle::Block(BlockParticle {
        block_state: BlockState::from(BlockKind::HoneyBlock),
    });
    for _ in 0..count {
        add(out, particle.clone(), body.pos, [0.0; 3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(kind: EntityKind, living: bool) -> Body {
        Body {
            kind,
            living,
            pos: [10.0, 64.0, -5.0],
            old: [10.0, 64.0, -5.0],
            width: 0.6,
            height: 1.8,
            eye_height: 1.62,
            x_rot: 0.0,
            y_rot: 0.0,
            velocity: [0.0; 3],
            item: None,
            ground: None,
        }
    }

    fn count(event: u8, body: &Body) -> usize {
        let mut out = Vec::new();
        spawn(event, body, &mut JavaRandom::new(42), &mut out);
        out.len()
    }

    fn levels(event: u8, body: &Body) -> Vec<LevelParticle> {
        let mut out = Vec::new();
        spawn(event, body, &mut JavaRandom::new(42), &mut out);
        out.into_iter()
            .filter_map(|emit| match emit {
                ParticleEmit::Level(level) => Some(level),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn love_hearts_follow_the_class() {
        assert_eq!(count(IN_LOVE_HEARTS, &body(EntityKind::Cow, true)), 7);
        assert_eq!(count(IN_LOVE_HEARTS, &body(EntityKind::Wolf, true)), 7);
        assert_eq!(count(IN_LOVE_HEARTS, &body(EntityKind::Allay, true)), 3);
        assert_eq!(count(IN_LOVE_HEARTS, &body(EntityKind::Villager, true)), 0);
        assert_eq!(count(IN_LOVE_HEARTS, &body(EntityKind::Dolphin, true)), 0);
    }

    #[test]
    fn taming_is_tamables_and_horses_only() {
        for kind in [
            EntityKind::Wolf,
            EntityKind::Nautilus,
            EntityKind::Horse,
            EntityKind::TraderLlama,
            EntityKind::CamelHusk,
        ] {
            assert_eq!(count(TAMING_SUCCEEDED, &body(kind, true)), 7, "{kind:?}");
            assert_eq!(count(TAMING_FAILED, &body(kind, true)), 7, "{kind:?}");
        }
        assert_eq!(count(TAMING_SUCCEEDED, &body(EntityKind::Cow, true)), 0);
        assert_eq!(count(TAMING_SUCCEEDED, &body(EntityKind::Ocelot, true)), 0);
        assert_eq!(
            count(TRUSTING_SUCCEEDED, &body(EntityKind::Ocelot, true)),
            7
        );
        assert_eq!(count(TRUSTING_FAILED, &body(EntityKind::Cat, true)), 0);
        let smoke = levels(TAMING_FAILED, &body(EntityKind::Wolf, true));
        assert!(smoke.iter().all(|p| matches!(p.particle, Particle::Smoke)));
    }

    #[test]
    fn villager_moods_skip_the_wandering_trader() {
        for event in [LOVE_HEARTS, VILLAGER_ANGRY, VILLAGER_HAPPY, VILLAGER_SWEAT] {
            assert_eq!(count(event, &body(EntityKind::Villager, true)), 5);
            assert_eq!(count(event, &body(EntityKind::WanderingTrader, true)), 0);
        }
    }

    #[test]
    fn dolphin_sits_lower_and_spreads_less() {
        let b = body(EntityKind::Dolphin, true);
        let out = levels(DOLPHIN_LOOKING_FOR_TREASURE, &b);
        assert_eq!(out.len(), 7);
        for p in &out {
            assert!(
                p.pos[1] >= b.pos[1] + DOLPHIN_LIFT
                    && p.pos[1] <= b.pos[1] + b.height + DOLPHIN_LIFT
            );
        }
    }

    #[test]
    fn witch_count_stays_in_its_bounds() {
        for seed in 0..64 {
            let mut out = Vec::new();
            spawn(
                WITCH_HAT_MAGIC,
                &body(EntityKind::Witch, true),
                &mut JavaRandom::new(seed),
                &mut out,
            );
            assert!((10..=44).contains(&out.len()), "{}", out.len());
        }
        assert_eq!(count(WITCH_HAT_MAGIC, &body(EntityKind::Evoker, true)), 0);
    }

    #[test]
    fn poofs_follow_mob_and_living() {
        assert_eq!(
            count(SILVERFISH_MERGE_ANIM, &body(EntityKind::Zombie, true)),
            20
        );
        assert_eq!(
            count(SILVERFISH_MERGE_ANIM, &body(EntityKind::Player, true)),
            0
        );
        assert_eq!(
            count(SILVERFISH_MERGE_ANIM, &body(EntityKind::ArmorStand, true)),
            0
        );
        assert_eq!(count(POOF, &body(EntityKind::ArmorStand, true)), 20);
        assert_eq!(count(POOF, &body(EntityKind::OakBoat, false)), 0);
        assert_eq!(count(RAVAGER_ROARED, &body(EntityKind::Ravager, true)), 40);
        assert_eq!(count(RAVAGER_ROARED, &body(EntityKind::Zombie, true)), 0);
    }

    #[test]
    fn teleport_trail_runs_from_old_to_new() {
        let mut b = body(EntityKind::Enderman, true);
        b.old = [0.0, 70.0, 0.0];
        b.pos = [12.0, 64.0, -3.0];
        b.width = 0.0;
        b.height = 0.0;
        let out = levels(TELEPORT, &b);
        assert_eq!(out.len(), 128);
        assert_eq!(out[0].pos, b.old);
        assert_eq!(out[127].pos, b.pos);
        assert!(out.iter().all(|p| p.dist.iter().all(|v| v.abs() <= 0.1)));
        assert_eq!(count(TELEPORT, &body(EntityKind::Snowball, false)), 0);
    }

    #[test]
    fn thrown_items_burst_on_hit() {
        assert_eq!(count(DEATH, &body(EntityKind::Snowball, false)), 8);
        let mut egg = body(EntityKind::Egg, false);
        egg.item = Some("blue_egg");
        let out = levels(DEATH, &egg);
        assert_eq!(out.len(), 8);
        assert!(out.iter().all(|p| p.dist.iter().all(|v| v.abs() <= 0.04)));
        assert_eq!(count(DEATH, &body(EntityKind::Zombie, true)), 0);
    }

    #[test]
    fn rabbit_dust_needs_a_visible_block() {
        let mut rabbit = body(EntityKind::Rabbit, true);
        assert_eq!(count(JUMP, &rabbit), 0);
        rabbit.ground = Some(BlockState::from(BlockKind::Air));
        assert_eq!(count(JUMP, &rabbit), 0);
        rabbit.ground = Some(BlockState::from(BlockKind::Water));
        assert_eq!(count(JUMP, &rabbit), 0);
        rabbit.ground = Some(BlockState::from(BlockKind::GrassBlock));
        assert_eq!(count(JUMP, &rabbit), 1);
        let mut cow = body(EntityKind::Cow, true);
        cow.ground = Some(BlockState::from(BlockKind::GrassBlock));
        assert_eq!(count(JUMP, &cow), 0);
    }

    #[test]
    fn breaks_and_fox_need_an_item() {
        let mut zombie = body(EntityKind::Zombie, true);
        assert_eq!(count(HEAD_BREAK, &zombie), 0);
        zombie.item = Some("iron_helmet");
        let mut out = Vec::new();
        spawn(HEAD_BREAK, &zombie, &mut JavaRandom::new(1), &mut out);
        assert!(
            matches!(out.as_slice(), [ParticleEmit::ItemUse(u)] if u.count == 5 && u.item == "iron_helmet")
        );
        let mut fox = body(EntityKind::Fox, true);
        assert_eq!(count(FOX_EAT, &fox), 0);
        fox.item = Some("sweet_berries");
        assert_eq!(count(FOX_EAT, &fox), 8);
    }

    #[test]
    fn honey_and_bubbles() {
        assert_eq!(count(HONEY_SLIDE, &body(EntityKind::Minecart, false)), 5);
        assert_eq!(
            count(HONEY_SLIDE, &body(EntityKind::SpawnerMinecart, false)),
            0
        );
        assert_eq!(count(HONEY_JUMP, &body(EntityKind::Pig, true)), 10);
        assert_eq!(count(HONEY_JUMP, &body(EntityKind::Minecart, false)), 0);
        let mut drowned = body(EntityKind::Player, true);
        drowned.velocity = [0.0, -0.1, 0.0];
        let out = levels(DROWN_PARTICLES, &drowned);
        assert_eq!(out.len(), 8);
        assert!(out.iter().all(|p| p.dist == [0.0, -0.1, 0.0]));
    }

    #[test]
    fn spawn_reacts_only_to_handled_ids() {
        for event in 0..=u8::MAX {
            if !handled(event) {
                assert_eq!(count(event, &body(EntityKind::Zombie, true)), 0);
            }
        }
    }
}
