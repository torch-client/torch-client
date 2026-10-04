#![cfg_attr(not(feature = "budget"), allow(dead_code))]

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Duration;

use crate::platform::time::{Instant, epoch};

fn now_us() -> u64 {
    Instant::now().duration_since(epoch()).as_micros() as u64
}

static OURS: AtomicU64 = AtomicU64::new(0);

counted! {
    pub enum Slot / SLOT_COUNT {
        Light,
        Mesh,
        Gui,
        Occlusion,
        Poll,
        Hand,
        Post,
    }
}

static SLOTS: [AtomicU64; SLOT_COUNT] = [const { AtomicU64::new(0) }; SLOT_COUNT];

static VISIBLE_COLUMNS: AtomicU64 = AtomicU64::new(0);
static VISIBLE_VERTS: AtomicU64 = AtomicU64::new(0);
static DRAWN_ENTITIES: AtomicU64 = AtomicU64::new(0);

static VIEWS: AtomicU64 = AtomicU64::new(0);
static DRAWS: AtomicU64 = AtomicU64::new(0);
static BOT: AtomicU64 = AtomicU64::new(0);

counted! {
    pub enum Phase / PHASE_COUNT {
        Upload,
        Acquire,
        Prepare,
        Draw,
    }
}

static PHASES: [AtomicU64; PHASE_COUNT] = [const { AtomicU64::new(0) }; PHASE_COUNT];
static FRAMES: AtomicU64 = AtomicU64::new(0);
static RENDERED: AtomicU64 = AtomicU64::new(0);
static TICKS: AtomicU64 = AtomicU64::new(0);

static OURS_AT: AtomicU64 = AtomicU64::new(0);
static BOT_AT: AtomicU64 = AtomicU64::new(0);
static RENDER_AT: AtomicU64 = AtomicU64::new(0);
static POST_AT: AtomicU64 = AtomicU64::new(0);
static WINDOW_AT: AtomicU64 = AtomicU64::new(0);

const WINDOW: Duration = Duration::from_millis(500);

pub fn frame_start() {
    OURS_AT.store(now_us(), Relaxed);
}

pub fn frame_end(rendered: bool) {
    close(&OURS_AT, &OURS);
    FRAMES.fetch_add(1, Relaxed);
    if rendered {
        RENDERED.fetch_add(1, Relaxed);
    }
}

pub fn render_begin() {
    RENDER_AT.store(now_us(), Relaxed);
}

pub fn render_mark(phase: Phase) {
    let now = now_us();
    close_at(now, &RENDER_AT, &PHASES[phase as usize]);
    RENDER_AT.store(now, Relaxed);
}

pub fn bot_start() {
    BOT_AT.store(now_us(), Relaxed);
}

pub fn bot_end() {
    close(&BOT_AT, &BOT);
}

pub fn note_tick() {
    TICKS.fetch_add(1, Relaxed);
}

pub fn add(slot: Slot, elapsed: Duration) {
    SLOTS[slot as usize].fetch_add(elapsed.as_micros() as u64, Relaxed);
}

pub fn timed(slot: Slot) -> Timer {
    Timer {
        slot,
        started: Instant::now(),
    }
}

pub struct Timer {
    slot: Slot,
    started: Instant,
}

impl Drop for Timer {
    fn drop(&mut self) {
        add(self.slot, self.started.elapsed());
    }
}

pub fn post_start() {
    POST_AT.store(now_us(), Relaxed);
}

pub fn post_end() {
    close(&POST_AT, &SLOTS[Slot::Post as usize]);
}

pub fn note_visible_columns(n: usize) {
    VISIBLE_COLUMNS.store(n as u64, Relaxed);
}

pub fn note_visible_verts(n: usize) {
    VISIBLE_VERTS.store(n as u64, Relaxed);
}

pub fn note_drawn_entities(n: usize) {
    DRAWN_ENTITIES.store(n as u64, Relaxed);
}

pub fn note_render_work(views: usize, draws: usize) {
    VIEWS.store(views as u64, Relaxed);
    DRAWS.store(draws as u64, Relaxed);
}

fn close(open: &AtomicU64, total: &AtomicU64) {
    close_at(now_us(), open, total);
}

fn close_at(now: u64, open: &AtomicU64, total: &AtomicU64) {
    let started = open.swap(0, Relaxed);
    if started == 0 {
        return;
    }
    total.fetch_add(now.saturating_sub(started), Relaxed);
}

#[derive(Clone, Copy, Default, PartialEq)]
pub struct Budget {
    pub frame_ms: f32,
    pub ours_ms: f32,
    pub gfx_ms: f32,
    pub phases: [f32; PHASE_COUNT],
    pub slots: [f32; SLOT_COUNT],
    pub rest_ms: f32,
    pub columns: u64,
    pub verts: u64,
    pub entities: u64,
    pub views: u64,
    pub draws: u64,
    pub bot_ms: f32,
    pub outside_ms: f32,
    pub tps: f32,
}

impl Budget {
    fn slot(&self, slot: Slot) -> f32 {
        self.slots[slot as usize]
    }

    fn phase(&self, phase: Phase) -> f32 {
        self.phases[phase as usize]
    }

    pub fn lines(&self) -> [String; 6] {
        [
            format!(
                "{:.1}ms us {:.1} gfx {:.1} bot {:.1} out {:.1}",
                self.frame_ms, self.ours_ms, self.gfx_ms, self.bot_ms, self.outside_ms
            ),
            format!(
                "upl {:.1} acq {:.1} pre {:.1} dr {:.1}",
                self.phase(Phase::Upload),
                self.phase(Phase::Acquire),
                self.phase(Phase::Prepare),
                self.phase(Phase::Draw),
            ),
            format!(
                "gui {:.1} occ {:.1} poll {:.1}",
                self.slot(Slot::Gui),
                self.slot(Slot::Occlusion),
                self.slot(Slot::Poll),
            ),
            format!(
                "hand {:.1} post {:.1} rest {:.1}",
                self.slot(Slot::Hand),
                self.slot(Slot::Post),
                self.rest_ms,
            ),
            format!(
                "lt {:.1} ms {:.1} c{} v{:.2}M e{} {:.0}tps",
                self.slot(Slot::Light),
                self.slot(Slot::Mesh),
                self.columns,
                self.verts as f32 / 1.0e6,
                self.entities,
                self.tps,
            ),
            format!("views {} draws {}", self.views, self.draws),
        ]
    }

    pub fn line(&self) -> String {
        self.lines().join(" | ")
    }
}

pub fn sample() -> Option<Budget> {
    let now = now_us();
    let opened = WINDOW_AT.load(Relaxed);
    if opened == 0 {
        for total in [&OURS, &BOT, &FRAMES, &RENDERED, &TICKS] {
            total.store(0, Relaxed);
        }
        for total in SLOTS.iter().chain(PHASES.iter()) {
            total.store(0, Relaxed);
        }
        WINDOW_AT.store(now, Relaxed);
        return None;
    }
    let span_us = now - opened;
    if span_us < WINDOW.as_micros() as u64 {
        return None;
    }
    WINDOW_AT.store(now, Relaxed);

    let frames = FRAMES.swap(0, Relaxed).max(1) as f32;
    let per_frame = |total: u64| ms(total) / frames;
    let ours = per_frame(OURS.swap(0, Relaxed));
    let drawn = RENDERED.swap(0, Relaxed).max(1) as f32;
    let mut phases = [0.0f32; PHASE_COUNT];
    for (out, total) in phases.iter_mut().zip(PHASES.iter()) {
        *out = ms(total.swap(0, Relaxed)) / drawn;
    }
    let gfx: f32 = phases.iter().sum();
    let bot = per_frame(BOT.swap(0, Relaxed));
    let frame = per_frame(span_us);
    let ticks = TICKS.swap(0, Relaxed) as f32;
    let mut slots = [0.0f32; SLOT_COUNT];
    for (out, total) in slots.iter_mut().zip(SLOTS.iter()) {
        *out = per_frame(total.swap(0, Relaxed));
    }
    Some(Budget {
        frame_ms: frame,
        ours_ms: ours,
        gfx_ms: gfx,
        phases,
        slots,
        rest_ms: (ours - slots.iter().sum::<f32>()).max(0.0),
        bot_ms: bot,
        outside_ms: frame - ours - gfx - bot,
        tps: ticks / (span_us as f32 / 1_000_000.0),
        columns: VISIBLE_COLUMNS.load(Relaxed),
        verts: VISIBLE_VERTS.load(Relaxed),
        entities: DRAWN_ENTITIES.load(Relaxed),
        views: VIEWS.load(Relaxed),
        draws: DRAWS.load(Relaxed),
    })
}

fn ms(us: u64) -> f32 {
    us as f32 / 1000.0
}
