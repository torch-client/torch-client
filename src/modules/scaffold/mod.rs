mod aim;
mod legit;
mod target;
mod tower;

use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

use azalea::Client;
use azalea::entity::Physics;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::inventory::Inventory;
use azalea::movement::LastSentInput;
use azalea_core::aabb::Aabb;
use azalea_core::position::BlockPos;
use azalea_inventory::ItemStack;
use azalea_registry::builtin::BlockKind;

use self::aim::{Aimer, WorldCells, turn_toward};
use self::target::{Cells, Me};
use super::registry::{Id, options, scaffold as setting};
use super::sent::{self, Look, WALK_KEYS};
use super::{Edge, Phase, roll, store, turn};
use crate::session::{
    MOVE_BACK, MOVE_FORWARD, MOVE_JUMP, MOVE_LEFT, MOVE_RIGHT, MOVE_SNEAK, SharedMutex,
};
use crate::util::mth::wrap_degrees;

options! {
    pub enum ScaffoldMode {
        Legit = "Legit",
        GodBridge = "GodBridge",
        TellyBridge = "TellyBridge",
    }
}

options! {
    pub enum Tower {
        Off = "Off",
        Jump = "Jump",
        Motion = "Motion",
        Step = "Step",
        Smooth = "Smooth",
    }
}

options! {
    pub enum BlockFilter {
        All = "All",
        Blacklist = "Blacklist",
        Whitelist = "Whitelist",
    }
}

const WARM_TICKS: u32 = 6;

const STEER_TICKS: u32 = 16;

static EDGE: Edge = Edge::new(Id::Scaffold);

static RUN: Mutex<Run> = Mutex::new(Run::IDLE);

static HOMING: AtomicBool = AtomicBool::new(false);

static COUNT: AtomicU32 = AtomicU32::new(0);

static HAND_PLACES: Mutex<Vec<BlockPos>> = Mutex::new(Vec::new());

struct Run {
    steering: bool,
    aim: Aimer,
    plan: Option<Plan>,
    place_timer: u32,
    start_y: f64,
    locked_y: f64,
    ground_y: f64,
    tower: tower::State,
    restore: Option<(u8, u8)>,
    switch_cooldown: u32,
    sneaking: bool,
    fall_ticks: u32,
    still_ticks: u32,
    last_xz: (f64, f64),
    steer_ticks: u32,
    steer_dir: Option<(f64, f64)>,
    eagle: legit::Eagle,
    activation: u32,
    activated: bool,
    y_threshold: f64,
    now: u64,
}

impl Run {
    const IDLE: Run = Run {
        steering: false,
        aim: Aimer::IDLE,
        plan: None,
        place_timer: 0,
        start_y: 0.0,
        locked_y: 0.0,
        ground_y: 0.0,
        tower: tower::State::IDLE,
        restore: None,
        switch_cooldown: 0,
        sneaking: false,
        fall_ticks: 0,
        still_ticks: 0,
        last_xz: (0.0, 0.0),
        steer_ticks: 0,
        steer_dir: None,
        eagle: legit::Eagle::IDLE,
        activation: 0,
        activated: false,
        y_threshold: 3.0,
        now: 0,
    };

    fn reset_activation(&mut self) {
        self.activated = false;
        self.activation = 0;
        HAND_PLACES.lock().unwrap().clear();
    }
}

struct Plan {
    me: Me,
    target_y: i32,
    reach: f64,
}

enum LookAct {
    Turn(Look),
    Hold,
    Idle,
}

#[derive(Clone, Copy, PartialEq)]
enum Sneak {
    Leave,
    Press,
    Own(bool),
}

struct Out {
    look: LookAct,
    walk: Option<u8>,
    sneak: Sneak,
    jump: Option<bool>,
}

impl Out {
    fn idle() -> Out {
        Out {
            look: LookAct::Idle,
            walk: None,
            sneak: Sneak::Leave,
            jump: None,
        }
    }
}

pub fn enabled() -> bool {
    store().enabled(Id::Scaffold)
}

pub fn count() -> Option<u32> {
    match COUNT.load(Relaxed) {
        0 => None,
        n => Some(n),
    }
}

pub(crate) fn note_hand_place(pos: BlockPos) {
    if !enabled() {
        return;
    }
    let mut places = HAND_PLACES.lock().unwrap();
    if places.len() < 16 {
        places.push(pos);
    }
}

pub fn steer(
    bot: &Client,
    shared: &Arc<SharedMutex>,
    yaw: f32,
    pitch: f32,
    flags: u8,
) -> (Option<Look>, u8) {
    let s = store();
    let camera = Look { yaw, pitch };
    match EDGE.poll(s) {
        Phase::Off => {
            if !HOMING.load(Relaxed) {
                return (None, flags);
            }
            let mut run = RUN.lock().unwrap();
            let out = release(bot, &mut run, camera, flags);
            HOMING.store(run.steering, Relaxed);
            return out;
        }
        Phase::Stopped => {
            let mut run = RUN.lock().unwrap();
            if let Some((from, to)) = run.restore
                && bot.selected_hotbar_slot().is_ok_and(|s| s == to)
            {
                crate::play::interaction::select_hotbar_slot(bot, from);
            }
            let steering = run.steering;
            *run = Run::IDLE;
            run.steering = steering;
            COUNT.store(0, Relaxed);
            HAND_PLACES.lock().unwrap().clear();
            let out = release(bot, &mut run, camera, flags);
            HOMING.store(run.steering, Relaxed);
            return out;
        }
        Phase::Started => {
            let mut run = RUN.lock().unwrap();
            let steering = run.steering;
            *run = Run::IDLE;
            run.steering = steering;
            if let Ok(p) = bot.position() {
                run.start_y = p.y;
                run.locked_y = p.y;
            }
            HAND_PLACES.lock().unwrap().clear();
        }
        Phase::Running => {}
    }

    let mut run = RUN.lock().unwrap();
    run.now += 1;
    run.plan = None;
    let Some(snap) = snapshot(bot, camera, flags) else {
        let out = release(bot, &mut run, camera, flags);
        HOMING.store(run.steering, Relaxed);
        return out;
    };
    let out = decide(bot, shared, &mut run, &snap);
    let result = apply(bot, &mut run, camera, flags, &snap, out);
    HOMING.store(run.steering, Relaxed);
    result
}

pub fn tick(bot: &Client, shared: &Arc<SharedMutex>) {
    if !enabled() {
        return;
    }
    let s = store();
    let held = bot.component::<Inventory>().ok().map(|inv| {
        let menu = &inv.inventory_menu;
        let held = inv.held_item().clone();
        let total: i32 = menu
            .player_slots_without_hotbar_range()
            .chain(menu.hotbar_slots_range())
            .filter_map(|i| menu.slot(i))
            .filter(|st| st.is_present() && st.kind() == held.kind())
            .map(|st| st.count())
            .sum();
        (held, total)
    });
    let (held, total) = held.unwrap_or((ItemStack::Empty, 0));
    let item = block_of(&held);
    COUNT.store(
        if s.flag(setting::BLOCK_COUNT) && item.is_some() {
            total.max(0) as u32
        } else {
            0
        },
        Relaxed,
    );

    let mut run = RUN.lock().unwrap();
    let Some(plan) = run.plan.take() else {
        return;
    };
    let Some(item) = item.filter(|&k| usable(k)) else {
        return;
    };
    let Some((eye, look)) = sent::sent_view(bot) else {
        return;
    };
    let secondary_use = bot.component::<LastSentInput>().is_ok_and(|i| i.0.shift);
    let hit = {
        let Ok(world) = bot.world() else {
            return;
        };
        let world = world.read();
        let c = WorldCells {
            world: &world,
            item,
        };
        aim::click(&c, &plan.me, eye, look, plan.target_y, plan.reach)
    };
    let Some(hit) = hit else {
        return;
    };
    if crate::play::interaction::module_place(
        bot,
        shared,
        &hit,
        (look.pitch, look.yaw),
        secondary_use,
    ) {
        run.place_timer = 0;
        run.still_ticks = 0;
        run.last_xz = (plan.me.pos[0], plan.me.pos[2]);
        run.steer_ticks = 0;
        run.steer_dir = None;
        run.fall_ticks = 0;
    }
}

struct Snap {
    me: Me,
    bb: Aabb,
    sent: Option<Look>,
    camera_pitch: f32,
    hand_sneak: bool,
}

fn snapshot(bot: &Client, camera: Look, flags: u8) -> Option<Snap> {
    let pos = bot.position().ok()?;
    let (vel, on_ground, bb) = {
        let p = bot.component::<Physics>().ok()?;
        (
            [p.velocity.x, p.velocity.y, p.velocity.z],
            p.on_ground(),
            p.bounding_box,
        )
    };
    let eye_height = bot.component::<EntityDimensions>().ok()?.eye_height as f64;
    let key = |bit: u8| if flags & bit != 0 { 1.0 } else { 0.0 };
    Some(Snap {
        me: Me {
            pos: [pos.x, pos.y, pos.z],
            vel,
            on_ground,
            eye_height,
            forward: key(MOVE_FORWARD) - key(MOVE_BACK),
            strafe: key(MOVE_LEFT) - key(MOVE_RIGHT),
            yaw: camera.yaw,
            jump: flags & MOVE_JUMP != 0,
        },
        bb,
        sent: sent::sent_view(bot).map(|(_, l)| l),
        camera_pitch: camera.pitch,
        hand_sneak: flags & MOVE_SNEAK != 0,
    })
}

fn block_of(stack: &ItemStack) -> Option<BlockKind> {
    let ItemStack::Present(item) = stack else {
        return None;
    };
    BlockKind::from_str(item.kind.to_str())
        .ok()
        .filter(|&k| k != BlockKind::Air)
}

fn usable(kind: BlockKind) -> bool {
    if !target::is_scaffold_block(kind) {
        return false;
    }
    let s = store();
    let listed = || {
        let id = azalea::block::BlockState::from(kind).id() as u32;
        crate::modules::esp::blocks::SCAFFOLD.key_enabled(id)
    };
    match s.choice(setting::BLOCKS) {
        BlockFilter::All => true,
        BlockFilter::Blacklist => !listed(),
        BlockFilter::Whitelist => listed(),
    }
}

fn auto_block(bot: &Client, run: &mut Run) {
    let found = {
        let Ok(inv) = bot.component::<Inventory>() else {
            return;
        };
        if block_of(inv.held_item()).is_some_and(usable) {
            run.switch_cooldown = 0;
            return;
        }
        if run.switch_cooldown > 0 {
            run.switch_cooldown -= 1;
            return;
        }
        let menu = &inv.inventory_menu;
        let mut best: Option<(u8, i32)> = None;
        for (i, slot) in menu.hotbar_slots_range().enumerate() {
            let Some(stack) = menu.slot(slot) else {
                continue;
            };
            if !block_of(stack).is_some_and(usable) {
                continue;
            }
            if best.is_none_or(|(_, n)| stack.count() > n) {
                best = Some((i as u8, stack.count()));
            }
        }
        (best.map(|(i, _)| i), inv.selected_hotbar_slot)
    };
    match found {
        (None, _) => run.switch_cooldown = 10,
        (Some(best), current) if best == current => {}
        (Some(best), current) => {
            crate::play::interaction::select_hotbar_slot(bot, best);
            run.restore = Some((run.restore.map_or(current, |(from, _)| from), best));
            run.switch_cooldown = 3;
        }
    }
}

fn decide(bot: &Client, shared: &Arc<SharedMutex>, run: &mut Run, snap: &Snap) -> Out {
    let s = store();
    let me = &snap.me;

    let (screen, use_held) = {
        let st = shared.lock().unwrap();
        (st.screen_open || !st.in_world, st.session.use_held)
    };
    if screen {
        run.aim.idle();
        run.sneaking = false;
        run.eagle.reset();
        return Out::idle();
    }

    if s.flag(setting::AUTO_BLOCK) {
        auto_block(bot, run);
    }

    let tower_mode = s.choice(setting::TOWER);
    let tower = run.tower.step(tower_mode, me, &mut run.locked_y);
    if let Some(vy) = tower.vy {
        let mut ecs = bot.ecs.write();
        if let Some(mut p) = ecs.get_mut::<Physics>(bot.entity) {
            p.velocity.y = vy;
        }
    }
    let with_tower = |mut out: Out| {
        if tower.jump.is_some() {
            out.jump = tower.jump;
        }
        out
    };

    let item = bot
        .component::<Inventory>()
        .ok()
        .and_then(|inv| block_of(inv.held_item()))
        .filter(|&k| usable(k));
    let Some(item) = item else {
        run.aim.idle();
        run.sneaking = false;
        run.eagle.reset();
        run.reset_activation();
        return with_tower(Out::idle());
    };

    if s.flag(setting::PITCH_CHECK) && snap.camera_pitch < s.num(setting::PITCH_ANGLE) {
        run.aim.idle();
        run.sneaking = false;
        run.eagle.reset();
        return with_tower(Out::idle());
    }

    run.place_timer += 1;
    if run.place_timer <= s.num(setting::DELAY) as u32 {
        return with_tower(Out {
            look: LookAct::Hold,
            ..Out::idle()
        });
    }

    let Ok(world) = bot.world() else {
        return with_tower(Out::idle());
    };
    let world = world.read();
    let c = WorldCells {
        world: &world,
        item,
    };
    let ctx = Ctx {
        c: &c,
        snap,
        from: if run.steering {
            snap.sent.unwrap_or(snap.camera())
        } else {
            snap.camera()
        },
        turn: s.range(setting::TURN),
        reach: s.num(setting::REACH) as f64,
        expand: s.num(setting::EXPAND) as f64,
    };

    let towering = tower_mode != Tower::Off
        && me.jump
        && (me.on_ground || me.vel[0].abs() + me.vel[2].abs() < 0.2);
    if towering {
        let y = run.locked_y;
        let out = match target::place_position(&c, me, false, y, ctx.expand)
            .filter(|&t| c.replaceable(t))
        {
            Some(t) => ctx.prepare(run, t, false, y),
            None => {
                run.aim.idle();
                LookAct::Idle
            }
        };
        run.sneaking = false;
        return with_tower(Out {
            look: out,
            ..Out::idle()
        });
    }

    let out = match s.choice(setting::MODE) {
        ScaffoldMode::Legit => {
            run.reset_activation();
            legit_eagle(&ctx, run)
        }
        ScaffoldMode::GodBridge => {
            update_activation(run, me);
            god_bridge(&ctx, run)
        }
        ScaffoldMode::TellyBridge => {
            update_activation(run, me);
            if run.activated && !telly_gates(me, use_held) {
                run.reset_activation();
            }
            telly_bridge(&ctx, run, use_held)
        }
    };
    with_tower(out)
}

impl Snap {
    fn camera(&self) -> Look {
        Look {
            yaw: self.me.yaw,
            pitch: self.camera_pitch,
        }
    }
}

struct Ctx<'a> {
    c: &'a WorldCells<'a>,
    snap: &'a Snap,
    from: Look,
    turn: (f32, f32),
    reach: f64,
    expand: f64,
}

impl Ctx<'_> {
    fn prepare(&self, run: &mut Run, target: BlockPos, airborne: bool, y: f64) -> LookAct {
        let me = &self.snap.me;
        let (target, look) = run.aim.prepare(
            self.c,
            me,
            target,
            airborne,
            y,
            self.from,
            run.steering,
            self.turn,
            self.reach,
        );
        run.aim.warm = 0;
        run.plan = Some(Plan {
            me: *me,
            target_y: target.y,
            reach: self.reach,
        });
        LookAct::Turn(look)
    }

    fn falls(&self, shrink: f64) -> bool {
        legit::falls(self.c.world, &self.snap.bb, self.snap.me.vel, shrink)
    }
}

fn legit_eagle(ctx: &Ctx, run: &mut Run) -> Out {
    let s = store();
    let me = &ctx.snap.me;
    run.aim.idle();
    let require = s.flag(setting::REQUIRE_SNEAK);
    if !me.on_ground || (require && !ctx.snap.hand_sneak) {
        run.eagle.reset();
        return Out::idle();
    }
    let falling = ctx.falls(0.2);
    let sneak = run
        .eagle
        .step(run.now, falling, s.range(setting::SNEAK_DELAY));
    Out {
        sneak: if require {
            Sneak::Own(sneak)
        } else if sneak {
            Sneak::Press
        } else {
            Sneak::Leave
        },
        ..Out::idle()
    }
}

fn steerable(c: &impl Cells, me: &Me, t: BlockPos) -> bool {
    let (bx, bz) = me.column();
    (t.x - bx).abs() <= 1 && (t.z - bz).abs() <= 1 && target::has_h_support(c, t)
}

fn anchor_for(c: &impl Cells, me: &Me, t: BlockPos) -> (f64, f64) {
    let (mut x, mut z) = (t.x as f64 + 0.5, t.z as f64 + 0.5);
    let mut best: Option<(f64, azalea_core::direction::Direction)> = None;
    for &d in &target::HORIZONTAL {
        let n = target::step(t, d);
        if c.solid(n) {
            let (nx, nz) = (n.x as f64 + 0.5 - me.pos[0], n.z as f64 + 0.5 - me.pos[2]);
            let dist = nx * nx + nz * nz;
            if best.is_none_or(|(b, _)| dist > b) {
                best = Some((dist, d));
            }
        }
    }
    use azalea_core::direction::Direction;
    match best.map(|(_, d)| d) {
        Some(Direction::North) => z = t.z as f64 + 0.15,
        Some(Direction::South) => z = t.z as f64 + 0.85,
        Some(Direction::East) => x = t.x as f64 + 0.85,
        Some(Direction::West) => x = t.x as f64 + 0.15,
        _ => {}
    }
    (x, z)
}

fn glide_overshoots(me: &Me, a: (f64, f64)) -> bool {
    let (dx, dz) = (a.0 - me.pos[0], a.1 - me.pos[2]);
    let len = dx.hypot(dz);
    if len < 1e-4 {
        return true;
    }
    (me.pos[0] + me.vel[0] * 2.2 - a.0) * (dx / len)
        + (me.pos[2] + me.vel[2] * 2.2 - a.1) * (dz / len)
        > 0.05
}

fn god_bridge(ctx: &Ctx, run: &mut Run) -> Out {
    let me = &ctx.snap.me;
    let c = ctx.c;
    if !run.activated {
        run.sneaking = false;
        run.aim.idle();
        return Out::idle();
    }
    if me.on_ground {
        run.ground_y = me.pos[1];
    }
    let airborne = !me.on_ground;
    let placeable = target::place_position(c, me, airborne, run.ground_y, ctx.expand)
        .filter(|&t| c.replaceable(t));

    if !(me.on_ground && me.moving() && placeable.is_some()) {
        run.still_ticks = 0;
        run.last_xz = (me.pos[0], me.pos[2]);
        if placeable.is_none() {
            run.steer_ticks = 0;
            run.steer_dir = None;
        }
    } else {
        let (dx, dz) = (me.pos[0] - run.last_xz.0, me.pos[2] - run.last_xz.1);
        if dx * dx + dz * dz <= 0.2 {
            run.still_ticks += 1;
        } else {
            run.still_ticks = 0;
            run.last_xz = (me.pos[0], me.pos[2]);
        }
    }
    let steer_target = placeable.filter(|&t| me.on_ground && steerable(c, me, t));
    if steer_target.is_some() && !(run.fall_ticks < 6 && run.still_ticks < 10) {
        run.steer_ticks = STEER_TICKS;
        run.steer_dir = None;
    }

    let mut walk = None;
    let steering = run.steer_ticks > 0 && steer_target.is_some();
    if let Some(t) = steer_target.filter(|_| steering) {
        run.steer_ticks -= 1;
        let a = anchor_for(c, me, t);
        let (dx, dz) = (a.0 - me.pos[0], a.1 - me.pos[2]);
        let d2 = dx * dx + dz * dz;
        let dir = *run.steer_dir.get_or_insert_with(|| {
            let len = d2.sqrt().max(1e-9);
            (dx / len, dz / len)
        });
        let past = (me.pos[0] - a.0) * dir.0 + (me.pos[2] - a.1) * dir.1 > 0.13;
        if d2 >= 0.01 && !past {
            walk = if !run.sneaking && glide_overshoots(me, a) {
                Some(0)
            } else {
                sent::keys_toward(dx, dz, me.yaw)
            };
        } else {
            run.steer_ticks = 0;
            run.steer_dir = None;
            walk = Some(0);
        }
    }

    let about_to_fall = me.on_ground && ctx.falls(0.2);
    let sneak = me.on_ground && ctx.falls(0.0);
    run.sneaking = sneak;
    if about_to_fall {
        if !steering {
            walk = Some(0);
        }
        run.fall_ticks += 1;
    } else {
        run.fall_ticks = 0;
    }

    let ground_y = run.ground_y;
    let look = match placeable {
        Some(t) => ctx.prepare(run, t, airborne, ground_y),
        None => LookAct::Hold,
    };
    Out {
        look,
        walk,
        sneak: if sneak { Sneak::Press } else { Sneak::Leave },
        jump: None,
    }
}

fn telly_gates(me: &Me, use_held: bool) -> bool {
    let idle = !me.moving() && me.vel[0].abs() <= 0.05 && me.vel[2].abs() <= 0.05;
    !idle && !(store().flag(setting::REQUIRE_RIGHT_CLICK) && !use_held)
}

fn telly_bridge(ctx: &Ctx, run: &mut Run, use_held: bool) -> Out {
    let s = store();
    let me = &ctx.snap.me;
    let c = ctx.c;
    run.sneaking = false;
    if !run.activated || !telly_gates(me, use_held) {
        run.aim.idle();
        return Out::idle();
    }
    let start_y = run.start_y;
    let cell =
        || target::place_position(c, me, true, start_y, ctx.expand).filter(|&t| c.replaceable(t));
    if !me.on_ground {
        if me.pos[1] > start_y + run.y_threshold.max(1.5) {
            run.aim.idle();
            return Out::idle();
        }
        let look = match cell() {
            Some(t) => ctx.prepare(run, t, true, start_y),
            None => LookAct::Hold,
        };
        return Out {
            look,
            ..Out::idle()
        };
    }

    run.y_threshold = roll_y_threshold(s.num(setting::Y_INCREASE) as f64);
    match cell() {
        Some(t) => Out {
            look: ctx.prepare(run, t, true, start_y),
            jump: Some(true),
            ..Out::idle()
        },
        None => Out {
            look: LookAct::Hold,
            ..Out::idle()
        },
    }
}

fn roll_y_threshold(y: f64) -> f64 {
    if y <= 0.0 {
        return 0.0;
    }
    let r = roll(0.0, 1.0);
    if r < 0.15 {
        y + 1.0
    } else if r < 0.25 {
        (y - 1.0).max(1.0)
    } else {
        y
    }
}

fn update_activation(run: &mut Run, me: &Me) {
    let places = std::mem::take(&mut *HAND_PLACES.lock().unwrap());
    if !me.on_ground || run.activated {
        return;
    }
    let (bx, bz) = me.column();
    let layer = me.pos[1].floor() as i32 - 1;
    run.activation += places
        .iter()
        .filter(|p| p.y == layer && (p.x - bx).abs() <= 2 && (p.z - bz).abs() <= 2)
        .count() as u32;
    if run.activation >= store().num(setting::ACTIVATION) as u32 {
        run.activated = true;
        run.activation = 0;
        run.start_y = me.pos[1];
    }
}

fn apply(
    bot: &Client,
    run: &mut Run,
    camera: Look,
    flags: u8,
    snap: &Snap,
    out: Out,
) -> (Option<Look>, u8) {
    let mut f = flags;
    if let Some(w) = out.walk {
        f = f & !WALK_KEYS | w;
    }
    match out.sneak {
        Sneak::Leave => {}
        Sneak::Press => f |= MOVE_SNEAK,
        Sneak::Own(true) => f |= MOVE_SNEAK,
        Sneak::Own(false) => f &= !MOVE_SNEAK,
    }
    match out.jump {
        Some(true) => f |= MOVE_JUMP,
        Some(false) => f &= !MOVE_JUMP,
        None => {}
    }

    let look = match out.look {
        LookAct::Turn(l) => {
            run.steering = true;
            Some(l)
        }
        LookAct::Hold => {
            run.aim.warm += 1;
            if run.steering && run.aim.warm <= WARM_TICKS {
                snap.sent
            } else {
                run.aim.idle();
                return release(bot, run, camera, f);
            }
        }
        LookAct::Idle => {
            run.aim.idle();
            return release(bot, run, camera, f);
        }
    };
    match look {
        Some(l) => (Some(l), sent::move_fix(f, camera.yaw, l.yaw)),
        None => (None, f),
    }
}

fn release(bot: &Client, run: &mut Run, camera: Look, flags: u8) -> (Option<Look>, u8) {
    if !run.steering {
        return (None, flags);
    }
    let Some((_, from)) = sent::sent_view(bot) else {
        run.steering = false;
        return (None, flags);
    };
    if wrap_degrees(camera.yaw - from.yaw).abs() < turn::DEADZONE
        && (camera.pitch - from.pitch).abs() < turn::DEADZONE
    {
        run.steering = false;
        return (None, flags);
    }
    let (lo, hi) = store().range(setting::TURN);
    let next = turn_toward(from, camera, roll(lo, hi));
    (Some(next), sent::move_fix(flags, camera.yaw, next.yaw))
}
