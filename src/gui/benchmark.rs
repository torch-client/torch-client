use std::time::Duration;

use crate::gui::options::LAYOUT;
use crate::gui::painter::Painter;
use crate::gui::widgets::{
    self, Button, FOOTER_GAP, FOOTER_W, WIDGET_HEIGHT, WIDGET_WIDTH_BIG, centered_row_x,
};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::platform::time::Instant;
use crate::text::LINE_HEIGHT;

pub const BUDGET_MS: f32 = 35.0;

pub const GPU_TRIANGLES_PER_POINT: f32 = 10_000.0;

#[derive(Default)]
pub struct Benchmark {
    pub phase: Phase,
    pub progress: f32,
    pub result: Option<BenchResult>,
    pub copied_at: Option<Instant>,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    #[default]
    Idle,
    Cpu,
    Gpu,
    Done,
}

#[derive(Clone, Debug)]
pub struct BenchResult {
    pub cpu: CpuStats,
    pub gpu: GpuResult,
    pub adapter: String,
    pub cores: Option<u32>,
    pub seconds: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct CpuStats {
    pub ms_per_section: f32,
    pub best_ms: f32,
    pub worst_ms: f32,
    #[allow(dead_code)]
    pub sections: u32,
    #[allow(dead_code)]
    pub passes: u32,
    pub quads: u64,
}

#[derive(Clone, Copy, Debug)]
pub enum GpuResult {
    Measured {
        load: f32,
        triangles_per_frame: u64,
        at_cap: bool,
        baseline_ms: f32,
        #[allow(dead_code)]
        steps: u32,
    },
    Unmeasurable {
        baseline_ms: f32,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Scores {
    pub cpu: u64,
    pub gpu: Option<u64>,
    pub overall: Option<u64>,
}

impl Scores {
    pub fn new(r: &BenchResult) -> Self {
        let cpu = 1000.0 / r.cpu.ms_per_section;
        let gpu = match r.gpu {
            GpuResult::Measured {
                triangles_per_frame,
                ..
            } => Some(triangles_per_frame as f32 / GPU_TRIANGLES_PER_POINT),
            GpuResult::Unmeasurable { .. } => None,
        };
        Self {
            cpu: cpu.round() as u64,
            gpu: gpu.map(|g| g.round() as u64),
            overall: gpu.map(|g| (cpu * g).sqrt().round() as u64),
        }
    }
}

impl Benchmark {
    pub fn start(&mut self) {
        self.phase = Phase::Cpu;
        self.progress = 0.0;
        self.result = None;
        self.copied_at = None;
    }

    pub fn running(&self) -> bool {
        matches!(self.phase, Phase::Cpu | Phase::Gpu)
    }
}

const TITLE: &str = "Benchmark";
const RUNNING: &str = "Running benchmark";
const MEASURING_CPU: &str = "Measuring CPU";
const MEASURING_GPU: &str = "Measuring GPU";
const CANCEL: &str = "Cancel";
const COPY: &str = "Copy";
const COPIED: &str = "Copied";
const DONE: &str = "Done";

const COPIED_FOR: Duration = Duration::from_secs(2);

const BAR_W: f32 = 220.0;
const BAR_H: f32 = 10.0;
const BAR_GAP: f32 = 8.0;
const BAR_TRACK: u32 = 0xFF0A_0A0C;
const BAR_FILL: u32 = 0xFF4C_C46A;
const BAR_EDGE: u32 = 0xFF6E_6E7A;
const STRIP: u32 = 0xA000_0000;
const STRIP_PAD: f32 = 6.0;

const HEAD_SCALE: f32 = 2.0;
const HEAD_GAP: f32 = 4.0;
const TABLE_GAP: f32 = 10.0;
const TABLE_W: f32 = 300.0;
const TEXT_MARGIN: f32 = 10.0;
const ROW_H: f32 = LINE_HEIGHT + 1.0;
const LABEL_GAP: f32 = 8.0;
const WHITE: u32 = 0xFFFFFF;
const GREY: u32 = 0xA0A0A0;
const LIGHT: u32 = 0xE0E0E0;

const DROP_ORDER: [&str; 3] = [RUN_TIME, RANGE, ONE_TILE];
const RUN_TIME: &str = "Run time";
const RANGE: &str = "Range";
const ONE_TILE: &str = "One tile";

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    let footer_y = LAYOUT.footer_y(ctx.vh);
    let over_terrain = state.benchmark.phase == Phase::Gpu;
    let block_h = 2.0 * LINE_HEIGHT + BAR_GAP + BAR_H;
    let strip_y = LAYOUT.title_y() + LINE_HEIGHT + STRIP_PAD;
    if over_terrain {
        p.fill(0.0, 0.0, ctx.vw, strip_y + block_h + STRIP_PAD, STRIP);
        p.fill(
            0.0,
            footer_y - STRIP_PAD,
            ctx.vw,
            ctx.vh - footer_y + STRIP_PAD,
            STRIP,
        );
    }
    p.text_centered(TITLE, ctx.vw / 2.0, LAYOUT.title_y(), WHITE, true);

    match state.benchmark.phase {
        Phase::Cpu | Phase::Gpu => {
            let stage = if state.benchmark.phase == Phase::Cpu {
                MEASURING_CPU
            } else {
                MEASURING_GPU
            };
            let mut y = if over_terrain {
                strip_y
            } else {
                block_top(ctx, block_h)
            };
            p.text_centered(RUNNING, ctx.vw / 2.0, y, WHITE, true);
            y += LINE_HEIGHT;
            p.text_centered(stage, ctx.vw / 2.0, y, GREY, true);
            y += LINE_HEIGHT + BAR_GAP;
            bar(p, ctx, y, state.benchmark.progress);

            let x = widgets::centered_x(ctx.vw);
            if Button::new(x, footer_y, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, CANCEL).draw(p, ctx) {
                state.benchmark.phase = Phase::Idle;
                state.nav = Some(Screen::GameSettings);
            }
        }
        Phase::Done if state.benchmark.result.is_some() => {
            if let Some(result) = state.benchmark.result.as_ref() {
                draw_result(p, ctx, result);
            }

            let copied = state
                .benchmark
                .copied_at
                .is_some_and(|at| at.elapsed() < COPIED_FOR);
            let x = centered_row_x(ctx.vw, 2.0 * FOOTER_W + FOOTER_GAP);
            let copy_label = if copied { COPIED } else { COPY };
            if Button::new(x, footer_y, FOOTER_W, WIDGET_HEIGHT, copy_label).draw(p, ctx) {
                if let Some(result) = state.benchmark.result.as_ref() {
                    crate::platform::clipboard::set(&copy_text(result));
                }
                state.benchmark.copied_at = Some(Instant::now());
            }
            if Button::new(
                x + FOOTER_W + FOOTER_GAP,
                footer_y,
                FOOTER_W,
                WIDGET_HEIGHT,
                DONE,
            )
            .draw(p, ctx)
            {
                state.nav = Some(Screen::GameSettings);
            }
        }
        Phase::Idle | Phase::Done => {
            state.nav = Some(Screen::GameSettings);
        }
    }
}

fn draw_result(p: &mut Painter, ctx: &ScreenCtx, r: &BenchResult) {
    let scores = Scores::new(r);
    let (headline, sub) = match (scores.overall, scores.gpu) {
        (Some(overall), Some(gpu)) => (
            format!("Score {overall}"),
            Some(format!("CPU {}   GPU {gpu}", scores.cpu)),
        ),
        _ => (format!("CPU score {}", scores.cpu), None),
    };
    let mut rows = table_rows(r);

    let (_, _, _, content_h) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let head_h = HEAD_SCALE * LINE_HEIGHT + sub.as_ref().map_or(0.0, |_| HEAD_GAP + LINE_HEIGHT);
    let block_h = |rows: usize| head_h + TABLE_GAP + rows as f32 * ROW_H;
    for drop in DROP_ORDER {
        if block_h(rows.len()) <= content_h {
            break;
        }
        rows.retain(|(label, _)| *label != drop);
    }

    let mut y = block_top(ctx, block_h(rows.len()));
    let head_w = p.atlas.font.width_str(&headline) * HEAD_SCALE;
    let head_x = ((ctx.vw - head_w) / 2.0).floor();
    p.scaled(HEAD_SCALE, head_x, y, |p| {
        p.text_plain(&headline, 0.0, 0.0, WHITE, true);
    });
    y += HEAD_SCALE * LINE_HEIGHT;
    if let Some(sub) = &sub {
        y += HEAD_GAP;
        p.text_centered(sub, ctx.vw / 2.0, y, GREY, true);
        y += LINE_HEIGHT;
    }
    y += TABLE_GAP;

    let table_w = TABLE_W.min(ctx.vw - 2.0 * TEXT_MARGIN).floor();
    let table_x = ((ctx.vw - table_w) / 2.0).floor();
    let label_w = rows
        .iter()
        .map(|(label, _)| p.atlas.font.width_str(label))
        .fold(0.0, f32::max);
    let value_room = (table_w - label_w - LABEL_GAP).max(0.0);
    for (label, value) in &rows {
        p.text_plain(label, table_x, y, GREY, true);
        let value = p.fit(value, value_room);
        let value_x = table_x + table_w - p.atlas.font.width_str(&value);
        p.text_plain(&value, value_x, y, LIGHT, true);
        y += ROW_H;
    }
}

fn table_rows(r: &BenchResult) -> Vec<(&'static str, String)> {
    let c = &r.cpu;
    let mut rows = vec![
        ("Meshing", format!("{:.3} ms per section", c.ms_per_section)),
        (RANGE, format!("{:.3} to {:.3} ms", c.best_ms, c.worst_ms)),
        (
            "Throughput",
            format!("{:.0} sections/s", 1000.0 / c.ms_per_section),
        ),
    ];
    match r.gpu {
        GpuResult::Measured {
            load,
            triangles_per_frame,
            at_cap,
            baseline_ms,
            ..
        } => {
            let cap = if at_cap { " (at cap)" } else { "" };
            rows.push(("GPU load", format!("{load:.1} tiles{cap}")));
            rows.push((
                "Triangles",
                format!(
                    "{:.1}M per frame at {BUDGET_MS:.0} ms",
                    millions(triangles_per_frame)
                ),
            ));
            rows.push((ONE_TILE, format!("{baseline_ms:.1} ms per frame")));
        }
        GpuResult::Unmeasurable { baseline_ms } => {
            rows.push((
                "GPU",
                format!("not measurable ({baseline_ms:.1} ms with one tile)"),
            ));
        }
    }
    rows.push(("Device", r.adapter.clone()));
    rows.push(("System", system(r.cores)));
    rows.push((RUN_TIME, format!("{:.0} s", r.seconds)));
    rows
}

fn copy_text(r: &BenchResult) -> String {
    let scores = Scores::new(r);
    let c = &r.cpu;
    let mut lines = Vec::with_capacity(7);
    lines.push(format!(
        "Torch Client {} benchmark",
        env!("CARGO_PKG_VERSION")
    ));
    lines.push(match (scores.overall, scores.gpu) {
        (Some(overall), Some(gpu)) => format!("Score {overall} (CPU {}, GPU {gpu})", scores.cpu),
        _ => format!("CPU score {}", scores.cpu),
    });
    lines.push(format!(
        "CPU: {:.3} ms per section, {:.0} sections/s, {} quads",
        c.ms_per_section,
        1000.0 / c.ms_per_section,
        c.quads
    ));
    lines.push(match r.gpu {
        GpuResult::Measured {
            load,
            triangles_per_frame,
            at_cap,
            ..
        } => {
            let cap = if at_cap { " (at cap)" } else { "" };
            format!(
                "GPU: {load:.1} tiles, {:.1}M triangles per frame at {BUDGET_MS:.0} ms{cap}",
                millions(triangles_per_frame)
            )
        }
        GpuResult::Unmeasurable { baseline_ms } => {
            format!("GPU: not measurable ({baseline_ms:.1} ms with one tile)")
        }
    });
    lines.push(format!("Device: {}", r.adapter));
    lines.push(format!("System: {}", system(r.cores)));
    if cfg!(target_arch = "wasm32") {
        lines.push("Frames synced to the display".to_owned());
    }
    lines.join("\n")
}

fn millions(triangles: u64) -> f64 {
    triangles as f64 / 1_000_000.0
}

fn system(cores: Option<u32>) -> String {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    let mut out = if os.is_empty() {
        arch.to_owned()
    } else {
        format!("{os} {arch}")
    };
    if let Some(n) = cores {
        out.push_str(&format!(", {n} threads"));
    }
    out
}

fn block_top(ctx: &ScreenCtx, h: f32) -> f32 {
    let (_, content_y, _, content_h) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    (content_y + (content_h - h) * 0.5).floor().max(content_y)
}

fn bar(p: &mut Painter, ctx: &ScreenCtx, y: f32, frac: f32) {
    let w = BAR_W.min(ctx.vw - 40.0).max(60.0).floor();
    let x = ((ctx.vw - w) * 0.5).floor();
    p.fill(x, y, w, BAR_H, BAR_TRACK);
    let filled = ((w - 2.0) * frac.clamp(0.0, 1.0)).floor();
    if filled >= 1.0 {
        p.fill(x + 1.0, y + 1.0, filled, BAR_H - 2.0, BAR_FILL);
    }
    p.outline(x, y, w, BAR_H, BAR_EDGE);
}
