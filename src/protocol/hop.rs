use std::fmt;
use std::sync::OnceLock;

use azalea_protocol::packets::ConnectionProtocol;

use azalea_buf::AzBuf;
use azalea_registry::Registry as _;
use azalea_registry::builtin::{DataComponentKind, EntityKind, ParticleKind};

use super::packets::{Direction, PacketTable, Phase};
use super::remap::{IdSpace, Remap};
use super::wire::{Reader, varint_len, write_varint, write_varlong};

const ACTION_INTERACT: i32 = 0;
const ACTION_ATTACK: i32 = 1;

pub(crate) struct Hop {
    inbound_game: Box<[u32]>,
    outbound_game: Box<[Option<u32>]>,
    set_time: u32,
    level_chunk_with_light: u32,
    block_update: u32,
    section_blocks_update: u32,
    set_entity_data: u32,
    container_set_content: u32,
    container_set_slot: u32,
    set_cursor_item: u32,
    set_player_inventory: u32,
    set_equipment: u32,
    add_entity: u32,
    block_event: u32,
    award_stats: u32,
    level_event: u32,
    level_particles: u32,
    explode: u32,
    sound: u32,
    sound_entity: u32,
    update_recipes: u32,
    attack: u32,
    interact: u32,
    container_click: u32,
    set_creative_mode_slot: u32,
    trace: bool,
}

impl Hop {
    pub(crate) fn v774() -> &'static Hop {
        static HOP: OnceLock<Hop> = OnceLock::new();
        HOP.get_or_init(|| {
            let old = PacketTable::v1_21_11();
            let new = PacketTable::native();
            let id = |dir, name: &str| {
                new.id(Phase::Game, dir, name)
                    .unwrap_or_else(|| panic!("775 has no game packet {name}"))
            };

            let inbound_game = (0..old.count(Phase::Game, Direction::Clientbound))
                .map(|wire| {
                    let name = old
                        .name_of(Phase::Game, Direction::Clientbound, wire as u32)
                        .expect("in range");
                    id(Direction::Clientbound, name)
                })
                .collect();
            let outbound_game = (0..new.count(Phase::Game, Direction::Serverbound))
                .map(|native| {
                    let name = new
                        .name_of(Phase::Game, Direction::Serverbound, native as u32)
                        .expect("in range");
                    old.id(Phase::Game, Direction::Serverbound, name)
                })
                .collect();

            Hop {
                inbound_game,
                outbound_game,
                set_time: id(Direction::Clientbound, "set_time"),
                level_chunk_with_light: id(Direction::Clientbound, "level_chunk_with_light"),
                block_update: id(Direction::Clientbound, "block_update"),
                section_blocks_update: id(Direction::Clientbound, "section_blocks_update"),
                set_entity_data: id(Direction::Clientbound, "set_entity_data"),
                container_set_content: id(Direction::Clientbound, "container_set_content"),
                container_set_slot: id(Direction::Clientbound, "container_set_slot"),
                set_cursor_item: id(Direction::Clientbound, "set_cursor_item"),
                set_player_inventory: id(Direction::Clientbound, "set_player_inventory"),
                set_equipment: id(Direction::Clientbound, "set_equipment"),
                add_entity: id(Direction::Clientbound, "add_entity"),
                block_event: id(Direction::Clientbound, "block_event"),
                award_stats: id(Direction::Clientbound, "award_stats"),
                level_event: id(Direction::Clientbound, "level_event"),
                level_particles: id(Direction::Clientbound, "level_particles"),
                explode: id(Direction::Clientbound, "explode"),
                sound: id(Direction::Clientbound, "sound"),
                sound_entity: id(Direction::Clientbound, "sound_entity"),
                update_recipes: id(Direction::Clientbound, "update_recipes"),
                attack: id(Direction::Serverbound, "attack"),
                interact: id(Direction::Serverbound, "interact"),
                container_click: id(Direction::Serverbound, "container_click"),
                set_creative_mode_slot: id(Direction::Serverbound, "set_creative_mode_slot"),
                trace: crate::diag::debug_on("proto"),
            }
        })
    }

    pub(crate) fn inbound(&self, phase: ConnectionProtocol, raw: Box<[u8]>) -> Box<[u8]> {
        if phase != ConnectionProtocol::Game {
            if self.trace {
                passed_through("in", phase, &raw);
            }
            return raw;
        }
        let mut r = Reader::new(&raw);
        let Some(wire_id) = r.varint().and_then(|id| u32::try_from(id).ok()) else {
            return raw;
        };
        let Some(&native_id) = self.inbound_game.get(wire_id as usize) else {
            return raw;
        };
        let body = r.rest();

        let rewritten = if native_id == self.set_time {
            set_time(body)
        } else if native_id == self.level_chunk_with_light {
            level_chunk(body)
        } else if native_id == self.block_update {
            block_update(body)
        } else if native_id == self.section_blocks_update {
            section_blocks_update(body)
        } else if native_id == self.set_entity_data {
            entity_data(body)
        } else if native_id == self.container_set_content {
            stacks(body, 2, true, 1)
        } else if native_id == self.container_set_slot {
            container_set_slot(body)
        } else if native_id == self.set_cursor_item {
            stacks(body, 0, false, 0)
        } else if native_id == self.set_player_inventory {
            stacks(body, 1, false, 0)
        } else if native_id == self.set_equipment {
            set_equipment(body)
        } else if native_id == self.add_entity {
            add_entity(body)
        } else if native_id == self.block_event {
            block_event(body)
        } else if native_id == self.award_stats {
            award_stats(body)
        } else if native_id == self.level_event {
            level_event(body)
        } else if native_id == self.level_particles {
            level_particles(body)
        } else if native_id == self.explode {
            explode(body)
        } else if native_id == self.sound || native_id == self.sound_entity {
            sound(body)
        } else if native_id == self.update_recipes {
            update_recipes(body)
        } else {
            None
        };

        if self.trace {
            trace(
                "in",
                Direction::Clientbound,
                wire_id,
                Some(native_id),
                rewritten.is_some(),
            );
        }
        match rewritten {
            Some(body) => frame(native_id, &body),
            None if native_id != wire_id => frame(native_id, body),
            None => raw,
        }
    }

    pub(crate) fn outbound(&self, phase: ConnectionProtocol, raw: Box<[u8]>) -> Box<[u8]> {
        if phase != ConnectionProtocol::Game {
            if self.trace {
                passed_through("out", phase, &raw);
            }
            return raw;
        }
        let mut r = Reader::new(&raw);
        let Some(native_id) = r.varint().and_then(|id| u32::try_from(id).ok()) else {
            return raw;
        };
        let body = r.rest();

        if native_id == self.attack || native_id == self.interact {
            let wire_id = self
                .outbound_game
                .get(self.interact as usize)
                .copied()
                .flatten();
            let body = interact(body, native_id == self.attack);
            if self.trace {
                trace(
                    "out",
                    Direction::Serverbound,
                    native_id,
                    wire_id.filter(|_| body.is_some()),
                    body.is_some(),
                );
            }
            return match (body, wire_id) {
                (Some(body), Some(wire_id)) => frame(wire_id, &body),
                _ => Box::new([]),
            };
        }

        if native_id == self.container_click || native_id == self.set_creative_mode_slot {
            let body = if native_id == self.container_click {
                container_click(body)
            } else {
                creative_slot(body)
            };
            let wire_id = self
                .outbound_game
                .get(native_id as usize)
                .copied()
                .flatten();
            if self.trace {
                trace(
                    "out",
                    Direction::Serverbound,
                    native_id,
                    wire_id.filter(|_| body.is_some()),
                    body.is_some(),
                );
            }
            return match (body, wire_id) {
                (Some(body), Some(wire_id)) => frame(wire_id, &body),
                _ => Box::new([]),
            };
        }

        let wire_id = self
            .outbound_game
            .get(native_id as usize)
            .copied()
            .flatten();
        if self.trace {
            trace("out", Direction::Serverbound, native_id, wire_id, false);
        }
        match wire_id {
            Some(wire_id) if wire_id != native_id => frame(wire_id, body),
            Some(_) => raw,
            None => Box::new([]),
        }
    }
}

fn passed_through(way: &str, phase: ConnectionProtocol, raw: &[u8]) {
    let phase_key = match phase {
        ConnectionProtocol::Handshake => Phase::Handshake,
        ConnectionProtocol::Status => Phase::Status,
        ConnectionProtocol::Login => Phase::Login,
        ConnectionProtocol::Configuration => Phase::Configuration,
        ConnectionProtocol::Game => Phase::Game,
    };
    let dir = if way == "in" {
        Direction::Clientbound
    } else {
        Direction::Serverbound
    };
    let id = Reader::new(raw).varint().unwrap_or(-1);
    let name = u32::try_from(id)
        .ok()
        .and_then(|id| PacketTable::native().name_of(phase_key, dir, id))
        .unwrap_or("?");
    crate::log_debug!("proto", "{way} {phase:?} {name}({id}) passed through");
}

fn trace(way: &str, dir: Direction, from: u32, to: Option<u32>, body: bool) {
    let (from_table, to_table) = match dir {
        Direction::Clientbound => (PacketTable::v1_21_11(), PacketTable::native()),
        Direction::Serverbound => (PacketTable::native(), PacketTable::v1_21_11()),
    };
    let name = |table: &PacketTable, id: Option<u32>| match id {
        Some(id) => table
            .name_of(Phase::Game, dir, id)
            .map_or_else(|| format!("?{id}"), |name| format!("{name}({id})")),
        None => "suppressed".to_owned(),
    };
    crate::log_debug!(
        "proto",
        "{way} {} -> {}{}",
        name(from_table, Some(from)),
        name(to_table, to),
        if body { " +body" } else { "" }
    );
}

pub(crate) struct Translator {
    hop: &'static Hop,
}

impl Translator {
    pub(crate) fn v774() -> Self {
        Self { hop: Hop::v774() }
    }
}

impl azalea::connection::PacketTranslator for Translator {
    fn inbound(&mut self, phase: ConnectionProtocol, raw: Box<[u8]>) -> Box<[u8]> {
        self.hop.inbound(phase, raw)
    }

    fn outbound(&mut self, phase: ConnectionProtocol, raw: Box<[u8]>) -> Box<[u8]> {
        self.hop.outbound(phase, raw)
    }
}

fn frame(id: u32, body: &[u8]) -> Box<[u8]> {
    let mut out = Vec::with_capacity(varint_len(id as i32) + body.len());
    write_varint(&mut out, id as i32);
    out.extend_from_slice(body);
    out.into_boxed_slice()
}

fn set_time(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let game_time = r.take(8)?;
    let day_time = r.u64()? as i64;
    let ticking = r.u8()? != 0;

    let mut out = Vec::with_capacity(24);
    out.extend_from_slice(game_time);
    write_varint(&mut out, 1);
    write_varint(&mut out, 0);
    write_varlong(&mut out, day_time);
    out.extend_from_slice(&0f32.to_be_bytes());
    out.extend_from_slice(&if ticking { 1f32 } else { 0f32 }.to_be_bytes());
    Some(out.into_boxed_slice())
}

fn interact(body: &[u8], attacking: bool) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let entity_id = r.varint()?;

    let (hand, secondary) = if attacking {
        (None, false)
    } else {
        let hand = r.varint()?;
        skip_lp_vec3(&mut r)?;
        (Some(hand), r.u8()? != 0)
    };

    let mut out = Vec::with_capacity(8);
    write_varint(&mut out, entity_id);
    match hand {
        Some(hand) => {
            write_varint(&mut out, ACTION_INTERACT);
            write_varint(&mut out, hand);
        }
        None => write_varint(&mut out, ACTION_ATTACK),
    }
    out.push(u8::from(secondary));
    Some(out.into_boxed_slice())
}

fn state(id: i32) -> Option<i32> {
    Remap::to_native().map(IdSpace::BlockState, id)
}

fn block(id: i32) -> Option<i32> {
    Remap::to_native().map(IdSpace::Block, id)
}

fn skip_lp_vec3(r: &mut Reader) -> Option<()> {
    let flags = r.u8()?;
    if flags != 0 {
        r.take(5)?;
        if flags & 4 != 0 {
            r.varint()?;
        }
    }
    Some(())
}

fn copy_payload(
    r: &mut Reader,
    out: &mut Vec<u8>,
    probe: &mut Vec<u8>,
    native: i32,
    read: impl FnOnce(&mut std::io::Cursor<&[u8]>) -> bool,
) -> Option<()> {
    probe.clear();
    write_varint(probe, native);
    let prefix = probe.len();
    probe.extend_from_slice(r.rest());
    let mut cursor = std::io::Cursor::new(&probe[..]);
    if !read(&mut cursor) {
        return None;
    }
    let length = usize::try_from(cursor.position())
        .ok()?
        .checked_sub(prefix)?;
    out.extend_from_slice(r.take(length)?);
    Some(())
}

fn particle(r: &mut Reader, out: &mut Vec<u8>, probe: &mut Vec<u8>) -> Option<()> {
    let native = Remap::to_native().map(IdSpace::ParticleType, r.varint()?)?;
    write_varint(out, native);

    match ParticleKind::from_u32(u32::try_from(native).ok()?)? {
        ParticleKind::Block
        | ParticleKind::BlockMarker
        | ParticleKind::FallingDust
        | ParticleKind::DustPillar
        | ParticleKind::BlockCrumble => {
            let id = state(r.varint()?)?;
            write_varint(out, id);
        }
        ParticleKind::Item => item_stack(r, out)?,
        _ => copy_payload(r, out, probe, native, |cursor| {
            azalea::entity::particle::Particle::azalea_read(cursor).is_ok()
        })?,
    }
    Some(())
}

fn sound_holder(r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let raw = r.varint()?;
    if raw == 0 {
        write_varint(out, 0);
        let name = r.count()?;
        write_varint(out, name as i32);
        out.extend_from_slice(r.take(name)?);
        let ranged = r.u8()?;
        out.push(ranged);
        if ranged != 0 {
            out.extend_from_slice(r.take(4)?);
        }
        return Some(());
    }
    let moved = Remap::to_native().map(IdSpace::SoundEvent, raw - 1)?;
    write_varint(out, moved + 1);
    Some(())
}

const SERIALIZER_ITEM_STACK: i32 = 7;
const SERIALIZER_BLOCK_STATE: i32 = 14;
const SERIALIZER_OPTIONAL_BLOCK_STATE: i32 = 15;
const SERIALIZER_PARTICLE: i32 = 16;
const SERIALIZER_PARTICLES: i32 = 17;

fn item_stack(r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let count = r.varint()?;
    write_varint(out, count);
    if count <= 0 {
        return Some(());
    }
    let item = Remap::to_native().map(IdSpace::Item, r.varint()?)?;
    write_varint(out, item);

    let with_payload = r.count()?;
    let without = r.count()?;
    write_varint(out, with_payload as i32);
    write_varint(out, without as i32);

    for _ in 0..with_payload {
        let kind = component(r, out)?;
        let mut cursor = std::io::Cursor::new(r.rest());
        azalea_inventory::components::DataComponentUnion::azalea_read_as(kind, &mut cursor).ok()?;
        let length = usize::try_from(cursor.position()).ok()?;
        out.extend_from_slice(r.take(length)?);
    }
    for _ in 0..without {
        component(r, out)?;
    }
    Some(())
}

fn component(r: &mut Reader, out: &mut Vec<u8>) -> Option<DataComponentKind> {
    let id = Remap::to_native().map(IdSpace::DataComponentType, r.varint()?)?;
    write_varint(out, id);
    DataComponentKind::from_u32(u32::try_from(id).ok()?)
}

fn entity_data(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 8);
    write_varint(&mut out, r.varint()?);

    let mut probe = Vec::new();
    loop {
        let index = r.u8()?;
        out.push(index);
        if index == 0xFF {
            return Some(out.into_boxed_slice());
        }
        let serializer = Remap::to_native().map(IdSpace::EntityDataSerializer, r.varint()?)?;
        write_varint(&mut out, serializer);

        match serializer {
            SERIALIZER_ITEM_STACK => {
                item_stack(&mut r, &mut out)?;
                continue;
            }
            SERIALIZER_BLOCK_STATE | SERIALIZER_OPTIONAL_BLOCK_STATE => {
                let id = state(r.varint()?)?;
                write_varint(&mut out, id);
                continue;
            }
            SERIALIZER_PARTICLE => {
                particle(&mut r, &mut out, &mut probe)?;
                continue;
            }
            SERIALIZER_PARTICLES => {
                let count = r.count()?;
                write_varint(&mut out, count as i32);
                for _ in 0..count {
                    particle(&mut r, &mut out, &mut probe)?;
                }
                continue;
            }
            _ => {}
        }

        copy_payload(&mut r, &mut out, &mut probe, serializer, |cursor| {
            azalea::entity::EntityDataValue::azalea_read(cursor).is_ok()
        })?;
    }
}

fn stacks(body: &[u8], header: usize, counted: bool, trailing: usize) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 16);
    for _ in 0..header {
        write_varint(&mut out, r.varint()?);
    }
    let count = if counted {
        let count = r.count()?;
        write_varint(&mut out, count as i32);
        count
    } else {
        1
    };
    for _ in 0..count {
        item_stack(&mut r, &mut out)?;
    }
    for _ in 0..trailing {
        item_stack(&mut r, &mut out)?;
    }
    Some(out.into_boxed_slice())
}

const SLOT_DISPLAY_TO_NATIVE: [i32; 8] = [0, 1, 4, 5, 6, 8, 9, 10];

const SLOT_DISPLAY_ITEM: i32 = 2;
const SLOT_DISPLAY_ITEM_STACK: i32 = 3;
const SLOT_DISPLAY_TAG: i32 = 4;
const SLOT_DISPLAY_SMITHING_TRIM: i32 = 5;
const SLOT_DISPLAY_WITH_REMAINDER: i32 = 6;
const SLOT_DISPLAY_COMPOSITE: i32 = 7;

fn update_recipes(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 64);

    let sets = r.count()?;
    write_varint(&mut out, sets as i32);
    for _ in 0..sets {
        identifier(&mut r, &mut out)?;
        let items = r.count()?;
        write_varint(&mut out, items as i32);
        for _ in 0..items {
            item(&mut r, &mut out)?;
        }
    }

    let entries = r.count()?;
    write_varint(&mut out, entries as i32);
    for _ in 0..entries {
        ingredient(&mut r, &mut out)?;
        slot_display(&mut r, &mut out)?;
    }

    (r.remaining() == 0).then(|| out.into_boxed_slice())
}

fn identifier(r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let len = r.count()?;
    write_varint(out, len as i32);
    out.extend_from_slice(r.take(len)?);
    Some(())
}

fn item(r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let id = Remap::to_native().map(IdSpace::Item, r.varint()?)?;
    write_varint(out, id);
    Some(())
}

fn ingredient(r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let size = r.count()?;
    write_varint(out, size as i32);
    if size == 0 {
        return identifier(r, out);
    }
    for _ in 1..size {
        item(r, out)?;
    }
    Some(())
}

fn slot_display(r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let kind = r.varint()?;
    let native = *SLOT_DISPLAY_TO_NATIVE.get(usize::try_from(kind).ok()?)?;
    write_varint(out, native);
    match kind {
        SLOT_DISPLAY_ITEM => item(r, out)?,
        SLOT_DISPLAY_ITEM_STACK => {
            let mut stack = Vec::new();
            item_stack(r, &mut stack)?;
            let mut s = Reader::new(&stack);
            let count = s.varint()?;
            if count <= 0 {
                return None;
            }
            write_varint(out, s.varint()?);
            write_varint(out, count);
            out.extend_from_slice(s.rest());
        }
        SLOT_DISPLAY_TAG => identifier(r, out)?,
        SLOT_DISPLAY_SMITHING_TRIM => {
            slot_display(r, out)?;
            slot_display(r, out)?;
            let pattern = r.varint()?;
            if pattern == 0 {
                return None;
            }
            write_varint(out, pattern);
        }
        SLOT_DISPLAY_WITH_REMAINDER => {
            slot_display(r, out)?;
            slot_display(r, out)?;
        }
        SLOT_DISPLAY_COMPOSITE => {
            let count = r.count()?;
            write_varint(out, count as i32);
            for _ in 0..count {
                slot_display(r, out)?;
            }
        }
        _ => {}
    }
    Some(())
}

fn container_set_slot(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 4);
    write_varint(&mut out, r.varint()?);
    write_varint(&mut out, r.varint()?);
    out.extend_from_slice(r.take(2)?);
    item_stack(&mut r, &mut out)?;
    Some(out.into_boxed_slice())
}

fn set_equipment(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 8);
    write_varint(&mut out, r.varint()?);
    loop {
        let slot = r.u8()?;
        out.push(slot);
        item_stack(&mut r, &mut out)?;
        if slot & 0x80 == 0 {
            return Some(out.into_boxed_slice());
        }
    }
}

fn hashed_stack(r: &mut Reader, out: &mut Vec<u8>) -> Option<bool> {
    let present = r.u8()?;
    out.push(present);
    if present == 0 {
        return Some(true);
    }

    let mut ok = true;
    back(IdSpace::Item, r.varint()?, out, &mut ok);
    write_varint(out, r.varint()?);
    let added = r.count()?;
    let removed = r.count()?;
    write_varint(out, added as i32);
    write_varint(out, removed as i32);
    for _ in 0..added {
        back(IdSpace::DataComponentType, r.varint()?, out, &mut ok);
        out.extend_from_slice(r.take(4)?);
    }
    for _ in 0..removed {
        back(IdSpace::DataComponentType, r.varint()?, out, &mut ok);
    }
    Some(ok)
}

fn back(space: IdSpace, raw: i32, out: &mut Vec<u8>, ok: &mut bool) {
    let id = Remap::from_native().map(space, raw);
    *ok &= id.is_some();
    write_varint(out, id.unwrap_or_default());
}

fn container_click(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len());
    write_varint(&mut out, r.varint()?);
    write_varint(&mut out, r.varint()?);
    out.extend_from_slice(r.take(2 + 1)?);
    write_varint(&mut out, r.varint()?);

    let count = r.count()?;
    let mut slots = Vec::new();
    let mut ok = true;
    for _ in 0..count {
        slots.extend_from_slice(r.take(2)?);
        ok &= hashed_stack(&mut r, &mut slots)?;
    }
    if ok {
        write_varint(&mut out, count as i32);
        out.extend_from_slice(&slots);
    } else {
        write_varint(&mut out, 0);
    }

    let carried = out.len();
    if !hashed_stack(&mut r, &mut out)? {
        out.truncate(carried);
        out.push(0);
    }
    Some(out.into_boxed_slice())
}

fn creative_slot(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 4);
    out.extend_from_slice(r.take(2)?);

    let count = r.varint()?;
    write_varint(&mut out, count);
    if count <= 0 {
        return Some(out.into_boxed_slice());
    }

    let Some(item) = Remap::from_native().map(IdSpace::Item, r.varint()?) else {
        crate::log_warn!(
            "net",
            "clearing a creative slot: this server has no id for what was put in it"
        );
        out.truncate(2);
        write_varint(&mut out, 0);
        return Some(out.into_boxed_slice());
    };
    write_varint(&mut out, item);

    let with_payload = r.count()?;
    let without = r.count()?;
    write_varint(&mut out, with_payload as i32);
    write_varint(&mut out, without as i32);
    for _ in 0..with_payload {
        let kind = Remap::from_native().map(IdSpace::DataComponentType, r.varint()?)?;
        write_varint(&mut out, kind);
        let length = r.count()?;
        write_varint(&mut out, length as i32);
        out.extend_from_slice(r.take(length)?);
    }
    for _ in 0..without {
        let kind = Remap::from_native().map(IdSpace::DataComponentType, r.varint()?)?;
        write_varint(&mut out, kind);
    }
    Some(out.into_boxed_slice())
}

fn level_particles(body: &[u8]) -> Option<Box<[u8]>> {
    const HEADER: usize = 1 + 1 + 3 * 8 + 4 * 4 + 4;
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 4);
    out.extend_from_slice(r.take(HEADER)?);
    particle(&mut r, &mut out, &mut Vec::new())?;
    Some(out.into_boxed_slice())
}

fn explode(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(r.take(3 * 8 + 4 + 4)?);
    let knockback = r.u8()?;
    out.push(knockback);
    if knockback != 0 {
        out.extend_from_slice(r.take(3 * 8)?);
    }

    let mut probe = Vec::new();
    particle(&mut r, &mut out, &mut probe)?;
    sound_holder(&mut r, &mut out)?;

    let count = r.count()?;
    write_varint(&mut out, count as i32);
    for _ in 0..count {
        particle(&mut r, &mut out, &mut probe)?;
        out.extend_from_slice(r.take(4 + 4)?);
        write_varint(&mut out, r.varint()?);
    }
    Some(out.into_boxed_slice())
}

fn sound(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 2);
    sound_holder(&mut r, &mut out)?;
    out.extend_from_slice(r.rest());
    Some(out.into_boxed_slice())
}

fn block_event(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 1);
    out.extend_from_slice(r.take(8 + 1 + 1)?);
    let id = block(r.varint()?)?;
    write_varint(&mut out, id);
    Some(out.into_boxed_slice())
}

const STAT_MINED: i32 = 0;
const STAT_DROPPED: i32 = 5;

fn award_stats(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let count = r.count()?;
    let mut out = Vec::with_capacity(body.len() + count);
    write_varint(&mut out, count as i32);
    for _ in 0..count {
        let kind = r.varint()?;
        write_varint(&mut out, kind);
        let value = r.varint()?;
        let value = match kind {
            STAT_MINED => block(value)?,
            k if (STAT_MINED + 1..=STAT_DROPPED).contains(&k) => {
                Remap::to_native().map(IdSpace::Item, value)?
            }
            _ => value,
        };
        write_varint(&mut out, value);
        write_varint(&mut out, r.varint()?);
    }
    Some(out.into_boxed_slice())
}

fn add_entity(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    r.varint()?;
    r.take(16)?;
    if r.varint()? != EntityKind::FallingBlock.to_u32() as i32 {
        return None;
    }
    r.take(3 * 8)?;
    skip_lp_vec3(&mut r)?;
    r.take(3)?;

    let head = r.slice_from(0);
    let data = state(r.varint()?)?;
    let mut out = Vec::with_capacity(head.len() + varint_len(data));
    out.extend_from_slice(head);
    write_varint(&mut out, data);
    Some(out.into_boxed_slice())
}

const LEVEL_EVENT_DESTROY_BLOCK: u32 = 2001;
const LEVEL_EVENT_BRUSH_COMPLETE: u32 = 3008;

fn level_event(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let event = r.take(4)?;
    let kind = u32::from_be_bytes(event.try_into().ok()?);
    if !matches!(kind, LEVEL_EVENT_DESTROY_BLOCK | LEVEL_EVENT_BRUSH_COMPLETE) {
        return None;
    }
    let pos = r.take(8)?;
    let data = state(i32::from_be_bytes(r.take(4)?.try_into().ok()?))?;

    let mut out = Vec::with_capacity(body.len());
    out.extend_from_slice(event);
    out.extend_from_slice(pos);
    out.extend_from_slice(&data.to_be_bytes());
    out.extend_from_slice(r.rest());
    Some(out.into_boxed_slice())
}

fn block_update(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let pos = r.take(8)?;
    let id = state(r.varint()?)?;
    let mut out = Vec::with_capacity(8 + varint_len(id));
    out.extend_from_slice(pos);
    write_varint(&mut out, id);
    Some(out.into_boxed_slice())
}

fn section_blocks_update(body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let section = r.take(8)?;
    let count = r.count()?;

    let mut out = Vec::with_capacity(body.len() + count);
    out.extend_from_slice(section);
    write_varint(&mut out, count as i32);
    for _ in 0..count {
        let packed = r.varlong()? as u64;
        let id = state(i32::try_from(packed >> 12).ok()?)?;
        write_varlong(
            &mut out,
            ((i64::from(id) as u64) << 12 | (packed & 0xFFF)) as i64,
        );
    }
    Some(out.into_boxed_slice())
}

const SECTION_STATES: usize = 4096;
const SECTION_BIOMES: usize = 64;
const STATES_INDIRECT_MAX: u8 = 8;
const BIOMES_INDIRECT_MAX: u8 = 3;

fn level_chunk(body: &[u8]) -> Option<Box<[u8]>> {
    match rewrite_level_chunk(body) {
        Ok(out) => Some(out),
        Err(why) => {
            crate::log_error!(
                "proto",
                "{why}. The 774 body is passing through untranslated and will not \
                 decode as 775; that column is lost"
            );
            None
        }
    }
}

fn rewrite_level_chunk(body: &[u8]) -> Result<Box<[u8]>, String> {
    let mut r = Reader::new(body);
    let pos = r
        .u64()
        .ok_or_else(|| "a chunk packet ended before its position".to_owned())?;
    let (x, z) = ((pos >> 32) as i32, pos as i32);
    let short = |what: &str| format!("chunk {x},{z} ended early reading {what}");

    let heightmaps = r.count().ok_or_else(|| short("the heightmap count"))?;
    for _ in 0..heightmaps {
        r.varint().ok_or_else(|| short("a heightmap kind"))?;
        let longs = r.count().ok_or_else(|| short("a heightmap length"))?;
        r.take(longs.checked_mul(8).ok_or_else(|| short("a heightmap"))?)
            .ok_or_else(|| short("a heightmap"))?;
    }
    let head = r.slice_from(0);
    let length = r.count().ok_or_else(|| short("the section blob length"))?;
    let blob = r.take(length).ok_or_else(|| short("the section blob"))?;
    let tail = r.rest();

    let sections = sections(blob).map_err(|why| {
        format!("chunk {x},{z} was not translated: {why} (the blob is {length} bytes)")
    })?;
    let mut out = Vec::with_capacity(head.len() + 5 + sections.len() + tail.len());
    out.extend_from_slice(head);
    let length = i32::try_from(sections.len())
        .map_err(|_| format!("chunk {x},{z} rewrote to a blob too long to length-prefix"))?;
    write_varint(&mut out, length);
    out.extend_from_slice(&sections);
    out.extend_from_slice(tail);
    Ok(out.into_boxed_slice())
}

#[derive(Debug)]
enum Bail {
    Short,
    NoTarget(i32),
    TooWide { id: i32, bits: u8 },
}

#[derive(Debug)]
struct Failure {
    bail: Bail,
    section: usize,
    part: &'static str,
    at: usize,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "section {} {}, at blob byte {}: ",
            self.section, self.part, self.at
        )?;
        match self.bail {
            Bail::Short => write!(f, "the blob ended early"),
            Bail::NoTarget(id) => {
                write!(f, "774 block state {id} has no 775 id")
            }
            Bail::TooWide { id, bits } => write!(
                f,
                "774 block state {id} moved to an id too wide for the {bits}-bit \
                 container the server packed"
            ),
        }
    }
}

fn sections(blob: &[u8]) -> Result<Vec<u8>, Failure> {
    let mut r = Reader::new(blob);
    let mut out = Vec::with_capacity(blob.len() + 64);
    let mut section = 0;
    while r.remaining() > 0 {
        let Some(count) = r.take(2) else {
            let bail = Bail::Short;
            return Err(Failure {
                bail,
                section,
                part: "the block count",
                at: r.pos(),
            });
        };
        out.extend_from_slice(count);
        out.extend_from_slice(&[0, 0]);
        if let Err(bail) = container(&mut r, &mut out, SECTION_STATES, STATES_INDIRECT_MAX, true) {
            return Err(Failure {
                bail,
                section,
                part: "block states",
                at: r.pos(),
            });
        }
        if let Err(bail) = container(&mut r, &mut out, SECTION_BIOMES, BIOMES_INDIRECT_MAX, false) {
            return Err(Failure {
                bail,
                section,
                part: "biomes",
                at: r.pos(),
            });
        }
        section += 1;
    }
    Ok(out)
}

fn container(
    r: &mut Reader,
    out: &mut Vec<u8>,
    size: usize,
    indirect_max: u8,
    remap: bool,
) -> Result<(), Bail> {
    let bits = r.u8().ok_or(Bail::Short)?;
    out.push(bits);

    let id = |r: &mut Reader, out: &mut Vec<u8>| -> Result<(), Bail> {
        let raw = r.varint().ok_or(Bail::Short)?;
        let moved = if remap {
            state(raw).ok_or(Bail::NoTarget(raw))?
        } else {
            raw
        };
        write_varint(out, moved);
        Ok(())
    };

    if bits == 0 {
        id(r, out)?;
        return Ok(());
    }
    if bits <= indirect_max {
        let entries = r.count().ok_or(Bail::Short)?;
        write_varint(out, entries as i32);
        for _ in 0..entries {
            id(r, out)?;
        }
    }

    let per_long = 64 / usize::from(bits);
    let longs = size.div_ceil(per_long);
    let packed = r
        .take(longs.checked_mul(8).ok_or(Bail::Short)?)
        .ok_or(Bail::Short)?;

    if bits > indirect_max && remap {
        let mask = (1u64 << bits) - 1;
        let mut index = 0;
        for word in packed.chunks_exact(8) {
            let word = u64::from_be_bytes(word.try_into().map_err(|_| Bail::Short)?);
            let mut rewritten = 0u64;
            for slot in 0..per_long {
                if index >= size {
                    break;
                }
                let raw = i32::try_from((word >> (slot * usize::from(bits))) & mask)
                    .map_err(|_| Bail::Short)?;
                let moved = state(raw).ok_or(Bail::NoTarget(raw))?;
                let moved = u64::try_from(moved).map_err(|_| Bail::TooWide { id: raw, bits })?;
                if moved > mask {
                    return Err(Bail::TooWide { id: raw, bits });
                }
                rewritten |= moved << (slot * usize::from(bits));
                index += 1;
            }
            out.extend_from_slice(&rewritten.to_be_bytes());
        }
    } else {
        out.extend_from_slice(packed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hop() -> &'static Hop {
        Hop::v774()
    }

    #[test]
    fn every_inbound_id_lands_on_the_same_name() {
        let h = hop();
        let old = PacketTable::v1_21_11();
        let new = PacketTable::native();
        for wire in 0..old.count(Phase::Game, Direction::Clientbound) {
            let name = old
                .name_of(Phase::Game, Direction::Clientbound, wire as u32)
                .unwrap();
            let native = h.inbound_game[wire];
            assert_eq!(
                new.name_of(Phase::Game, Direction::Clientbound, native),
                Some(name),
                "774 clientbound {wire} ({name}) landed wrong"
            );
        }
    }

    #[test]
    fn every_outbound_id_lands_on_the_same_name_or_is_suppressed() {
        let h = hop();
        let old = PacketTable::v1_21_11();
        let new = PacketTable::native();
        let mut suppressed = Vec::new();
        for native in 0..new.count(Phase::Game, Direction::Serverbound) {
            let name = new
                .name_of(Phase::Game, Direction::Serverbound, native as u32)
                .unwrap();
            match h.outbound_game[native] {
                Some(wire) => assert_eq!(
                    old.name_of(Phase::Game, Direction::Serverbound, wire),
                    Some(name)
                ),
                None => suppressed.push(name),
            }
        }
        suppressed.sort_unstable();
        assert_eq!(
            suppressed,
            ["attack", "set_game_rule", "spectate_entity"],
            "774 has no packet of any of these names"
        );
    }

    #[test]
    fn attack_is_rewritten_rather_than_suppressed() {
        let h = hop();
        let mut frame = Vec::new();
        write_varint(&mut frame, h.attack as i32);
        write_varint(&mut frame, 99);
        let out = h.outbound(ConnectionProtocol::Game, frame.into_boxed_slice());
        assert!(!out.is_empty(), "attacking must reach the server");

        let old = PacketTable::v1_21_11();
        let mut r = Reader::new(&out);
        assert_eq!(
            old.name_of(
                Phase::Game,
                Direction::Serverbound,
                r.varint().unwrap() as u32
            ),
            Some("interact")
        );
        assert_eq!(r.varint(), Some(99));
        assert_eq!(r.varint(), Some(ACTION_ATTACK));
    }

    #[test]
    fn a_packet_774_never_had_is_dropped() {
        let h = hop();
        let native = PacketTable::native();
        for name in ["set_game_rule", "spectate_entity"] {
            let id = native
                .id(Phase::Game, Direction::Serverbound, name)
                .unwrap_or_else(|| panic!("775 has {name}"));
            let mut frame = Vec::new();
            write_varint(&mut frame, id as i32);
            frame.push(0);
            assert!(
                h.outbound(ConnectionProtocol::Game, frame.into_boxed_slice())
                    .is_empty(),
                "{name} has no 774 id and must not be sent at someone else's"
            );
        }
    }

    #[test]
    fn the_other_phases_pass_through() {
        let h = hop();
        for phase in [
            ConnectionProtocol::Handshake,
            ConnectionProtocol::Status,
            ConnectionProtocol::Login,
            ConnectionProtocol::Configuration,
        ] {
            let raw: Box<[u8]> = Box::new([0x03, 0xAA, 0xBB]);
            assert_eq!(&*h.inbound(phase, raw.clone()), &*raw);
            assert_eq!(&*h.outbound(phase, raw.clone()), &*raw);
        }
    }

    #[test]
    fn a_truncated_frame_is_passed_through_untouched() {
        let h = hop();
        for raw in [vec![], vec![0x80], vec![0xFF, 0xFF]] {
            let raw: Box<[u8]> = raw.into_boxed_slice();
            assert_eq!(&*h.inbound(ConnectionProtocol::Game, raw.clone()), &*raw);
        }
    }

    #[test]
    fn set_time_becomes_a_one_entry_clock_map() {
        let mut body = Vec::new();
        body.extend_from_slice(&1234u64.to_be_bytes());
        body.extend_from_slice(&600u64.to_be_bytes());
        body.push(1);
        let out = set_time(&body).expect("17 bytes is a whole packet");

        let mut r = Reader::new(&out);
        assert_eq!(r.u64(), Some(1234));
        assert_eq!(r.count(), Some(1), "exactly one clock, for the fallback");
        assert_eq!(r.varint(), Some(0));
        assert_eq!(r.varint(), Some(600), "day time became total ticks");
        assert_eq!(r.take(4), Some(&0f32.to_be_bytes()[..]));
        assert_eq!(r.take(4), Some(&1f32.to_be_bytes()[..]));
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_frozen_day_time_becomes_a_rate_of_zero() {
        let mut body = Vec::new();
        body.extend_from_slice(&0u64.to_be_bytes());
        body.extend_from_slice(&18000u64.to_be_bytes());
        body.push(0);
        let out = set_time(&body).unwrap();
        let mut r = Reader::new(&out);
        r.u64();
        r.count();
        r.varint();
        assert_eq!(r.varint(), Some(18000));
        r.take(4);
        assert_eq!(r.take(4), Some(&0f32.to_be_bytes()[..]));
    }

    #[test]
    fn a_short_set_time_does_not_rewrite() {
        assert!(set_time(&[0u8; 16]).is_none());
    }

    #[test]
    fn attack_becomes_an_interact_action() {
        let mut body = Vec::new();
        write_varint(&mut body, 42);
        let out = interact(&body, true).unwrap();
        let mut r = Reader::new(&out);
        assert_eq!(r.varint(), Some(42));
        assert_eq!(r.varint(), Some(ACTION_ATTACK));
        assert_eq!(r.u8(), Some(0), "775 carries no sneaking flag to forward");
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn interact_keeps_the_hand_and_drops_the_location() {
        for location in [
            vec![0x00],
            vec![0x01, 0x02, 0xAA, 0xBB, 0xCC, 0xDD],
            vec![0x05, 0x02, 0xAA, 0xBB, 0xCC, 0xDD, 0x81, 0x01],
        ] {
            let mut body = Vec::new();
            write_varint(&mut body, 7);
            write_varint(&mut body, 1);
            body.extend_from_slice(&location);
            body.push(1);
            let out = interact(&body, false)
                .unwrap_or_else(|| panic!("location {location:?} was not skipped"));
            let mut r = Reader::new(&out);
            assert_eq!(r.varint(), Some(7));
            assert_eq!(r.varint(), Some(ACTION_INTERACT));
            assert_eq!(r.varint(), Some(1), "off hand survived");
            assert_eq!(r.u8(), Some(1), "sneaking survived");
            assert_eq!(r.remaining(), 0);
        }
    }

    const BELOW_FIRST_RUN: i32 = 100;
    const FIRST_RUN: i32 = 1381;
    const SECOND_RUN: i32 = 2500;
    const LAST_RUN: i32 = 20000;

    fn moved(id: i32) -> i32 {
        state(id).unwrap_or_else(|| panic!("774 state {id} does not map"))
    }

    fn container_bytes(bits: u8, palette: &[i32], packed: &[u64]) -> Vec<u8> {
        let mut out = vec![bits];
        if bits == 0 {
            write_varint(&mut out, palette[0]);
            return out;
        }
        if !palette.is_empty() {
            write_varint(&mut out, palette.len() as i32);
            for id in palette {
                write_varint(&mut out, *id);
            }
        }
        for word in packed {
            out.extend_from_slice(&word.to_be_bytes());
        }
        out
    }

    fn empty_biomes() -> Vec<u8> {
        container_bytes(0, &[7], &[])
    }

    fn section_bytes(states: &[u8]) -> Vec<u8> {
        let mut out = vec![0x01, 0x00];
        out.extend_from_slice(states);
        out.extend_from_slice(&empty_biomes());
        out
    }

    #[test]
    fn a_section_gains_a_fluid_count_and_keeps_its_block_count() {
        let out = sections(&section_bytes(&container_bytes(0, &[FIRST_RUN], &[])))
            .expect("a whole section");
        let mut r = Reader::new(&out);
        assert_eq!(r.take(2), Some(&[0x01, 0x00][..]), "block count survived");
        assert_eq!(r.take(2), Some(&[0x00, 0x00][..]), "fluid count was added");
        assert_eq!(r.u8(), Some(0), "still a single-value palette");
        assert_eq!(r.varint(), Some(moved(FIRST_RUN)));
    }

    #[test]
    fn an_indirect_palette_is_remapped_and_its_indices_are_not() {
        let palette = [BELOW_FIRST_RUN, FIRST_RUN, SECOND_RUN, LAST_RUN];
        let packed = [0x0123_4567_89AB_CDEFu64; 4096 / (64 / 4)];
        let states = container_bytes(4, &palette, &packed);
        let out = sections(&section_bytes(&states)).expect("a whole section");

        let mut r = Reader::new(&out);
        r.take(4);
        assert_eq!(r.u8(), Some(4));
        assert_eq!(r.count(), Some(palette.len()));
        for id in palette {
            assert_eq!(r.varint(), Some(moved(id)));
        }
        for word in packed {
            assert_eq!(
                r.u64(),
                Some(word),
                "an index into the palette is not an id and must not move"
            );
        }
    }

    #[test]
    fn a_direct_palette_is_unpacked_remapped_and_repacked() {
        const BITS: usize = 15;
        let per_long = 64 / BITS;
        let ids: Vec<i32> = (0..SECTION_STATES)
            .map(|i| match i % 4 {
                0 => BELOW_FIRST_RUN,
                1 => FIRST_RUN,
                2 => SECOND_RUN,
                _ => LAST_RUN,
            })
            .collect();

        let pack = |values: &[i32]| -> Vec<u64> {
            let mut out = vec![0u64; SECTION_STATES.div_ceil(per_long)];
            for (i, id) in values.iter().enumerate() {
                out[i / per_long] |= (*id as u64) << ((i % per_long) * BITS);
            }
            out
        };

        let states = container_bytes(BITS as u8, &[], &pack(&ids));
        let out = sections(&section_bytes(&states)).expect("a whole section");

        let expected: Vec<i32> = ids.iter().map(|id| moved(*id)).collect();
        let mut r = Reader::new(&out);
        r.take(4);
        assert_eq!(r.u8(), Some(BITS as u8), "the width did not need to change");
        for word in pack(&expected) {
            assert_eq!(r.u64(), Some(word));
        }
    }

    #[test]
    fn every_section_in_the_blob_is_rewritten() {
        let one = section_bytes(&container_bytes(0, &[FIRST_RUN], &[]));
        let blob: Vec<u8> = one.iter().chain(&one).chain(&one).copied().collect();
        let out = sections(&blob).expect("three whole sections");
        assert_eq!(out.len(), blob.len() + 3 * 2, "one fluid count each");
        let mut r = Reader::new(&out);
        for _ in 0..3 {
            r.take(4);
            r.u8();
            assert_eq!(r.varint(), Some(moved(FIRST_RUN)));
            r.u8();
            r.varint();
        }
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn the_chunk_packet_keeps_its_head_and_tail() {
        let blob = section_bytes(&container_bytes(0, &[SECOND_RUN], &[]));
        let tail = [0xDEu8, 0xAD, 0xBE, 0xEF];

        let mut body = Vec::new();
        body.extend_from_slice(&1i32.to_be_bytes());
        body.extend_from_slice(&(-2i32).to_be_bytes());
        write_varint(&mut body, 1);
        write_varint(&mut body, 4);
        write_varint(&mut body, 2);
        body.extend_from_slice(&7u64.to_be_bytes());
        body.extend_from_slice(&9u64.to_be_bytes());
        let head_len = body.len();
        write_varint(&mut body, blob.len() as i32);
        body.extend_from_slice(&blob);
        body.extend_from_slice(&tail);

        let out = level_chunk(&body).expect("a whole chunk");
        assert_eq!(&out[..head_len], &body[..head_len], "head is untouched");
        let mut r = Reader::new(&out[head_len..]);
        assert_eq!(
            r.count(),
            Some(blob.len() + 2),
            "the length prefix grew by the one fluid count"
        );
        r.take(blob.len() + 2);
        assert_eq!(r.rest(), tail, "block entities and light are untouched");
    }

    #[test]
    fn a_truncated_chunk_does_not_rewrite() {
        assert!(level_chunk(&[]).is_none());
        assert!(matches!(
            sections(&[0x01]),
            Err(Failure {
                bail: Bail::Short,
                ..
            })
        ));
        let short = sections(&section_bytes(&container_bytes(4, &[1], &[0; 2])));
        assert!(matches!(
            short,
            Err(Failure {
                bail: Bail::Short,
                section: 0,
                part: "block states",
                ..
            })
        ));
    }

    #[test]
    fn an_unmappable_state_says_which_id_stopped_it() {
        let past_the_end = (0..)
            .find(|id| state(*id).is_none())
            .expect("the table ends somewhere");

        let single = sections(&section_bytes(&container_bytes(0, &[past_the_end], &[])));
        assert!(matches!(
            single,
            Err(Failure {
                bail: Bail::NoTarget(id),
                part: "block states",
                ..
            }) if id == past_the_end
        ));

        let indirect = sections(&section_bytes(&container_bytes(
            4,
            &[past_the_end],
            &[0; 256],
        )));
        assert!(matches!(
            indirect,
            Err(Failure { bail: Bail::NoTarget(id), .. }) if id == past_the_end
        ));

        let mut packed = [0u64; 1024];
        packed[0] = u64::try_from(past_the_end).expect("a positive id");
        let direct = sections(&section_bytes(&container_bytes(15, &[], &packed)));
        assert!(matches!(
            direct,
            Err(Failure { bail: Bail::NoTarget(id), .. }) if id == past_the_end
        ));
    }

    #[test]
    fn a_rewritten_column_decodes_as_775() {
        let palette = [BELOW_FIRST_RUN, FIRST_RUN, SECOND_RUN, LAST_RUN];
        let one = section_bytes(&container_bytes(4, &palette, &[0x3210_3210_3210_3210; 256]));
        let mut blob = Vec::new();
        for _ in 0..24 {
            blob.extend_from_slice(&one);
        }

        let out = sections(&blob).expect("a whole column");
        let chunk = azalea_world::Chunk::read_with_dimension_height(
            &mut std::io::Cursor::new(&out[..]),
            384,
            -64,
            &[],
        )
        .expect("azalea decodes the rewritten column");
        assert_eq!(chunk.sections.len(), 24, "one section per sixteen blocks");
        assert_eq!(
            chunk.sections[0].fluid_count, 0,
            "the invented fluid count landed where 775 reads it"
        );
    }

    #[test]
    fn the_failure_names_its_section() {
        let good = section_bytes(&container_bytes(0, &[FIRST_RUN], &[]));
        let bad = section_bytes(&container_bytes(0, &[i32::MAX], &[]));
        let mut blob = good.clone();
        blob.extend_from_slice(&good);
        blob.extend_from_slice(&bad);
        assert!(matches!(
            sections(&blob),
            Err(Failure {
                bail: Bail::NoTarget(_),
                section: 2,
                ..
            })
        ));
    }

    #[test]
    fn a_block_update_moves_its_state() {
        let mut body = vec![0u8; 8];
        body[7] = 0x2A;
        write_varint(&mut body, LAST_RUN);
        let out = block_update(&body).unwrap();
        let mut r = Reader::new(&out);
        assert_eq!(r.take(8), Some(&body[..8]), "the position is untouched");
        assert_eq!(r.varint(), Some(moved(LAST_RUN)));
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_section_update_moves_states_and_keeps_positions() {
        let changes = [
            (FIRST_RUN, 0xABCu64),
            (SECOND_RUN, 0x001),
            (LAST_RUN, 0xFFF),
        ];
        let mut body = vec![0u8; 8];
        write_varint(&mut body, changes.len() as i32);
        for (id, pos) in changes {
            write_varlong(&mut body, ((*&id as u64) << 12 | pos) as i64);
        }

        let out = section_blocks_update(&body).unwrap();
        let mut r = Reader::new(&out);
        assert_eq!(r.take(8), Some(&[0u8; 8][..]));
        assert_eq!(r.count(), Some(changes.len()));
        for (id, pos) in changes {
            let packed = r.varlong().unwrap() as u64;
            assert_eq!(packed & 0xFFF, pos, "the position moved");
            assert_eq!(i32::try_from(packed >> 12).unwrap(), moved(id));
        }
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn an_impossible_state_does_not_rewrite() {
        let mut body = vec![0u8; 8];
        write_varint(&mut body, 29_671);
        assert!(block_update(&body).is_none());
    }

    const ITEM_AT_THE_SEAM: i32 = 230;
    const COMPONENT_AT_THE_SEAM: i32 = 41;
    const COW_VARIANT_774: i32 = 22;
    const COW_VARIANT_775: i32 = 23;

    fn translated(f: impl FnOnce(&mut Reader, &mut Vec<u8>) -> Option<()>, body: &[u8]) -> Vec<u8> {
        let mut r = Reader::new(body);
        let mut out = Vec::new();
        f(&mut r, &mut out).expect("a whole value");
        assert_eq!(r.remaining(), 0, "the walker did not consume the input");
        out
    }

    #[test]
    fn the_item_stack_serializer_is_where_we_think() {
        let mut probe = Vec::new();
        write_varint(&mut probe, SERIALIZER_ITEM_STACK);
        write_varint(&mut probe, 0);
        let mut cursor = std::io::Cursor::new(&probe[..]);
        assert!(matches!(
            azalea::entity::EntityDataValue::azalea_read(&mut cursor),
            Ok(azalea::entity::EntityDataValue::ItemStack(_))
        ));
    }

    #[test]
    fn an_empty_stack_is_a_count_and_nothing_else() {
        let mut body = Vec::new();
        write_varint(&mut body, 0);
        assert_eq!(translated(item_stack, &body), body);
    }

    #[test]
    fn a_stack_moves_its_item_and_its_component_names() {
        let mut body = Vec::new();
        write_varint(&mut body, 3);
        write_varint(&mut body, ITEM_AT_THE_SEAM);
        write_varint(&mut body, 0);
        write_varint(&mut body, 2);
        write_varint(&mut body, COMPONENT_AT_THE_SEAM - 1);
        write_varint(&mut body, COMPONENT_AT_THE_SEAM);

        let out = translated(item_stack, &body);
        let mut r = Reader::new(&out);
        assert_eq!(r.varint(), Some(3), "the count is not an id");
        assert_eq!(r.varint(), Some(ITEM_AT_THE_SEAM + 1));
        assert_eq!(r.count(), Some(0));
        assert_eq!(r.count(), Some(2));
        assert_eq!(
            r.varint(),
            Some(COMPONENT_AT_THE_SEAM - 1),
            "below the seam"
        );
        assert_eq!(r.varint(), Some(COMPONENT_AT_THE_SEAM + 1), "at it");
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_component_past_the_seam_keeps_its_payload() {
        let native = DataComponentKind::MapId.to_u32() as i32;
        let wire = Remap::from_native()
            .map(IdSpace::DataComponentType, native)
            .expect("775 map_id exists in 774");
        assert_ne!(wire, native, "pick a component that actually moved");

        let mut body = Vec::new();
        write_varint(&mut body, 1);
        write_varint(&mut body, 0);
        write_varint(&mut body, 1);
        write_varint(&mut body, 0);
        write_varint(&mut body, wire);
        write_varint(&mut body, 1234);

        let out = translated(item_stack, &body);
        let mut r = Reader::new(&out);
        r.varint();
        r.varint();
        assert_eq!(r.count(), Some(1));
        assert_eq!(r.count(), Some(0));
        assert_eq!(r.varint(), Some(native), "the component moved");
        assert_eq!(r.varint(), Some(1234), "and its payload came with it");
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn entity_metadata_moves_serializer_ids_and_keeps_values() {
        let mut body = Vec::new();
        write_varint(&mut body, 77);
        body.push(0);
        write_varint(&mut body, 0);
        body.push(0x1F);
        body.push(9);
        write_varint(&mut body, COW_VARIANT_774);
        write_varint(&mut body, 3);
        body.push(0xFF);

        let out = entity_data(&body).expect("a whole packet");
        let mut r = Reader::new(&out);
        assert_eq!(r.varint(), Some(77));
        assert_eq!(r.u8(), Some(0));
        assert_eq!(r.varint(), Some(0));
        assert_eq!(r.u8(), Some(0x1F), "a byte value survived");
        assert_eq!(r.u8(), Some(9));
        assert_eq!(r.varint(), Some(COW_VARIANT_775), "the serializer moved");
        assert_eq!(r.varint(), Some(3), "and its value did not");
        assert_eq!(r.u8(), Some(0xFF));
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_stack_inside_metadata_is_translated_too() {
        let mut body = Vec::new();
        write_varint(&mut body, 5);
        body.push(2);
        write_varint(&mut body, SERIALIZER_ITEM_STACK);
        write_varint(&mut body, 1);
        write_varint(&mut body, ITEM_AT_THE_SEAM);
        write_varint(&mut body, 0);
        write_varint(&mut body, 0);
        body.push(0xFF);

        let out = entity_data(&body).unwrap();
        let mut r = Reader::new(&out);
        r.varint();
        r.u8();
        assert_eq!(r.varint(), Some(SERIALIZER_ITEM_STACK));
        assert_eq!(r.varint(), Some(1));
        assert_eq!(
            r.varint(),
            Some(ITEM_AT_THE_SEAM + 1),
            "the held item moved"
        );
    }

    #[test]
    fn a_stonecutter_result_becomes_a_template_under_the_moved_type() {
        let mut body = Vec::new();
        write_varint(&mut body, 0);
        write_varint(&mut body, 1);
        write_varint(&mut body, 2);
        write_varint(&mut body, 1);
        write_varint(&mut body, SLOT_DISPLAY_ITEM_STACK);
        write_varint(&mut body, 2);
        write_varint(&mut body, 3);
        write_varint(&mut body, 0);
        write_varint(&mut body, 0);

        let out = update_recipes(&body).expect("rewrites");
        let mut expected = Vec::new();
        write_varint(&mut expected, 0);
        write_varint(&mut expected, 1);
        write_varint(&mut expected, 2);
        write_varint(&mut expected, 1);
        write_varint(&mut expected, 5);
        write_varint(&mut expected, 3);
        write_varint(&mut expected, 2);
        write_varint(&mut expected, 0);
        write_varint(&mut expected, 0);
        assert_eq!(&*out, &expected[..]);
    }

    #[test]
    fn slot_display_types_land_on_the_same_names() {
        use azalea_registry::builtin::SlotDisplay;
        let names = [
            "empty",
            "any_fuel",
            "item",
            "item_stack",
            "tag",
            "smithing_trim",
            "with_remainder",
            "composite",
        ];
        for (old, name) in names.iter().enumerate() {
            let native = SLOT_DISPLAY_TO_NATIVE[old] as u32;
            let kind = SlotDisplay::from_u32(native).expect("in range");
            assert_eq!(kind.to_str().trim_start_matches("minecraft:"), *name);
        }
    }

    #[test]
    fn a_truncated_stack_or_metadata_does_not_rewrite() {
        assert!(entity_data(&[]).is_none());
        assert!(entity_data(&[0x01, 0x00]).is_none());
        let mut body = Vec::new();
        write_varint(&mut body, 1);
        assert!(item_stack(&mut Reader::new(&body), &mut Vec::new()).is_none());
    }

    #[test]
    fn a_short_interact_does_not_rewrite() {
        assert!(interact(&[], true).is_none());
        let mut body = Vec::new();
        write_varint(&mut body, 7);
        write_varint(&mut body, 0);
        body.push(0x01);
        assert!(interact(&body, false).is_none());
    }

    #[test]
    fn an_unrewritable_attack_is_dropped_rather_than_sent() {
        let h = hop();
        let mut frame = Vec::new();
        write_varint(&mut frame, h.attack as i32);
        assert!(
            h.outbound(ConnectionProtocol::Game, frame.into_boxed_slice())
                .is_empty()
        );
    }

    const PAYLOAD_774: [i32; 18] = [
        1, 2, 8, 14, 15, 16, 21, 29, 36, 38, 42, 46, 47, 48, 49, 103, 109, 113,
    ];
    const PARTICLES_774: i32 = 115;

    #[test]
    fn a_particles_payload_shape_survives_the_remap() {
        for wire in 0..PARTICLES_774 {
            let native = Remap::to_native()
                .map(IdSpace::ParticleType, wire)
                .unwrap_or_else(|| panic!("774 particle {wire} does not map"));
            let mut alone = Vec::new();
            write_varint(&mut alone, native);
            let mut cursor = std::io::Cursor::new(&alone[..]);
            let payload_free = azalea::entity::particle::Particle::azalea_read(&mut cursor).is_ok();
            assert_eq!(
                payload_free,
                !PAYLOAD_774.contains(&wire),
                "774 particle {wire} lands on 775 {native}, which has the other shape"
            );
        }
    }

    #[test]
    fn without_the_remap_five_ids_would_change_shape() {
        let mismatched: Vec<i32> = (0..PARTICLES_774)
            .filter(|wire| {
                let mut alone = Vec::new();
                write_varint(&mut alone, *wire);
                let mut cursor = std::io::Cursor::new(&alone[..]);
                let payload_free =
                    azalea::entity::particle::Particle::azalea_read(&mut cursor).is_ok();
                payload_free == PAYLOAD_774.contains(wire)
            })
            .collect();
        assert_eq!(
            mismatched,
            [103, 105, 109, 111, 113],
            "shriek, dust_plume, dust_pillar, raid_omen and block_crumble"
        );
    }

    #[test]
    fn a_block_particle_moves_its_id_and_its_state() {
        let mut body = Vec::new();
        write_varint(&mut body, 109);
        write_varint(&mut body, SECOND_RUN);

        let mut r = Reader::new(&body);
        let mut out = Vec::new();
        particle(&mut r, &mut out, &mut Vec::new()).expect("a whole particle");
        assert_eq!(r.remaining(), 0);

        let mut r = Reader::new(&out);
        assert_eq!(
            r.varint(),
            Some(111),
            "dust_pillar moved by the two inserts"
        );
        assert_eq!(r.varint(), Some(moved(SECOND_RUN)));
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_measured_particle_keeps_its_payload() {
        let mut body = Vec::new();
        write_varint(&mut body, 103);
        write_varint(&mut body, 40);

        let mut r = Reader::new(&body);
        let mut out = Vec::new();
        particle(&mut r, &mut out, &mut Vec::new()).unwrap();
        assert_eq!(r.remaining(), 0, "the delay was consumed");

        let mut r = Reader::new(&out);
        assert_eq!(r.varint(), Some(105));
        assert_eq!(r.varint(), Some(40));
    }

    #[test]
    fn a_payload_free_particle_is_its_id_alone() {
        let mut body = Vec::new();
        write_varint(&mut body, 3);
        body.extend_from_slice(&[0xAA, 0xBB]);

        let mut r = Reader::new(&body);
        let mut out = Vec::new();
        particle(&mut r, &mut out, &mut Vec::new()).unwrap();
        assert_eq!(out, [3], "only the id");
        assert_eq!(r.rest(), [0xAA, 0xBB], "and nothing after it was taken");
    }

    #[test]
    fn level_particles_keeps_its_header() {
        let mut body = vec![1, 0];
        body.extend_from_slice(&[0u8; 3 * 8]);
        body.extend_from_slice(&[0u8; 4 * 4]);
        body.extend_from_slice(&7i32.to_be_bytes());
        let header = body.len();
        write_varint(&mut body, 109);
        write_varint(&mut body, FIRST_RUN);

        let out = level_particles(&body).expect("a whole packet");
        assert_eq!(&out[..header], &body[..header]);
        let mut r = Reader::new(&out[header..]);
        assert_eq!(r.varint(), Some(111));
        assert_eq!(r.varint(), Some(moved(FIRST_RUN)));
    }

    const SOUND_774: i32 = 280;

    #[test]
    fn a_sound_moves_through_the_holder() {
        let native = Remap::to_native()
            .map(IdSpace::SoundEvent, SOUND_774)
            .expect("774 sound 280 exists in 775");
        assert_ne!(native, SOUND_774, "pick a sound that actually moved");

        let mut body = Vec::new();
        write_varint(&mut body, SOUND_774 + 1);
        body.extend_from_slice(&[0xAA; 4]);

        let out = sound(&body).expect("a whole packet");
        let mut r = Reader::new(&out);
        assert_eq!(
            r.varint(),
            Some(native + 1),
            "the holder is id + 1, not the id"
        );
        assert_eq!(r.rest(), [0xAA; 4], "the rest of the packet is copied");
    }

    #[test]
    fn an_inline_sound_is_passed_through() {
        let mut body = Vec::new();
        write_varint(&mut body, 0);
        write_varint(&mut body, 3);
        body.extend_from_slice(b"a:b");
        body.push(1);
        body.extend_from_slice(&1f32.to_be_bytes());
        body.push(0x77);

        let out = sound(&body).expect("a whole packet");
        assert_eq!(&*out, &body[..], "an inline sound is copied verbatim");
    }

    #[test]
    fn explode_moves_its_particle_its_sound_and_every_weighted_one() {
        let mut body = Vec::new();
        body.extend_from_slice(&[0u8; 3 * 8]);
        body.extend_from_slice(&2f32.to_be_bytes());
        body.extend_from_slice(&5i32.to_be_bytes());
        body.push(0);
        write_varint(&mut body, 109);
        write_varint(&mut body, FIRST_RUN);
        write_varint(&mut body, SOUND_774 + 1);
        write_varint(&mut body, 1);
        write_varint(&mut body, 3);
        body.extend_from_slice(&[0u8; 8]);
        write_varint(&mut body, 4);

        let out = explode(&body).expect("a whole packet");
        let mut r = Reader::new(&out);
        assert_eq!(r.take(3 * 8 + 4 + 4), Some(&body[..32]));
        assert_eq!(r.u8(), Some(0));
        assert_eq!(r.varint(), Some(111));
        assert_eq!(r.varint(), Some(moved(FIRST_RUN)));
        assert_eq!(
            r.varint(),
            Remap::to_native()
                .map(IdSpace::SoundEvent, SOUND_774)
                .map(|id| id + 1)
        );
        assert_eq!(r.count(), Some(1));
        assert_eq!(r.varint(), Some(3), "below the seam, so unmoved");
        assert_eq!(r.take(8), Some(&[0u8; 8][..]));
        assert_eq!(r.varint(), Some(4));
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn the_serializers_with_ids_are_where_we_think() {
        use azalea::entity::EntityDataValue as V;
        let read = |serializer: i32, payload: &[u8]| {
            let mut probe = Vec::new();
            write_varint(&mut probe, serializer);
            probe.extend_from_slice(payload);
            let mut cursor = std::io::Cursor::new(&probe[..]);
            V::azalea_read(&mut cursor)
        };
        let mut state = Vec::new();
        write_varint(&mut state, 1);
        assert!(matches!(
            read(SERIALIZER_BLOCK_STATE, &state),
            Ok(V::BlockState(_))
        ));
        assert!(matches!(
            read(SERIALIZER_OPTIONAL_BLOCK_STATE, &state),
            Ok(V::OptionalBlockState(_))
        ));
        assert!(matches!(
            read(SERIALIZER_PARTICLE, &[3]),
            Ok(V::Particle(_))
        ));
        assert!(matches!(
            read(SERIALIZER_PARTICLES, &[0]),
            Ok(V::Particles(_))
        ));
    }

    #[test]
    fn metadata_moves_block_states_and_particles() {
        let mut body = Vec::new();
        write_varint(&mut body, 4);
        body.push(0);
        write_varint(&mut body, SERIALIZER_BLOCK_STATE);
        write_varint(&mut body, LAST_RUN);
        body.push(1);
        write_varint(&mut body, SERIALIZER_PARTICLES);
        write_varint(&mut body, 1);
        write_varint(&mut body, 109);
        write_varint(&mut body, FIRST_RUN);
        body.push(0xFF);

        let out = entity_data(&body).expect("a whole packet");
        let mut r = Reader::new(&out);
        r.varint();
        r.u8();
        assert_eq!(r.varint(), Some(SERIALIZER_BLOCK_STATE));
        assert_eq!(r.varint(), Some(moved(LAST_RUN)));
        r.u8();
        assert_eq!(r.varint(), Some(SERIALIZER_PARTICLES));
        assert_eq!(r.count(), Some(1));
        assert_eq!(r.varint(), Some(111));
        assert_eq!(r.varint(), Some(moved(FIRST_RUN)));
        assert_eq!(r.u8(), Some(0xFF));
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_block_event_moves_the_block_and_not_the_position() {
        let mut body = vec![0u8; 8];
        body[7] = 0x11;
        body.push(1);
        body.push(2);
        write_varint(&mut body, 500);

        let out = block_event(&body).unwrap();
        let mut r = Reader::new(&out);
        assert_eq!(r.take(10), Some(&body[..10]));
        assert_eq!(r.varint(), Some(502), "past both inserted blocks");
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn award_stats_moves_blocks_and_items_by_their_own_tables() {
        let mut body = Vec::new();
        write_varint(&mut body, 3);
        write_varint(&mut body, STAT_MINED);
        write_varint(&mut body, 500);
        write_varint(&mut body, 7);
        write_varint(&mut body, STAT_DROPPED);
        write_varint(&mut body, ITEM_AT_THE_SEAM);
        write_varint(&mut body, 8);
        write_varint(&mut body, 6);
        write_varint(&mut body, 40);
        write_varint(&mut body, 9);

        let out = award_stats(&body).unwrap();
        let mut r = Reader::new(&out);
        assert_eq!(r.count(), Some(3));
        assert_eq!(
            (r.varint(), r.varint(), r.varint()),
            (Some(0), Some(502), Some(7))
        );
        assert_eq!(
            (r.varint(), r.varint(), r.varint()),
            (Some(5), Some(ITEM_AT_THE_SEAM + 1), Some(8))
        );
        assert_eq!(
            (r.varint(), r.varint(), r.varint()),
            (Some(6), Some(40), Some(9)),
            "entity types are identity between these versions"
        );
    }

    fn add_entity_bytes(kind: i32, data: i32) -> Vec<u8> {
        let mut body = Vec::new();
        write_varint(&mut body, 11);
        body.extend_from_slice(&[0u8; 16]);
        write_varint(&mut body, kind);
        body.extend_from_slice(&[0u8; 3 * 8]);
        body.push(0);
        body.extend_from_slice(&[0u8; 3]);
        write_varint(&mut body, data);
        body
    }

    #[test]
    fn a_falling_block_moves_its_state_and_nothing_else_does() {
        let falling = EntityKind::FallingBlock.to_u32() as i32;
        let out = add_entity(&add_entity_bytes(falling, LAST_RUN)).expect("a falling block");
        let mut r = Reader::new(&out);
        r.varint();
        r.take(16);
        assert_eq!(r.varint(), Some(falling));
        r.take(3 * 8 + 1 + 3);
        assert_eq!(r.varint(), Some(moved(LAST_RUN)));

        let arrow = EntityKind::Arrow.to_u32() as i32;
        assert!(
            add_entity(&add_entity_bytes(arrow, LAST_RUN)).is_none(),
            "only a falling block reads that field as a state"
        );
    }

    #[test]
    fn the_two_block_breaking_level_events_move_their_data() {
        for kind in [LEVEL_EVENT_DESTROY_BLOCK, LEVEL_EVENT_BRUSH_COMPLETE] {
            let mut body = Vec::new();
            body.extend_from_slice(&kind.to_be_bytes());
            body.extend_from_slice(&[0u8; 8]);
            body.extend_from_slice(&FIRST_RUN.to_be_bytes());
            body.push(1);

            let out = level_event(&body).unwrap_or_else(|| panic!("event {kind}"));
            let mut r = Reader::new(&out);
            assert_eq!(r.take(4 + 8), Some(&body[..12]));
            assert_eq!(
                i32::from_be_bytes(r.take(4).unwrap().try_into().unwrap()),
                moved(FIRST_RUN)
            );
            assert_eq!(r.u8(), Some(1));
        }
        let mut body = Vec::new();
        body.extend_from_slice(&2013u32.to_be_bytes());
        body.extend_from_slice(&[0u8; 8]);
        body.extend_from_slice(&30i32.to_be_bytes());
        body.push(0);
        assert!(level_event(&body).is_none());
    }

    #[test]
    fn a_hashed_stack_moves_its_ids_backwards() {
        let mut body = vec![1];
        write_varint(&mut body, ITEM_AT_THE_SEAM + 1);
        write_varint(&mut body, 2);
        write_varint(&mut body, 1);
        write_varint(&mut body, 1);
        write_varint(&mut body, COMPONENT_AT_THE_SEAM + 1);
        body.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        write_varint(&mut body, COMPONENT_AT_THE_SEAM + 3);

        let mut r = Reader::new(&body);
        let mut out = Vec::new();
        assert_eq!(hashed_stack(&mut r, &mut out), Some(true));
        assert_eq!(r.remaining(), 0);

        let mut r = Reader::new(&out);
        assert_eq!(r.u8(), Some(1));
        assert_eq!(r.varint(), Some(ITEM_AT_THE_SEAM));
        assert_eq!(r.varint(), Some(2), "the count is not an id");
        assert_eq!((r.count(), r.count()), (Some(1), Some(1)));
        assert_eq!(r.varint(), Some(COMPONENT_AT_THE_SEAM));
        assert_eq!(r.take(4), Some(&[0xDE, 0xAD, 0xBE, 0xEF][..]));
        assert_eq!(r.varint(), Some(COMPONENT_AT_THE_SEAM + 1));
    }

    #[test]
    fn a_775_only_item_empties_the_desync_check_rather_than_the_frame() {
        const GOLDEN_DANDELION: i32 = 230;
        assert_eq!(
            Remap::from_native().map(IdSpace::Item, GOLDEN_DANDELION),
            None,
            "775 inserted this one, so 774 has nothing to call it"
        );

        let mut body = Vec::new();
        write_varint(&mut body, 1);
        write_varint(&mut body, 2);
        body.extend_from_slice(&3i16.to_be_bytes());
        body.push(0);
        write_varint(&mut body, 0);
        write_varint(&mut body, 1);
        body.extend_from_slice(&4u16.to_be_bytes());
        body.push(1);
        write_varint(&mut body, GOLDEN_DANDELION);
        write_varint(&mut body, 1);
        write_varint(&mut body, 0);
        write_varint(&mut body, 0);
        body.push(0);

        let out = container_click(&body).expect("the click still goes out");
        let mut r = Reader::new(&out);
        assert_eq!((r.varint(), r.varint()), (Some(1), Some(2)));
        r.take(3);
        r.varint();
        assert_eq!(r.count(), Some(0), "the map was emptied, not the frame");
        assert_eq!(r.u8(), Some(0), "the cursor was empty already");
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_creative_slot_moves_its_item_and_copies_delimited_payloads() {
        let mut body = 9u16.to_be_bytes().to_vec();
        write_varint(&mut body, 1);
        write_varint(&mut body, ITEM_AT_THE_SEAM + 1);
        write_varint(&mut body, 1);
        write_varint(&mut body, 0);
        write_varint(&mut body, COMPONENT_AT_THE_SEAM + 1);
        write_varint(&mut body, 3);
        body.extend_from_slice(&[1, 2, 3]);

        let out = creative_slot(&body).expect("a whole stack");
        let mut r = Reader::new(&out);
        assert_eq!(r.take(2), Some(&9u16.to_be_bytes()[..]));
        assert_eq!(r.varint(), Some(1));
        assert_eq!(r.varint(), Some(ITEM_AT_THE_SEAM));
        assert_eq!((r.count(), r.count()), (Some(1), Some(0)));
        assert_eq!(r.varint(), Some(COMPONENT_AT_THE_SEAM));
        assert_eq!(r.count(), Some(3), "the payload keeps its own length");
        assert_eq!(r.take(3), Some(&[1, 2, 3][..]));
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_creative_slot_of_a_775_only_item_is_cleared() {
        let mut body = 9u16.to_be_bytes().to_vec();
        write_varint(&mut body, 1);
        write_varint(&mut body, 230);
        write_varint(&mut body, 0);
        write_varint(&mut body, 0);

        let out = creative_slot(&body).expect("the slot is still set");
        let mut r = Reader::new(&out);
        assert_eq!(r.take(2), Some(&9u16.to_be_bytes()[..]));
        assert_eq!(r.varint(), Some(0), "an empty stack is a zero count");
        assert_eq!(r.remaining(), 0);
    }
}
