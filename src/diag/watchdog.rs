use crate::platform::time::Instant;
use crate::{log_error, log_info, log_warn};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use super::stats::{Stat, bump, counts, get};

struct Session {
    connected_at: Option<Instant>,
    first_chunk_at: Option<Instant>,
    spawned: bool,
}

const REPORT_COUNT: usize = 8;
const R_NO_CHUNKS: usize = 0;
const R_NOT_SPAWNED: usize = 1;
const R_NO_MESHES: usize = 2;
const R_LIGHT_DEAD: usize = 3;
const R_BOT_STALLED: usize = 4;
const R_TICK_STUCK: usize = 5;
const R_BOT_KILLED: usize = 6;
const R_RENDER_STALLED: usize = 7;

static REPORTED: [AtomicBool; REPORT_COUNT] = [const { AtomicBool::new(false) }; REPORT_COUNT];

static SESSION: Mutex<Session> = Mutex::new(Session {
    connected_at: None,
    first_chunk_at: None,
    spawned: false,
});

static LIGHT_DEAD: AtomicBool = AtomicBool::new(false);

static VIEW_CENTER: [AtomicI32; 2] = [AtomicI32::new(0), AtomicI32::new(0)];
static PLAYER_CHUNK: [AtomicI32; 2] = [AtomicI32::new(0), AtomicI32::new(0)];
static WINDOW_RADIUS: AtomicI32 = AtomicI32::new(-1);

pub fn on_connected() {
    let mut s = SESSION.lock().unwrap();
    *s = Session {
        connected_at: Some(Instant::now()),
        first_chunk_at: None,
        spawned: false,
    };
    for flag in REPORTED.iter() {
        flag.store(false, Ordering::Relaxed);
    }
    super::stats::reset();
    WINDOW_RADIUS.store(-1, Ordering::Relaxed);
    LIGHT_DEAD.store(false, Ordering::Relaxed);
}

pub fn on_spawned() {
    SESSION.lock().unwrap().spawned = true;
}

pub fn on_disconnected() {
    let mut s = SESSION.lock().unwrap();
    s.connected_at = None;
    s.spawned = false;
    drop(s);

    azalea_protocol::traffic::reset();
    super::runtime::reset_sampler();
}

pub fn on_chunk_received() {
    bump(Stat::ChunksReceived);
    let mut s = SESSION.lock().unwrap();
    if s.first_chunk_at.is_none() {
        s.first_chunk_at = Some(Instant::now());
    }
}

pub fn light_thread_died() {
    LIGHT_DEAD.store(true, Ordering::Relaxed);
}

pub fn note_view(center: (i32, i32), player: (i32, i32)) {
    VIEW_CENTER[0].store(center.0, Ordering::Relaxed);
    VIEW_CENTER[1].store(center.1, Ordering::Relaxed);
    PLAYER_CHUNK[0].store(player.0, Ordering::Relaxed);
    PLAYER_CHUNK[1].store(player.1, Ordering::Relaxed);
}

pub fn note_chunk_packet(pos: (i32, i32), in_window: bool, center: (i32, i32), radius: u32) {
    bump(Stat::ChunkPacketsSeen);
    WINDOW_RADIUS.store(radius as i32, Ordering::Relaxed);
    if in_window {
        return;
    }
    let n = get(Stat::ChunkPacketsOutOfWindow);
    bump(Stat::ChunkPacketsOutOfWindow);
    if n >= LOUD_REJECTS && (n + 1) % REJECT_EVERY != 0 {
        return;
    }
    let (_, player) = view_pair();
    log_warn!(
        "view",
        "chunk {},{} arrived outside azalea's window (centred {},{}, radius {}), \
         player in chunk {},{}; it will be dropped and the server will not send it \
         again. {} dropped so far this session.",
        pos.0,
        pos.1,
        center.0,
        center.1,
        radius,
        player.0,
        player.1,
        n + 1
    );
}

pub fn note_packet(name: &'static str) {
    trace_packet(name, false);
}

pub fn note_sent_packet(name: &'static str) {
    trace_packet(name, true);
}

struct Trace {
    seen: u32,
    last: Option<&'static str>,
    run: u32,
}

static TRACE: Mutex<Trace> = Mutex::new(Trace {
    seen: 0,
    last: None,
    run: 0,
});

pub fn trace_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| matches!(std::env::var("MC_TRACE").as_deref(), Ok("1") | Ok("true")))
}

fn trace_packet(name: &'static str, sent: bool) {
    if !trace_enabled() {
        return;
    }
    if SESSION.lock().unwrap().spawned {
        return;
    }
    let mut t = TRACE.lock().unwrap();
    if t.seen >= TRACE_PACKETS {
        return;
    }
    t.seen += 1;
    if !sent && t.last == Some(name) {
        t.run += 1;
        return;
    }
    let flush = t.last.take().map(|prev| (prev, t.run));
    t.last = if sent { None } else { Some(name) };
    t.run = if sent { 0 } else { 1 };
    let n = t.seen;
    drop(t);
    if let Some((prev, run)) = flush
        && run > 1
    {
        log_info!("packet", "  ... {prev} x{run}");
    }
    if sent {
        log_info!("packet", "{n:>3} -> {name}");
    } else {
        log_info!("packet", "{n:>3} {name}");
    }
}

pub fn tracing_packets() -> bool {
    trace_enabled()
        && TRACE.lock().unwrap().seen < TRACE_PACKETS
        && !SESSION.lock().unwrap().spawned
}

pub fn note_packet_detail(detail: &str) {
    log_info!("packet", "     {detail}");
}

const TRACE_PACKETS: u32 = 80;

const LOUD_REJECTS: u64 = 5;
const REJECT_EVERY: u64 = 200;

pub fn note_cache_center(center: (i32, i32)) {
    let n = get(Stat::CacheCenterMoves);
    bump(Stat::CacheCenterMoves);
    if n == 0 {
        log_info!(
            "view",
            "server centred the chunk window on {},{}",
            center.0,
            center.1
        );
    }
}

fn window_radius() -> Option<u32> {
    match WINDOW_RADIUS.load(Ordering::Relaxed) {
        -1 => None,
        r => Some(r as u32),
    }
}

fn view_pair() -> ((i32, i32), (i32, i32)) {
    (
        (
            VIEW_CENTER[0].load(Ordering::Relaxed),
            VIEW_CENTER[1].load(Ordering::Relaxed),
        ),
        (
            PLAYER_CHUNK[0].load(Ordering::Relaxed),
            PLAYER_CHUNK[1].load(Ordering::Relaxed),
        ),
    )
}

const NOT_SPAWNED_AFTER: Duration = Duration::from_secs(10);
const NO_MESH_AFTER: Duration = Duration::from_secs(5);

fn now_ms() -> u64 {
    crate::platform::time::epoch().elapsed().as_millis() as u64
}

static BOT_LAST_UPDATE_MS: AtomicU64 = AtomicU64::new(0);
static TICK_ENTERED_MS: AtomicU64 = AtomicU64::new(0);
static RENDER_LAST_FRAME_MS: AtomicU64 = AtomicU64::new(0);

pub fn note_frame() {
    RENDER_LAST_FRAME_MS.store(now_ms().max(1), Ordering::Relaxed);
}

pub fn note_bot_update() {
    BOT_LAST_UPDATE_MS.store(now_ms().max(1), Ordering::Relaxed);
}

pub fn note_bot_stopped() {
    BOT_LAST_UPDATE_MS.store(0, Ordering::Relaxed);
    TICK_ENTERED_MS.store(0, Ordering::Relaxed);
}

pub struct TickGuard;

impl TickGuard {
    pub fn new() -> Self {
        TICK_ENTERED_MS.store(now_ms().max(1), Ordering::Relaxed);
        TickGuard
    }
}

impl Default for TickGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TickGuard {
    fn drop(&mut self) {
        TICK_ENTERED_MS.store(0, Ordering::Relaxed);
    }
}

const BOT_STALL_AFTER: Duration = Duration::from_secs(3);

fn bot_kill_after() -> Option<Duration> {
    static SECS: OnceLock<Option<Duration>> = OnceLock::new();
    *SECS.get_or_init(|| {
        let secs = std::env::var("MC_BOT_KILL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5);
        (secs > 0).then(|| Duration::from_secs(secs))
    })
}

static FORCE_DISCONNECT: AtomicBool = AtomicBool::new(false);

static EVENT_COUNT: AtomicU64 = AtomicU64::new(0);
static EVENT_NANOS: AtomicU64 = AtomicU64::new(0);
static PACKET_COUNT: AtomicU64 = AtomicU64::new(0);
static PACKET_NANOS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Phase {
    Nearest,
    Meta,
    World,
    Anim,
    Tick,
    Snapshot,
}

const PHASE_COUNT: usize = Phase::Snapshot as usize + 1;
static PHASE_NANOS: [AtomicU64; PHASE_COUNT] = [const { AtomicU64::new(0) }; PHASE_COUNT];

static LAST_REPORT: Mutex<Option<Instant>> = Mutex::new(None);

pub struct Timed(Phase, Instant);

impl Drop for Timed {
    fn drop(&mut self) {
        PHASE_NANOS[self.0 as usize]
            .fetch_add(self.1.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
}

pub fn timed(phase: Phase) -> Timed {
    Timed(phase, Instant::now())
}

pub fn note_event(nanos: u64, is_packet: bool) {
    EVENT_COUNT.fetch_add(1, Ordering::Relaxed);
    EVENT_NANOS.fetch_add(nanos, Ordering::Relaxed);
    if is_packet {
        PACKET_COUNT.fetch_add(1, Ordering::Relaxed);
        PACKET_NANOS.fetch_add(nanos, Ordering::Relaxed);
    }
}

fn report_event_load() {
    let events = EVENT_COUNT.swap(0, Ordering::Relaxed);
    let nanos = EVENT_NANOS.swap(0, Ordering::Relaxed);
    let packets = PACKET_COUNT.swap(0, Ordering::Relaxed);
    let packet_nanos = PACKET_NANOS.swap(0, Ordering::Relaxed);
    let ms = |p: Phase| PHASE_NANOS[p as usize].swap(0, Ordering::Relaxed) as f64 / 1e6;
    let now = Instant::now();
    let window = {
        let mut last = LAST_REPORT.lock().unwrap_or_else(|e| e.into_inner());
        let elapsed = last
            .map(|t: Instant| now.duration_since(t))
            .unwrap_or_default();
        *last = Some(now);
        elapsed.as_secs_f64() * 1e3
    };
    if events < 10_000 {
        return;
    }
    let (nearest, meta, world, anim, tick, snapshot) = (
        ms(Phase::Nearest),
        ms(Phase::Meta),
        ms(Phase::World),
        ms(Phase::Anim),
        ms(Phase::Tick),
        ms(Phase::Snapshot),
    );
    log_warn!(
        "net",
        "event handler ({:.0} ms window): {} events costing {:.0} ms, of which {} were packets \
         costing {:.0} ms ({:.2} us each); entity collection: nearest {:.0} ms, meta {:.0} ms, \
         world {:.0} ms, anim {:.0} ms (tick {:.0} ms, snapshot {:.0} ms). The bot thread has one core: whatever this does not \
         account for is azalea's own dispatch around it.",
        window,
        events,
        nanos as f64 / 1e6,
        packets,
        packet_nanos as f64 / 1e6,
        if packets > 0 {
            packet_nanos as f64 / packets as f64 / 1e3
        } else {
            0.0
        },
        nearest,
        meta,
        world,
        anim,
        tick,
        snapshot
    );
}

static SESSION_KILLED: AtomicBool = AtomicBool::new(false);

pub fn session_killed() -> bool {
    SESSION_KILLED.load(Ordering::Relaxed)
}

pub fn clear_session_killed() {
    SESSION_KILLED.store(false, Ordering::Relaxed);
}

pub fn force_disconnect_pending() -> bool {
    FORCE_DISCONNECT.load(Ordering::Relaxed)
}

pub fn clear_force_disconnect() {
    FORCE_DISCONNECT.store(false, Ordering::Relaxed);
}

static LAST_REPORTED_LEAKS: AtomicU64 = AtomicU64::new(0);

fn check_leaked_threads() {
    let leaked = crate::client::bot::leaked_bot_threads() as u64;
    let last = LAST_REPORTED_LEAKS.swap(leaked, Ordering::Relaxed);
    if leaked == last {
        return;
    }
    if leaked > last {
        log_error!(
            "net",
            "{leaked} bot thread(s) are now leaked: disowned by a forced disconnect but never \
             confirmed to have exited, each still holding its own copy of whatever world state \
             it had loaded. This is expected only for a thread that was genuinely wedged on a \
             lock the socket close could not free; it stays leaked until the process exits."
        );
    } else {
        log_info!(
            "net",
            "a previously-leaked bot thread has finished; {leaked} still unaccounted for."
        );
    }
}

fn check_bot_liveness() {
    let now = now_ms();
    let stall_ms = BOT_STALL_AFTER.as_millis() as u64;

    let entered = TICK_ENTERED_MS.load(Ordering::Relaxed);
    if entered != 0 && now.saturating_sub(entered) > stall_ms && !latch(R_TICK_STUCK) {
        log_error!(
            "net",
            "the per-tick handler has been running for {:.0}s and has not returned, so the bot \
             app is wedged inside this crate's own code: no packet will be read and no \
             keep-alive answered until the process is restarted. A lock taken in the handler \
             and not released is the usual cause; `Client::component` hands out a read guard \
             over the whole ECS, and any ECS write taken while one is alive waits forever.",
            (now.saturating_sub(entered)) as f32 / 1000.0
        );
    }

    let frame = RENDER_LAST_FRAME_MS.load(Ordering::Relaxed);
    if frame != 0 && now.saturating_sub(frame) > stall_ms && !latch(R_RENDER_STALLED) {
        log_error!(
            "render",
            "the render thread has not completed a frame in {:.0}s, so the window is frozen \
             rather than slow. Nothing here can recover it: the usual cause is a lock this \
             thread is holding while it waits for another it also wants, and `parking_lot`'s \
             read locks are not recursive, so a second read of a world or lightmap guard \
             already held on this thread is the shape to look for. If no lock-cycle report \
             appears above this line, the deadlock detector found no cycle, and the cause is \
             one of the two things it cannot see: a panic (look for a panic report), or a \
             lock it does not track. Rebuild with `--features deadlock-detection` and \
             reproduce: it names every thread in the cycle and where each is blocked.",
            (now.saturating_sub(frame)) as f32 / 1000.0
        );
    }

    let last = BOT_LAST_UPDATE_MS.load(Ordering::Relaxed);
    if last != 0 && now.saturating_sub(last) > stall_ms && !latch(R_BOT_STALLED) {
        log_error!(
            "net",
            "the bot app has not completed an update in {:.0}s, so it has stopped reading \
             packets and answering keep-alives; the server will time this connection out. The \
             renderer is unaffected, which is why the window still looks alive.",
            (now.saturating_sub(last)) as f32 / 1000.0
        );
    }

    if let Some(kill) = bot_kill_after() {
        let kill_ms = kill.as_millis() as u64;
        let stalled = |stamp: u64| stamp != 0 && now.saturating_sub(stamp) > kill_ms;
        if (stalled(last) || stalled(entered)) && !latch(R_BOT_KILLED) {
            FORCE_DISCONNECT.store(true, Ordering::Relaxed);
            log_error!(
                "net",
                "the bot app has been stalled for over {}s, so this connection is being dropped \
                 and the session cleared. Nothing was reading packets or answering keep-alives \
                 for that whole time, so the server has either already timed it out or is about \
                 to. Set MC_BOT_KILL_SECS=0 to keep the frozen session on screen instead.",
                kill.as_secs()
            );
        }
    }
}

pub fn start_watchdog() {
    #[cfg(not(target_arch = "wasm32"))]
    start_watchdog_thread();
}

#[cfg(not(target_arch = "wasm32"))]
fn start_watchdog_thread() {
    let spawned = std::thread::Builder::new()
        .name("watchdog".into())
        .spawn(|| {
            start_deadlock_detector();
            let mut failed_teardowns: u32 = 0;
            let mut passes: u32 = 0;
            loop {
                std::thread::sleep(WATCHDOG_INTERVAL);
                watchdog();
                passes += 1;
                if passes.is_multiple_of(TRIM_EVERY) {
                    crate::diag::alloc::trim();
                }
                if force_disconnect_pending() {
                    if crate::client::bot::force_disconnect() {
                        SESSION_KILLED.store(true, Ordering::Relaxed);
                        clear_force_disconnect();
                        failed_teardowns = 0;
                        log_warn!(
                            "net",
                            "session cleared and marked dead; the bot thread may still be \
                             working through its backlog, but nothing it does can put this \
                             client back in the world."
                        );
                    } else {
                        failed_teardowns += 1;
                        if failed_teardowns.is_multiple_of(10) {
                            log_error!(
                                "net",
                                "the forced disconnect has not gone through in {} attempts \
                                 ({:.0}s): every one has failed to take a lock the stalled \
                                 thread is holding. The session is still on screen and still \
                                 dead.",
                                failed_teardowns,
                                failed_teardowns as f32 * WATCHDOG_INTERVAL.as_secs_f32()
                            );
                        }
                    }
                }
            }
        });
    if spawned.is_err() {
        log_error!(
            "net",
            "the watchdog thread could not be started, so a wedged bot or render thread will \
             now fail silently: no stall report, and no automatic disconnect."
        );
    }
}

const WATCHDOG_INTERVAL: Duration = Duration::from_millis(500);

const TRIM_EVERY: u32 = 16;

#[cfg(feature = "deadlock-detection")]
fn start_deadlock_detector() {
    let _ = std::thread::Builder::new()
        .name("deadlock".into())
        .spawn(|| {
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let deadlocks = parking_lot::deadlock::check_deadlock();
                if deadlocks.is_empty() {
                    continue;
                }
                log_error!(
                    "net",
                    "{} deadlock cycle(s) detected; each thread below is waiting on a lock \
                     another one in the same cycle holds.",
                    deadlocks.len()
                );
                for (i, threads) in deadlocks.iter().enumerate() {
                    for t in threads {
                        log_error!(
                            "net",
                            "cycle {} thread {:?}:\n{:?}",
                            i,
                            t.thread_id(),
                            t.backtrace()
                        );
                    }
                }
            }
        });
}

#[cfg(not(feature = "deadlock-detection"))]
fn start_deadlock_detector() {}

pub fn watchdog() {
    check_bot_liveness();
    check_leaked_threads();
    report_event_load();

    if LIGHT_DEAD.load(Ordering::Relaxed) && !latch(R_LIGHT_DEAD) {
        log_error!(
            "light",
            "the light engine thread is dead, so no chunk will ever be meshed; \
             the panic above is the cause and this session cannot recover"
        );
    }

    let (connected_for, first_chunk_for, spawned) = {
        let Ok(s) = SESSION.try_lock() else { return };
        let Some(at) = s.connected_at else { return };
        (
            at.elapsed(),
            s.first_chunk_at.map(|t| t.elapsed()),
            s.spawned,
        )
    };

    let c = counts();

    if !spawned && connected_for >= NOT_SPAWNED_AFTER && !latch(R_NOT_SPAWNED) {
        let (center, player) = view_pair();
        if c.chunk_packets_seen == 0 {
            log_warn!(
                "net",
                "{:.0}s after joining, the server has not sent a single chunk packet \
                 ({} chunk-cache-centre packets seen). Nothing has been dropped and \
                 nothing has stalled: the world is not being streamed at all, so the \
                 login and configuration sequence is where to look, not the renderer.",
                connected_for.as_secs_f32(),
                c.cache_center_moves
            );
        } else if c.chunks_received == 0 {
            log_warn!(
                "view",
                "{:.0}s after joining, the server sent {} chunk packets and azalea \
                 kept none of them: {} fell outside its window, which is centred on \
                 {},{} with radius {} while the player is in chunk {},{}. The window \
                 is the problem, not the server and not the renderer.",
                connected_for.as_secs_f32(),
                c.chunk_packets_seen,
                c.chunk_packets_out_of_window,
                center.0,
                center.1,
                window_radius()
                    .map(|r| r.to_string())
                    .unwrap_or_else(|| "?".to_string()),
                player.0,
                player.1
            );
        } else {
            log_warn!(
                "net",
                "{:.0}s after joining, the server sent {} chunk packets, azalea kept \
                 {} and dropped {}, but it has not reported the player's own chunk \
                 ({},{}) loaded, so no spawn event has fired and the client is still \
                 on the menu. The window is centred {},{} with radius {}. A column is \
                 only kept if it is inside that window, and the player's own column \
                 has to be one of them.",
                connected_for.as_secs_f32(),
                c.chunk_packets_seen,
                c.chunks_received,
                c.chunk_packets_out_of_window,
                player.0,
                player.1,
                center.0,
                center.1,
                window_radius()
                    .map(|r| r.to_string())
                    .unwrap_or_else(|| "?".to_string())
            );
        }
    }

    if let Some(since_first) = first_chunk_for
        && since_first >= NO_MESH_AFTER
        && c.sections_uploaded == 0
        && !latch(R_NO_MESHES)
    {
        log_warn!(
            "mesh",
            "{} chunk columns arrived {:.0}s ago and not one section has reached the \
             GPU: {} columns lit, {} mesh jobs queued, {} sections meshed, {} worker \
             panics, {} light jobs dropped. The first of those that is zero is the \
             stage that stopped.",
            c.chunks_received,
            since_first.as_secs_f32(),
            c.columns_lit,
            c.mesh_jobs,
            c.sections_meshed,
            c.mesh_panics,
            c.light_jobs_dropped
        );
    }

    if c.chunks_dropped > 0 && c.chunks_received > 0 && !latch(R_NO_CHUNKS) {
        let (center, player) = view_pair();
        log_warn!(
            "view",
            "azalea has dropped {} chunk columns for falling outside its window \
             (centred {},{}, player in {},{}); those columns will not be sent again \
             and will read as air until the server resends them.",
            c.chunks_dropped,
            center.0,
            center.1,
            player.0,
            player.1
        );
    }
}

fn latch(which: usize) -> bool {
    REPORTED[which].swap(true, Ordering::Relaxed)
}
