use std::sync::Mutex;

use seedcrack::cracker::{DECORATOR_MARGIN, LIFT_BITS, LIFT_COUNT, PILLAR_BITS};
use seedcrack::{Cracker, Hint, Stage};

use super::HUD_W as W;
use super::run::{finder_on, to_seedcrack};
use crate::gui::Snapshot;
use crate::gui::painter::Painter;
use crate::modules::hud_panels::TextPanel;
use crate::modules::registry::seed_cracker as setting;
use crate::modules::store;
use crate::text::{AQUA, GRAY, GREEN, RED, WHITE, YELLOW};

const HINTS: usize = 5;
const LAST: seedcrack::FinderKind = seedcrack::FinderKind::Shipwreck;

#[derive(Default)]
struct View {
    stage: Option<Stage>,
    progress: f32,
    structure_bits: f64,
    liftable: usize,
    decorator_bits: f64,
    pillars: bool,
    hashed: bool,
    structure_seeds: Option<usize>,
    world_seeds: Option<usize>,
    world_seed: Option<i64>,
    off: bool,
    hints: Vec<Hint>,
}

static VIEW: Mutex<Option<View>> = Mutex::new(None);

fn with(f: impl FnOnce(&mut View)) {
    if let Ok(mut slot) = VIEW.lock() {
        f(slot.get_or_insert_with(View::default));
    }
}

pub fn publish(c: &Cracker) {
    with(|v| {
        v.stage = Some(c.stage());
        v.progress = c.progress();
        v.structure_bits = c.structure_bits();
        v.liftable = c.liftable_count();
        v.decorator_bits = c.decorator_bits();
        v.pillars = c.pillars().is_some();
        v.hashed = c.hashed_seed().is_some();
        v.structure_seeds = c.structure_seeds().map(<[i64]>::len);
        v.world_seeds = c.world_seeds().map(<[i64]>::len);
        v.world_seed = c.world_seed();
        v.hints = c.hints();
    });
}

pub fn off() {
    with(|v| v.off = true);
}

pub fn reset() {
    if let Ok(mut slot) = VIEW.lock() {
        *slot = None;
    }
}

fn status(v: &View) -> (String, u32, f32) {
    if v.off {
        return ("Off on this server".to_string(), RED, 0.0);
    }
    let stage = v.stage.unwrap_or(Stage::Idle);
    let pct = v.progress * 100.0;
    match stage {
        Stage::Done => return ("Seed found".to_string(), GREEN, 1.0),
        Stage::LiftLow | Stage::LiftHigh | Stage::Pillars => {
            return (format!("Searching {pct:.0}%"), YELLOW, v.progress);
        }
        Stage::WorldByHash | Stage::WorldByDecorators => {
            return (
                format!("Finding the exact seed {pct:.0}%"),
                YELLOW,
                v.progress,
            );
        }
        Stage::Idle => {}
    }
    if v.structure_seeds == Some(0) || v.world_seeds == Some(0) {
        return ("Finds disagree".to_string(), RED, 0.0);
    }
    if v.structure_seeds.is_some() && v.hashed && v.world_seeds.is_none() {
        return ("Waiting".to_string(), GRAY, 0.0);
    }
    let (have, want) = match v.structure_seeds {
        None => (
            v.structure_bits,
            if v.pillars { PILLAR_BITS } else { LIFT_BITS },
        ),
        Some(n) => (
            v.decorator_bits,
            16.0 + (n.max(1) as f64).log2() + DECORATOR_MARGIN,
        ),
    };
    let have = if have > 0.0 { have } else { 0.0 };
    let bits = (have / want).min(1.0) as f32;
    let lifting = v.structure_seeds.is_none() && !v.pillars;
    if !lifting || v.liftable >= LIFT_COUNT {
        return (format!("Collecting {have:.0}/{want:.0} bits"), WHITE, bits);
    }
    let key = v.liftable as f32 / LIFT_COUNT as f32;
    let missing = LIFT_COUNT - v.liftable;
    let text = if have >= want {
        format!(
            "Need {missing} more key find{}",
            if missing == 1 { "" } else { "s" }
        )
    } else {
        format!(
            "Collecting {have:.0}/{want:.0} bits, key {}/{LIFT_COUNT}",
            v.liftable
        )
    };
    (text, YELLOW, (bits + key) / 2.0)
}

pub fn draw(p: &mut Painter, _snap: &Snapshot) {
    if !store().flag(setting::HUD) {
        return;
    }
    let Ok(slot) = VIEW.lock() else { return };
    let mut panel = TextPanel::new(p, W);
    panel.text("Seed Cracker", AQUA);
    let Some(v) = slot.as_ref() else {
        panel.text("Starting", GRAY);
        return;
    };

    let (text, rgb, share) = status(v);
    panel.text(&text, rgb);
    panel.bar(share, if rgb == WHITE { AQUA } else { rgb });

    if let Some(seed) = v.world_seed {
        panel.text(&format!("Seed {seed}"), GREEN);
    }

    let here = to_seedcrack(crate::renderer::dimension::current());
    let ranked: Vec<&Hint> = v
        .hints
        .iter()
        .filter(|h| h.finder.dimension() == here && finder_on(h.finder))
        .collect();
    let mut shown: Vec<&Hint> = ranked.iter().take(HINTS).copied().collect();
    let extra = match ranked.iter().position(|h| h.finder == LAST) {
        Some(i) if i >= HINTS => Some(ranked[i]),
        _ => ranked.get(HINTS).copied(),
    };
    shown.extend(extra);
    if !shown.is_empty() {
        panel.text("Best finds:", WHITE);
        for h in shown {
            let rgb = if h.rank == 0 { YELLOW } else { GRAY };
            panel.text(&format!("{} ~{:.1} bits", h.finder.name(), h.bits), rgb);
        }
    }
}
