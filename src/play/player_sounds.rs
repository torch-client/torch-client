use std::str::FromStr;

use azalea::Client;
use azalea::block::properties::{ComparatorKind, Drag, Lit, Open, Powered, Waterlogged};
use azalea::block::{BlockState, BlockTrait};
use azalea::entity::inventory::{BundleSound, Inventory};
use azalea::entity::metadata::{FallFlying, Health, SheepSheared, Silent, Swimming};
use azalea::entity::{
    ActiveEffects, Crouching, EntityKindComponent, OnClimbable, Physics, PlayerAbilities, Position,
};
use azalea::mining::{MineDelay, MineTicks, Mining};
use azalea::physics::collision::BlockWithShape;
use azalea::physics::collision::sturdy::is_face_sturdy;
use azalea::world::World;
use azalea_core::aabb::Aabb;
use azalea_core::direction::Direction;
use azalea_core::game_type::GameMode;
use azalea_core::hit_result::BlockHitResult;
use azalea_core::position::{BlockPos, Vec3};
use azalea_inventory::components::{BundleContents, Instrument};
use azalea_registry::builtin::{Attribute, BlockKind, EntityKind, ItemKind, MobEffect};
use azalea_registry::tags::blocks as tags;

use crate::audio::random_f32;
use crate::blocks::{sound_types, state_is_air};
use crate::client::tracking::{self, EntityAttributes};
use crate::session::{InteractionHand, SharedMutex, SideMutex};
use crate::util::level_events::{Bus, play};

const MOVE_DIST_SCALE: f64 = 0.6000000238418579;
const SUPPORT_OFFSET: f32 = 1.0e-5;
const EFFECT_OFFSET: f32 = 0.2;
const MAX_STRIDE: f64 = 10.0;

struct State {
    last: Option<Vec3>,
    move_dist: f32,
    next_step: f32,
    fall: f64,
    velocity: Vec3,
    was_in_water: Option<bool>,
    in_bubbles: Option<bool>,
    mine_ticks: f32,
    mine_target: Option<(BlockPos, BlockState)>,
    bundle_left: usize,
    tick_count: i32,
    crystal_tick: i32,
    crystal_intensity: f32,
}

impl State {
    const NEW: State = State {
        last: None,
        move_dist: 0.0,
        next_step: 1.0,
        fall: 0.0,
        velocity: Vec3::ZERO,
        was_in_water: None,
        in_bubbles: None,
        mine_ticks: 0.0,
        mine_target: None,
        bundle_left: 0,
        tick_count: 0,
        crystal_tick: 0,
        crystal_intensity: 0.0,
    };
}

static STATE: SideMutex<State> = SideMutex::new(State::NEW);

pub(crate) fn forget_step_origin() {
    STATE.lock().unwrap().last = None;
}

pub(crate) fn reset(blocking: bool) {
    let state = if blocking {
        STATE.lock().ok()
    } else {
        STATE.try_lock().ok()
    };
    if let Some(mut state) = state {
        *state = State::NEW;
    }
}

struct Me {
    pos: Vec3,
    velocity: Vec3,
    bounds: Aabb,
    on_ground: bool,
    in_water: bool,
    falling: bool,
    flying: bool,
    may_fly: bool,
    crouching: bool,
    swimming: bool,
    spectator: bool,
    climbing: bool,
    fall_slowed: bool,
    fall_flying: bool,
    silent: bool,
    mining: Option<(BlockPos, f32)>,
    mine_delay: u32,
}

fn read_me(bot: &Client) -> Option<Me> {
    let ecs = bot.ecs.read();
    let id = bot.entity;
    let physics = ecs.get::<Physics>(id)?;
    let abilities = ecs.get::<PlayerAbilities>(id)?;
    let effects = ecs.get::<ActiveEffects>(id);
    let has = |effect| effects.is_some_and(|e| e.get_level(effect).is_some());
    Some(Me {
        pos: **ecs.get::<Position>(id)?,
        velocity: physics.velocity,
        bounds: physics.bounding_box,
        on_ground: physics.on_ground(),
        in_water: physics.is_in_water(),
        falling: physics.fall_distance > 0.0,
        flying: abilities.flying,
        may_fly: abilities.can_fly,
        crouching: ecs.get::<Crouching>(id).is_some_and(|c| **c),
        swimming: ecs.get::<Swimming>(id).is_some_and(|s| s.0),
        spectator: ecs
            .get::<GameMode>(id)
            .is_some_and(|gm| *gm == GameMode::Spectator),
        climbing: ecs.get::<OnClimbable>(id).is_some_and(|c| **c),
        fall_slowed: has(MobEffect::SlowFalling) || has(MobEffect::Levitation),
        fall_flying: ecs.get::<FallFlying>(id).is_some_and(|f| f.0),
        silent: ecs.get::<Silent>(id).is_some_and(|s| s.0),
        mining: ecs
            .get::<Mining>(id)
            .zip(ecs.get::<MineTicks>(id))
            .map(|(mining, ticks)| (mining.pos, ticks.0)),
        mine_delay: ecs.get::<MineDelay>(id).map_or(0, |delay| delay.0),
    })
}

impl Me {
    fn play(&self, event: &str, volume: f32, pitch: f32) {
        play(
            event,
            Bus::Players,
            [self.pos.x, self.pos.y, self.pos.z],
            volume,
            pitch,
        );
    }
}

fn play_block(event: &str, pos: BlockPos, volume: f32, pitch: f32) {
    let at = crate::util::block_centre([pos.x, pos.y, pos.z]).to_array();
    play(event, Bus::Blocks, at, volume, pitch);
}

fn state_at(world: &World, pos: BlockPos) -> BlockState {
    world.get_block_state(pos).unwrap_or_default()
}

fn wobble() -> f32 {
    1.0 + (random_f32() - random_f32()) * 0.4
}

fn water_speed(movement: Vec3, modifier: f32) -> f32 {
    let Vec3 { x, y, z } = movement;
    let squared = x * x * 0.20000000298023224 + y * y + z * z * 0.20000000298023224;
    (squared.sqrt() as f32 * modifier).min(1.0)
}

fn swim(me: &Me, movement: Vec3) {
    me.play("entity.player.swim", water_speed(movement, 0.35), wobble());
}

fn splash(me: &Me, movement: Vec3) {
    let speed = water_speed(movement, 0.2);
    let event = if speed < 0.25 {
        "entity.player.splash"
    } else {
        "entity.player.splash.high_speed"
    };
    me.play(event, speed, wobble());
}

fn is_climbable(state: BlockState) -> bool {
    let kind = state.as_block_kind();
    kind == BlockKind::PowderSnow || tags::CLIMBABLE.contains(&kind)
}

fn is_tall(state: BlockState) -> bool {
    let kind = state.as_block_kind();
    tags::FENCES.contains(&kind) || tags::WALLS.contains(&kind) || tags::FENCE_GATES.contains(&kind)
}

fn main_support(world: &World, me: &Me) -> Option<BlockPos> {
    let Aabb { min, max } = me.bounds;
    let top = (min.y - 1.0e-6).floor() as i32;
    let mut best: Option<(f64, BlockPos)> = None;
    for x in min.x.floor() as i32..=max.x.floor() as i32 {
        for z in min.z.floor() as i32..=max.z.floor() as i32 {
            for y in [top, top - 1] {
                let pos = BlockPos::new(x, y, z);
                let state = state_at(world, pos);
                let supports = if y == top {
                    !state.is_collision_shape_empty()
                } else {
                    is_tall(state)
                };
                let dx = f64::from(x) + 0.5 - me.pos.x;
                let dy = f64::from(y) + 0.5 - me.pos.y;
                let dz = f64::from(z) + 0.5 - me.pos.z;
                let distance = dx * dx + dy * dy + dz * dz;
                if supports && best.is_none_or(|(nearest, _)| distance < nearest) {
                    best = Some((distance, pos));
                }
            }
        }
    }
    best.map(|(_, pos)| pos)
}

fn on_pos(world: &World, me: &Me, support: Option<BlockPos>, offset: f32) -> BlockPos {
    let y = (me.pos.y - f64::from(offset)).floor() as i32;
    let Some(support) = support else {
        return BlockPos::new(me.pos.x.floor() as i32, y, me.pos.z.floor() as i32);
    };
    if offset <= SUPPORT_OFFSET || is_tall(state_at(world, support)) {
        support
    } else {
        BlockPos::new(support.x, y, support.z)
    }
}

struct Ground {
    effect: (BlockPos, BlockState),
    support: (BlockPos, BlockState),
}

impl Ground {
    fn read(world: &World, me: &Me) -> Ground {
        let main = if me.on_ground {
            main_support(world, me)
        } else {
            None
        };
        let cell = |offset| {
            let pos = on_pos(world, me, main, offset);
            (pos, state_at(world, pos))
        };
        Ground {
            effect: cell(EFFECT_OFFSET),
            support: cell(SUPPORT_OFFSET),
        }
    }
}

fn steps_on(me: &Me, stride: Vec3, state: BlockState) -> bool {
    !state_is_air(state)
        && (me.on_ground || is_climbable(state) || (me.crouching && stride.y == 0.0))
        && !me.swimming
}

fn step_sound(me: &Me, state: BlockState, volume: f32, pitch: f32) {
    let group = sound_types::of(state);
    me.play(group.step_event, group.volume * volume, group.pitch * pitch);
}

fn walking_step(
    s: &mut State,
    world: &World,
    me: &Me,
    stride: Vec3,
    (pos, state): (BlockPos, BlockState),
) {
    if me.in_water {
        swim(me, stride);
        step_sound(me, state, 0.05, 0.8);
    } else {
        let above = state_at(world, pos.up(1));
        let kind = above.as_block_kind();
        if tags::COMBINATION_STEP_SOUND_BLOCKS.contains(&kind) {
            step_sound(me, above, 0.15, 1.0);
            step_sound(me, state, 0.05, 0.8);
        } else if tags::INSIDE_STEP_SOUND_BLOCKS.contains(&kind) {
            step_sound(me, above, 0.15, 1.0);
        } else {
            step_sound(me, state, 0.15, 1.0);
        }
    }
    if tags::CRYSTAL_SOUND_BLOCKS.contains(&state.as_block_kind())
        && s.tick_count >= s.crystal_tick + 20
    {
        s.crystal_intensity *= 0.997_f64.powi(s.tick_count - s.crystal_tick) as f32;
        s.crystal_intensity = (s.crystal_intensity + 0.07).min(1.0);
        let pitch = 0.5 + s.crystal_intensity * random_f32() * 1.2;
        let volume = 0.1 + s.crystal_intensity * 1.2;
        me.play("block.amethyst_block.chime", volume, pitch);
        s.crystal_tick = s.tick_count;
    }
}

fn step(s: &mut State, world: &World, me: &Me, ground: &Ground, stride: Vec3) {
    let (support_pos, support) = ground.support;
    let (effect_pos, effect) = ground.effect;
    let moved = if is_climbable(support) {
        stride.length()
    } else {
        stride.horizontal_distance()
    };
    s.move_dist += (moved * MOVE_DIST_SCALE) as f32;
    if s.move_dist <= s.next_step || state_is_air(support) {
        return;
    }
    let mut stepped = steps_on(me, stride, effect);
    if stepped {
        walking_step(s, world, me, stride, ground.effect);
    }
    if support_pos != effect_pos {
        stepped |= steps_on(me, stride, support);
    }
    if stepped || me.in_water {
        s.next_step = (s.move_dist as i32 + 1) as f32;
    }
    if !stepped && me.in_water {
        swim(me, stride);
    }
}

fn fall_damage(distance: f64, modifier: f64) -> i32 {
    let attributes = tracking::attributes().lock().unwrap();
    let own = EntityAttributes::of(&attributes, tracking::local_entity_id());
    let power = distance + 1.0e-6 - own.value_or_default(Attribute::SafeFallDistance);
    (power * modifier * own.value_or_default(Attribute::FallDamageMultiplier)).floor() as i32
}

fn fall_voice(me: &Me, big: bool) {
    let event = if big {
        "entity.player.big_fall"
    } else {
        "entity.player.small_fall"
    };
    me.play(event, 1.0, 1.0);
}

fn land(world: &World, me: &Me, ground: &Ground, distance: f64) {
    let state = ground.effect.1;
    let kind = state.as_block_kind();
    let (distance, modifier) = match kind {
        BlockKind::SlimeBlock => return,
        BlockKind::PowderSnow => {
            if distance >= 4.0 {
                fall_voice(me, distance >= 7.0);
            }
            return;
        }
        BlockKind::HayBlock => (distance, 0.2),
        BlockKind::HoneyBlock => {
            me.play("block.honey_block.slide", 1.0, 1.0);
            (distance, 0.2)
        }
        BlockKind::PointedDripstone => {
            let block = Box::<dyn BlockTrait>::from(state);
            let tip = block.get_property("vertical_direction") == Some("up")
                && block.get_property("thickness") == Some("tip");
            if tip {
                (distance + 2.5, 2.0)
            } else {
                (distance, 1.0)
            }
        }
        _ if tags::BEDS.contains(&kind) => (distance * 0.5, 1.0),
        _ => (distance, 1.0),
    };
    if me.may_fly {
        return;
    }
    let damage = fall_damage(distance, modifier);
    if damage <= 0 {
        return;
    }
    fall_voice(me, damage > 4);
    if !me.silent {
        let below = BlockPos::new(
            me.pos.x.floor() as i32,
            (me.pos.y - 0.20000000298023224).floor() as i32,
            me.pos.z.floor() as i32,
        );
        let state = state_at(world, below);
        if !state_is_air(state) {
            let group = sound_types::of(state);
            me.play(group.fall_event, group.volume * 0.5, group.pitch * 0.75);
        }
    }
    if kind == BlockKind::HoneyBlock {
        let group = sound_types::of(ground.effect.1);
        me.play(group.fall_event, group.volume * 0.5, group.pitch * 0.75);
    }
}

fn bubble_column(world: &World, me: &Me) -> Option<bool> {
    const DEFLATE: f64 = 1.0e-6;
    const INSET: f64 = 0.4000000059604645;
    let Aabb { min, max } = me.bounds;
    let low_y = (min.y + INSET + DEFLATE).floor() as i32;
    let high_y = (max.y - INSET - DEFLATE).floor() as i32;
    for x in (min.x + DEFLATE).floor() as i32..=(max.x - DEFLATE).floor() as i32 {
        for z in (min.z + DEFLATE).floor() as i32..=(max.z - DEFLATE).floor() as i32 {
            for y in low_y..=high_y {
                let state = state_at(world, BlockPos::new(x, y, z));
                if state.as_block_kind() == BlockKind::BubbleColumn {
                    return state.property::<Drag>();
                }
            }
        }
    }
    None
}

fn honey_slide(world: &World, me: &Me) -> bool {
    const DEFLATE: f64 = 1.0e-5;
    const EPSILON: f64 = 1.0e-7;
    let Aabb { min, max } = me.bounds;
    let allowed = 0.4375 + (max.x - min.x) / 2.0;
    for x in (min.x + DEFLATE).floor() as i32..=(max.x - DEFLATE).floor() as i32 {
        for z in (min.z + DEFLATE).floor() as i32..=(max.z - DEFLATE).floor() as i32 {
            for y in (min.y + DEFLATE).floor() as i32..=(max.y - DEFLATE).floor() as i32 {
                let side = (f64::from(x) + 0.5 - me.pos.x)
                    .abs()
                    .max((f64::from(z) + 0.5 - me.pos.z).abs());
                if state_at(world, BlockPos::new(x, y, z)).as_block_kind() == BlockKind::HoneyBlock
                    && me.pos.y <= f64::from(y) + 0.9375 - EPSILON
                    && side + EPSILON > allowed
                {
                    return true;
                }
            }
        }
    }
    false
}

fn mine_hit(pos: BlockPos, target: BlockState) {
    if !state_is_air(target) {
        let group = sound_types::of(target);
        play_block(
            group.hit_event,
            pos,
            (group.volume + 1.0) / 8.0,
            group.pitch * 0.5,
        );
    }
}

pub(crate) fn tick(bot: &Client) {
    let Some(me) = read_me(bot) else { return };
    let Ok(world) = bot.world() else { return };
    let world = world.read();
    let mut state = STATE.lock().unwrap();
    let s = &mut *state;
    s.tick_count += 1;

    let stride = s
        .last
        .map(|last| me.pos - last)
        .filter(|stride| stride.length() <= MAX_STRIDE);
    s.last = Some(me.pos);
    let walking = !me.spectator && tracking::local_vehicle_id().is_none();

    if walking && me.in_water && s.was_in_water == Some(false) {
        splash(&me, s.velocity);
    }
    s.was_in_water = Some(me.in_water);
    if walking
        && !me.on_ground
        && s.velocity.y < -0.08
        && random_f32() < 0.2
        && honey_slide(&world, &me)
    {
        me.play("block.honey_block.slide", 1.0, 1.0);
    }
    s.velocity = me.velocity;

    let column = if me.in_water {
        bubble_column(&world, &me)
    } else {
        None
    };
    if let Some(drag_down) = column
        && s.in_bubbles == Some(false)
        && !me.spectator
    {
        let event = if drag_down {
            "block.bubble_column.whirlpool_inside"
        } else {
            "block.bubble_column.upwards_inside"
        };
        me.play(event, 1.0, 1.0);
    }
    s.in_bubbles = Some(column.is_some());

    if walking {
        let fallen = stride
            .filter(|_| !me.in_water)
            .map_or(0.0, |stride| f64::from(-stride.y as f32).max(0.0));
        let landed = me.on_ground && s.fall + fallen > 0.0;
        let emits = !me.flying && !(me.on_ground && me.crouching);
        let moved = stride.filter(|stride| emits && stride.length_squared() > 0.0);
        if landed || moved.is_some() {
            let ground = Ground::read(&world, &me);
            if landed {
                land(&world, &me, &ground, s.fall + fallen);
            }
            if let Some(moved) = moved {
                step(s, &world, &me, &ground, moved);
            }
        }
        s.fall = if me.on_ground || me.climbing || me.flying || me.fall_slowed || !me.falling {
            0.0
        } else {
            s.fall + fallen
        };
        if me.fall_flying && me.velocity.y > -0.5 {
            s.fall = s.fall.min(1.0);
        }
    } else {
        s.fall = 0.0;
    }

    let ticks = me.mining.map_or(0.0, |(_, ticks)| ticks);
    if let Some((pos, _)) = me.mining
        && ticks != s.mine_ticks
        && (ticks - 1.0) % 4.0 == 0.0
    {
        mine_hit(pos, state_at(&world, pos));
    }
    if me.mining.is_none()
        && me.mine_delay > 0
        && s.mine_ticks > 0.0
        && s.mine_ticks % 4.0 == 0.0
        && let Some((pos, target)) = s.mine_target
        && state_at(&world, pos) != target
    {
        mine_hit(pos, target);
    }
    s.mine_ticks = ticks;
    s.mine_target = me.mining.map(|(pos, _)| (pos, state_at(&world, pos)));
}

fn voice_pitch() -> f32 {
    (random_f32() - random_f32()) * 0.2 + 1.0
}

pub(crate) fn hurt(damage_type: Option<&str>, at: [f64; 3]) {
    let data = damage_type.and_then(|id| crate::util::datapack::entry("damage_type", id));
    let effects = data
        .as_ref()
        .and_then(|data| data.get("effects"))
        .and_then(|effects| effects.as_str());
    let event = match effects {
        Some("drowning") => "entity.player.hurt_drown",
        Some("burning") => "entity.player.hurt_on_fire",
        Some("poking") => "entity.player.hurt_sweet_berry_bush",
        Some("freezing") => "entity.player.hurt_freeze",
        _ => "entity.player.hurt",
    };
    play(event, Bus::Players, at, 1.0, voice_pitch());
}

pub(crate) fn death(at: [f64; 3]) {
    play("entity.player.death", Bus::Players, at, 1.0, voice_pitch());
}

pub(crate) fn portal_trigger() {
    crate::util::level_events::play_local(
        "block.portal.trigger",
        Bus::Ambient,
        0.25,
        random_f32() * 0.4 + 0.8,
    );
}

pub(crate) fn block_used(state: BlockState, pos: BlockPos, may_build: bool) {
    let kind = state.as_block_kind();
    if kind == BlockKind::Comparator {
        if may_build {
            let subtract = state.property::<ComparatorKind>() == Some(ComparatorKind::Compare);
            play_block(
                "block.comparator.click",
                pos,
                0.3,
                if subtract { 0.55 } else { 0.5 },
            );
        }
        return;
    }
    let Some(sounds) = sound_types::block_set(state) else {
        return;
    };
    if tags::BUTTONS.contains(&kind) {
        if state.property::<Powered>() == Some(false) {
            play_block(sounds.on, pos, 1.0, 1.0);
        }
        return;
    }
    let toggles =
        tags::FENCE_GATES.contains(&kind) || crate::play::placement::toggled_open(state).is_some();
    if toggles && let Some(open) = state.property::<Open>() {
        let event = if open { sounds.off } else { sounds.on };
        play_block(event, pos, 1.0, random_f32() * 0.1 + 0.9);
    }
}

fn strippable(kind: BlockKind) -> bool {
    use BlockKind as B;
    matches!(
        kind,
        B::OakWood
            | B::OakLog
            | B::DarkOakWood
            | B::DarkOakLog
            | B::PaleOakWood
            | B::PaleOakLog
            | B::AcaciaWood
            | B::AcaciaLog
            | B::CherryWood
            | B::CherryLog
            | B::BirchWood
            | B::BirchLog
            | B::JungleWood
            | B::JungleLog
            | B::SpruceWood
            | B::SpruceLog
            | B::WarpedStem
            | B::WarpedHyphae
            | B::CrimsonStem
            | B::CrimsonHyphae
            | B::MangroveWood
            | B::MangroveLog
            | B::BambooBlock
    )
}

fn axe_sound(state: BlockState) -> Option<&'static str> {
    if strippable(state.as_block_kind()) {
        return Some("item.axe.strip");
    }
    let name = crate::blocks::block_name(state);
    if ["exposed_", "weathered_", "oxidized_"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        Some("item.axe.scrape")
    } else if name.starts_with("waxed_") {
        Some("item.axe.wax_off")
    } else {
        None
    }
}

fn hoe_sound(kind: BlockKind, face: Direction, above: BlockState) -> Option<&'static str> {
    use BlockKind as B;
    let tilled = match kind {
        B::RootedDirt => true,
        B::GrassBlock | B::DirtPath | B::Dirt | B::CoarseDirt => {
            face != Direction::Down && state_is_air(above)
        }
        _ => false,
    };
    tilled.then_some("item.hoe.till")
}

fn shovel_sound(kind: BlockKind, face: Direction, above: BlockState) -> Option<&'static str> {
    use BlockKind as B;
    let flattens = matches!(
        kind,
        B::GrassBlock | B::Dirt | B::Podzol | B::CoarseDirt | B::Mycelium | B::RootedDirt
    );
    (flattens && face != Direction::Down && state_is_air(above)).then_some("item.shovel.flatten")
}

fn shears_sound(state: BlockState) -> Option<&'static str> {
    use BlockKind as B;
    let head = matches!(
        state.as_block_kind(),
        B::Kelp | B::WeepingVines | B::TwistingVines | B::CaveVines
    );
    let grown = Box::<dyn BlockTrait>::from(state).get_property("age") == Some("25");
    (head && !grown).then_some("block.growing_plant.crop")
}

fn flint_target(world: &World, state: BlockState, hit: &BlockHitResult) -> Option<BlockPos> {
    if lightable(state) {
        return Some(hit.block_pos);
    }
    let fire = hit.block_pos + hit.direction.normal();
    let below = state_at(world, fire.down(1));
    let survives = is_face_sturdy(below, Direction::Up)
        || tags::SOUL_FIRE_BASE_BLOCKS.contains(&below.as_block_kind())
        || Direction::VERTICAL
            .iter()
            .chain(&Direction::HORIZONTAL)
            .any(|face| can_burn(state_at(world, fire + face.normal())));
    (state_is_air(state_at(world, fire)) && survives).then_some(fire)
}

#[rustfmt::skip]
fn can_burn(state: BlockState) -> bool {
    use BlockKind as B;
    state.property::<Waterlogged>() != Some(true) && matches!(
        state.as_block_kind(),
        B::OakPlanks | B::SprucePlanks | B::BirchPlanks | B::JunglePlanks | B::AcaciaPlanks
        | B::CherryPlanks | B::DarkOakPlanks | B::PaleOakPlanks | B::MangrovePlanks
        | B::BambooPlanks | B::BambooMosaic | B::OakSlab | B::SpruceSlab | B::BirchSlab
        | B::JungleSlab | B::AcaciaSlab | B::CherrySlab | B::DarkOakSlab | B::PaleOakSlab
        | B::MangroveSlab | B::BambooSlab | B::BambooMosaicSlab | B::OakFenceGate
        | B::SpruceFenceGate | B::BirchFenceGate | B::JungleFenceGate | B::AcaciaFenceGate
        | B::CherryFenceGate | B::DarkOakFenceGate | B::PaleOakFenceGate | B::MangroveFenceGate
        | B::BambooFenceGate | B::OakFence | B::SpruceFence | B::BirchFence | B::JungleFence
        | B::AcaciaFence | B::CherryFence | B::DarkOakFence | B::PaleOakFence | B::MangroveFence
        | B::BambooFence | B::OakStairs | B::BirchStairs | B::SpruceStairs | B::JungleStairs
        | B::AcaciaStairs | B::CherryStairs | B::DarkOakStairs | B::PaleOakStairs
        | B::MangroveStairs | B::BambooStairs | B::BambooMosaicStairs | B::OakLog | B::SpruceLog
        | B::BirchLog | B::JungleLog | B::AcaciaLog | B::CherryLog | B::PaleOakLog | B::DarkOakLog
        | B::MangroveLog | B::BambooBlock | B::StrippedOakLog | B::StrippedSpruceLog
        | B::StrippedBirchLog | B::StrippedJungleLog | B::StrippedAcaciaLog | B::StrippedCherryLog
        | B::StrippedDarkOakLog | B::StrippedPaleOakLog | B::StrippedMangroveLog
        | B::StrippedBambooBlock | B::StrippedOakWood | B::StrippedSpruceWood
        | B::StrippedBirchWood | B::StrippedJungleWood | B::StrippedAcaciaWood
        | B::StrippedCherryWood | B::StrippedDarkOakWood | B::StrippedPaleOakWood
        | B::StrippedMangroveWood | B::OakWood | B::SpruceWood | B::BirchWood | B::JungleWood
        | B::AcaciaWood | B::CherryWood | B::PaleOakWood | B::DarkOakWood | B::MangroveWood
        | B::MangroveRoots | B::OakLeaves | B::SpruceLeaves | B::BirchLeaves | B::JungleLeaves
        | B::AcaciaLeaves | B::CherryLeaves | B::DarkOakLeaves | B::PaleOakLeaves
        | B::MangroveLeaves | B::Bookshelf | B::Tnt | B::ShortGrass | B::Fern | B::DeadBush
        | B::ShortDryGrass | B::TallDryGrass | B::Sunflower | B::Lilac | B::RoseBush | B::Peony
        | B::TallGrass | B::LargeFern | B::Dandelion | B::GoldenDandelion | B::Poppy
        | B::OpenEyeblossom | B::ClosedEyeblossom | B::BlueOrchid | B::Allium | B::AzureBluet
        | B::RedTulip | B::OrangeTulip | B::WhiteTulip | B::PinkTulip | B::OxeyeDaisy
        | B::Cornflower | B::LilyOfTheValley | B::Torchflower | B::PitcherPlant | B::WitherRose
        | B::PinkPetals | B::Wildflowers | B::LeafLitter | B::CactusFlower | B::WhiteWool
        | B::OrangeWool | B::MagentaWool | B::LightBlueWool | B::YellowWool | B::LimeWool
        | B::PinkWool | B::GrayWool | B::LightGrayWool | B::CyanWool | B::PurpleWool | B::BlueWool
        | B::BrownWool | B::GreenWool | B::RedWool | B::BlackWool | B::Vine | B::CoalBlock
        | B::HayBlock | B::Target | B::WhiteCarpet | B::OrangeCarpet | B::MagentaCarpet
        | B::LightBlueCarpet | B::YellowCarpet | B::LimeCarpet | B::PinkCarpet | B::GrayCarpet
        | B::LightGrayCarpet | B::CyanCarpet | B::PurpleCarpet | B::BlueCarpet | B::BrownCarpet
        | B::GreenCarpet | B::RedCarpet | B::BlackCarpet | B::PaleMossBlock | B::PaleMossCarpet
        | B::PaleHangingMoss | B::DriedKelpBlock | B::Bamboo | B::Scaffolding | B::Lectern
        | B::Composter | B::SweetBerryBush | B::Beehive | B::BeeNest | B::AzaleaLeaves
        | B::FloweringAzaleaLeaves | B::CaveVines | B::CaveVinesPlant | B::SporeBlossom | B::Azalea
        | B::FloweringAzalea | B::BigDripleaf | B::BigDripleafStem | B::SmallDripleaf
        | B::HangingRoots | B::GlowLichen | B::FireflyBush | B::Bush | B::AcaciaShelf
        | B::BambooShelf | B::BirchShelf | B::CherryShelf | B::DarkOakShelf | B::JungleShelf
        | B::MangroveShelf | B::OakShelf | B::PaleOakShelf | B::SpruceShelf
    )
}

fn lightable(state: BlockState) -> bool {
    let kind = state.as_block_kind();
    state.property::<Lit>() == Some(false)
        && state.property::<Waterlogged>() != Some(true)
        && (tags::CAMPFIRES.contains(&kind)
            || tags::CANDLES.contains(&kind)
            || tags::CANDLE_CAKES.contains(&kind))
}

fn clicked_state(bot: &Client, hit: &BlockHitResult) -> Option<BlockState> {
    bot.world()
        .ok()
        .and_then(|world| world.read().get_block_state(hit.block_pos))
}

pub(crate) fn item_runs_past_block(bot: &Client, item: ItemKind, hit: &BlockHitResult) -> bool {
    item == ItemKind::FlintAndSteel && clicked_state(bot, hit).is_some_and(lightable)
}

fn chest_neighbour(state: BlockState, pos: BlockPos) -> Option<BlockPos> {
    let block = Box::<dyn BlockTrait>::from(state);
    let (x, z) = match (block.get_property("facing")?, block.get_property("type")?) {
        ("north", "left") | ("south", "right") => (1, 0),
        ("north", "right") | ("south", "left") => (-1, 0),
        ("east", "left") | ("west", "right") => (0, 1),
        ("east", "right") | ("west", "left") => (0, -1),
        _ => return None,
    };
    Some(BlockPos::new(pos.x + x, pos.y, pos.z + z))
}

fn wax(bot: &Client, shared: &SharedMutex, hit: &BlockHitResult) {
    let Some(state) = clicked_state(bot, hit) else {
        return;
    };
    if BlockKind::from_str(&format!("waxed_{}", crate::blocks::block_name(state))).is_err() {
        return;
    }
    copper_particles(shared, 3003, state, hit.block_pos);
}

fn copper_particles(shared: &SharedMutex, id: u32, state: BlockState, pos: BlockPos) {
    use crate::session::{LevelEventEmit, ParticleEmit, push_particle_emits};

    let cells = [Some(pos), chest_neighbour(state, pos)];
    let emits = cells.into_iter().flatten().map(|pos| {
        ParticleEmit::Event(LevelEventEmit {
            id,
            pos: [pos.x, pos.y, pos.z],
            data: 0,
            global: false,
        })
    });
    push_particle_emits(&mut shared.lock().unwrap().session.particle_emits, emits);
}

pub(crate) fn item_used_on(
    bot: &Client,
    shared: &SharedMutex,
    item: ItemKind,
    hit: &BlockHitResult,
    axe_declines: bool,
) {
    let may_build = matches!(
        bot.component::<GameMode>().map(|gm| *gm),
        Ok(GameMode::Survival | GameMode::Creative)
    );
    if !may_build || bot.is_mining() {
        return;
    }
    if item == ItemKind::Honeycomb {
        wax(bot, shared, hit);
        return;
    }
    let Ok(world) = bot.world() else { return };
    let world = world.read();
    let Some(state) = world.get_block_state(hit.block_pos) else {
        return;
    };
    if item == ItemKind::FlintAndSteel {
        if let Some(pos) = flint_target(&world, state, hit) {
            play_block("item.flintandsteel.use", pos, 1.0, random_f32() * 0.4 + 0.8);
        }
        return;
    }
    let id = item.to_str().trim_start_matches("minecraft:");
    let kind = state.as_block_kind();
    let above = state_at(&world, hit.block_pos.up(1));
    let event = if item == ItemKind::Shears {
        shears_sound(state)
    } else if id.ends_with("_axe") && !axe_declines {
        axe_sound(state)
    } else if id.ends_with("_hoe") {
        hoe_sound(kind, hit.direction, above)
    } else if id.ends_with("_shovel") {
        shovel_sound(kind, hit.direction, above)
    } else {
        None
    };
    if let Some(event) = event {
        play_block(event, hit.block_pos, 1.0, 1.0);
        let particle = match event {
            "item.axe.scrape" => Some(3005),
            "item.axe.wax_off" => Some(3004),
            _ => None,
        };
        drop(world);
        if let Some(id) = particle {
            copper_particles(shared, id, state, hit.block_pos);
        }
    }
}

pub(crate) fn brush_stroke(state: BlockState, pos: BlockPos) {
    let event = match state.as_block_kind() {
        BlockKind::SuspiciousSand => "item.brush.brushing.sand",
        BlockKind::SuspiciousGravel => "item.brush.brushing.gravel",
        _ => "item.brush.brushing.generic",
    };
    play_block(event, pos, 1.0, 1.0);
}

fn play_at_feet(bot: &Client, event: &str, bus: Bus, volume: f32, pitch: f32) {
    if let Ok(pos) = bot.position() {
        play(event, bus, [pos.x, pos.y, pos.z], volume, pitch);
    }
}

pub(crate) fn spyglass(bot: &Client, started: bool) {
    let event = if started {
        "item.spyglass.use"
    } else {
        "item.spyglass.stop_using"
    };
    play_at_feet(bot, event, Bus::Players, 1.0, 1.0);
}

pub(crate) fn bottle_filled(bot: &Client) {
    play_at_feet(bot, "item.bottle.fill", Bus::Neutral, 1.0, 1.0);
}

pub(crate) fn bottle_on_hive(bot: &Client, item: ItemKind, hit: &BlockHitResult) {
    let full = clicked_state(bot, hit).is_some_and(|state| {
        Box::<dyn BlockTrait>::from(state).get_property("honey_level") == Some("5")
    });
    if item == ItemKind::GlassBottle && full {
        play_at_feet(bot, "item.bottle.fill", Bus::Blocks, 1.0, 1.0);
    }
}

pub(crate) fn goat_horn(bot: &Client, hand: InteractionHand) {
    use azalea_core::data_registry::DataRegistryWithKey;
    use azalea_registry::{DataRegistryKey, Holder};

    let held = bot.component::<Inventory>().ok().and_then(|inventory| {
        Some(
            inventory
                .get_equipment(hand.equipment())?
                .get_component::<Instrument>()?
                .into_owned(),
        )
    });
    let Some(Instrument { value }) = held else {
        return;
    };
    let (event, range) = match value {
        Holder::Direct(data) => (
            crate::client::server_sounds::event_name(&data.sound_event).into_owned(),
            data.range,
        ),
        Holder::Reference(instrument) => {
            let Ok(world) = bot.world() else { return };
            let key = instrument.key_owned(&world.read().registries);
            let Some(key) = key else { return };
            let path = DataRegistryKey::into_ident(key);
            let Some(data) = crate::util::datapack::entry("instrument", path.path()) else {
                return;
            };
            let event = data
                .get("sound_event")
                .and_then(crate::util::datapack::bare_id);
            let range = data.get("range").and_then(|range| range.as_f64());
            let (Some(event), Some(range)) = (event, range) else {
                return;
            };
            (event.to_owned(), range as f32)
        }
    };
    if let Ok(pos) = bot.position() {
        let bus = crate::audio::SoundCategory::Records;
        crate::audio::play_at(&event, bus, [pos.x, pos.y, pos.z], range / 16.0, 1.0);
    }
}

pub(crate) fn dye_sheep(bot: &Client, entity: azalea::ecs::entity::Entity, item: ItemKind) {
    use azalea_core::entity_id::MinecraftEntityId;

    let id = item.to_str().trim_start_matches("minecraft:");
    let Some(dye) = id.strip_suffix("_dye") else {
        return;
    };
    let Some(colour) = crate::util::dye::DYES.iter().position(|d| d.name == dye) else {
        return;
    };
    let target = {
        let ecs = bot.ecs.read();
        let sheep = ecs
            .get::<EntityKindComponent>(entity)
            .is_some_and(|kind| **kind == EntityKind::Sheep)
            && ecs
                .get::<Health>(entity)
                .is_some_and(|health| health.0 > 0.0)
            && !ecs
                .get::<SheepSheared>(entity)
                .is_some_and(|sheared| sheared.0);
        ecs.get::<Position>(entity)
            .zip(ecs.get::<MinecraftEntityId>(entity))
            .filter(|_| sheep)
            .map(|(pos, id)| (**pos, id.0))
    };
    let Some((pos, id)) = target else { return };
    let wool = tracking::meta_index_18().lock().unwrap().get(&id).copied();
    let wool = wool.map_or(0, crate::entities::feed::sheep_wool_color);
    if usize::from(wool) != colour {
        play(
            "item.dye.use",
            Bus::Players,
            [pos.x, pos.y, pos.z],
            1.0,
            1.0,
        );
    }
}

fn bundle_removed(bot: &Client) {
    play_at_feet(
        bot,
        "item.bundle.remove_one",
        Bus::Players,
        0.8,
        0.8 + random_f32() * 0.4,
    );
}

pub(crate) fn bundle_click(bot: &Client, sound: BundleSound) {
    match sound {
        BundleSound::Insert => {
            play_at_feet(
                bot,
                "item.bundle.insert",
                Bus::Players,
                0.8,
                0.8 + random_f32() * 0.4,
            );
        }
        BundleSound::InsertFail => {
            play_at_feet(bot, "item.bundle.insert_fail", Bus::Players, 1.0, 1.0);
        }
        BundleSound::RemoveOne => bundle_removed(bot),
    }
}

pub(crate) fn bundle_drop(bot: &Client, hand: InteractionHand, first: bool) {
    let held = first.then(|| {
        bot.component::<Inventory>().ok().and_then(|inventory| {
            let stack = inventory.get_equipment(hand.equipment())?;
            Some(stack.get_component::<BundleContents>()?.items.len())
        })
    });
    let mut state = STATE.lock().unwrap();
    if let Some(held) = held {
        state.bundle_left = held.unwrap_or(0);
    }
    if state.bundle_left > 0 {
        state.bundle_left -= 1;
        drop(state);
        bundle_removed(bot);
    }
}
