use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{Mutex, OnceLock};

use azalea::block::{BlockState, BlockTrait};
use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_registry::builtin::SoundEvent;
use azalea_registry::{Holder, Registry};

use super::esp::{Found, Slot, table as esp_table};
use super::list::{BitList, Built, Entry, NO_ROW, Preset};
use super::registry::{Id, sound_esp as setting};
use super::seed_xray::Tick;
use super::{Edge, Phase, store};
use crate::items::kinetic::{event_name, sound_name};
use crate::lang::Key;

static EDGE: Edge = Edge::new(Id::SoundEsp);

static GROUPS: [Key; 3] = [
    Key::of("lists.sounds.group.blocks"),
    Key::of("lists.sounds.group.entities"),
    Key::of("lists.sounds.group.other"),
];

const COLOR: u32 = 0xFF_55_55;

static UNNATURAL: &[&str] = &[
    "block.chest.",
    "block.ender_chest.",
    "block.barrel.",
    "block.shulker_box.",
    "block.copper_chest.",
    "door.open",
    "door.close",
    "trapdoor.",
    "fence_gate.",
    "block.piston.",
    "block.dispenser.",
    "block.anvil.",
    "block.furnace.",
    "block.blastfurnace.",
    "block.smoker.",
    "block.brewing_stand.",
    "block.enchantment_table.",
    "block.note_block.",
    "button.click",
    "pressure_plate.click",
    "block.lever.",
    "block.comparator.",
    "block.beacon.",
    "block.conduit.activate",
    "block.respawn_anchor.",
    "block.portal.",
    "block.end_portal.",
    "block.end_portal_frame.fill",
    "item.book.page_turn",
    "entity.generic.explode",
    "block.grindstone.",
    "block.smithing_table.",
    "block.crafter.",
    "block.redstone_torch.burnout",
    "block.lodestone.",
    "block.chiseled_bookshelf.",
    "block.decorated_pot.insert",
    "block.copper_bulb.",
    "entity.tnt.primed",
    "entity.firework_rocket.",
    "entity.item_frame.",
    "entity.glow_item_frame.",
    "entity.armor_stand.",
    "entity.painting.",
    "entity.experience_orb.pickup",
    "entity.minecart.",
];

fn build() -> Built {
    let mut entries = Vec::with_capacity(2000);
    let mut index = Vec::with_capacity(2000);
    for id in 0u32.. {
        let Some(event) = <SoundEvent as Registry>::from_u32(id) else {
            break;
        };
        let name = sound_name(event);
        let group = if name.starts_with("block.") {
            0
        } else if name.starts_with("entity.") {
            1
        } else {
            2
        };
        index.push((id, entries.len() as u32));
        entries.push(Entry {
            group,
            label: Cow::Borrowed(name),
            shown: Key::NONE,
            on: UNNATURAL.iter().any(|p| name.contains(p)),
            color: COLOR,
        });
    }
    (entries, index)
}

pub static SOUNDS: BitList = BitList::new(
    "sound_esp",
    Key::of("lists.sounds.title"),
    214.0,
    &GROUPS,
    true,
    &[Preset::Defaults, Preset::Clear],
    build,
);

static NOW: AtomicU64 = AtomicU64::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static MARKS: Mutex<Option<HashMap<[i32; 3], (u16, u64)>>> = Mutex::new(None);

const MAX_MARKS: usize = 2048;
const NEW_PER_TICK: u32 = 64;
static ADDED: AtomicU32 = AtomicU32::new(0);

static LEVEL_EVENTS: &[(u32, &str)] = &[
    (1000, "block.dispenser.dispense"),
    (1001, "block.dispenser.fail"),
    (1002, "block.dispenser.launch"),
    (1004, "entity.firework_rocket.shoot"),
    (1029, "block.anvil.destroy"),
    (1030, "block.anvil.use"),
    (1031, "block.anvil.land"),
    (1035, "block.brewing_stand.brew"),
    (1042, "block.grindstone.use"),
    (1043, "item.book.page_turn"),
    (1044, "block.smithing_table.use"),
    (1049, "block.crafter.craft"),
    (1050, "block.crafter.fail"),
    (1502, "block.redstone_torch.burnout"),
    (1503, "block.end_portal_frame.fill"),
];

fn by_name() -> &'static HashMap<&'static str, u16> {
    static BY_NAME: OnceLock<HashMap<&'static str, u16>> = OnceLock::new();
    BY_NAME.get_or_init(|| {
        let mut m = HashMap::with_capacity(SOUNDS.count());
        for id in 0u32.. {
            let Some(event) = <SoundEvent as Registry>::from_u32(id) else {
                break;
            };
            if let Some(&row) = SOUNDS.index().get(id as usize) {
                m.insert(sound_name(event), row);
            }
        }
        m
    })
}

fn by_state() -> &'static [u16] {
    static BY_STATE: OnceLock<Vec<u16>> = OnceLock::new();
    BY_STATE.get_or_init(|| {
        let names = by_name();
        let row = |n: &str| names.get(n).copied().unwrap_or(NO_ROW);
        let (furnace, blast, smoker, portal) = (
            row("block.furnace.fire_crackle"),
            row("block.blastfurnace.fire_crackle"),
            row("block.smoker.smoke"),
            row("block.portal.ambient"),
        );
        (0..=BlockState::MAX_STATE)
            .map(|id| {
                let Ok(state) = BlockState::try_from(id) else {
                    return NO_ROW;
                };
                const PREFIX: usize = "minecraft:".len();
                let name = &state.as_block_kind().to_str()[PREFIX..];
                if name == "nether_portal" {
                    return portal;
                }
                let lit = || Box::<dyn BlockTrait>::from(state).get_property("lit") == Some("true");
                match name {
                    "furnace" if lit() => furnace,
                    "blast_furnace" if lit() => blast,
                    "smoker" if lit() => smoker,
                    _ => NO_ROW,
                }
            })
            .collect()
    })
}

pub fn packet(packet: &ClientboundGamePacket) {
    if !matches!(
        packet,
        ClientboundGamePacket::Sound(_)
            | ClientboundGamePacket::LevelEvent(_)
            | ClientboundGamePacket::BlockUpdate(_)
            | ClientboundGamePacket::SectionBlocksUpdate(_)
            | ClientboundGamePacket::Explode(_)
    ) || !store().enabled(Id::SoundEsp)
    {
        return;
    }
    match packet {
        ClientboundGamePacket::Sound(p) => {
            mark([p.x, p.y, p.z].map(|v| v.div_euclid(8)), row_of(&p.sound));
        }
        ClientboundGamePacket::Explode(p) => {
            let at = [p.center.x, p.center.y, p.center.z].map(|v| v.floor() as i32);
            mark(at, row_of(&p.explosion_sound));
        }
        ClientboundGamePacket::LevelEvent(p) => {
            let Some((_, name)) = LEVEL_EVENTS.iter().find(|(id, _)| *id == p.event_type) else {
                return;
            };
            mark([p.pos.x, p.pos.y, p.pos.z], by_name().get(name).copied());
        }
        ClientboundGamePacket::BlockUpdate(p) => {
            let row = by_state()[p.block_state.id() as usize];
            mark([p.pos.x, p.pos.y, p.pos.z], Some(row));
        }
        ClientboundGamePacket::SectionBlocksUpdate(p) => {
            let table = by_state();
            let base = [p.section_pos.x, p.section_pos.y, p.section_pos.z].map(|v| v * 16);
            for s in &p.states {
                let row = table[s.state.id() as usize];
                if row != NO_ROW {
                    let at = [
                        base[0] + s.pos.x as i32,
                        base[1] + s.pos.y as i32,
                        base[2] + s.pos.z as i32,
                    ];
                    mark(at, Some(row));
                }
            }
        }
        _ => {}
    }
}

fn row_of(sound: &Holder<SoundEvent, azalea_core::sound::CustomSound>) -> Option<u16> {
    match sound {
        Holder::Reference(event) => SOUNDS.index().get(event.to_u32() as usize).copied(),
        Holder::Direct(_) => by_name().get(&*event_name(sound)).copied(),
    }
}

fn mark(at: [i32; 3], row: Option<u16>) {
    let Some(row) = row.filter(|&r| r != NO_ROW && SOUNDS.enabled(r as usize)) else {
        return;
    };
    let heard = NOW.load(Relaxed);
    let mut guard = MARKS.lock().unwrap();
    let marks = guard.get_or_insert_default();
    if let Some(m) = marks.get_mut(&at) {
        if m.0 != row {
            GENERATION.fetch_add(1, Relaxed);
        }
        *m = (row, heard);
        return;
    }
    if marks.len() >= MAX_MARKS || ADDED.fetch_add(1, Relaxed) >= NEW_PER_TICK {
        return;
    }
    marks.insert(at, (row, heard));
    GENERATION.fetch_add(1, Relaxed);
}

pub fn spawned(dimension: &str) {
    let mut last = DIMENSION.lock().unwrap();
    if *last != dimension {
        *last = dimension.to_owned();
        drop(last);
        clear();
    }
}
static DIMENSION: Mutex<String> = Mutex::new(String::new());

pub fn disconnected() {
    if let Ok(mut d) = DIMENSION.try_lock() {
        d.clear();
    }
    if let Ok(mut m) = MARKS.try_lock() {
        *m = None;
    }
    GENERATION.fetch_add(1, Relaxed);
}

pub fn clear() {
    if MARKS.lock().unwrap().take().is_some_and(|m| !m.is_empty()) {
        GENERATION.fetch_add(1, Relaxed);
    }
}

pub fn tick() -> Tick {
    let now = NOW.fetch_add(1, Relaxed) + 1;
    ADDED.store(0, Relaxed);
    let phase = EDGE.poll(store());
    match phase {
        Phase::Off => return Tick::default(),
        Phase::Stopped => {
            clear();
            return Tick {
                running: false,
                edged: true,
                key: 0,
            };
        }
        Phase::Started | Phase::Running => {}
    }
    if let Some(m) = MARKS.lock().unwrap().as_mut() {
        let before = m.len();
        let life = (store().num(setting::DURATION).max(1.0) * 20.0) as u64;
        m.retain(|_, (_, heard)| now - *heard < life);
        if m.len() != before {
            GENERATION.fetch_add(1, Relaxed);
        }
    }
    let s = store();
    let key = [
        GENERATION.load(Relaxed),
        SOUNDS.generation() as u64,
        s.choice(setting::SHAPE) as u64,
        s.num(setting::FILL).to_bits() as u64,
    ]
    .into_iter()
    .fold(0xcbf2_9ce4_8422_2325_u64, |h, v| {
        (h ^ v).wrapping_mul(0x100_0000_01b3)
    });
    Tick {
        running: true,
        edged: phase == Phase::Started,
        key,
    }
}

pub fn append(cap: usize, boxes: &mut Vec<Found>, slots: &mut Vec<Slot>) {
    let guard = MARKS.lock().unwrap();
    let Some(marks) = guard.as_ref() else { return };
    let s = store();
    let (alpha, lines, sides) = esp_table::look(s.choice(setting::SHAPE), s.num(setting::FILL));
    for (at, &(row, _)) in marks {
        if boxes.len() >= cap {
            return;
        }
        let row = row as usize;
        if !SOUNDS.enabled(row) {
            continue;
        }
        let slot = Slot {
            rgb: SOUNDS.color(row),
            alpha,
            lines,
            sides,
            range2: u32::MAX,
            exposed: false,
            fade: false,
        };
        let slot = slots.iter().position(|s| *s == slot).unwrap_or_else(|| {
            slots.push(slot);
            slots.len() - 1
        }) as u16;
        boxes.push(Found {
            x: at[0],
            y: at[1],
            z: at[2],
            neigh: 0,
            slot,
        });
    }
}
