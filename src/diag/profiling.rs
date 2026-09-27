use crate::platform::time::epoch;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

struct Record {
    name: u32,
    thread: u32,
    start_ns: u64,
    end_ns: u64,
    depth: u32,
}

#[derive(Default)]
struct Interner {
    ids: HashMap<Box<str>, u32>,
    texts: Vec<Box<str>>,
}

impl Interner {
    fn intern(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.ids.get(s) {
            return id;
        }
        let id = self.texts.len() as u32;
        let boxed: Box<str> = s.into();
        self.texts.push(boxed.clone());
        self.ids.insert(boxed, id);
        id
    }
}

#[derive(Default)]
struct Profiler {
    records: Vec<Record>,
    intern: Interner,
    dropped: u64,
}

static PROF: OnceLock<Mutex<Profiler>> = OnceLock::new();

static CAPTURE_UNTIL_NS: AtomicU64 = AtomicU64::new(0);

fn prof() -> &'static Mutex<Profiler> {
    PROF.get_or_init(Default::default)
}

fn max_records() -> usize {
    static MAX: OnceLock<usize> = OnceLock::new();
    *MAX.get_or_init(|| {
        std::env::var("MC_PROF_MAX_RECORDS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(2_000_000)
    })
}

thread_local! {
    static OPEN: RefCell<Vec<(u32, u64)>> = const { RefCell::new(Vec::new()) };
    static THREAD_ID: Cell<Option<u32>> = const { Cell::new(None) };
}

fn thread_id() -> u32 {
    if let Some(id) = THREAD_ID.with(Cell::get) {
        return id;
    }
    let current = std::thread::current();
    let name = current
        .name()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{:?}", current.id()));
    let id = match prof().lock() {
        Ok(mut p) => p.intern.intern(&name),
        Err(_) => 0,
    };
    THREAD_ID.with(|c| c.set(Some(id)));
    id
}

fn intern(s: &str) -> u32 {
    match prof().lock() {
        Ok(mut p) => p.intern.intern(s),
        Err(_) => 0,
    }
}

fn open(name: u32) {
    let start_ns = epoch().elapsed().as_nanos() as u64;
    OPEN.with(|o| o.borrow_mut().push((name, start_ns)));
}

fn close() {
    let end_ns = epoch().elapsed().as_nanos() as u64;
    let Some((name, start_ns, depth)) = OPEN.with(|o| {
        let mut open = o.borrow_mut();
        let (name, start_ns) = open.pop()?;
        Some((name, start_ns, open.len() as u32))
    }) else {
        return;
    };
    let until = CAPTURE_UNTIL_NS.load(Ordering::Relaxed);
    if until == 0 || start_ns > until {
        return;
    }
    let thread = thread_id();
    let Ok(mut p) = prof().lock() else { return };
    if p.records.len() >= max_records() {
        p.dropped += 1;
        return;
    }
    p.records.push(Record {
        name,
        thread,
        start_ns,
        end_ns,
        depth,
    });
}

pub fn arm(window: Duration) {
    if let Ok(mut p) = prof().lock() {
        p.records.clear();
        p.dropped = 0;
    }
    let end = epoch().elapsed().as_nanos() as u64 + window.as_nanos() as u64;
    CAPTURE_UNTIL_NS.store(end, Ordering::Relaxed);
}

pub fn capture_elapsed() -> bool {
    let until = CAPTURE_UNTIL_NS.load(Ordering::Relaxed);
    if until == 0 || (epoch().elapsed().as_nanos() as u64) < until {
        return false;
    }
    CAPTURE_UNTIL_NS
        .compare_exchange(until, 0, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
}

pub fn window() -> Duration {
    static MS: OnceLock<u64> = OnceLock::new();
    Duration::from_millis(*MS.get_or_init(|| {
        std::env::var("MC_PROF_WINDOW_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(1000)
    }))
}

pub struct Span {
    _private: (),
}

pub fn span(name: &'static str) -> Span {
    open(intern(name));
    Span { _private: () }
}

pub fn span_cached(id: &'static OnceLock<u32>, name: &'static str) -> Span {
    open(*id.get_or_init(|| intern(name)));
    Span { _private: () }
}

impl Drop for Span {
    fn drop(&mut self) {
        close();
    }
}

fn report_coverage<'a>(
    records: &[Record],
    text: &dyn Fn(u32) -> &'a str,
    cpu: &[crate::diag::ThreadCpu],
) {
    if records.is_empty() {
        return;
    }
    let lo = records.iter().map(|r| r.start_ns).min().unwrap_or(0);
    let hi = records.iter().map(|r| r.end_ns).max().unwrap_or(0);
    let window = hi.saturating_sub(lo);
    if window == 0 {
        return;
    }

    let mut cpu_by_name: HashMap<&str, f64> = HashMap::new();
    for t in cpu {
        *cpu_by_name.entry(t.name.as_str()).or_default() += t.percent as f64;
    }

    let mut spanned: HashMap<&str, u64> = HashMap::new();
    for r in records.iter().filter(|r| r.depth == 0) {
        let name = text(r.thread);
        let Some(key) = cpu_by_name
            .keys()
            .find(|k| name == **k || name.starts_with(**k))
            .copied()
        else {
            continue;
        };
        *spanned.entry(key).or_default() += r.end_ns.saturating_sub(r.start_ns);
    }

    let mut rows: Vec<(&str, f64, u64)> = cpu_by_name
        .iter()
        .map(|(name, pct)| (*name, *pct, spanned.get(name).copied().unwrap_or(0)))
        .collect();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1));

    for (name, pct, ns) in rows {
        let busy_ns = (pct / 100.0) * window as f64;
        if busy_ns <= 0.0 {
            continue;
        }
        let cover = ns as f64 / busy_ns * 100.0;
        let note = if cover < 50.0 {
            "   <-- work here is NOT covered by any span"
        } else if cover > 110.0 {
            "   (blocks inside its spans)"
        } else {
            ""
        };
        crate::log_info!(
            "prof",
            "cpu {:>6.1}%  spans cover {:>5.1}% of it  {}{}",
            pct,
            cover,
            name,
            note
        );
    }
}

pub fn dump_html(path: &str, cpu: &[crate::diag::ThreadCpu]) -> std::io::Result<()> {
    let (records, texts, dropped) = match prof().lock() {
        Ok(mut p) => {
            let records = std::mem::take(&mut p.records);
            let texts = p.intern.texts.clone();
            let dropped = std::mem::replace(&mut p.dropped, 0);
            (records, texts, dropped)
        }
        Err(_) => (Vec::new(), Vec::new(), 0),
    };

    let text = |id: u32| texts.get(id as usize).map(|s| &**s).unwrap_or("<unknown>");

    report_coverage(&records, &text, cpu);

    let mut thread_ids: Vec<u32> = records.iter().map(|r| r.thread).collect();
    thread_ids.sort_unstable();
    thread_ids.dedup();
    thread_ids.sort_by_key(|id| text(*id).to_owned());
    let slot: HashMap<u32, usize> = thread_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i))
        .collect();

    let min_ns = records.iter().map(|r| r.start_ns).min().unwrap_or(0);
    let max_ns = records.iter().map(|r| r.end_ns).max().unwrap_or(0);

    let mut json = String::with_capacity(records.len() * 72 + 2);
    json.push('[');
    for (i, r) in records.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        let _ = write!(
            json,
            "{{\"n\":{:?},\"t\":{},\"s\":{},\"e\":{},\"d\":{}}}",
            text(r.name),
            slot.get(&r.thread).copied().unwrap_or(0),
            r.start_ns.saturating_sub(min_ns),
            r.end_ns.saturating_sub(min_ns),
            r.depth,
        );
    }
    json.push(']');

    let mut threads_json = String::from("[");
    for (i, id) in thread_ids.iter().enumerate() {
        if i > 0 {
            threads_json.push(',');
        }
        let _ = write!(threads_json, "{:?}", text(*id));
    }
    threads_json.push(']');

    let count = if dropped > 0 {
        format!(
            "{} ({dropped} dropped, raise MC_PROF_MAX_RECORDS)",
            records.len()
        )
    } else {
        records.len().to_string()
    };

    let mut cpu_label = String::from("measured CPU: ");
    if cpu.is_empty() {
        cpu_label.push_str("(no sample)");
    } else {
        let total: f32 = cpu.iter().map(|t| t.percent).sum();
        let _ = write!(cpu_label, "{:.0}% total &mdash; ", total);
        for (i, t) in cpu.iter().take(12).enumerate() {
            if i > 0 {
                cpu_label.push_str(" &middot; ");
            }
            let name = t.name.replace('&', "&amp;").replace('<', "&lt;");
            let _ = write!(cpu_label, "{name} {:.0}%", t.percent);
        }
    }

    let html = HTML_TEMPLATE
        .replace("__SPANS__", &json)
        .replace("__THREADS__", &threads_json)
        .replace("__TOTAL_NS__", &max_ns.saturating_sub(min_ns).to_string())
        .replace("__COUNT__", &count)
        .replace("__CPU__", &cpu_label);

    std::fs::write(path, html)
}

struct Label(u32);

struct Bridge;

struct NameField(Option<String>);

impl bevy::log::tracing::field::Visit for NameField {
    fn record_str(&mut self, field: &bevy::log::tracing::field::Field, value: &str) {
        if field.name() == "name" {
            self.0 = Some(value.to_owned());
        }
    }

    fn record_debug(
        &mut self,
        field: &bevy::log::tracing::field::Field,
        value: &dyn std::fmt::Debug,
    ) {
        if field.name() == "name" && self.0.is_none() {
            self.0 = Some(format!("{value:?}"));
        }
    }
}

fn short_name(path: &str) -> &str {
    let path = match path.find('<') {
        Some(i) => &path[..i],
        None => path,
    };
    let path = path.trim_end_matches("::");
    match path.rmatch_indices("::").nth(1) {
        Some((i, _)) => &path[i + 2..],
        None => path,
    }
}

impl<S> bevy::log::tracing_subscriber::Layer<S> for Bridge
where
    S: bevy::log::tracing::Subscriber,
    S: for<'a> bevy::log::tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &bevy::log::tracing::span::Attributes<'_>,
        id: &bevy::log::tracing::Id,
        ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>,
    ) {
        let Some(span) = ctx.span(id) else { return };
        let mut field = NameField(None);
        attrs.record(&mut field);
        let kind = span.metadata().name();
        let label = match field.0 {
            Some(name) => format!("{kind}:{}", short_name(&name)),
            None => kind.to_owned(),
        };
        span.extensions_mut().insert(Label(intern(&label)));
    }

    fn on_enter(
        &self,
        id: &bevy::log::tracing::Id,
        ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>,
    ) {
        let label = ctx
            .span(id)
            .and_then(|s| s.extensions().get::<Label>().map(|l| l.0));
        if let Some(label) = label {
            open(label);
        }
    }

    fn on_exit(
        &self,
        id: &bevy::log::tracing::Id,
        ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>,
    ) {
        let labelled = ctx
            .span(id)
            .map(|s| s.extensions().get::<Label>().is_some())
            .unwrap_or(false);
        if labelled {
            close();
        }
    }
}

pub fn bridge_layer() -> Option<bevy::log::BoxedLayer> {
    use bevy::log::tracing_subscriber::Layer;
    Some(Box::new(Bridge))
}

const HTML_TEMPLATE: &str = r##"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>flamegraph</title>
<style>
  html, body {
    margin: 0; height: 100%; overflow: hidden; background: #1e1e1e; color: #ccc;
    font: 12px -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
  }
  #bar {
    display: flex; align-items: center; gap: 14px; height: 32px; padding: 0 10px;
    background: #2d2d30; border-bottom: 1px solid #3e3e42; box-sizing: border-box;
  }
  #bar button {
    font: 12px inherit; background: #3c3c3c; color: #ccc; border: 1px solid #555;
    border-radius: 3px; padding: 3px 9px; cursor: pointer;
  }
  #bar button:hover { background: #4a4a4a; }
  #bar .stat { color: #8a8a8a; }
  #bar .hint { color: #6a6a6a; margin-left: auto; }
  #main { position: absolute; top: 32px; left: 0; right: 0; bottom: 118px; display: flex; }
  #viewport {
    flex: 1 1 auto; min-width: 0; overflow-y: auto; overflow-x: hidden;
    position: relative; background: #1e1e1e;
  }
  canvas { display: block; }
  #canvas { cursor: grab; }
  #canvas.dragging { cursor: grabbing; }
  #summary {
    width: 340px; flex: 0 0 340px; overflow-y: auto; overflow-x: hidden;
    border-left: 1px solid #3e3e42; background: #232326;
  }
  #summary table { width: 100%; border-collapse: collapse; font-size: 11px; }
  #summary th {
    text-align: left; padding: 6px 8px; color: #8a8a8a; font-weight: 600;
    border-bottom: 1px solid #3e3e42; position: sticky; top: 0; background: #232326;
  }
  #summary td { padding: 4px 8px; border-bottom: 1px solid #2a2a2d; white-space: nowrap; }
  #summary tr.srow { cursor: pointer; }
  #summary tr.srow:hover td { background: #2a2a2e; }
  #summary tr.selected td { background: #2f3d4d; }
  .bar-cell { position: relative; max-width: 170px; overflow: hidden; text-overflow: ellipsis; }
  .bar-cell .bar { position: absolute; left: 0; top: 0; bottom: 0; background: rgba(79,193,255,0.16); }
  .bar-cell span { position: relative; }
  #overviewWrap {
    position: absolute; left: 0; right: 0; bottom: 0; height: 118px; box-sizing: border-box;
    border-top: 1px solid #3e3e42; background: #232326; padding: 6px 0 0;
  }
  #overviewLabel { padding: 0 10px 4px; color: #7a7a7a; font-size: 11px; }
  #cpuLabel {
    padding: 0 10px 4px; color: #d7a35c; font: 11px monospace;
    white-space: nowrap; overflow-x: auto;
  }
  #overviewCanvas { display: block; width: 100%; height: 90px; cursor: pointer; }
  #tooltip {
    position: fixed; display: none; background: #252526; border: 1px solid #454545;
    padding: 6px 9px; border-radius: 4px; font: 11px monospace; color: #ddd;
    pointer-events: none; z-index: 10; max-width: 420px; box-shadow: 0 2px 10px rgba(0,0,0,.5);
  }
  #tooltip .name { color: #4fc1ff; font-weight: bold; display: block; margin-bottom: 2px; }
</style>
</head>
<body>
<div id="bar">
  <span class="stat">spans: __COUNT__</span>
  <span class="stat">total: <b id="totalMs"></b> ms</span>
  <button id="zin">Zoom in</button>
  <button id="zout">Zoom out</button>
  <button id="zreset">Reset</button>
  <span class="hint">ctrl/⌘ + scroll to zoom &middot; drag to pan &middot; shift+scroll to pan &middot; click a row to jump to it</span>
</div>
<div id="main">
  <div id="viewport"><canvas id="canvas"></canvas></div>
  <div id="summary">
    <table>
      <thead><tr><th>Function</th><th>Count</th><th>Total ms</th><th>%</th><th>Avg ms</th></tr></thead>
      <tbody id="summaryBody"></tbody>
    </table>
  </div>
</div>
<div id="overviewWrap">
  <div id="overviewLabel">Timeline overview &mdash; drag the window to pan, drag its edges to zoom, click elsewhere to jump</div>
  <div id="cpuLabel">__CPU__</div>
  <canvas id="overviewCanvas"></canvas>
</div>
<div id="tooltip"></div>
<script>
const rawSpans = __SPANS__;
const threadNames = __THREADS__;
const totalNs = __TOTAL_NS__;
document.getElementById('totalMs').textContent = (totalNs / 1e6).toFixed(2);

const ROW_H = 20;
const ROW_GAP = 2;
const HEADER_H = 18;
const THREAD_GAP = 10;
const RULER_H = 22;

function clamp(v, lo, hi) { return Math.min(hi, Math.max(lo, v)); }

const byThread = threadNames.map((_, i) => rawSpans.filter(s => s.t === i));
const threadLayout = [];
const spans = [];
let cursorY = RULER_H + 8;
threadNames.forEach((name, ti) => {
  const rows = byThread[ti];
  const maxDepth = rows.reduce((m, s) => Math.max(m, s.d), 0);
  const labelY = cursorY;
  const rowsTop = cursorY + HEADER_H;
  const rowsBottom = rowsTop + (maxDepth + 1) * ROW_H;
  threadLayout.push({ name, labelY, count: rows.length });
  for (const s of rows) spans.push({ n: s.n, s: s.s, e: s.e, absY: rowsBottom - (s.d + 1) * ROW_H });
  cursorY = rowsBottom + THREAD_GAP;
});
const contentHeight = Math.max(cursorY, 200);

const durs = spans.map(s => (s.e - s.s) / 1e6).filter(d => d > 0).sort((a, b) => a - b);
const medianMs = durs.length ? durs[Math.floor(durs.length / 2)] : 1;
const DEFAULT_PXPERMS = Math.min(2000, Math.max(0.02, 6 / medianMs));
let pxPerMs = DEFAULT_PXPERMS;
let viewStartMs = 0;
let selectedName = null;

const viewport = document.getElementById('viewport');
const canvas = document.getElementById('canvas');
const ctx = canvas.getContext('2d');
const overviewCanvas = document.getElementById('overviewCanvas');
const octx = overviewCanvas.getContext('2d');
const tooltip = document.getElementById('tooltip');
const dpr = window.devicePixelRatio || 1;

function colorFor(name, emph) {
  let h = 0;
  for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) >>> 0;
  return `hsl(${h % 360}, ${emph ? 60 : 42}%, ${emph ? 58 : 45}%)`;
}

function niceStep(msPerDivision) {
  const steps = [0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5,
    1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
  for (const s of steps) if (msPerDivision <= s) return s;
  return steps[steps.length - 1];
}

let hovered = null;

function draw() {
  const w = viewport.clientWidth;
  ctx.fillStyle = '#1e1e1e';
  ctx.fillRect(0, 0, w, contentHeight);

  const visibleMs = w / pxPerMs;
  const endMs = viewStartMs + visibleMs;

  const step = niceStep(visibleMs / 8);
  const first = Math.floor(viewStartMs / step) * step;
  ctx.font = '10px monospace';
  ctx.textBaseline = 'top';
  for (let t = first; t < endMs; t += step) {
    const x = (t - viewStartMs) * pxPerMs;
    ctx.strokeStyle = '#333';
    ctx.beginPath(); ctx.moveTo(x + 0.5, RULER_H); ctx.lineTo(x + 0.5, contentHeight); ctx.stroke();
    ctx.fillStyle = '#8a8a8a';
    ctx.fillText((step < 1 ? t.toFixed(3) : t.toFixed(0)) + ' ms', x + 3, 4);
  }
  ctx.strokeStyle = '#4a4a4a';
  ctx.beginPath(); ctx.moveTo(0, RULER_H + 0.5); ctx.lineTo(w, RULER_H + 0.5); ctx.stroke();

  ctx.font = 'bold 11px -apple-system, sans-serif';
  ctx.fillStyle = '#9c9c9c';
  for (const t of threadLayout) ctx.fillText(`${t.name}  (${t.count})`, 6, t.labelY + 3);

  ctx.font = '10px monospace';
  ctx.textBaseline = 'middle';
  for (const s of spans) {
    const sMs = s.s / 1e6, eMs = s.e / 1e6;
    if (eMs < viewStartMs || sMs > endMs) continue;
    const x = (sMs - viewStartMs) * pxPerMs;
    const wpx = Math.max(1, (eMs - sMs) * pxPerMs);
    const isHover = hovered === s;
    const isSelected = selectedName !== null && s.n === selectedName;
    ctx.fillStyle = colorFor(s.n, isHover || isSelected);
    const rh = ROW_H - ROW_GAP;
    if (ctx.roundRect) {
      ctx.beginPath(); ctx.roundRect(x, s.absY, wpx, rh, 2); ctx.fill();
    } else {
      ctx.fillRect(x, s.absY, wpx, rh);
    }
    if (isHover || isSelected) {
      ctx.strokeStyle = isSelected ? '#ffd479' : '#fff';
      ctx.lineWidth = isSelected ? 1.5 : 1;
      ctx.strokeRect(x + 0.5, s.absY + 0.5, wpx - 1, rh - 1);
    }
    if (wpx > 22) {
      ctx.save();
      ctx.beginPath(); ctx.rect(x + 3, s.absY, wpx - 6, rh); ctx.clip();
      ctx.fillStyle = '#0c0c0c';
      ctx.fillText(s.n, x + 4, s.absY + rh / 2 + 1);
      ctx.restore();
    }
  }
}

function spanAt(px, py) {
  const mx = viewStartMs + px / pxPerMs;
  for (let i = spans.length - 1; i >= 0; i--) {
    const s = spans[i];
    if (py >= s.absY && py <= s.absY + ROW_H - ROW_GAP && mx >= s.s / 1e6 && mx <= s.e / 1e6) return s;
  }
  return null;
}

const agg = new Map();
for (const s of spans) {
  const d = s.e - s.s;
  let a = agg.get(s.n);
  if (!a) { a = { count: 0, total: 0, first: s }; agg.set(s.n, a); }
  a.count++; a.total += d;
  if (s.s < a.first.s) a.first = s;
}
let grandTotal = 0;
for (const a of agg.values()) grandTotal += a.total;
if (grandTotal <= 0) grandTotal = 1;
const summaryRows = Array.from(agg.entries())
  .map(([name, a]) => ({ name, count: a.count, total: a.total, avg: a.total / a.count, pct: a.total / grandTotal * 100, first: a.first }))
  .sort((a, b) => b.total - a.total);

const summaryBody = document.getElementById('summaryBody');
summaryBody.innerHTML = summaryRows.map(r => `
  <tr class="srow" data-name="${r.name}">
    <td class="bar-cell"><div class="bar" style="width:${r.pct.toFixed(1)}%"></div><span>${r.name}</span></td>
    <td>${r.count}</td>
    <td>${(r.total / 1e6).toFixed(2)}</td>
    <td>${r.pct.toFixed(1)}%</td>
    <td>${(r.avg / 1e6).toFixed(3)}</td>
  </tr>`).join('');

summaryBody.addEventListener('click', e => {
  const tr = e.target.closest('tr.srow');
  if (!tr) return;
  const name = tr.dataset.name;
  Array.from(summaryBody.children).forEach(r => r.classList.remove('selected'));
  if (selectedName === name) {
    selectedName = null;
  } else {
    selectedName = name;
    tr.classList.add('selected');
    const row = summaryRows.find(r => r.name === name);
    if (row) {
      const target = row.first;
      const durMs = Math.max((target.e - target.s) / 1e6, 0.001);
      pxPerMs = clamp((viewport.clientWidth * 0.3) / durMs, 0.0005, 5000);
      viewStartMs = Math.max(0, target.s / 1e6 - (viewport.clientWidth / pxPerMs) * 0.3);
    }
  }
  drawAll();
});

function resize() {
  const w = viewport.clientWidth;
  canvas.style.width = w + 'px';
  canvas.style.height = contentHeight + 'px';
  canvas.width = Math.round(w * dpr);
  canvas.height = Math.round(contentHeight * dpr);
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
}

function computeActivity() {
  const totalMsAll = totalNs / 1e6;
  const N = 1500;
  const buckets = new Float32Array(N);
  if (totalMsAll <= 0) return buckets;
  const bucketMs = totalMsAll / N;
  for (const s of spans) {
    const sMs = s.s / 1e6, eMs = s.e / 1e6;
    const bi = clamp(Math.floor(sMs / bucketMs), 0, N - 1);
    const bj = clamp(Math.floor(eMs / bucketMs), 0, N - 1);
    for (let b = bi; b <= bj; b++) {
      const bStart = b * bucketMs, bEnd = bStart + bucketMs;
      const ov = Math.min(eMs, bEnd) - Math.max(sMs, bStart);
      if (ov > 0) buckets[b] += ov;
    }
  }
  return buckets;
}
const activityBuckets = computeActivity();

function resizeOverview() {
  const w = overviewCanvas.clientWidth, h = overviewCanvas.clientHeight;
  overviewCanvas.width = Math.round(w * dpr);
  overviewCanvas.height = Math.round(h * dpr);
  octx.setTransform(dpr, 0, 0, dpr, 0, 0);
}

function drawOverview() {
  const w = overviewCanvas.clientWidth, h = overviewCanvas.clientHeight;
  octx.fillStyle = '#1e1e1e';
  octx.fillRect(0, 0, w, h);

  let maxV = 1e-9;
  for (const v of activityBuckets) if (v > maxV) maxV = v;
  const perPx = activityBuckets.length / Math.max(1, w);
  octx.fillStyle = '#5b8dd6';
  for (let x = 0; x < w; x++) {
    const b0 = Math.floor(x * perPx), b1 = Math.min(activityBuckets.length, Math.floor((x + 1) * perPx) + 1);
    let v = 0;
    for (let b = b0; b < b1; b++) if (activityBuckets[b] > v) v = activityBuckets[b];
    const hgt = Math.max(1, (v / maxV) * (h - 4));
    octx.fillRect(x, h - hgt, 1, hgt);
  }

  const totalMsAll = Math.max(totalNs / 1e6, 1e-6);
  const visibleMs = viewport.clientWidth / pxPerMs;
  const wx = clamp((viewStartMs / totalMsAll) * w, 0, w);
  const ww = clamp((visibleMs / totalMsAll) * w, 2, w);
  octx.fillStyle = 'rgba(255,255,255,0.10)';
  octx.fillRect(wx, 0, ww, h);
  octx.strokeStyle = '#4fc1ff';
  octx.lineWidth = 1.5;
  octx.strokeRect(wx + 0.75, 0.75, Math.max(1, ww - 1.5), h - 1.5);
  octx.fillStyle = '#4fc1ff';
  octx.fillRect(wx - 2, 0, 4, h);
  octx.fillRect(wx + ww - 2, 0, 4, h);
}

function drawAll() { draw(); drawOverview(); }

window.addEventListener('resize', () => { resize(); resizeOverview(); drawAll(); });

let dragging = false, dragStartX = 0, dragOrigStart = 0;
canvas.addEventListener('mousedown', e => {
  dragging = true; dragStartX = e.clientX; dragOrigStart = viewStartMs;
  canvas.classList.add('dragging');
});
window.addEventListener('mousemove', e => {
  if (dragging) {
    const dxMs = (e.clientX - dragStartX) / pxPerMs;
    viewStartMs = Math.max(0, dragOrigStart - dxMs);
    drawAll();
    return;
  }
  const rect = canvas.getBoundingClientRect();
  const px = e.clientX - rect.left, py = e.clientY - rect.top;
  const s = spanAt(px, py);
  if (s !== hovered) { hovered = s; draw(); }
  if (s) {
    tooltip.style.display = 'block';
    tooltip.style.left = (e.clientX + 14) + 'px';
    tooltip.style.top = (e.clientY + 14) + 'px';
    tooltip.innerHTML = `<span class="name">${s.n}</span>${((s.e - s.s) / 1e6).toFixed(3)} ms`;
  } else {
    tooltip.style.display = 'none';
  }
});
window.addEventListener('mouseup', () => { dragging = false; canvas.classList.remove('dragging'); });
canvas.addEventListener('mouseleave', () => { if (!dragging) { hovered = null; tooltip.style.display = 'none'; draw(); } });

viewport.addEventListener('wheel', e => {
  if (e.ctrlKey || e.metaKey) {
    e.preventDefault();
    const rect = canvas.getBoundingClientRect();
    const mx = e.clientX - rect.left;
    const mouseMs = viewStartMs + mx / pxPerMs;
    const factor = Math.pow(1.0025, -e.deltaY);
    pxPerMs = clamp(pxPerMs * factor, 0.0005, 5000);
    viewStartMs = Math.max(0, mouseMs - mx / pxPerMs);
    drawAll();
  } else if (e.shiftKey || Math.abs(e.deltaX) > Math.abs(e.deltaY)) {
    e.preventDefault();
    viewStartMs = Math.max(0, viewStartMs + (e.deltaX || e.deltaY) / pxPerMs);
    drawAll();
  }
}, { passive: false });

function zoomBy(factor) {
  const w = viewport.clientWidth;
  const centerMs = viewStartMs + (w / 2) / pxPerMs;
  pxPerMs = clamp(pxPerMs * factor, 0.0005, 5000);
  viewStartMs = Math.max(0, centerMs - (w / 2) / pxPerMs);
  drawAll();
}
document.getElementById('zin').onclick = () => zoomBy(1.6);
document.getElementById('zout').onclick = () => zoomBy(1 / 1.6);
document.getElementById('zreset').onclick = () => { pxPerMs = DEFAULT_PXPERMS; viewStartMs = 0; drawAll(); };

let ovDrag = null;
let ovDragStartX = 0, ovDragStartViewStart = 0, ovDragStartPxPerMs = 0;
overviewCanvas.addEventListener('mousedown', e => {
  const w = overviewCanvas.clientWidth;
  const totalMsAll = Math.max(totalNs / 1e6, 1e-6);
  const visibleMs = viewport.clientWidth / pxPerMs;
  const wx = (viewStartMs / totalMsAll) * w;
  const ww = Math.max(2, (visibleMs / totalMsAll) * w);
  const rect = overviewCanvas.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const EDGE = 6;
  if (Math.abs(x - wx) <= EDGE) {
    ovDrag = 'left';
  } else if (Math.abs(x - (wx + ww)) <= EDGE) {
    ovDrag = 'right';
  } else if (x >= wx && x <= wx + ww) {
    ovDrag = 'pan';
  } else {
    const clickMs = (x / w) * totalMsAll;
    viewStartMs = Math.max(0, clickMs - visibleMs / 2);
    ovDrag = 'pan';
    drawAll();
  }
  ovDragStartX = e.clientX;
  ovDragStartViewStart = viewStartMs;
  ovDragStartPxPerMs = pxPerMs;
});
window.addEventListener('mousemove', e => {
  if (!ovDrag) return;
  const w = overviewCanvas.clientWidth;
  const totalMsAll = Math.max(totalNs / 1e6, 1e-6);
  const dxMs = ((e.clientX - ovDragStartX) / w) * totalMsAll;
  const rightMs0 = ovDragStartViewStart + viewport.clientWidth / ovDragStartPxPerMs;
  const MIN_MS = 1e-3;
  if (ovDrag === 'pan') {
    viewStartMs = Math.max(0, ovDragStartViewStart + dxMs);
  } else if (ovDrag === 'right') {
    const newRight = Math.max(ovDragStartViewStart + MIN_MS, rightMs0 + dxMs);
    pxPerMs = clamp(viewport.clientWidth / (newRight - ovDragStartViewStart), 0.0005, 5000);
    viewStartMs = ovDragStartViewStart;
  } else if (ovDrag === 'left') {
    const newLeft = clamp(ovDragStartViewStart + dxMs, 0, rightMs0 - MIN_MS);
    pxPerMs = clamp(viewport.clientWidth / (rightMs0 - newLeft), 0.0005, 5000);
    viewStartMs = newLeft;
  }
  drawAll();
});
window.addEventListener('mouseup', () => { ovDrag = null; });

resize();
resizeOverview();
drawAll();
</script>
</body>
</html>
"##;
