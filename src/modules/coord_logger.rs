use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{Mutex, PoisonError};

use azalea_protocol::packets::game::ClientboundGamePacket;

use super::registry::{Id, coord_logger as setting};
use super::{Edge, Phase, store};
use crate::lang::{Key, tr, trf};
use crate::renderer::world_draw::{self, Node};
use crate::session::SharedMutex;

static EDGE: Edge = Edge::new(Id::CoordLogger);

const OWNER: &str = "coord_logger";

const CLAMP: f64 = 32.0;
const CLAMP_SLACK: f64 = 4.0;
const NEAR: f64 = 4.0;

const LINE_LENGTH: f64 = 256.0;
const LINE_ARGB: u32 = 0xFF_FF_D0_40;

static LINES: Mutex<Vec<(Node, u64)>> = Mutex::new(Vec::new());
static NOW: AtomicU64 = AtomicU64::new(0);
static ADDED: AtomicBool = AtomicBool::new(false);
static SESSION: AtomicU32 = AtomicU32::new(0);

fn event_name(id: u32) -> Option<(&'static str, bool)> {
    let s = store();
    Some(match id {
        1023 => (tr!("modmsg.coord_logger.wither"), s.flag(setting::WITHER)),
        1028 => (tr!("modmsg.coord_logger.dragon"), s.flag(setting::DRAGON)),
        1038 => (
            tr!("modmsg.coord_logger.end_portal"),
            s.flag(setting::END_PORTAL),
        ),
        _ => return None,
    })
}

fn compass(bearing: f64) -> &'static str {
    const NAMES: [Key; 8] = [
        Key::of("modmsg.coord_logger.compass.n"),
        Key::of("modmsg.coord_logger.compass.ne"),
        Key::of("modmsg.coord_logger.compass.e"),
        Key::of("modmsg.coord_logger.compass.se"),
        Key::of("modmsg.coord_logger.compass.s"),
        Key::of("modmsg.coord_logger.compass.sw"),
        Key::of("modmsg.coord_logger.compass.w"),
        Key::of("modmsg.coord_logger.compass.nw"),
    ];
    NAMES[((bearing / 45.0).round() as usize) % 8].get()
}

pub fn packet(shared: &SharedMutex, packet: &ClientboundGamePacket) {
    let ClientboundGamePacket::LevelEvent(p) = packet else {
        return;
    };
    if !store().enabled(Id::CoordLogger) {
        return;
    }
    let Some((name, on)) = event_name(p.event_type) else {
        return;
    };
    if !on {
        return;
    }
    let mut s = shared.lock().unwrap();
    let me = s.session.player_pos;
    let at = [p.pos.x, p.pos.y, p.pos.z].map(|v| f64::from(v) + 0.5);
    let d = [at[0] - me[0], at[1] - me[1], at[2] - me[2]];
    let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();

    let exact = trf!("modmsg.coord_logger.at", name, p.pos.x, p.pos.y, p.pos.z);
    let text = if !p.global_event {
        exact
    } else if dist < NEAR {
        trf!("modmsg.coord_logger.other_dimension", exact)
    } else if (dist - CLAMP).abs() <= CLAMP_SLACK {
        let bearing = d[0].atan2(-d[2]).to_degrees().rem_euclid(360.0);
        let yaw = (-d[0]).atan2(d[2]).to_degrees();
        if store().flag(setting::LINE) {
            let unit = d.map(|v| v / dist);
            let from = [me[0], me[1] + 1.62, me[2]];
            let to = [0, 1, 2].map(|i| from[i] + unit[i] * LINE_LENGTH);
            let until = NOW.load(Relaxed) + store().num(setting::LINE_SECONDS) as u64 * 20;
            LINES
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((Node::line(from, to, 0.15, LINE_ARGB, true), until));
            ADDED.store(true, Relaxed);
        }
        let (bearing_text, yaw_text) = (format!("{bearing:.0}"), format!("{yaw:.1}"));
        let direction = trf!(
            "modmsg.coord_logger.bearing",
            bearing_text,
            compass(bearing),
            yaw_text
        );
        if dist < CLAMP {
            trf!("modmsg.coord_logger.further", exact, direction)
        } else {
            trf!("modmsg.coord_logger.far", name, direction)
        }
    } else {
        exact
    };
    super::chat_math::show_local_colored(&mut s.session, &text, crate::text::YELLOW);
}

pub fn tick() {
    match EDGE.poll(store()) {
        Phase::Off => return,
        Phase::Stopped => {
            LINES.lock().unwrap_or_else(PoisonError::into_inner).clear();
            world_draw::clear(OWNER);
            return;
        }
        Phase::Started | Phase::Running => {}
    }
    let now = NOW.fetch_add(1, Relaxed) + 1;
    let mut lines = LINES.lock().unwrap_or_else(PoisonError::into_inner);
    let before = lines.len();
    let session = super::basefinder::session();
    if SESSION.swap(session, Relaxed) != session {
        lines.clear();
    }
    lines.retain(|&(_, until)| until > now);
    if ADDED.swap(false, Relaxed) || lines.len() != before {
        world_draw::set(OWNER, lines.iter().map(|&(n, _)| n).collect());
    }
}
