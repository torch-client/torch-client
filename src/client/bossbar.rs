use std::sync::{Arc, Mutex, OnceLock};

use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_protocol::packets::game::c_boss_event::{
    BossBarColor, BossBarOverlay, Operation, Style as BossStyle,
};

use crate::client::chat_text::to_spans;
use crate::platform::time::Instant;
use crate::session::SharedMutex;
use crate::text::Span;

#[derive(Clone)]
pub struct BossBar {
    pub name: Vec<Span>,
    pub from: f32,
    pub target: f32,
    pub set_at: Instant,
    pub color: u8,
    pub overlay: u8,
    #[allow(
        dead_code,
        reason = "published boss-bar state the renderer does not consume yet"
    )]
    pub darken: bool,
    #[allow(
        dead_code,
        reason = "published boss-bar state the renderer does not consume yet"
    )]
    pub fog: bool,
}

impl BossBar {
    pub fn progress(&self) -> f32 {
        let t = (self.set_at.elapsed().as_secs_f32() / 0.1).clamp(0.0, 1.0);
        (self.from + (self.target - self.from) * t).clamp(0.0, 1.0)
    }
}

struct Bar {
    id: u128,
    bar: BossBar,
}

static STATE: OnceLock<Mutex<Vec<Bar>>> = OnceLock::new();

fn state() -> &'static Mutex<Vec<Bar>> {
    STATE.get_or_init(Default::default)
}

pub(crate) fn reset() {
    state().lock().unwrap().clear();
}

pub(crate) fn packet(shared: &Arc<SharedMutex>, packet: &ClientboundGamePacket) {
    let ClientboundGamePacket::BossEvent(p) = packet else {
        return;
    };
    let id = p.id.as_u128();
    let mut bars = state().lock().unwrap();

    match &p.operation {
        Operation::Add(add) => {
            let bar = BossBar {
                name: to_spans(&add.name),
                from: add.progress.clamp(0.0, 1.0),
                target: add.progress.clamp(0.0, 1.0),
                set_at: Instant::now(),
                color: color_index(add.style.color),
                overlay: overlay_index(add.style.overlay),
                darken: add.properties.darken_screen,
                fog: add.properties.create_world_fog,
            };
            match bars.iter_mut().find(|b| b.id == id) {
                Some(existing) => existing.bar = bar,
                None => bars.push(Bar { id, bar }),
            }
        }
        Operation::Remove => bars.retain(|b| b.id != id),
        Operation::UpdateProgress(progress) => {
            if let Some(b) = bars.iter_mut().find(|b| b.id == id) {
                b.bar.from = b.bar.progress();
                b.bar.target = progress.clamp(0.0, 1.0);
                b.bar.set_at = Instant::now();
            }
        }
        Operation::UpdateName(name) => {
            if let Some(b) = bars.iter_mut().find(|b| b.id == id) {
                b.bar.name = to_spans(name);
            }
        }
        Operation::UpdateStyle(BossStyle { color, overlay }) => {
            if let Some(b) = bars.iter_mut().find(|b| b.id == id) {
                b.bar.color = color_index(*color);
                b.bar.overlay = overlay_index(*overlay);
            }
        }
        Operation::UpdateProperties(properties) => {
            if let Some(b) = bars.iter_mut().find(|b| b.id == id) {
                b.bar.darken = properties.darken_screen;
                b.bar.fog = properties.create_world_fog;
            }
        }
    }

    let published: Vec<BossBar> = bars.iter().map(|b| b.bar.clone()).collect();
    drop(bars);
    shared.lock().unwrap().session.boss_bars = Arc::new(published);
}

fn color_index(color: BossBarColor) -> u8 {
    color as u8
}

fn overlay_index(overlay: BossBarOverlay) -> u8 {
    overlay as u8
}
