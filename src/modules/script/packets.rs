use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, LazyLock};

use azalea_protocol::packets::ProtocolPacket;
use azalea_protocol::packets::game::{ClientboundGamePacket, ServerboundGamePacket};
use mlua::{Error, Lua, Table};

use super::api::{Class, Ctx};
use super::game::{game_fn, waypoint_fields};
use super::perm;
use crate::session::{WaypointInfo, WaypointKey};
use crate::util::wire::{Reader, write_varint};

const WORDS: usize = 4;
const _: () = assert!(ClientboundGamePacket::NAMES.len() <= WORDS * 64);

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(super) struct PacketSet([u64; WORDS]);

impl PacketSet {
    pub fn has(&self, id: u32) -> bool {
        let i = id as usize;
        i < WORDS * 64 && self.0[i >> 6] >> (i & 63) & 1 != 0
    }

    pub fn is_empty(&self) -> bool {
        self.0 == [0; WORDS]
    }

    pub fn union(mut self, other: PacketSet) -> PacketSet {
        for (a, b) in self.0.iter_mut().zip(other.0) {
            *a |= b;
        }
        self
    }

    fn insert(&mut self, id: u32) {
        let i = id as usize;
        self.0[i >> 6] |= 1 << (i & 63);
    }
}

static MASK: [AtomicU64; WORDS] = [const { AtomicU64::new(0) }; WORDS];

pub(super) fn store_mask(set: PacketSet) {
    for (slot, word) in MASK.iter().zip(set.0) {
        slot.store(word, Relaxed);
    }
}

fn fold(name: &str) -> String {
    let bare = name.strip_prefix("minecraft:").unwrap_or(name);
    bare.chars()
        .filter(|&c| c != '_')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn find(names: &[&str], name: &str) -> Option<u32> {
    let want = fold(name);
    names.iter().position(|n| fold(n) == want).map(|i| i as u32)
}

fn near(names: &[&str], name: &str) -> String {
    let want = fold(name);
    (names.iter())
        .map(|n| (super::api::distance(&want, &fold(n)), n))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d)
        .map_or_else(String::new, |(_, n)| format!(" (did you mean `{n}`?)"))
}

fn variant(resource: &str) -> String {
    let mut out = String::with_capacity(resource.len());
    let mut up = true;
    for c in resource.chars() {
        if c == '_' {
            up = true;
        } else {
            out.push(if up { c.to_ascii_uppercase() } else { c });
            up = false;
        }
    }
    out
}

static EVENT_NAMES: LazyLock<Box<[[String; 2]]>> = LazyLock::new(|| {
    (ClientboundGamePacket::NAMES.iter())
        .map(|resource| [variant(resource), format!("minecraft:{resource}")])
        .collect()
});

pub(super) fn packet_set(names: &[String]) -> Result<PacketSet, String> {
    let all = ClientboundGamePacket::NAMES;
    let mut set = PacketSet::default();
    for n in names {
        match find(all, n) {
            Some(id) => set.insert(id),
            None => {
                return Err(format!(
                    "packets: `{n}` is not a clientbound game packet{}; torch.net.names(\"clientbound\") lists them",
                    near(all, n)
                ));
            }
        }
    }
    Ok(set)
}

const DEBUG_CAP: usize = 16 << 10;

struct Capped(String);

impl std::fmt::Write for Capped {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let room = DEBUG_CAP - self.0.len();
        if s.len() <= room {
            self.0.push_str(s);
            return Ok(());
        }
        let mut cut = room;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        self.0.push_str(&s[..cut]);
        Err(std::fmt::Error)
    }
}

type SoundHolder =
    azalea_registry::Holder<azalea_registry::builtin::SoundEvent, azalea_core::sound::CustomSound>;

const SOUND_SOURCES: [&str; 11] = [
    "master", "music", "record", "weather", "block", "hostile", "neutral", "player", "ambient",
    "voice", "ui",
];

pub(super) enum Fields {
    Title(String),
    Subtitle(String),
    ActionBar(String),
    Health {
        health: f32,
        food: u32,
        saturation: f32,
    },
    Experience {
        progress: f32,
        level: u32,
        total: u32,
    },
    Waypoint {
        op: &'static str,
        key: WaypointKey,
        info: Option<WaypointInfo>,
    },
    Sound {
        sound: SoundHolder,
        source: &'static str,
        pos: [f64; 3],
        volume: f32,
        pitch: f32,
    },
    SoundEntity {
        sound: SoundHolder,
        source: &'static str,
        entity: i32,
        volume: f32,
        pitch: f32,
    },
    LevelEvent {
        event: u32,
        pos: [i32; 3],
        data: u32,
        global: bool,
    },
    Debug,
}

pub(super) struct Packet {
    pub id: u32,
    bytes: Vec<u8>,
    fields: Fields,
}

impl Packet {
    pub fn class(&self) -> Class {
        match self.fields {
            Fields::Debug => Class::PacketDebug,
            _ => Class::Plain,
        }
    }

    pub fn fill(&self, lua: &Lua, e: &Table) -> mlua::Result<()> {
        if let Some([name, resource]) = EVENT_NAMES.get(self.id as usize) {
            e.raw_set("name", name.as_str())?;
            e.raw_set("resource", resource.as_str())?;
        }
        e.raw_set("id", self.id)?;
        e.raw_set("bytes", lua.create_string(&self.bytes)?)?;
        match &self.fields {
            Fields::Title(t) | Fields::Subtitle(t) | Fields::ActionBar(t) => {
                e.raw_set("text", t.as_str())
            }
            Fields::Health {
                health,
                food,
                saturation,
            } => {
                e.raw_set("health", *health as f64)?;
                e.raw_set("food", *food)?;
                e.raw_set("saturation", *saturation as f64)
            }
            Fields::Experience {
                progress,
                level,
                total,
            } => {
                e.raw_set("progress", *progress as f64)?;
                e.raw_set("level", *level)?;
                e.raw_set("total", *total)
            }
            Fields::Waypoint { op, key, info } => {
                e.raw_set("op", *op)?;
                waypoint_fields(e, key, info.as_ref())
            }
            Fields::Sound {
                sound,
                source,
                pos,
                volume,
                pitch,
            } => {
                e.raw_set("sound", crate::items::kinetic::event_name(sound))?;
                e.raw_set("source", *source)?;
                e.raw_set("x", pos[0])?;
                e.raw_set("y", pos[1])?;
                e.raw_set("z", pos[2])?;
                e.raw_set("volume", *volume as f64)?;
                e.raw_set("pitch", *pitch as f64)
            }
            Fields::SoundEntity {
                sound,
                source,
                entity,
                volume,
                pitch,
            } => {
                e.raw_set("sound", crate::items::kinetic::event_name(sound))?;
                e.raw_set("source", *source)?;
                e.raw_set("entity", *entity)?;
                e.raw_set("volume", *volume as f64)?;
                e.raw_set("pitch", *pitch as f64)
            }
            Fields::LevelEvent {
                event,
                pos,
                data,
                global,
            } => {
                e.raw_set("event", *event)?;
                e.raw_set("x", pos[0])?;
                e.raw_set("y", pos[1])?;
                e.raw_set("z", pos[2])?;
                e.raw_set("data", *data)?;
                e.raw_set("global", *global)
            }
            Fields::Debug => Ok(()),
        }
    }
}

fn debug_text(id: u32, bytes: &[u8]) -> (String, bool) {
    let mut cursor = std::io::Cursor::new(bytes);
    match ClientboundGamePacket::read(id, &mut cursor) {
        Ok(packet) => {
            let mut out = Capped(String::new());
            let cut = write!(out, "{packet:?}").is_err();
            (out.0, cut)
        }
        Err(e) => (
            format!("(azalea could not read this packet back: {e})"),
            false,
        ),
    }
}

pub(super) fn debug_index(lua: &Lua, base: Table) -> mlua::Result<mlua::Function> {
    lua.create_function(move |_, (e, key): (Table, mlua::Value)| {
        let wanted = matches!(&key, mlua::Value::String(s) if *s == "debug" || *s == "truncated");
        if !wanted {
            return base.raw_get::<mlua::Value>(key);
        }
        let id: u32 = e.raw_get("id")?;
        let bytes: mlua::String = e.raw_get("bytes")?;
        let (text, cut) = debug_text(id, &bytes.as_bytes());
        e.set_readonly(false);
        let written = e
            .raw_set("debug", text)
            .and_then(|()| e.raw_set("truncated", cut));
        e.set_readonly(true);
        written?;
        e.raw_get::<mlua::Value>(key)
    })
}

pub(super) fn convert(packet: &ClientboundGamePacket) -> Option<Packet> {
    use ClientboundGamePacket as P;
    let id = packet.id();
    let word = MASK.get(id as usize >> 6)?.load(Relaxed);
    if word >> (id & 63) & 1 == 0 {
        return None;
    }
    let plain = |t: &azalea_chat::FormattedText| -> String {
        crate::client::chat_text::to_spans(t)
            .iter()
            .map(|s| s.text.as_str())
            .collect()
    };
    let fields = match packet {
        P::SetTitleText(p) => Fields::Title(plain(&p.text)),
        P::SetSubtitleText(p) => Fields::Subtitle(plain(&p.text)),
        P::SetActionBarText(p) => Fields::ActionBar(plain(&p.text)),
        P::SetHealth(p) => Fields::Health {
            health: p.health,
            food: p.food,
            saturation: p.saturation,
        },
        P::SetExperience(p) => Fields::Experience {
            progress: p.experience_progress,
            level: p.experience_level,
            total: p.total_experience,
        },
        P::Waypoint(p) => {
            use azalea_protocol::packets::game::c_waypoint::WaypointOperation as Op;
            let (key, info) = crate::client::packets::waypoint_parts(p);
            let op = match p.operation {
                Op::Track => "track",
                Op::Untrack => "untrack",
                Op::Update => "update",
            };
            Fields::Waypoint { op, key, info }
        }
        P::Sound(p) => Fields::Sound {
            sound: p.sound.clone(),
            source: SOUND_SOURCES[p.source as usize],
            pos: [p.x, p.y, p.z].map(|v| f64::from(v) / 8.0),
            volume: p.volume,
            pitch: p.pitch,
        },
        P::SoundEntity(p) => Fields::SoundEntity {
            sound: p.sound.clone(),
            source: SOUND_SOURCES[p.source as usize],
            entity: p.id.0,
            volume: p.volume,
            pitch: p.pitch,
        },
        P::LevelEvent(p) => Fields::LevelEvent {
            event: p.event_type,
            pos: [p.pos.x, p.pos.y, p.pos.z],
            data: p.data,
            global: p.global_event,
        },
        _ => Fields::Debug,
    };
    let mut bytes = Vec::new();
    let _ = packet.write(&mut bytes);
    Some(Packet { id, bytes, fields })
}

const REFUSED: [&str; 2] = ["configuration_acknowledged", "chat_session_update"];

const SEND_CAP: usize = 8 << 20;

pub(super) fn table(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set(
        "send",
        game_fn(lua, ctx, "torch.net.send", |_, ctx, (name, body): (String, mlua::String)| {
            let what = "torch.net.send";
            let (_, mod_id) = ctx.caller(what)?;
            if !perm::declared(ctx, perm::PACKETS, mod_id) {
                return Err(perm::undeclared(what, perm::PACKETS));
            }
            let names = ServerboundGamePacket::NAMES;
            let Some(id) = find(names, &name) else {
                return Err(Error::runtime(format!(
                    "{what}: `{name}` is not a serverbound game packet{}; torch.net.names(\"serverbound\") lists them",
                    near(names, &name)
                )));
            };
            let resource = names[id as usize];
            if REFUSED.contains(&resource) {
                return Err(Error::runtime(format!("{what}: `{resource}` would break the connection")));
            }
            let body = body.as_bytes();
            if body.len() > SEND_CAP {
                return Err(Error::runtime(format!("{what}: more than {} MiB", SEND_CAP >> 20)));
            }
            if !perm::granted(perm::PACKETS, mod_id) {
                return Ok(false);
            }
            let mut cursor = std::io::Cursor::new(&body[..]);
            let packet = ServerboundGamePacket::read(id, &mut cursor)
                .map_err(|e| Error::runtime(format!("{what}: `{resource}` did not parse: {e}")))?;
            let left = body.len() - cursor.position() as usize;
            if left != 0 {
                return Err(Error::runtime(format!("{what}: `{resource}` has {left} bytes past its end")));
            }
            ctx.bot(what)?.write_packet(packet);
            Ok(true)
        })?,
    )?;
    let c = ctx.clone();
    t.raw_set(
        "allowed",
        lua.create_function(move |_, ()| {
            let (_, mod_id) = c.caller("torch.net.allowed")?;
            Ok(perm::declared(&c, perm::PACKETS, mod_id) && perm::granted(perm::PACKETS, mod_id))
        })?,
    )?;
    t.raw_set(
        "names",
        lua.create_function(|lua, side: String| {
            let names = match side.as_str() {
                "clientbound" => ClientboundGamePacket::NAMES,
                "serverbound" => ServerboundGamePacket::NAMES,
                other => {
                    return Err(Error::runtime(format!(
                        "torch.net.names: `{other}` is neither \"clientbound\" nor \"serverbound\""
                    )));
                }
            };
            lua.create_sequence_from(names.iter().copied())
        })?,
    )?;
    t.raw_set(
        "varint",
        lua.create_function(|lua, v: f64| {
            if !(v.is_finite()
                && v.fract() == 0.0
                && v >= f64::from(i32::MIN)
                && v <= f64::from(u32::MAX))
            {
                return Err(Error::runtime(format!(
                    "torch.net.varint takes a 32-bit whole number, not {v}"
                )));
            }
            let bits = if v < 0.0 { v as i32 as u32 } else { v as u32 };
            let mut out = Vec::with_capacity(5);
            write_varint(&mut out, bits as i32);
            lua.create_string(out)
        })?,
    )?;
    t.raw_set(
        "readvarint",
        lua.create_function(|_, (s, at): (mlua::String, Option<usize>)| {
            let bytes = s.as_bytes();
            let start = at.unwrap_or(1).max(1) - 1;
            let rest = bytes.get(start..).unwrap_or_default();
            let mut r = Reader::new(rest);
            match r.varint() {
                Some(v) => Ok((Some(v), Some(start + r.pos() + 1))),
                None if rest.len() >= 5 => {
                    Err(Error::runtime("torch.net.readvarint: more than 5 bytes"))
                }
                None => Ok((None, None)),
            }
        })?,
    )?;
    t.set_readonly(true);
    Ok(t)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use azalea_protocol::packets::ProtocolPacket;
    use azalea_protocol::packets::game::ClientboundGamePacket;
    use azalea_protocol::packets::game::c_keep_alive::ClientboundKeepAlive;

    use super::super::api::{Api, Class, Ctx, PACKET};
    use super::{Fields, Packet};

    #[test]
    fn debug_is_formatted_on_first_read() {
        let api = Api::new(Arc::new(Ctx::new())).unwrap();
        let packet = ClientboundGamePacket::KeepAlive(ClientboundKeepAlive { id: 4242 });
        let mut bytes = Vec::new();
        packet.write(&mut bytes).unwrap();
        let p = Packet {
            id: packet.id(),
            bytes,
            fields: Fields::Debug,
        };
        assert_eq!(p.class(), Class::PacketDebug);
        let e = api.event_table().unwrap();
        p.fill(&api.lua, &e).unwrap();
        api.seal(&e, PACKET, p.class()).unwrap();
        let check: mlua::Function = (api.lua)
            .load(
                r#"
                local e = ...
                assert(rawget(e, "debug") == nil, "debug was made before it was read")
                assert(e.name == "KeepAlive" and e.resource == "minecraft:keep_alive", e.name)
                assert(e.type == "packet" and e.cancelable == false and e.defaultPrevented == false)
                assert(string.find(e.debug, "4242", 1, true), e.debug)
                assert(e.truncated == false)
                assert(rawget(e, "debug") ~= nil, "debug was not kept once read")
                assert(not pcall(function() e.x = 1 end), "the event is writable")
                assert(getmetatable(e) ~= nil and type(getmetatable(e)) == "string")
                e:preventDefault()
                return true
                "#,
            )
            .into_function()
            .unwrap();
        assert!(check.call::<bool>(&e).unwrap());
    }
}
