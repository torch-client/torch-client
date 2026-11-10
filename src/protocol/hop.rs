use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use azalea_protocol::packets::ConnectionProtocol;

use azalea::block::BlockState;
use azalea_buf::AzBuf;
use azalea_protocol::packets::config::c_registry_data::ClientboundRegistryData;
use azalea_registry::Registry as _;
use azalea_registry::builtin::{DataComponentKind, EntityKind, ParticleKind};
use azalea_registry::data::Biome;
use azalea_registry::identifier::Identifier;
use azalea_world::palette::PalletedContainerKind;

use super::fields::{self, Fields, Run};
use super::packets::{Direction, PacketTable, Phase};
use super::remap::{IdSpace, Remap};
use super::version::NATIVE;
use super::wire::{Reader, varint_len, write_varint, write_varlong};

const ACTION_INTERACT: i32 = 0;
const ACTION_ATTACK: i32 = 1;

pub(crate) struct Ids {
    to_native: &'static Remap,
    from_native: &'static Remap,
    wire: i32,
}

impl Ids {
    fn v774(&self) -> bool {
        self.wire == 774
    }
}

type Rewrite = fn(&Ids, &[u8]) -> Option<Box<[u8]>>;

struct HopSpec {
    protocol: i32,
    login: &'static [(&'static str, Rewrite)],
    configuration: &'static [(&'static str, Rewrite)],
    layout_in: &'static [(&'static str, Rewrite)],
    inbound: &'static [(&'static str, Rewrite)],
    outbound: &'static [(&'static str, &'static str, Rewrite)],
    layout_out: &'static [(&'static str, &'static str, Rewrite)],
}

const RENAMED: &[(&str, &str)] = &[("spectator_action", "spectate_entity")];

fn older_name(native: &str) -> Option<&'static str> {
    RENAMED
        .iter()
        .find(|(name, _)| *name == native)
        .map(|(_, older)| *older)
}

fn newer_name(wire: &str) -> Option<&'static str> {
    RENAMED
        .iter()
        .find(|(_, older)| *older == wire)
        .map(|(name, _)| *name)
}

const LOGIN_LAYOUT: &[(&str, Rewrite)] = &[("login_finished", login_finished)];

const CONFIGURATION: &[(&str, Rewrite)] = &[("update_tags", update_tags)];

const LAYOUT_IN: &[(&str, Rewrite)] = &[("login", login), ("set_player_team", set_player_team)];

const V774: HopSpec = HopSpec {
    protocol: 774,
    login: LOGIN_LAYOUT,
    configuration: CONFIGURATION,
    layout_in: LAYOUT_IN,
    inbound: &[
        ("set_time", set_time),
        ("level_chunk_with_light", level_chunk),
        ("block_update", block_update),
        ("section_blocks_update", section_blocks_update),
        ("block_entity_data", block_entity_data),
        ("set_entity_data", entity_data),
        ("container_set_content", container_set_content),
        ("container_set_slot", container_set_slot),
        ("set_cursor_item", set_cursor_item),
        ("set_player_inventory", set_player_inventory),
        ("set_equipment", set_equipment),
        ("add_entity", add_entity),
        ("update_attributes", update_attributes),
        ("block_event", block_event),
        ("award_stats", award_stats),
        ("level_event", level_event),
        ("level_particles", level_particles),
        ("explode", explode),
        ("sound", sound),
        ("sound_entity", sound),
        ("update_recipes", update_recipes),
        ("recipe_book_add", recipe_book_add),
        ("place_ghost_recipe", place_ghost_recipe),
        ("update_advancements", update_advancements),
        ("merchant_offers", merchant_offers),
        ("update_tags", update_tags),
    ],
    outbound: &[
        ("attack", "interact", attack),
        ("interact", "interact", interact_use),
        ("container_click", "container_click", container_click),
        (
            "set_creative_mode_slot",
            "set_creative_mode_slot",
            creative_slot,
        ),
    ],
    layout_out: &[],
};

const V775: HopSpec = HopSpec {
    protocol: 775,
    login: LOGIN_LAYOUT,
    configuration: CONFIGURATION,
    layout_in: LAYOUT_IN,
    inbound: &[
        ("level_chunk_with_light", level_chunk),
        ("block_update", block_update),
        ("section_blocks_update", section_blocks_update),
        ("block_entity_data", block_entity_data),
        ("set_entity_data", entity_data),
        ("container_set_content", container_set_content),
        ("container_set_slot", container_set_slot),
        ("set_cursor_item", set_cursor_item),
        ("set_player_inventory", set_player_inventory),
        ("set_equipment", set_equipment),
        ("add_entity", add_entity),
        ("update_attributes", update_attributes),
        ("block_event", block_event),
        ("award_stats", award_stats),
        ("level_event", level_event),
        ("level_particles", level_particles),
        ("explode", explode),
        ("sound", sound),
        ("sound_entity", sound),
        ("update_recipes", update_recipes),
        ("recipe_book_add", recipe_book_add),
        ("place_ghost_recipe", place_ghost_recipe),
        ("update_advancements", update_advancements),
        ("merchant_offers", merchant_offers),
        ("update_tags", update_tags),
    ],
    outbound: &[
        ("container_click", "container_click", container_click),
        (
            "set_creative_mode_slot",
            "set_creative_mode_slot",
            creative_slot,
        ),
    ],
    layout_out: &[("spectator_action", "spectate_entity", spectator_action)],
};

#[derive(Clone, Copy, Default)]
struct Stages {
    layout: Option<Rewrite>,
    ids: Option<Rewrite>,
}

#[derive(Clone, Copy)]
struct Outbound {
    wire: Option<u32>,
    ids: Option<Rewrite>,
    layout: Option<Rewrite>,
}

enum Rewritten {
    Untouched,
    Failed,
    Body(Box<[u8]>),
}

struct Watched {
    add_entity: u32,
    remove_entities: u32,
    set_entity_data: u32,
    login: u32,
    registry_data: u32,
}

pub(crate) struct Hop {
    ids: Ids,
    wire: &'static PacketTable,
    native: &'static PacketTable,
    inbound_game: Box<[u32]>,
    outbound_game: Box<[Option<u32>]>,
    inbound_rewrite: Box<[Stages]>,
    outbound_rewrite: Box<[Option<Outbound>]>,
    login_rewrite: Box<[Option<Rewrite>]>,
    config_rewrite: Box<[Option<Rewrite>]>,
    watched: Watched,
    fields: &'static Fields,
    trace: bool,
}

impl Hop {
    pub(crate) fn v774() -> &'static Hop {
        static HOP: OnceLock<Hop> = OnceLock::new();
        HOP.get_or_init(|| Hop::build(&V774))
    }

    pub(crate) fn v775() -> &'static Hop {
        static HOP: OnceLock<Hop> = OnceLock::new();
        HOP.get_or_init(|| Hop::build(&V775))
    }

    fn build(spec: &HopSpec) -> Hop {
        let old = PacketTable::for_protocol(spec.protocol)
            .unwrap_or_else(|| panic!("no packet table for {}", spec.protocol));
        let new = PacketTable::native();
        let id = |dir, name: &str| {
            new.id(Phase::Game, dir, name)
                .unwrap_or_else(|| panic!("{} has no game packet {name}", NATIVE.name))
        };
        let wire_id = |dir, name: &str| {
            old.id(Phase::Game, dir, name)
                .or_else(|| older_name(name).and_then(|older| old.id(Phase::Game, dir, older)))
        };

        let inbound_game = (0..old.count(Phase::Game, Direction::Clientbound))
            .map(|wire| {
                let name = old
                    .name_of(Phase::Game, Direction::Clientbound, wire as u32)
                    .expect("in range");
                match (
                    new.id(Phase::Game, Direction::Clientbound, name),
                    newer_name(name),
                ) {
                    (Some(native), _) => native,
                    (None, Some(newer)) => id(Direction::Clientbound, newer),
                    (None, None) => id(Direction::Clientbound, name),
                }
            })
            .collect();
        let outbound_game = (0..new.count(Phase::Game, Direction::Serverbound))
            .map(|native| {
                let name = new
                    .name_of(Phase::Game, Direction::Serverbound, native as u32)
                    .expect("in range");
                wire_id(Direction::Serverbound, name)
            })
            .collect();

        let mut inbound_rewrite: Box<[Stages]> =
            vec![Stages::default(); new.count(Phase::Game, Direction::Clientbound)]
                .into_boxed_slice();
        for &(name, rewrite) in spec.layout_in {
            let slot = &mut inbound_rewrite[id(Direction::Clientbound, name) as usize];
            assert!(
                slot.layout.replace(rewrite).is_none(),
                "{name} has two inbound layout rewrites"
            );
        }
        for &(name, rewrite) in spec.inbound {
            let slot = &mut inbound_rewrite[id(Direction::Clientbound, name) as usize];
            assert!(
                slot.ids.replace(rewrite).is_none(),
                "{name} is rewritten inbound twice"
            );
        }

        let mut outbound_rewrite: Box<[Option<Outbound>]> =
            vec![None; new.count(Phase::Game, Direction::Serverbound)].into_boxed_slice();
        for (list, layout) in [(spec.outbound, false), (spec.layout_out, true)] {
            for &(name, wire_name, rewrite) in list {
                let wire = old.id(Phase::Game, Direction::Serverbound, wire_name);
                let slot = outbound_rewrite[id(Direction::Serverbound, name) as usize]
                    .get_or_insert(Outbound {
                        wire,
                        ids: None,
                        layout: None,
                    });
                assert_eq!(slot.wire, wire, "{name} goes out under two names");
                let stage = if layout {
                    &mut slot.layout
                } else {
                    &mut slot.ids
                };
                assert!(
                    stage.replace(rewrite).is_none(),
                    "{name} is rewritten outbound twice"
                );
            }
        }

        let fixed = |phase: Phase, list: &[(&str, Rewrite)]| {
            let mut table: Box<[Option<Rewrite>]> =
                vec![None; new.count(phase, Direction::Clientbound)].into_boxed_slice();
            for &(name, rewrite) in list {
                let native = new
                    .id(phase, Direction::Clientbound, name)
                    .unwrap_or_else(|| panic!("{} has no {phase:?} packet {name}", NATIVE.name));
                assert_eq!(
                    old.id(phase, Direction::Clientbound, name),
                    Some(native),
                    "{phase:?} packet {name} moved"
                );
                table[native as usize] = Some(rewrite);
            }
            table
        };
        let login_rewrite = fixed(Phase::Login, spec.login);
        let config_rewrite = fixed(Phase::Configuration, spec.configuration);

        let watched = Watched {
            add_entity: id(Direction::Clientbound, "add_entity"),
            remove_entities: id(Direction::Clientbound, "remove_entities"),
            set_entity_data: id(Direction::Clientbound, "set_entity_data"),
            login: id(Direction::Clientbound, "login"),
            registry_data: new
                .id(
                    Phase::Configuration,
                    Direction::Clientbound,
                    "registry_data",
                )
                .unwrap_or_else(|| panic!("{} has no configuration registry_data", NATIVE.name)),
        };

        Hop {
            ids: Ids {
                to_native: Remap::to_native(spec.protocol)
                    .unwrap_or_else(|| panic!("no remap from {}", spec.protocol)),
                from_native: Remap::from_native(spec.protocol)
                    .unwrap_or_else(|| panic!("no remap to {}", spec.protocol)),
                wire: spec.protocol,
            },
            wire: old,
            native: new,
            inbound_game,
            outbound_game,
            inbound_rewrite,
            outbound_rewrite,
            login_rewrite,
            config_rewrite,
            watched,
            fields: Fields::for_wire(spec.protocol)
                .unwrap_or_else(|| panic!("no metadata table from {}", spec.protocol)),
            trace: crate::diag::debug_on("proto"),
        }
    }

    pub(crate) fn inbound(&self, phase: ConnectionProtocol, raw: Box<[u8]>) -> Box<[u8]> {
        match phase {
            ConnectionProtocol::Game => self.inbound_game(raw),
            ConnectionProtocol::Login => {
                self.inbound_fixed(Phase::Login, &self.login_rewrite, phase, raw)
            }
            ConnectionProtocol::Configuration => {
                self.inbound_fixed(Phase::Configuration, &self.config_rewrite, phase, raw)
            }
            _ => {
                if self.trace {
                    passed_through("in", phase, &raw);
                }
                raw
            }
        }
    }

    fn inbound_fixed(
        &self,
        phase_key: Phase,
        table: &[Option<Rewrite>],
        phase: ConnectionProtocol,
        raw: Box<[u8]>,
    ) -> Box<[u8]> {
        let mut r = Reader::new(&raw);
        let id = r.varint().and_then(|id| u32::try_from(id).ok());
        let rewrite = id.and_then(|id| table.get(id as usize).copied().flatten());
        let (Some(id), Some(rewrite)) = (id, rewrite) else {
            if self.trace {
                passed_through("in", phase, &raw);
            }
            return raw;
        };
        let head = r.pos();
        let body = r.rest();
        let name = self
            .native
            .name_of(phase_key, Direction::Clientbound, id)
            .unwrap_or("?");
        trail::reset();
        let Some(rewritten) = rewrite(&self.ids, body) else {
            crate::log_warn!(
                "proto",
                "{name}: could not translate the body; passed through as the wire sent it. {}",
                trail::report(body)
            );
            return raw;
        };
        if self.trace {
            crate::log_debug!("proto", "in {phase:?} {name}({id}) +body");
        }
        let mut out = Vec::with_capacity(head + rewritten.len());
        out.extend_from_slice(&raw[..head]);
        out.extend_from_slice(&rewritten);
        out.into_boxed_slice()
    }

    fn inbound_game(&self, raw: Box<[u8]>) -> Box<[u8]> {
        let mut r = Reader::new(&raw);
        let Some(wire_id) = r.varint().and_then(|id| u32::try_from(id).ok()) else {
            return raw;
        };
        let Some(&native_id) = self.inbound_game.get(wire_id as usize) else {
            return raw;
        };
        let body = r.rest();

        let stages = self
            .inbound_rewrite
            .get(native_id as usize)
            .copied()
            .unwrap_or_default();
        let rewritten = match self.stages(
            Direction::Clientbound,
            native_id,
            [stages.layout, stages.ids],
            body,
        ) {
            Rewritten::Body(body) => Some(body),
            Rewritten::Untouched | Rewritten::Failed => None,
        };

        if self.trace {
            self.trace_frame(
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

    fn stages(
        &self,
        dir: Direction,
        native_id: u32,
        stages: [Option<Rewrite>; 2],
        body: &[u8],
    ) -> Rewritten {
        let mut current: Option<Box<[u8]>> = None;
        for rewrite in stages.into_iter().flatten() {
            let input = current.as_deref().unwrap_or(body);
            trail::reset();
            match rewrite(&self.ids, input) {
                Some(out) => current = Some(out),
                None => {
                    let name = self
                        .native
                        .name_of(Phase::Game, dir, native_id)
                        .unwrap_or("?");
                    match dir {
                        Direction::Clientbound => crate::log_warn!(
                            "proto",
                            "{name}: could not translate the body; passed through as the wire sent it. {}",
                            trail::report(input)
                        ),
                        Direction::Serverbound => crate::log_warn!(
                            "proto",
                            "{name}: could not rewrite the body for the wire; the frame was dropped. {}",
                            trail::report(input)
                        ),
                    }
                    return Rewritten::Failed;
                }
            }
        }
        match current {
            Some(body) => Rewritten::Body(body),
            None => Rewritten::Untouched,
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

        if let Some(&Some(out)) = self.outbound_rewrite.get(native_id as usize) {
            let body = match self.stages(
                Direction::Serverbound,
                native_id,
                [out.ids, out.layout],
                body,
            ) {
                Rewritten::Body(body) => Some(body),
                Rewritten::Untouched => Some(Box::from(body)),
                Rewritten::Failed => None,
            };
            let body = body.filter(|body| out.layout.is_none() || !body.is_empty());
            if self.trace {
                self.trace_frame(
                    "out",
                    Direction::Serverbound,
                    native_id,
                    out.wire.filter(|_| body.is_some()),
                    body.is_some(),
                );
            }
            return match (body, out.wire) {
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
            self.trace_frame("out", Direction::Serverbound, native_id, wire_id, false);
        }
        match wire_id {
            Some(wire_id) if wire_id != native_id => frame(wire_id, body),
            Some(_) => raw,
            None => Box::new([]),
        }
    }

    fn trace_frame(&self, way: &str, dir: Direction, from: u32, to: Option<u32>, body: bool) {
        let (from_table, to_table) = match dir {
            Direction::Clientbound => (self.wire, self.native),
            Direction::Serverbound => (self.native, self.wire),
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

    fn field_indices(&self, moved: &mut HashMap<i32, &'static [Run]>, raw: Box<[u8]>) -> Box<[u8]> {
        let mut r = Reader::new(&raw);
        let Some(id) = r.varint().and_then(|id| u32::try_from(id).ok()) else {
            return raw;
        };
        let watched = &self.watched;
        if id == watched.add_entity {
            if let Some((entity, kind)) = spawned(&mut r) {
                let fields: &'static Fields = self.fields;
                match fields.runs(kind) {
                    Some(runs) => {
                        moved.insert(entity, runs);
                    }
                    None => {
                        moved.remove(&entity);
                    }
                }
            }
        } else if id == watched.remove_entities {
            if let Some(n) = r.count() {
                for _ in 0..n {
                    let Some(entity) = r.varint() else {
                        break;
                    };
                    moved.remove(&entity);
                }
            }
        } else if id == watched.login {
            moved.clear();
        } else if id == watched.set_entity_data {
            let head = r.pos();
            let runs = r.varint().and_then(|entity| moved.get(&entity).copied());
            if let Some(runs) = runs {
                if let Some(body) = shifted_metadata(&raw[head..], runs) {
                    return frame(id, &body);
                }
            }
        }
        raw
    }
}

fn spawned(r: &mut Reader) -> Option<(i32, i32)> {
    let entity = r.varint()?;
    r.take(16)?;
    Some((entity, r.varint()?))
}

fn shifted_metadata(body: &[u8], runs: &[Run]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len());
    write_varint(&mut out, r.varint()?);
    let mut probe = Vec::new();
    loop {
        let index = r.u8()?;
        if index == fields::END {
            out.push(index);
            return Some(out.into_boxed_slice());
        }
        out.push(fields::map(runs, index)?);
        let serializer = r.varint()?;
        write_varint(&mut out, serializer);
        copy_payload(&mut r, &mut out, &mut probe, serializer, |cursor| {
            azalea::entity::EntityDataValue::azalea_read(cursor).is_ok()
        })?;
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

pub(crate) struct Translator {
    hop: &'static Hop,
    moved: HashMap<i32, &'static [Run]>,
    order: Option<Arc<EntryOrder>>,
}

impl Translator {
    pub(crate) fn for_protocol(protocol: i32) -> Option<Self> {
        let hop = match protocol {
            774 => Hop::v774(),
            775 => Hop::v775(),
            _ => return None,
        };
        Some(Self {
            hop,
            moved: HashMap::new(),
            order: None,
        })
    }

    fn record_registry(&mut self, raw: &[u8]) {
        let mut r = Reader::new(raw);
        if r.varint() != Some(self.hop.watched.registry_data as i32) {
            return;
        }
        let mut cursor = std::io::Cursor::new(r.rest());
        let Ok(packet) = ClientboundRegistryData::azalea_read(&mut cursor) else {
            return;
        };
        if let Some(registry) = LazyRegistry::of(&packet.registry_id) {
            let order = Arc::make_mut(self.order.get_or_insert_with(Default::default));
            order.record(registry, &packet);
        }
    }
}

#[derive(Clone, Copy)]
enum LazyRegistry {
    Instrument,
    JukeboxSong,
    DamageType,
    TrimMaterial,
    ChickenVariant,
    ZombieNautilusVariant,
}

impl LazyRegistry {
    const ALL: [LazyRegistry; 6] = [
        Self::Instrument,
        Self::JukeboxSong,
        Self::DamageType,
        Self::TrimMaterial,
        Self::ChickenVariant,
        Self::ZombieNautilusVariant,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Instrument => "instrument",
            Self::JukeboxSong => "jukebox_song",
            Self::DamageType => "damage_type",
            Self::TrimMaterial => "trim_material",
            Self::ChickenVariant => "chicken_variant",
            Self::ZombieNautilusVariant => "zombie_nautilus_variant",
        }
    }

    fn of(registry: &Identifier) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| *registry == Identifier::new(candidate.name()))
    }
}

#[derive(Clone, Default)]
struct EntryOrder {
    ids: [HashMap<Identifier, u32>; 6],
}

impl EntryOrder {
    fn record(&mut self, registry: LazyRegistry, packet: &ClientboundRegistryData) {
        self.ids[registry as usize] = packet
            .entries
            .iter()
            .enumerate()
            .map(|(id, (key, _))| (key.clone(), id as u32))
            .collect();
    }

    fn id(&self, registry: LazyRegistry, key: &Identifier) -> Option<u32> {
        self.ids[registry as usize].get(key).copied()
    }
}

thread_local! {
    static ORDER: std::cell::RefCell<Option<Arc<EntryOrder>>> = const { std::cell::RefCell::new(None) };
}

struct OrderScope;

impl OrderScope {
    fn enter(order: &Option<Arc<EntryOrder>>) -> Option<Self> {
        let order = order.as_ref()?;
        ORDER.with(|slot| *slot.borrow_mut() = Some(order.clone()));
        Some(Self)
    }
}

impl Drop for OrderScope {
    fn drop(&mut self) {
        ORDER.with(|slot| *slot.borrow_mut() = None);
    }
}

impl azalea::connection::PacketTranslator for Translator {
    fn inbound(&mut self, phase: ConnectionProtocol, raw: Box<[u8]>) -> Box<[u8]> {
        if phase == ConnectionProtocol::Configuration && self.hop.ids.v774() {
            self.record_registry(&raw);
        }
        let _order = OrderScope::enter(&self.order);
        let frame = self.hop.inbound(phase, raw);
        if phase == ConnectionProtocol::Game {
            self.hop.field_indices(&mut self.moved, frame)
        } else {
            frame
        }
    }

    fn outbound(&mut self, phase: ConnectionProtocol, raw: Box<[u8]>) -> Box<[u8]> {
        self.hop.outbound(phase, raw)
    }
}

mod trail {
    use std::cell::RefCell;
    use std::fmt::Write as _;

    use azalea_registry::builtin::DataComponentKind;

    use crate::protocol::remap::IdSpace;
    use crate::protocol::version::NATIVE;

    #[derive(Clone, Copy)]
    pub(super) enum Step {
        Stack {
            at: usize,
            item: i32,
            count: i32,
        },
        Component {
            at: usize,
            wire: i32,
            kind: Option<DataComponentKind>,
            len: usize,
        },
    }

    #[derive(Clone, Copy)]
    pub(super) enum Why {
        Unmapped {
            space: IdSpace,
            raw: i32,
            to_native: bool,
        },
        Decode(DataComponentKind),
        Shape(&'static str),
    }

    #[derive(Default)]
    struct Trail {
        steps: Vec<Step>,
        why: Option<Why>,
        detail: String,
        at: Option<usize>,
    }

    thread_local! {
        static TRAIL: RefCell<Trail> = RefCell::new(Trail::default());
    }

    pub(super) fn reset() {
        TRAIL.with(|t| {
            let mut t = t.borrow_mut();
            t.steps.clear();
            t.why = None;
            t.detail.clear();
            t.at = None;
        });
    }

    pub(super) fn step(step: Step) {
        TRAIL.with(|t| t.borrow_mut().steps.push(step));
    }

    pub(super) fn finished(len: usize) {
        TRAIL.with(|t| {
            if let Some(Step::Component { len: slot, .. }) = t.borrow_mut().steps.last_mut() {
                *slot = len;
            }
        });
    }

    pub(super) fn fail(why: Why) {
        TRAIL.with(|t| {
            let mut t = t.borrow_mut();
            if t.why.is_none() {
                t.why = Some(why);
            }
        });
    }

    pub(super) fn fail_decode(kind: DataComponentKind, error: &dyn std::fmt::Display) {
        TRAIL.with(|t| {
            let mut t = t.borrow_mut();
            if t.why.is_none() {
                t.why = Some(Why::Decode(kind));
                t.detail = error.to_string();
            }
        });
    }

    pub(super) fn stopped_at(pos: usize) {
        TRAIL.with(|t| {
            let mut t = t.borrow_mut();
            if t.at.is_none() {
                t.at = Some(pos);
            }
        });
    }

    pub(super) fn reason() -> String {
        TRAIL.with(|t| describe(&t.borrow()))
    }

    fn describe(t: &Trail) -> String {
        match t.why {
            Some(Why::Unmapped { space, raw, to_native: true }) => {
                format!("wire {space:?} id {raw} names nothing in {}", NATIVE.name)
            }
            Some(Why::Unmapped { space, raw, to_native: false }) => {
                format!("{} {space:?} id {raw} names nothing on the wire", NATIVE.name)
            }
            Some(Why::Decode(kind)) => format!("{kind:?} did not decode: {}", t.detail),
            Some(Why::Shape(what)) => what.to_owned(),
            None => {
                "the frame ended before the layout did (a read ran off the end, or a count was negative)"
                    .to_owned()
            }
        }
    }

    pub(super) fn report(body: &[u8]) -> String {
        TRAIL.with(|t| {
            let t = t.borrow();
            let mut s = describe(&t);
            let at = t.at.unwrap_or(body.len());
            let _ = write!(s, " at byte {at} of {}", body.len());

            let start = t
                .steps
                .iter()
                .rposition(|step| matches!(step, Step::Stack { .. }))
                .unwrap_or(0);
            let stacks_before = t.steps[..start]
                .iter()
                .filter(|step| matches!(step, Step::Stack { .. }))
                .count();
            let _ = write!(s, "; {stacks_before} stacks walked before this one; trail:");
            if t.steps.is_empty() {
                s.push_str(" (no stack reached)");
            }
            for step in &t.steps[start..] {
                match *step {
                    Step::Stack { at, item, count } => {
                        let _ = write!(s, " stack@{at}[item {item} x{count}]");
                    }
                    Step::Component {
                        at,
                        wire,
                        kind: Some(kind),
                        len: 0,
                    } => {
                        let _ = write!(s, " {kind:?}({wire})@{at}[unfinished]");
                    }
                    Step::Component {
                        at,
                        wire,
                        kind: Some(kind),
                        len,
                    } => {
                        let _ = write!(s, " {kind:?}({wire})@{at}[{len}b]");
                    }
                    Step::Component {
                        at,
                        wire,
                        kind: None,
                        ..
                    } => {
                        let _ = write!(s, " ?({wire})@{at}[unmapped]");
                    }
                }
            }

            let lo = at.saturating_sub(16).min(body.len());
            let hi = (at + 48).min(body.len());
            let _ = write!(s, "; bytes {lo}..{hi}:");
            for byte in &body[lo..hi] {
                let _ = write!(s, " {byte:02x}");
            }
            s
        })
    }
}

fn frame(id: u32, body: &[u8]) -> Box<[u8]> {
    let mut out = Vec::with_capacity(varint_len(id as i32) + body.len());
    write_varint(&mut out, id as i32);
    out.extend_from_slice(body);
    out.into_boxed_slice()
}

fn set_time(_ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
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

fn attack(_ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    interact(body, true)
}

fn interact_use(_ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    interact(body, false)
}

fn login_finished(_ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    r.take(16)?;
    skip_string(&mut r)?;
    let properties = r.count()?;
    for _ in 0..properties {
        skip_string(&mut r)?;
        skip_string(&mut r)?;
        if r.u8()? != 0 {
            skip_string(&mut r)?;
        }
    }
    if r.remaining() != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(body.len() + 16);
    out.extend_from_slice(body);
    out.extend_from_slice(&[0; 16]);
    Some(out.into_boxed_slice())
}

fn skip_string(r: &mut Reader) -> Option<()> {
    let len = r.count()?;
    r.take(len)?;
    Some(())
}

const ONLINE_MODE: u8 = 0;

fn login(_ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    r.take(4 + 1)?;
    let levels = r.count()?;
    for _ in 0..levels {
        skip_string(&mut r)?;
    }
    for _ in 0..3 {
        r.varint()?;
    }
    r.take(3)?;
    r.varint()?;
    skip_string(&mut r)?;
    r.take(8 + 1 + 1 + 1 + 1)?;
    if r.u8()? != 0 {
        skip_string(&mut r)?;
        r.take(8)?;
    }
    r.varint()?;
    r.varint()?;
    let spawn_info_end = r.pos();
    r.u8()?;
    if r.remaining() != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(body.len() + 1);
    out.extend_from_slice(&body[..spawn_info_end]);
    out.push(ONLINE_MODE);
    out.extend_from_slice(&body[spawn_info_end..]);
    Some(out.into_boxed_slice())
}

const TEAM_COLORS: i32 = 16;

fn set_player_team(_ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    skip_string(&mut r)?;
    let method = r.u8()?;
    let head = r.pos();
    if !matches!(method, 0 | 2) {
        return Some(body.into());
    }

    let mut display = Vec::new();
    nbt_tag(&mut r, &mut display)?;
    let options = r.u8()?;
    let visibility = r.varint()?;
    let collision = r.varint()?;
    let color = r.varint()?;
    let mut prefix_and_suffix = Vec::new();
    nbt_tag(&mut r, &mut prefix_and_suffix)?;
    nbt_tag(&mut r, &mut prefix_and_suffix)?;

    let mut out = Vec::with_capacity(body.len() + 2);
    out.extend_from_slice(&body[..head]);
    out.extend_from_slice(&display);
    out.extend_from_slice(&prefix_and_suffix);
    write_varint(&mut out, visibility);
    write_varint(&mut out, collision);
    if (0..TEAM_COLORS).contains(&color) {
        out.push(1);
        write_varint(&mut out, color);
    } else {
        out.push(0);
    }
    out.push(options);
    out.extend_from_slice(r.rest());
    Some(out.into_boxed_slice())
}

fn spectator_action(_ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let target = r.varint()?;
    if r.remaining() != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(5);
    if target != 0 {
        write_varint(&mut out, target.checked_sub(1)?);
    }
    Some(out.into_boxed_slice())
}

fn state(ids: &Ids, id: i32) -> Option<i32> {
    ids.to_native.map(IdSpace::BlockState, id)
}

fn block(ids: &Ids, id: i32) -> Option<i32> {
    ids.to_native.map(IdSpace::Block, id)
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

fn particle(ids: &Ids, r: &mut Reader, out: &mut Vec<u8>, probe: &mut Vec<u8>) -> Option<()> {
    let native = ids.to_native.map(IdSpace::ParticleType, r.varint()?)?;
    write_varint(out, native);

    match ParticleKind::from_u32(u32::try_from(native).ok()?)? {
        ParticleKind::Block
        | ParticleKind::BlockMarker
        | ParticleKind::FallingDust
        | ParticleKind::DustPillar
        | ParticleKind::BlockCrumble => {
            let id = state(ids, r.varint()?)?;
            write_varint(out, id);
        }
        ParticleKind::Item => stack_template(ids, r, out)?,
        _ => copy_payload(r, out, probe, native, |cursor| {
            azalea::entity::particle::Particle::azalea_read(cursor).is_ok()
        })?,
    }
    Some(())
}

fn sound_holder(ids: &Ids, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    sound_holder_way(ids, Way::In, r, out)
}

fn sound_holder_way(ids: &Ids, way: Way, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let raw = r.varint()?;
    if raw == 0 {
        write_varint(out, 0);
        let name = r.count()?;
        write_varint(out, name as i32);
        out.extend_from_slice(r.take(name)?);
        if flag(r, out)? {
            out.extend_from_slice(r.take(4)?);
        }
        return Some(());
    }
    let moved = way.map(ids, IdSpace::SoundEvent, raw.checked_sub(1)?)?;
    write_varint(out, moved + 1);
    Some(())
}

const SERIALIZER_ITEM_STACK: i32 = 7;
const SERIALIZER_BLOCK_STATE: i32 = 14;
const SERIALIZER_OPTIONAL_BLOCK_STATE: i32 = 15;
const SERIALIZER_PARTICLE: i32 = 16;
const SERIALIZER_PARTICLES: i32 = 17;

fn item_stack(ids: &Ids, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let at = r.pos();
    let count = r.varint()?;
    write_varint(out, count);
    if count <= 0 {
        return Some(());
    }
    let raw = r.varint()?;
    trail::step(trail::Step::Stack {
        at,
        item: raw,
        count,
    });
    let walked = Way::In.map(ids, IdSpace::Item, raw).and_then(|item| {
        write_varint(out, item);
        patch(ids, Way::In, r, out)
    });
    if walked.is_none() {
        trail::stopped_at(r.pos());
    }
    walked
}

#[derive(Clone, Copy)]
enum Way {
    In,
    Out,
}

impl Way {
    fn map(self, ids: &Ids, space: IdSpace, raw: i32) -> Option<i32> {
        let to_native = matches!(self, Way::In);
        let remap = if to_native {
            ids.to_native
        } else {
            ids.from_native
        };
        let moved = remap.map(space, raw);
        if moved.is_none() {
            trail::fail(trail::Why::Unmapped {
                space,
                raw,
                to_native,
            });
        }
        moved
    }
}

fn patch(ids: &Ids, way: Way, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let with_payload = r.count()?;
    let without = r.count()?;
    write_varint(out, with_payload as i32);
    write_varint(out, without as i32);

    for _ in 0..with_payload {
        component(ids, way, r, out)?;
    }
    for _ in 0..without {
        write_varint(out, way.map(ids, IdSpace::DataComponentType, r.varint()?)?);
    }
    Some(())
}

fn component(ids: &Ids, way: Way, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let at = r.pos();
    let raw = r.varint()?;
    let moved = way.map(ids, IdSpace::DataComponentType, raw);
    let native = match way {
        Way::In => moved,
        Way::Out => Some(raw),
    };
    let kind = native.and_then(|id| DataComponentKind::from_u32(u32::try_from(id).ok()?));
    trail::step(trail::Step::Component {
        at,
        wire: raw,
        kind,
        len: 0,
    });
    let (moved, kind) = (moved?, kind?);
    write_varint(out, moved);
    let walked = payload(ids, way, kind, r, out);
    match walked {
        Some(()) => trail::finished(r.pos() - at),
        None => trail::stopped_at(r.pos()),
    }
    walked
}

fn payload(
    ids: &Ids,
    way: Way,
    kind: DataComponentKind,
    r: &mut Reader,
    out: &mut Vec<u8>,
) -> Option<()> {
    if nested(ids, way, kind, r, out)? {
        return Some(());
    }
    let mut cursor = std::io::Cursor::new(r.rest());
    let mut value =
        match azalea_inventory::components::DataComponentUnion::azalea_read_as(kind, &mut cursor) {
            Ok(value) => value,
            Err(e) => {
                trail::fail_decode(kind, &e);
                return None;
            }
        };
    unsafe { value.drop_as(kind) };
    let length = usize::try_from(cursor.position()).ok()?;
    out.extend_from_slice(r.take(length)?);
    Some(())
}

fn nested(
    ids: &Ids,
    way: Way,
    kind: DataComponentKind,
    r: &mut Reader,
    out: &mut Vec<u8>,
) -> Option<bool> {
    let (list, optional) = match kind {
        DataComponentKind::Container => (true, true),
        DataComponentKind::BundleContents | DataComponentKind::ChargedProjectiles => (true, false),
        DataComponentKind::UseRemainder => (false, false),
        _ => return widened(ids, way, kind, r, out),
    };
    let n = if list {
        let n = r.count()?;
        write_varint(out, n as i32);
        n
    } else {
        1
    };
    for _ in 0..n {
        if !ids.v774() {
            if optional && !flag(r, out)? {
                continue;
            }
            write_varint(out, way.map(ids, IdSpace::Item, r.varint()?)?);
            write_varint(out, r.varint()?);
            patch(ids, way, r, out)?;
            continue;
        }
        match way {
            Way::In => {
                let count = r.varint()?;
                if count <= 0 {
                    if !optional {
                        trail::fail(trail::Why::Shape(
                            "an empty nested stack where 775 holds a template, which cannot be empty",
                        ));
                        return None;
                    }
                    out.push(0);
                    continue;
                }
                if optional {
                    out.push(1);
                }
                write_varint(out, way.map(ids, IdSpace::Item, r.varint()?)?);
                write_varint(out, count);
            }
            Way::Out => {
                if optional && r.u8()? == 0 {
                    write_varint(out, 0);
                    continue;
                }
                let item = way.map(ids, IdSpace::Item, r.varint()?)?;
                write_varint(out, r.varint()?);
                write_varint(out, item);
            }
        }
        patch(ids, way, r, out)?;
    }
    Some(true)
}

fn widened(
    ids: &Ids,
    way: Way,
    kind: DataComponentKind,
    r: &mut Reader,
    out: &mut Vec<u8>,
) -> Option<bool> {
    match kind {
        DataComponentKind::DamageResistant | DataComponentKind::ProvidesBannerPatterns => {
            tag_or_holder_set(ids, way, r, out)?;
        }
        DataComponentKind::BlocksAttacks => {
            out.extend_from_slice(r.take(8)?);
            let n = r.count()?;
            write_varint(out, n as i32);
            for _ in 0..n {
                out.extend_from_slice(r.take(4)?);
                if flag(r, out)? {
                    copy_holder_set(r, out)?;
                }
                out.extend_from_slice(r.take(8)?);
            }
            out.extend_from_slice(r.take(12)?);
            if flag(r, out)? {
                tag_or_holder_set(ids, way, r, out)?;
            }
            for _ in 0..2 {
                if flag(r, out)? {
                    sound_holder_way(ids, way, r, out)?;
                }
            }
        }
        DataComponentKind::CanPlaceOn | DataComponentKind::CanBreak => {
            let n = r.count()?;
            write_varint(out, n as i32);
            for _ in 0..n {
                block_predicate(ids, way, r, out)?;
            }
        }
        DataComponentKind::TooltipDisplay => {
            out.push(r.u8()?);
            let n = r.count()?;
            write_varint(out, n as i32);
            for _ in 0..n {
                write_varint(out, way.map(ids, IdSpace::DataComponentType, r.varint()?)?);
            }
        }
        _ => return resound(ids, way, kind, r, out),
    }
    Some(true)
}

fn resound(
    ids: &Ids,
    way: Way,
    kind: DataComponentKind,
    r: &mut Reader,
    out: &mut Vec<u8>,
) -> Option<bool> {
    use azalea_inventory::components as c;
    match kind {
        DataComponentKind::Equippable => rewrite_value::<c::Equippable>(kind, r, out, |v| {
            sound_entry(ids, way, &mut v.equip_sound)?;
            sound_entry(ids, way, &mut v.shearing_sound)?;
            v.allowed_entities.as_mut().map_or(Some(()), |set| {
                holder_set(ids, way, IdSpace::EntityType, set)
            })
        })?,
        DataComponentKind::Tool => rewrite_value::<c::Tool>(kind, r, out, |v| {
            v.rules
                .iter_mut()
                .try_for_each(|rule| holder_set(ids, way, IdSpace::Block, &mut rule.blocks))
        })?,
        DataComponentKind::Instrument => {
            if lazy_reference(ids, way, LazyRegistry::Instrument, r, out)? {
                return Some(true);
            }
            rewrite_value::<c::Instrument>(kind, r, out, |v| match &mut v.value {
                azalea_registry::Holder::Direct(inline) => {
                    sound_entry(ids, way, &mut inline.sound_event)
                }
                azalea_registry::Holder::Reference(_) => Some(()),
            })?
        }
        DataComponentKind::JukeboxPlayable => {
            if lazy_reference(ids, way, LazyRegistry::JukeboxSong, r, out)? {
                return Some(true);
            }
            rewrite_value::<c::JukeboxPlayable>(kind, r, out, |v| match &mut v.value {
                azalea_registry::Holder::Direct(inline) => {
                    sound_entry(ids, way, &mut inline.sound_event)
                }
                azalea_registry::Holder::Reference(_) => Some(()),
            })?
        }
        DataComponentKind::Consumable => rewrite_value::<c::Consumable>(kind, r, out, |v| {
            sound_entry(ids, way, &mut v.sound)?;
            effect_sounds(ids, way, &mut v.on_consume_effects)
        })?,
        DataComponentKind::DeathProtection => {
            rewrite_value::<c::DeathProtection>(kind, r, out, |v| {
                effect_sounds(ids, way, &mut v.death_effects)
            })?
        }
        DataComponentKind::BreakSound => {
            rewrite_value::<c::BreakSound>(kind, r, out, |v| sound_entry(ids, way, &mut v.sound))?
        }
        DataComponentKind::PiercingWeapon => {
            rewrite_value::<c::PiercingWeapon>(kind, r, out, |v| {
                v.sound
                    .as_mut()
                    .map_or(Some(()), |s| sound_entry(ids, way, s))?;
                v.hit_sound
                    .as_mut()
                    .map_or(Some(()), |s| sound_entry(ids, way, s))
            })?
        }
        DataComponentKind::KineticWeapon => rewrite_value::<c::KineticWeapon>(kind, r, out, |v| {
            v.sound
                .as_mut()
                .map_or(Some(()), |s| sound_entry(ids, way, s))?;
            v.hit_sound
                .as_mut()
                .map_or(Some(()), |s| sound_entry(ids, way, s))
        })?,
        DataComponentKind::Repairable => rewrite_value::<c::Repairable>(kind, r, out, |v| {
            holder_set(ids, way, IdSpace::Item, &mut v.items)
        })?,
        DataComponentKind::PotDecorations => {
            rewrite_value::<c::PotDecorations>(kind, r, out, |v| {
                v.items
                    .iter_mut()
                    .try_for_each(|item| registry_entry(ids, way, IdSpace::Item, item))
            })?
        }
        DataComponentKind::AttributeModifiers => {
            rewrite_value::<c::AttributeModifiers>(kind, r, out, |v| {
                v.modifiers.iter_mut().try_for_each(|entry| {
                    registry_entry(ids, way, IdSpace::Attribute, &mut entry.kind)
                })
            })?
        }
        DataComponentKind::EntityData => rewrite_value::<c::EntityData>(kind, r, out, |v| {
            registry_entry(ids, way, IdSpace::EntityType, &mut v.kind)
        })?,
        DataComponentKind::BlockEntityData => {
            rewrite_value::<c::BlockEntityData>(kind, r, out, |v| {
                registry_entry(ids, way, IdSpace::BlockEntityType, &mut v.kind)
            })?
        }
        DataComponentKind::Bees => rewrite_value::<c::Bees>(kind, r, out, |v| {
            v.occupants
                .iter_mut()
                .try_for_each(|bee| registry_entry(ids, way, IdSpace::EntityType, &mut bee.kind))
        })?,
        DataComponentKind::DamageType
        | DataComponentKind::ProvidesTrimMaterial
        | DataComponentKind::ChickenVariant
        | DataComponentKind::ZombieNautilusVariant => {
            let registry = match kind {
                DataComponentKind::DamageType => LazyRegistry::DamageType,
                DataComponentKind::ProvidesTrimMaterial => LazyRegistry::TrimMaterial,
                DataComponentKind::ChickenVariant => LazyRegistry::ChickenVariant,
                _ => LazyRegistry::ZombieNautilusVariant,
            };
            return Some(lazy_reference(ids, way, registry, r, out)?);
        }
        _ => return Some(false),
    }
    Some(true)
}

fn rewrite_value<T: AzBuf>(
    kind: DataComponentKind,
    r: &mut Reader,
    out: &mut Vec<u8>,
    edit: impl FnOnce(&mut T) -> Option<()>,
) -> Option<()> {
    let mut cursor = std::io::Cursor::new(r.rest());
    let mut value = match T::azalea_read(&mut cursor) {
        Ok(value) => value,
        Err(e) => {
            trail::fail_decode(kind, &e);
            return None;
        }
    };
    edit(&mut value)?;
    value.azalea_write(out).ok()?;
    r.take(usize::try_from(cursor.position()).ok()?)?;
    Some(())
}

fn registry_entry<R: azalea_registry::Registry>(
    ids: &Ids,
    way: Way,
    space: IdSpace,
    entry: &mut R,
) -> Option<()> {
    let moved = way.map(ids, space, i32::try_from(entry.to_u32()).ok()?)?;
    *entry = R::from_u32(u32::try_from(moved).ok()?)?;
    Some(())
}

fn holder_set<R: azalea_registry::Registry, K: AzBuf>(
    ids: &Ids,
    way: Way,
    space: IdSpace,
    set: &mut azalea_registry::HolderSet<R, K>,
) -> Option<()> {
    match set {
        azalea_registry::HolderSet::Direct { contents } => contents
            .iter_mut()
            .try_for_each(|entry| registry_entry(ids, way, space, entry)),
        azalea_registry::HolderSet::Named { .. } => Some(()),
    }
}

fn lazy_reference(
    ids: &Ids,
    way: Way,
    registry: LazyRegistry,
    r: &mut Reader,
    out: &mut Vec<u8>,
) -> Option<bool> {
    if !ids.v774() {
        return Some(false);
    }
    match way {
        Way::In => {
            if r.u8()? != 0 {
                return Some(false);
            }
            let len = usize::try_from(r.varint()?).ok()?;
            let key = Identifier::new(std::str::from_utf8(r.take(len)?).ok()?);
            let id = ORDER.with(|slot| {
                slot.borrow()
                    .as_ref()
                    .and_then(|order| order.id(registry, &key))
            });
            let Some(id) = id else {
                trail::fail(trail::Why::Shape(
                    "a registry key the server's registry_data did not list",
                ));
                return None;
            };
            write_varint(out, i32::try_from(id).ok()?.checked_add(1)?);
            Some(true)
        }
        Way::Out => {
            out.push(1);
            Some(false)
        }
    }
}

fn sound_entry(
    ids: &Ids,
    way: Way,
    holder: &mut azalea_registry::Holder<
        azalea_registry::builtin::SoundEvent,
        azalea_core::sound::CustomSound,
    >,
) -> Option<()> {
    match holder {
        azalea_registry::Holder::Reference(entry) => {
            registry_entry(ids, way, IdSpace::SoundEvent, entry)
        }
        azalea_registry::Holder::Direct(_) => Some(()),
    }
}

fn effect_sounds(
    ids: &Ids,
    way: Way,
    effects: &mut [azalea_inventory::item::consume_effect::ConsumeEffect],
) -> Option<()> {
    use azalea_inventory::item::consume_effect::ConsumeEffect;
    effects.iter_mut().try_for_each(|effect| match effect {
        ConsumeEffect::PlaySound { sound: entry } => sound_entry(ids, way, entry),
        _ => Some(()),
    })
}

fn flag(r: &mut Reader, out: &mut Vec<u8>) -> Option<bool> {
    let b = r.u8()?;
    out.push(b);
    Some(b != 0)
}

fn block_predicate(ids: &Ids, way: Way, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    if flag(r, out)? {
        let n = r.varint()?;
        write_varint(out, n);
        if n == 0 {
            identifier(r, out)?;
        }
        for _ in 1..n {
            write_varint(out, way.map(ids, IdSpace::Block, r.varint()?)?);
        }
    }
    if flag(r, out)? {
        let n = r.count()?;
        write_varint(out, n as i32);
        for _ in 0..n {
            identifier(r, out)?;
            if flag(r, out)? {
                identifier(r, out)?;
            } else {
                for _ in 0..2 {
                    if flag(r, out)? {
                        identifier(r, out)?;
                    }
                }
            }
        }
    }
    if flag(r, out)? {
        nbt_tag(r, out)?;
    }
    let exact = r.count()?;
    write_varint(out, exact as i32);
    for _ in 0..exact {
        component(ids, way, r, out)?;
    }
    let partial = r.count()?;
    write_varint(out, partial as i32);
    for _ in 0..partial {
        let predicate_type = flag(r, out)?;
        let id = r.varint()?;
        let id = if predicate_type {
            id
        } else {
            way.map(ids, IdSpace::DataComponentType, id)?
        };
        write_varint(out, id);
        nbt_tag(r, out)?;
    }
    Some(())
}

fn tag_or_holder_set(ids: &Ids, way: Way, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    if !ids.v774() {
        return copy_holder_set(r, out);
    }
    match way {
        Way::In => write_varint(out, 0),
        Way::Out => {
            if r.varint()? != 0 {
                trail::fail(trail::Why::Shape(
                    "a direct holder set where the wire version takes one tag key",
                ));
                return None;
            }
        }
    }
    identifier(r, out)
}

fn copy_holder_set(r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let n = r.varint()?;
    write_varint(out, n);
    if n == 0 {
        return identifier(r, out);
    }
    for _ in 1..n {
        write_varint(out, r.varint()?);
    }
    Some(())
}

fn entity_data(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
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
        let serializer = ids
            .to_native
            .map(IdSpace::EntityDataSerializer, r.varint()?)?;
        write_varint(&mut out, serializer);

        match serializer {
            SERIALIZER_ITEM_STACK => {
                item_stack(ids, &mut r, &mut out)?;
                continue;
            }
            SERIALIZER_BLOCK_STATE | SERIALIZER_OPTIONAL_BLOCK_STATE => {
                let id = state(ids, r.varint()?)?;
                write_varint(&mut out, id);
                continue;
            }
            SERIALIZER_PARTICLE => {
                particle(ids, &mut r, &mut out, &mut probe)?;
                continue;
            }
            SERIALIZER_PARTICLES => {
                let count = r.count()?;
                write_varint(&mut out, count as i32);
                for _ in 0..count {
                    particle(ids, &mut r, &mut out, &mut probe)?;
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

fn stacks(
    ids: &Ids,
    body: &[u8],
    header: usize,
    counted: bool,
    trailing: usize,
) -> Option<Box<[u8]>> {
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
        item_stack(ids, &mut r, &mut out)?;
    }
    for _ in 0..trailing {
        item_stack(ids, &mut r, &mut out)?;
    }
    Some(out.into_boxed_slice())
}

fn container_set_content(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    stacks(ids, body, 2, true, 1)
}

fn set_cursor_item(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    stacks(ids, body, 0, false, 0)
}

fn set_player_inventory(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    stacks(ids, body, 1, false, 0)
}

const SLOT_DISPLAY_TO_NATIVE: [i32; 8] = [0, 1, 4, 5, 6, 8, 9, 10];

const SLOT_DISPLAY_WITH_ANY_POTION: i32 = 2;
const SLOT_DISPLAY_ONLY_WITH_COMPONENT: i32 = 3;
const SLOT_DISPLAY_ITEM: i32 = 4;
const SLOT_DISPLAY_ITEM_STACK: i32 = 5;
const SLOT_DISPLAY_TAG: i32 = 6;
const SLOT_DISPLAY_DYED: i32 = 7;
const SLOT_DISPLAY_SMITHING_TRIM: i32 = 8;
const SLOT_DISPLAY_WITH_REMAINDER: i32 = 9;
const SLOT_DISPLAY_COMPOSITE: i32 = 10;

fn update_recipes(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 64);

    let sets = r.count()?;
    write_varint(&mut out, sets as i32);
    for _ in 0..sets {
        identifier(&mut r, &mut out)?;
        let items = r.count()?;
        write_varint(&mut out, items as i32);
        for _ in 0..items {
            item(ids, &mut r, &mut out)?;
        }
    }

    let entries = r.count()?;
    write_varint(&mut out, entries as i32);
    for _ in 0..entries {
        ingredient(ids, &mut r, &mut out)?;
        slot_display(ids, &mut r, &mut out)?;
    }

    (r.remaining() == 0).then(|| out.into_boxed_slice())
}

fn identifier(r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let len = r.count()?;
    write_varint(out, len as i32);
    out.extend_from_slice(r.take(len)?);
    Some(())
}

fn item(ids: &Ids, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let id = Way::In.map(ids, IdSpace::Item, r.varint()?)?;
    write_varint(out, id);
    Some(())
}

fn ingredient(ids: &Ids, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let size = r.count()?;
    write_varint(out, size as i32);
    if size == 0 {
        return identifier(r, out);
    }
    for _ in 1..size {
        item(ids, r, out)?;
    }
    Some(())
}

fn slot_display(ids: &Ids, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let kind = r.varint()?;
    let native = if ids.v774() {
        *SLOT_DISPLAY_TO_NATIVE.get(usize::try_from(kind).ok()?)?
    } else {
        kind
    };
    write_varint(out, native);
    match native {
        SLOT_DISPLAY_ITEM => item(ids, r, out)?,
        SLOT_DISPLAY_ITEM_STACK => stack_template(ids, r, out)?,
        SLOT_DISPLAY_TAG => identifier(r, out)?,
        SLOT_DISPLAY_WITH_ANY_POTION => slot_display(ids, r, out)?,
        SLOT_DISPLAY_ONLY_WITH_COMPONENT => {
            slot_display(ids, r, out)?;
            write_varint(
                out,
                Way::In.map(ids, IdSpace::DataComponentType, r.varint()?)?,
            );
        }
        SLOT_DISPLAY_SMITHING_TRIM => {
            slot_display(ids, r, out)?;
            slot_display(ids, r, out)?;
            let pattern = r.varint()?;
            if pattern == 0 {
                trail::fail(trail::Why::Shape(
                    "an inline trim pattern in a smithing_trim slot display is not walked",
                ));
                return None;
            }
            write_varint(out, pattern);
        }
        SLOT_DISPLAY_DYED | SLOT_DISPLAY_WITH_REMAINDER => {
            slot_display(ids, r, out)?;
            slot_display(ids, r, out)?;
        }
        SLOT_DISPLAY_COMPOSITE => {
            let count = r.count()?;
            write_varint(out, count as i32);
            for _ in 0..count {
                slot_display(ids, r, out)?;
            }
        }
        _ => {}
    }
    Some(())
}

fn stack_template(ids: &Ids, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    if !ids.v774() {
        item(ids, r, out)?;
        write_varint(out, r.varint()?);
        return patch(ids, Way::In, r, out);
    }
    let mut stack = Vec::new();
    item_stack(ids, r, &mut stack)?;
    let mut s = Reader::new(&stack);
    let count = s.varint()?;
    if count <= 0 {
        trail::fail(trail::Why::Shape(
            "an empty stack where 775 holds a template, which cannot be empty",
        ));
        return None;
    }
    write_varint(out, s.varint()?);
    write_varint(out, count);
    out.extend_from_slice(s.rest());
    Some(())
}

fn nbt_tag(r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let mut cursor = std::io::Cursor::new(r.rest());
    simdnbt::borrow::read_optional_tag(&mut cursor).ok()?;
    let length = usize::try_from(cursor.position()).ok()?;
    out.extend_from_slice(r.take(length)?);
    Some(())
}

fn update_advancements(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 16);
    out.push(r.u8()?);
    let added = r.count()?;
    write_varint(&mut out, added as i32);
    for _ in 0..added {
        identifier(&mut r, &mut out)?;
        if flag(&mut r, &mut out)? {
            identifier(&mut r, &mut out)?;
        }
        if flag(&mut r, &mut out)? {
            nbt_tag(&mut r, &mut out)?;
            nbt_tag(&mut r, &mut out)?;
            stack_template(ids, &mut r, &mut out)?;
            write_varint(&mut out, r.varint()?);
            let flags = r.take(4)?;
            out.extend_from_slice(flags);
            if flags[3] & 1 != 0 {
                identifier(&mut r, &mut out)?;
            }
            out.extend_from_slice(r.take(8)?);
        }
        let groups = r.count()?;
        write_varint(&mut out, groups as i32);
        for _ in 0..groups {
            let names = r.count()?;
            write_varint(&mut out, names as i32);
            for _ in 0..names {
                identifier(&mut r, &mut out)?;
            }
        }
        out.push(r.u8()?);
    }
    out.extend_from_slice(r.rest());
    Some(out.into_boxed_slice())
}

fn recipe_book_add(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 16);
    let entries = r.count()?;
    write_varint(&mut out, entries as i32);
    for _ in 0..entries {
        write_varint(&mut out, r.varint()?);
        recipe_display(ids, &mut r, &mut out)?;
        write_varint(&mut out, r.varint()?);
        write_varint(&mut out, r.varint()?);
        if flag(&mut r, &mut out)? {
            let n = r.count()?;
            write_varint(&mut out, n as i32);
            for _ in 0..n {
                ingredient(ids, &mut r, &mut out)?;
            }
        }
        out.push(r.u8()?);
    }
    out.push(r.u8()?);
    (r.remaining() == 0).then(|| out.into_boxed_slice())
}

fn place_ghost_recipe(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 16);
    write_varint(&mut out, r.varint()?);
    recipe_display(ids, &mut r, &mut out)?;
    (r.remaining() == 0).then(|| out.into_boxed_slice())
}

fn recipe_display(ids: &Ids, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let kind = r.varint()?;
    write_varint(out, kind);
    match kind {
        0 => {
            slot_display_list(ids, r, out)?;
            slot_displays(ids, 2, r, out)?;
        }
        1 => {
            write_varint(out, r.varint()?);
            write_varint(out, r.varint()?);
            slot_display_list(ids, r, out)?;
            slot_displays(ids, 2, r, out)?;
        }
        2 => {
            slot_displays(ids, 4, r, out)?;
            write_varint(out, r.varint()?);
            out.extend_from_slice(r.take(4)?);
        }
        3 => slot_displays(ids, 3, r, out)?,
        4 => slot_displays(ids, 5, r, out)?,
        _ => return None,
    }
    Some(())
}

fn slot_displays(ids: &Ids, n: usize, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    for _ in 0..n {
        slot_display(ids, r, out)?;
    }
    Some(())
}

fn slot_display_list(ids: &Ids, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let n = r.count()?;
    write_varint(out, n as i32);
    slot_displays(ids, n, r, out)
}

fn merchant_offers(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 16);
    write_varint(&mut out, r.varint()?);
    let offers = r.count()?;
    write_varint(&mut out, offers as i32);
    for _ in 0..offers {
        item_cost(ids, &mut r, &mut out)?;
        item_stack(ids, &mut r, &mut out)?;
        if flag(&mut r, &mut out)? {
            item_cost(ids, &mut r, &mut out)?;
        }
        out.extend_from_slice(r.take(1 + 6 * 4)?);
    }
    out.extend_from_slice(r.rest());
    Some(out.into_boxed_slice())
}

fn item_cost(ids: &Ids, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    item(ids, r, out)?;
    write_varint(out, r.varint()?);
    let n = r.count()?;
    write_varint(out, n as i32);
    for _ in 0..n {
        component(ids, Way::In, r, out)?;
    }
    Some(())
}

fn container_set_slot(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 4);
    write_varint(&mut out, r.varint()?);
    write_varint(&mut out, r.varint()?);
    out.extend_from_slice(r.take(2)?);
    item_stack(ids, &mut r, &mut out)?;
    Some(out.into_boxed_slice())
}

fn set_equipment(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 8);
    write_varint(&mut out, r.varint()?);
    loop {
        let slot = r.u8()?;
        out.push(slot);
        item_stack(ids, &mut r, &mut out)?;
        if slot & 0x80 == 0 {
            return Some(out.into_boxed_slice());
        }
    }
}

fn hashed_stack(ids: &Ids, r: &mut Reader, out: &mut Vec<u8>) -> Option<bool> {
    if !flag(r, out)? {
        return Some(true);
    }

    let mut ok = true;
    back(ids, IdSpace::Item, r.varint()?, out, &mut ok);
    write_varint(out, r.varint()?);
    let added = r.count()?;
    write_varint(out, added as i32);
    for _ in 0..added {
        back(ids, IdSpace::DataComponentType, r.varint()?, out, &mut ok);
        out.extend_from_slice(r.take(4)?);
    }
    let removed = r.count()?;
    write_varint(out, removed as i32);
    for _ in 0..removed {
        back(ids, IdSpace::DataComponentType, r.varint()?, out, &mut ok);
    }
    Some(ok)
}

fn back(ids: &Ids, space: IdSpace, raw: i32, out: &mut Vec<u8>, ok: &mut bool) {
    let id = Way::Out.map(ids, space, raw);
    *ok &= id.is_some();
    write_varint(out, id.unwrap_or_default());
}

fn container_click(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
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
        ok &= hashed_stack(ids, &mut r, &mut slots)?;
    }
    if ok {
        write_varint(&mut out, count as i32);
        out.extend_from_slice(&slots);
    } else {
        write_varint(&mut out, 0);
        crate::log_warn!(
            "proto",
            "container_click: changed_slots sent empty so the server resyncs: {}",
            trail::reason()
        );
    }

    let carried = out.len();
    if !hashed_stack(ids, &mut r, &mut out)? {
        out.truncate(carried);
        out.push(0);
        crate::log_warn!(
            "proto",
            "container_click: the carried item was sent empty: {}",
            trail::reason()
        );
    }
    Some(out.into_boxed_slice())
}

fn creative_slot(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 4);
    out.extend_from_slice(r.take(2)?);

    let count = r.varint()?;
    write_varint(&mut out, count);
    if count <= 0 {
        return Some(out.into_boxed_slice());
    }

    let Some(item) = ids.from_native.map(IdSpace::Item, r.varint()?) else {
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
    let (mut kept_with, mut kept_without) = (0, 0);
    let mut body = Vec::with_capacity(r.rest().len());
    let mut dropped = false;
    let mut nested_payload = Vec::new();
    for _ in 0..with_payload {
        let native = r.varint()?;
        let length = r.count()?;
        let payload = r.take(length)?;
        let Some(kind) = ids.from_native.map(IdSpace::DataComponentType, native) else {
            dropped = true;
            continue;
        };
        let payload = match u32::try_from(native)
            .ok()
            .and_then(DataComponentKind::from_u32)
        {
            Some(native_kind) => {
                nested_payload.clear();
                let mut pr = Reader::new(payload);
                match nested(ids, Way::Out, native_kind, &mut pr, &mut nested_payload) {
                    Some(false) => payload,
                    Some(true) if pr.remaining() == 0 => &nested_payload[..],
                    _ => {
                        dropped = true;
                        continue;
                    }
                }
            }
            None => payload,
        };
        write_varint(&mut body, kind);
        write_varint(&mut body, payload.len() as i32);
        body.extend_from_slice(payload);
        kept_with += 1;
    }
    for _ in 0..without {
        let Some(kind) = ids.from_native.map(IdSpace::DataComponentType, r.varint()?) else {
            dropped = true;
            continue;
        };
        write_varint(&mut body, kind);
        kept_without += 1;
    }
    if dropped {
        crate::log_warn!(
            "net",
            "creative slot: left off components this server cannot take: {}",
            trail::reason()
        );
    }
    write_varint(&mut out, kept_with);
    write_varint(&mut out, kept_without);
    out.extend_from_slice(&body);
    Some(out.into_boxed_slice())
}

fn update_tags(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len());
    let registries = r.count()?;
    write_varint(&mut out, registries as i32);
    let mut moved = Vec::new();
    for _ in 0..registries {
        let at = r.pos();
        let len = r.count()?;
        let name = std::str::from_utf8(r.take(len)?).ok()?;
        out.extend_from_slice(r.slice_from(at));
        let space = IdSpace::of_registry(name.strip_prefix("minecraft:").unwrap_or(name));
        let tags = r.count()?;
        write_varint(&mut out, tags as i32);
        for _ in 0..tags {
            identifier(&mut r, &mut out)?;
            let n = r.count()?;
            moved.clear();
            for _ in 0..n {
                let id = r.varint()?;
                match space {
                    Some(space) => moved.extend(ids.to_native.map(space, id)),
                    None => moved.push(id),
                }
            }
            write_varint(&mut out, moved.len() as i32);
            for id in &moved {
                write_varint(&mut out, *id);
            }
        }
    }
    (r.remaining() == 0).then(|| out.into_boxed_slice())
}

fn level_particles(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    const HEADER: usize = 1 + 1 + 3 * 8 + 4 * 4 + 4;
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 4);
    out.extend_from_slice(r.take(HEADER)?);
    particle(ids, &mut r, &mut out, &mut Vec::new())?;
    Some(out.into_boxed_slice())
}

fn explode(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(r.take(3 * 8 + 4 + 4)?);
    if flag(&mut r, &mut out)? {
        out.extend_from_slice(r.take(3 * 8)?);
    }

    let mut probe = Vec::new();
    particle(ids, &mut r, &mut out, &mut probe)?;
    sound_holder(ids, &mut r, &mut out)?;

    let count = r.count()?;
    write_varint(&mut out, count as i32);
    for _ in 0..count {
        particle(ids, &mut r, &mut out, &mut probe)?;
        out.extend_from_slice(r.take(4 + 4)?);
        write_varint(&mut out, r.varint()?);
    }
    Some(out.into_boxed_slice())
}

fn sound(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 2);
    sound_holder(ids, &mut r, &mut out)?;
    out.extend_from_slice(r.rest());
    Some(out.into_boxed_slice())
}

fn block_event(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len() + 1);
    out.extend_from_slice(r.take(8 + 1 + 1)?);
    let id = block(ids, r.varint()?)?;
    write_varint(&mut out, id);
    Some(out.into_boxed_slice())
}

const STAT_MINED: i32 = 0;
const STAT_DROPPED: i32 = 5;
const STAT_KILLED: i32 = 6;
const STAT_KILLED_BY: i32 = 7;

fn award_stats(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let count = r.count()?;
    let mut out = Vec::with_capacity(body.len() + count.min(body.len()));
    write_varint(&mut out, count as i32);
    for _ in 0..count {
        let kind = r.varint()?;
        write_varint(&mut out, kind);
        let value = r.varint()?;
        let value = match kind {
            STAT_MINED => block(ids, value)?,
            k if (STAT_MINED + 1..=STAT_DROPPED).contains(&k) => {
                ids.to_native.map(IdSpace::Item, value)?
            }
            STAT_KILLED | STAT_KILLED_BY => ids.to_native.map(IdSpace::EntityType, value)?,
            _ => value,
        };
        write_varint(&mut out, value);
        write_varint(&mut out, r.varint()?);
    }
    Some(out.into_boxed_slice())
}

fn add_entity(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    r.varint()?;
    r.take(16)?;
    let id_and_uuid = r.slice_from(0);
    let kind = ids.to_native.map(IdSpace::EntityType, r.varint()?)?;
    let mut out = Vec::with_capacity(body.len() + 1);
    out.extend_from_slice(id_and_uuid);
    write_varint(&mut out, kind);
    if kind != EntityKind::FallingBlock.to_u32() as i32 {
        out.extend_from_slice(r.rest());
        return Some(out.into_boxed_slice());
    }
    let before_data = r.pos();
    r.take(3 * 8)?;
    skip_lp_vec3(&mut r)?;
    r.take(3)?;
    out.extend_from_slice(&body[before_data..r.pos()]);
    let data = state(ids, r.varint()?)?;
    write_varint(&mut out, data);
    Some(out.into_boxed_slice())
}

fn update_attributes(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let mut out = Vec::with_capacity(body.len());
    write_varint(&mut out, r.varint()?);
    let n = r.count()?;
    write_varint(&mut out, n as i32);
    for _ in 0..n {
        write_varint(&mut out, Way::In.map(ids, IdSpace::Attribute, r.varint()?)?);
        out.extend_from_slice(r.take(8)?);
        let modifiers = r.count()?;
        write_varint(&mut out, modifiers as i32);
        for _ in 0..modifiers {
            identifier(&mut r, &mut out)?;
            out.extend_from_slice(r.take(8)?);
            write_varint(&mut out, r.varint()?);
        }
    }
    (r.remaining() == 0).then(|| out.into_boxed_slice())
}

const NO_BLOCK_ENTITY_TYPE: i32 = 49;

fn block_entity_data(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let pos = r.take(8)?;
    let kind = ids
        .to_native
        .map(IdSpace::BlockEntityType, r.varint()?)
        .unwrap_or(NO_BLOCK_ENTITY_TYPE);
    let mut out = Vec::with_capacity(body.len());
    out.extend_from_slice(pos);
    write_varint(&mut out, kind);
    out.extend_from_slice(r.rest());
    Some(out.into_boxed_slice())
}

const LEVEL_EVENT_DESTROY_BLOCK: u32 = 2001;
const LEVEL_EVENT_BRUSH_COMPLETE: u32 = 3008;

fn level_event(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let event = r.take(4)?;
    let kind = u32::from_be_bytes(event.try_into().ok()?);
    if !matches!(kind, LEVEL_EVENT_DESTROY_BLOCK | LEVEL_EVENT_BRUSH_COMPLETE) {
        return Some(body.into());
    }
    let pos = r.take(8)?;
    let data = state(ids, i32::from_be_bytes(r.take(4)?.try_into().ok()?))?;

    let mut out = Vec::with_capacity(body.len());
    out.extend_from_slice(event);
    out.extend_from_slice(pos);
    out.extend_from_slice(&data.to_be_bytes());
    out.extend_from_slice(r.rest());
    Some(out.into_boxed_slice())
}

fn block_update(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let pos = r.take(8)?;
    let id = state(ids, r.varint()?)?;
    let mut out = Vec::with_capacity(8 + varint_len(id));
    out.extend_from_slice(pos);
    write_varint(&mut out, id);
    Some(out.into_boxed_slice())
}

fn section_blocks_update(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    let mut r = Reader::new(body);
    let section = r.take(8)?;
    let count = r.count()?;

    let mut out = Vec::with_capacity(body.len() + count.min(body.len()));
    out.extend_from_slice(section);
    write_varint(&mut out, count as i32);
    for _ in 0..count {
        let packed = r.varlong()? as u64;
        let id = state(ids, i32::try_from(packed >> 12).ok()?)?;
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

fn level_chunk(ids: &Ids, body: &[u8]) -> Option<Box<[u8]>> {
    match rewrite_level_chunk(ids, body) {
        Ok(out) => Some(out),
        Err(why) => {
            crate::log_error!(
                "proto",
                "{why}. The wire's body is passing through untranslated and will not \
                 decode as {}; that column is lost",
                NATIVE.name
            );
            None
        }
    }
}

fn rewrite_level_chunk(ids: &Ids, body: &[u8]) -> Result<Box<[u8]>, String> {
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

    let sections = sections(ids, blob).map_err(|why| {
        format!("chunk {x},{z} was not translated: {why} (the blob is {length} bytes)")
    })?;
    let mut block_entities = Vec::new();
    chunk_block_entities(ids, &mut r, &mut block_entities).ok_or_else(|| {
        format!("chunk {x},{z} was not translated: its block entities did not walk")
    })?;
    let light = r.rest();

    let mut out =
        Vec::with_capacity(head.len() + 5 + sections.len() + block_entities.len() + light.len());
    out.extend_from_slice(head);
    let length = i32::try_from(sections.len())
        .map_err(|_| format!("chunk {x},{z} rewrote to a blob too long to length-prefix"))?;
    write_varint(&mut out, length);
    out.extend_from_slice(&sections);
    out.extend_from_slice(&block_entities);
    out.extend_from_slice(light);
    Ok(out.into_boxed_slice())
}

fn chunk_block_entities(ids: &Ids, r: &mut Reader, out: &mut Vec<u8>) -> Option<()> {
    let n = r.count()?;
    let mut kept = 0;
    let mut entries = Vec::new();
    for _ in 0..n {
        let start = entries.len();
        entries.extend_from_slice(r.take(1 + 2)?);
        let kind = ids.to_native.map(IdSpace::BlockEntityType, r.varint()?);
        if let Some(kind) = kind {
            write_varint(&mut entries, kind);
        }
        nbt_tag(r, &mut entries)?;
        if kind.is_some() {
            kept += 1;
        } else {
            entries.truncate(start);
        }
    }
    write_varint(out, kept);
    out.extend_from_slice(&entries);
    Some(())
}

#[derive(Debug)]
enum Bail {
    Short,
    NoTarget(i32),
    TooWide { id: i32, bits: u8 },
    BadWidth(u8),
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
                write!(f, "wire block state {id} has no {} id", NATIVE.name)
            }
            Bail::TooWide { id, bits } => write!(
                f,
                "wire block state {id} moved to an id too wide for the {bits}-bit \
                 container the server packed"
            ),
            Bail::BadWidth(bits) => {
                write!(f, "a container claimed {bits} bits per entry")
            }
        }
    }
}

fn sections(ids: &Ids, blob: &[u8]) -> Result<Vec<u8>, Failure> {
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
        if ids.v774() {
            out.extend_from_slice(&[0, 0]);
        } else {
            let Some(fluids) = r.take(2) else {
                let bail = Bail::Short;
                return Err(Failure {
                    bail,
                    section,
                    part: "the fluid count",
                    at: r.pos(),
                });
            };
            out.extend_from_slice(fluids);
        }
        if let Err(bail) = container(
            ids,
            &mut r,
            &mut out,
            SECTION_STATES,
            STATES_INDIRECT_MAX,
            <BlockState as PalletedContainerKind>::storage_bits,
            true,
        ) {
            return Err(Failure {
                bail,
                section,
                part: "block states",
                at: r.pos(),
            });
        }
        if let Err(bail) = container(
            ids,
            &mut r,
            &mut out,
            SECTION_BIOMES,
            BIOMES_INDIRECT_MAX,
            <Biome as PalletedContainerKind>::storage_bits,
            false,
        ) {
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
    ids: &Ids,
    r: &mut Reader,
    out: &mut Vec<u8>,
    size: usize,
    indirect_max: u8,
    width: fn(u8) -> u8,
    remap: bool,
) -> Result<(), Bail> {
    let sent = r.u8().ok_or(Bail::Short)?;
    let bits = width(sent);
    if bits > 32 {
        return Err(Bail::BadWidth(sent));
    }
    out.push(bits);

    let id = |r: &mut Reader, out: &mut Vec<u8>| -> Result<(), Bail> {
        let raw = r.varint().ok_or(Bail::Short)?;
        let moved = if remap {
            state(ids, raw).ok_or(Bail::NoTarget(raw))?
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
                let moved = state(ids, raw).ok_or(Bail::NoTarget(raw))?;
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

    fn ids() -> &'static Ids {
        &hop().ids
    }

    fn hop775() -> &'static Hop {
        Hop::v775()
    }

    fn ids775() -> &'static Ids {
        &hop775().ids
    }

    fn native_id(dir: Direction, name: &str) -> u32 {
        PacketTable::native()
            .id(Phase::Game, dir, name)
            .unwrap_or_else(|| panic!("{} has {name}", NATIVE.name))
    }

    fn native_serverbound(name: &str) -> u32 {
        native_id(Direction::Serverbound, name)
    }

    fn framed(id: u32, body: &[u8]) -> Box<[u8]> {
        frame(id, body)
    }

    #[test]
    fn every_inbound_id_lands_on_the_same_name() {
        for (h, protocol) in [(hop(), 774), (hop775(), 775)] {
            let old = PacketTable::for_protocol(protocol).unwrap();
            let new = PacketTable::native();
            for wire in 0..old.count(Phase::Game, Direction::Clientbound) {
                let name = old
                    .name_of(Phase::Game, Direction::Clientbound, wire as u32)
                    .unwrap();
                let native = h.inbound_game[wire];
                assert_eq!(
                    new.name_of(Phase::Game, Direction::Clientbound, native),
                    Some(name),
                    "{protocol} clientbound {wire} ({name}) landed wrong"
                );
            }
        }
    }

    fn suppressed(h: &Hop, protocol: i32) -> Vec<&'static str> {
        let old = PacketTable::for_protocol(protocol).unwrap();
        let new = PacketTable::native();
        let mut suppressed = Vec::new();
        for native in 0..new.count(Phase::Game, Direction::Serverbound) {
            let name = new
                .name_of(Phase::Game, Direction::Serverbound, native as u32)
                .unwrap();
            match h.outbound_game[native] {
                Some(wire) => {
                    let landed = old.name_of(Phase::Game, Direction::Serverbound, wire);
                    assert!(
                        landed == Some(name) || landed == older_name(name),
                        "{protocol}: {name} landed on {landed:?}"
                    );
                }
                None => suppressed.push(name),
            }
        }
        suppressed.sort_unstable();
        suppressed
    }

    #[test]
    fn every_outbound_id_lands_on_the_same_name_or_is_suppressed() {
        assert_eq!(
            suppressed(hop(), 774),
            ["attack", "set_game_rule", "spectator_action"],
            "774 has no packet of any of these names"
        );
        assert!(
            suppressed(hop775(), 775).is_empty(),
            "775 has every one of 26.2's serverbound packets, one under its older name"
        );
    }

    #[test]
    fn a_renamed_packet_lands_on_its_older_name() {
        let native = native_serverbound("spectator_action") as usize;
        let wire = hop775().outbound_game[native].expect("775 has it under its old name");
        assert_eq!(
            PacketTable::v26_1_1().name_of(Phase::Game, Direction::Serverbound, wire),
            Some("spectate_entity")
        );
        assert_eq!(newer_name("spectate_entity"), Some("spectator_action"));
        assert_eq!(older_name("attack"), None);
    }

    #[test]
    fn attack_is_rewritten_rather_than_suppressed() {
        let h = hop();
        let mut frame = Vec::new();
        write_varint(&mut frame, native_serverbound("attack") as i32);
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
        for name in ["set_game_rule", "spectator_action"] {
            let id = native_serverbound(name);
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
        let out = set_time(ids(), &body).expect("17 bytes is a whole packet");

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
        let out = set_time(ids(), &body).unwrap();
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
        assert!(set_time(ids(), &[0u8; 16]).is_none());
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
        state(ids(), id).unwrap_or_else(|| panic!("774 state {id} does not map"))
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
        let out = sections(
            ids(),
            &section_bytes(&container_bytes(0, &[FIRST_RUN], &[])),
        )
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
        let out = sections(ids(), &section_bytes(&states)).expect("a whole section");

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
        let out = sections(self::ids(), &section_bytes(&states)).expect("a whole section");

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
        let out = sections(ids(), &blob).expect("three whole sections");
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
        let tail = [0x00u8, 0xDE, 0xAD, 0xBE, 0xEF];

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

        let out = level_chunk(ids(), &body).expect("a whole chunk");
        assert_eq!(&out[..head_len], &body[..head_len], "head is untouched");
        let mut r = Reader::new(&out[head_len..]);
        assert_eq!(
            r.count(),
            Some(blob.len() + 2),
            "the length prefix grew by the one fluid count"
        );
        r.take(blob.len() + 2);
        assert_eq!(
            r.rest(),
            tail,
            "no block entities, and the light is untouched"
        );
    }

    #[test]
    fn a_truncated_chunk_does_not_rewrite() {
        assert!(level_chunk(ids(), &[]).is_none());
        assert!(matches!(
            sections(ids(), &[0x01]),
            Err(Failure {
                bail: Bail::Short,
                ..
            })
        ));
        let short = sections(ids(), &section_bytes(&container_bytes(4, &[1], &[0; 2])));
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
            .find(|id| state(ids(), *id).is_none())
            .expect("the table ends somewhere");

        let single = sections(
            ids(),
            &section_bytes(&container_bytes(0, &[past_the_end], &[])),
        );
        assert!(matches!(
            single,
            Err(Failure {
                bail: Bail::NoTarget(id),
                part: "block states",
                ..
            }) if id == past_the_end
        ));

        let indirect = sections(
            ids(),
            &section_bytes(&container_bytes(4, &[past_the_end], &[0; 256])),
        );
        assert!(matches!(
            indirect,
            Err(Failure { bail: Bail::NoTarget(id), .. }) if id == past_the_end
        ));

        let mut packed = [0u64; 1024];
        packed[0] = u64::try_from(past_the_end).expect("a positive id");
        let direct = sections(ids(), &section_bytes(&container_bytes(15, &[], &packed)));
        assert!(matches!(
            direct,
            Err(Failure { bail: Bail::NoTarget(id), .. }) if id == past_the_end
        ));
    }

    #[test]
    fn a_rewritten_column_decodes_in_azalea() {
        let palette = [BELOW_FIRST_RUN, FIRST_RUN, SECOND_RUN, LAST_RUN];
        let one = section_bytes(&container_bytes(4, &palette, &[0x3210_3210_3210_3210; 256]));
        let mut blob = Vec::new();
        for _ in 0..24 {
            blob.extend_from_slice(&one);
        }

        let out = sections(ids(), &blob).expect("a whole column");
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
    fn a_narrow_block_container_is_read_at_vanillas_width() {
        let palette = [BELOW_FIRST_RUN, FIRST_RUN, SECOND_RUN, LAST_RUN];
        let one = section_bytes(&container_bytes(2, &palette, &[0x3210_3210_3210_3210; 256]));
        let mut blob = Vec::new();
        for _ in 0..24 {
            blob.extend_from_slice(&one);
        }

        let out = sections(ids(), &blob).expect("a whole column");
        assert_eq!(out[4], 4, "the stored width goes out, not the sent one");
        let chunk = azalea_world::Chunk::read_with_dimension_height(
            &mut std::io::Cursor::new(&out[..]),
            384,
            -64,
            &[],
        )
        .expect("azalea decodes the rewritten column");
        assert_eq!(chunk.sections.len(), 24);
        assert_eq!(chunk.sections[23].states.bits_per_entry, 4);
    }

    #[test]
    fn the_failure_names_its_section() {
        let good = section_bytes(&container_bytes(0, &[FIRST_RUN], &[]));
        let bad = section_bytes(&container_bytes(0, &[i32::MAX], &[]));
        let mut blob = good.clone();
        blob.extend_from_slice(&good);
        blob.extend_from_slice(&bad);
        assert!(matches!(
            sections(ids(), &blob),
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
        let out = block_update(ids(), &body).unwrap();
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

        let out = section_blocks_update(ids(), &body).unwrap();
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
        assert!(block_update(ids(), &body).is_none());
    }

    const ITEM_AT_THE_SEAM: i32 = 230;
    const COMPONENT_AT_THE_SEAM: i32 = 41;
    const ITEM_AT_THE_SEAM_NATIVE: i32 = 258;
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
        assert_eq!(translated(|r, out| item_stack(ids(), r, out), &body), body);
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

        let out = translated(|r, out| item_stack(ids(), r, out), &body);
        let mut r = Reader::new(&out);
        assert_eq!(r.varint(), Some(3), "the count is not an id");
        assert_eq!(r.varint(), Some(ITEM_AT_THE_SEAM_NATIVE));
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
    fn an_advancement_icon_becomes_a_template() {
        let nbt_string = |out: &mut Vec<u8>, s: &[u8]| {
            out.push(8);
            out.extend_from_slice(&(s.len() as u16).to_be_bytes());
            out.extend_from_slice(s);
        };
        let mut head = vec![0u8];
        write_varint(&mut head, 1);
        identifier(&mut Reader::new(b"\x0dminecraft:a/b"), &mut head).unwrap();
        head.push(0);
        head.push(1);
        nbt_string(&mut head, b"title");
        nbt_string(&mut head, b"description");

        let mut body = head.clone();
        write_varint(&mut body, 1);
        write_varint(&mut body, ITEM_AT_THE_SEAM);
        write_varint(&mut body, 0);
        write_varint(&mut body, 0);
        let mut tail = Vec::new();
        write_varint(&mut tail, 2);
        tail.extend_from_slice(&0i32.to_be_bytes());
        tail.extend_from_slice(&[0u8; 8]);
        write_varint(&mut tail, 1);
        write_varint(&mut tail, 1);
        identifier(&mut Reader::new(b"\x04crit"), &mut tail).unwrap();
        tail.push(0);
        tail.extend_from_slice(&[0, 0, 1]);
        body.extend_from_slice(&tail);

        let out = update_advancements(ids(), &body).expect("rewrites");
        let mut want = head;
        write_varint(&mut want, ITEM_AT_THE_SEAM_NATIVE);
        write_varint(&mut want, 1);
        write_varint(&mut want, 0);
        write_varint(&mut want, 0);
        want.extend_from_slice(&tail);
        assert_eq!(&out[..], &want[..]);
    }

    #[test]
    fn a_damage_resistant_tag_becomes_a_named_holder_set() {
        let kind = DataComponentKind::DamageResistant;
        let id = kind.to_u32() as i32;
        assert_eq!(
            ids().from_native.map(IdSpace::DataComponentType, id),
            Some(id),
            "below the seam, so only the payload moves"
        );
        let tag = b"minecraft:is_fire";
        let mut payload_774 = Vec::new();
        write_varint(&mut payload_774, tag.len() as i32);
        payload_774.extend_from_slice(tag);

        let mut body = Vec::new();
        write_varint(&mut body, 1);
        write_varint(&mut body, 1);
        write_varint(&mut body, 1);
        write_varint(&mut body, 0);
        write_varint(&mut body, id);
        body.extend_from_slice(&payload_774);

        let out = translated(|r, out| item_stack(ids(), r, out), &body);
        let mut cursor = std::io::Cursor::new(&out[..]);
        let stack = azalea_inventory::ItemStack::azalea_read(&mut cursor).expect("decodes as 775");
        assert_eq!(cursor.position() as usize, out.len());
        assert!(!stack.is_empty());

        let mut payload_775 = Vec::new();
        write_varint(&mut payload_775, 0);
        payload_775.extend_from_slice(&payload_774);
        let back = translated(
            |r, out| {
                nested(ids(), Way::Out, kind, r, out)
                    .filter(|&b| b)
                    .map(|_| ())
            },
            &payload_775,
        );
        assert_eq!(back, payload_774);
    }

    #[test]
    fn an_equippable_moves_both_sounds_and_keeps_what_follows_aligned() {
        use azalea_inventory::components::{EquipmentSlot, Equippable, MaxStackSize};
        use azalea_registry::Holder;
        use azalea_registry::builtin::SoundEvent;

        let sound_native = ids()
            .to_native
            .map(IdSpace::SoundEvent, SOUND_774)
            .expect("774 sound 280 exists in 26.2");
        let wire = Equippable {
            slot: EquipmentSlot::Feet,
            equip_sound: Holder::Reference(SoundEvent::from_u32(SOUND_774 as u32).unwrap()),
            shearing_sound: Holder::Direct(azalea_core::sound::CustomSound {
                sound_id: "custom:clank".into(),
                range: Some(8.0),
            }),
            asset_id: Some("minecraft:iron".into()),
            ..Equippable::new()
        };
        let kind = DataComponentKind::Equippable;
        let id = kind.to_u32() as i32;
        assert_eq!(
            ids().to_native.map(IdSpace::DataComponentType, id),
            Some(id),
            "below the seam, so only the payload moves"
        );

        let mut body = Vec::new();
        write_varint(&mut body, 1);
        write_varint(&mut body, 1);
        write_varint(&mut body, 2);
        write_varint(&mut body, 0);
        write_varint(&mut body, id);
        wire.azalea_write(&mut body).unwrap();
        write_varint(&mut body, DataComponentKind::MaxStackSize.to_u32() as i32);
        write_varint(&mut body, 7);

        let out = translated(|r, out| item_stack(ids(), r, out), &body);
        let mut cursor = std::io::Cursor::new(&out[..]);
        let stack = azalea_inventory::ItemStack::azalea_read(&mut cursor).expect("decodes as 775");
        assert_eq!(cursor.position() as usize, out.len());
        let azalea_inventory::ItemStack::Present(data) = stack else {
            panic!("a present stack");
        };
        let got = data.component_patch.get::<Equippable>().expect("kept");
        assert_eq!(got.slot, EquipmentSlot::Feet);
        assert_eq!(
            got.equip_sound,
            Holder::Reference(SoundEvent::from_u32(sound_native as u32).unwrap()),
            "the reference moved"
        );
        assert_eq!(
            got.shearing_sound, wire.shearing_sound,
            "the inline sound did not"
        );
        assert_eq!(got.asset_id, wire.asset_id);
        assert_eq!(
            data.component_patch.get::<MaxStackSize>().map(|m| m.count),
            Some(7),
            "the component after it is intact"
        );
    }

    fn walked(ids: &Ids, way: Way, kind: DataComponentKind, payload: &[u8]) -> Vec<u8> {
        translated(
            |r, out| nested(ids, way, kind, r, out).filter(|&b| b).map(|_| ()),
            payload,
        )
    }

    fn encoded<T: AzBuf>(value: &T) -> Vec<u8> {
        let mut out = Vec::new();
        value.azalea_write(&mut out).unwrap();
        out
    }

    #[test]
    fn a_tools_direct_blocks_move() {
        use azalea_inventory::components::{Tool, ToolRule};
        use azalea_registry::HolderSet;
        use azalea_registry::builtin::BlockKind;

        let tool = |blocks: Vec<i32>| Tool {
            rules: vec![
                ToolRule {
                    blocks: HolderSet::Direct {
                        contents: blocks
                            .iter()
                            .map(|&b| BlockKind::from_u32(b as u32).unwrap())
                            .collect(),
                    },
                    speed: Some(4.0),
                    correct_for_drops: Some(true),
                },
                ToolRule {
                    blocks: HolderSet::Named {
                        key: "minecraft:mineable/pickaxe".into(),
                        contents: vec![],
                    },
                    speed: None,
                    correct_for_drops: None,
                },
            ],
            damage_per_block: 2,
            ..Tool::new()
        };
        let kind = DataComponentKind::Tool;
        for (ids, wire, native) in [
            (ids775(), vec![1, 998], vec![1, 1025]),
            (ids(), vec![157, 158], vec![157, 159]),
        ] {
            for (w, n) in wire.iter().zip(&native) {
                assert_eq!(
                    ids.to_native.map(IdSpace::Block, *w),
                    Some(*n),
                    "the table says so"
                );
            }
            assert_eq!(
                walked(ids, Way::In, kind, &encoded(&tool(wire.clone()))),
                encoded(&tool(native.clone()))
            );
            assert_eq!(
                walked(ids, Way::Out, kind, &encoded(&tool(native))),
                encoded(&tool(wire))
            );
        }
    }

    #[test]
    fn an_equippables_allowed_entities_move() {
        use azalea_inventory::components::{EquipmentSlot, Equippable};
        use azalea_registry::HolderSet;

        let saddle = |entities: [i32; 2]| Equippable {
            slot: EquipmentSlot::Saddle,
            allowed_entities: Some(HolderSet::Direct {
                contents: entities
                    .iter()
                    .map(|&e| EntityKind::from_u32(e as u32).unwrap())
                    .collect(),
            }),
            ..Equippable::new()
        };
        assert_eq!(ids775().to_native.map(IdSpace::EntityType, 130), Some(131));
        let kind = DataComponentKind::Equippable;
        assert_eq!(
            walked(ids775(), Way::In, kind, &encoded(&saddle([5, 130]))),
            encoded(&saddle([5, 131]))
        );
        assert_eq!(
            walked(ids775(), Way::Out, kind, &encoded(&saddle([5, 131]))),
            encoded(&saddle([5, 130]))
        );
    }

    #[test]
    fn an_inline_instrument_or_song_moves_its_sound() {
        use azalea_inventory::components::{
            Instrument, InstrumentData, JukeboxPlayable, JukeboxSongData,
        };
        use azalea_registry::Holder;
        use azalea_registry::builtin::SoundEvent;

        let native = ids775().to_native.map(IdSpace::SoundEvent, 300).unwrap();
        assert_ne!(native, 300, "pick a sound that moved");
        let sound = |id: i32| Holder::Reference(SoundEvent::from_u32(id as u32).unwrap());
        let horn = |id| Instrument {
            value: Holder::Direct(InstrumentData {
                sound_event: sound(id),
                use_duration: 7.0,
                range: 256.0,
                description: "horn".into(),
            }),
        };
        let disc = |id| JukeboxPlayable {
            value: Holder::Direct(JukeboxSongData {
                sound_event: sound(id),
                description: "song".into(),
                length_in_seconds: 90.0,
                comparator_output: 4,
            }),
        };
        let horn_kind = DataComponentKind::Instrument;
        let disc_kind = DataComponentKind::JukeboxPlayable;
        assert_eq!(
            walked(ids775(), Way::In, horn_kind, &encoded(&horn(300))),
            encoded(&horn(native))
        );
        assert_eq!(
            walked(ids775(), Way::Out, horn_kind, &encoded(&horn(native))),
            encoded(&horn(300))
        );
        assert_eq!(
            walked(ids775(), Way::In, disc_kind, &encoded(&disc(300))),
            encoded(&disc(native))
        );

        let by_reference = Instrument {
            value: Holder::Reference(
                <azalea_registry::data::Instrument as azalea_registry::DataRegistry>::new_raw(3),
            ),
        };
        assert_eq!(
            walked(ids775(), Way::In, horn_kind, &encoded(&by_reference)),
            encoded(&by_reference)
        );
    }

    #[test]
    fn a_774_lazy_reference_loses_and_regains_its_prefix() {
        use azalea_inventory::components::{Instrument, InstrumentData};
        use azalea_registry::Holder;

        let horn = Instrument {
            value: Holder::Direct(InstrumentData {
                sound_event: Holder::Reference(
                    azalea_registry::builtin::SoundEvent::from_u32(3).unwrap(),
                ),
                use_duration: 7.0,
                range: 256.0,
                description: "horn".into(),
            }),
        };
        assert_eq!(
            ids().to_native.map(IdSpace::SoundEvent, 3),
            Some(3),
            "below every sound run"
        );
        let kind = DataComponentKind::Instrument;
        let mut wire = vec![1];
        wire.extend(encoded(&horn));
        assert_eq!(walked(ids(), Way::In, kind, &wire), encoded(&horn));
        assert_eq!(walked(ids(), Way::Out, kind, &encoded(&horn)), wire);

        let mut chicken = vec![1];
        write_varint(&mut chicken, 2);
        let out = translated(
            |r, out| payload(ids(), Way::In, DataComponentKind::ChickenVariant, r, out),
            &chicken,
        );
        let mut want = Vec::new();
        write_varint(&mut want, 2);
        assert_eq!(out, want);

        let mut key = vec![0];
        string(&mut key, "minecraft:ponder_goat_horn");
        assert!(
            nested(
                ids(),
                Way::In,
                kind,
                &mut Reader::new(&key),
                &mut Vec::new()
            )
            .is_none()
        );
    }

    fn registry_frame(registry: &str, keys: &[&str]) -> Box<[u8]> {
        let packet = ClientboundRegistryData {
            registry_id: Identifier::new(registry),
            entries: keys
                .iter()
                .map(|key| (Identifier::new(*key), None))
                .collect(),
        };
        let id = PacketTable::native()
            .id(
                Phase::Configuration,
                Direction::Clientbound,
                "registry_data",
            )
            .expect("registry_data");
        let mut frame = Vec::new();
        write_varint(&mut frame, id as i32);
        packet.azalea_write(&mut frame).unwrap();
        frame.into_boxed_slice()
    }

    fn key_form(key: &str) -> Vec<u8> {
        let mut wire = vec![0];
        string(&mut wire, key);
        wire
    }

    #[test]
    fn a_774_lazy_reference_by_key_resolves_through_registry_data() {
        use azalea::connection::PacketTranslator as _;

        let mut translator = Translator::for_protocol(774).expect("a 774 hop");
        let frame = registry_frame(
            "minecraft:instrument",
            &[
                "minecraft:ponder_goat_horn",
                "minecraft:sing_goat_horn",
                "minecraft:seek_goat_horn",
            ],
        );
        assert_eq!(
            translator.inbound(ConnectionProtocol::Configuration, frame.clone()),
            frame
        );
        translator.inbound(
            ConnectionProtocol::Configuration,
            registry_frame(
                "minecraft:chicken_variant",
                &["minecraft:temperate", "minecraft:cold"],
            ),
        );
        let _scope = OrderScope::enter(&translator.order).expect("recorded");

        let mut holder = Vec::new();
        write_varint(&mut holder, 3);
        assert_eq!(
            walked(
                ids(),
                Way::In,
                DataComponentKind::Instrument,
                &key_form("seek_goat_horn")
            ),
            holder
        );

        let mut cold = Vec::new();
        write_varint(&mut cold, 2);
        let out = translated(
            |r, out| payload(ids(), Way::In, DataComponentKind::ChickenVariant, r, out),
            &key_form("minecraft:cold"),
        );
        assert_eq!(out, cold);

        for (kind, key) in [
            (DataComponentKind::Instrument, "minecraft:cold"),
            (DataComponentKind::Instrument, "minecraft:nope"),
            (
                DataComponentKind::JukeboxPlayable,
                "minecraft:seek_goat_horn",
            ),
        ] {
            let wire = key_form(key);
            assert!(
                nested(
                    ids(),
                    Way::In,
                    kind,
                    &mut Reader::new(&wire),
                    &mut Vec::new()
                )
                .is_none()
            );
        }
    }

    #[test]
    fn a_775_translator_records_nothing() {
        use azalea::connection::PacketTranslator as _;

        let mut translator = Translator::for_protocol(775).expect("a 775 hop");
        translator.inbound(
            ConnectionProtocol::Configuration,
            registry_frame("minecraft:instrument", &["minecraft:ponder_goat_horn"]),
        );
        assert!(translator.order.is_none());
        assert!(OrderScope::enter(&translator.order).is_none());
    }

    #[test]
    fn a_failed_walk_reports_the_unmapped_id_and_the_trail() {
        let mut body = Vec::new();
        write_varint(&mut body, 5);
        write_varint(&mut body, ITEM_AT_THE_SEAM);
        write_varint(&mut body, 2);
        write_varint(&mut body, 0);
        let first_at = body.len();
        write_varint(&mut body, DataComponentKind::MaxStackSize.to_u32() as i32);
        write_varint(&mut body, 64);
        let bad_at = body.len();
        write_varint(&mut body, 9999);

        trail::reset();
        assert!(item_stack(ids(), &mut Reader::new(&body), &mut Vec::new()).is_none());
        let report = trail::report(&body);
        assert!(
            report.contains("wire DataComponentType id 9999 names nothing in 26.2"),
            "{report}"
        );
        assert!(
            report.contains(&format!("at byte {} of {}", body.len(), body.len())),
            "{report}"
        );
        assert!(
            report.contains(&format!("stack@0[item {ITEM_AT_THE_SEAM} x5]")),
            "{report}"
        );
        assert!(
            report.contains(&format!(
                "MaxStackSize({})@{first_at}[2b]",
                DataComponentKind::MaxStackSize.to_u32()
            )),
            "{report}"
        );
        assert!(
            report.contains(&format!("?(9999)@{bad_at}[unmapped]")),
            "{report}"
        );
        assert!(report.contains("bytes 0.."), "{report}");
    }

    #[test]
    fn a_failed_walk_reports_the_decode_error_against_its_kind() {
        let mut body = Vec::new();
        write_varint(&mut body, 1);
        write_varint(&mut body, 1);
        write_varint(&mut body, 1);
        write_varint(&mut body, 0);
        write_varint(&mut body, DataComponentKind::Lore.to_u32() as i32);
        write_varint(&mut body, 1);
        body.push(0x7f);

        trail::reset();
        assert!(item_stack(ids(), &mut Reader::new(&body), &mut Vec::new()).is_none());
        let report = trail::report(&body);
        assert!(report.contains("Lore did not decode: "), "{report}");
        assert!(report.contains("[unfinished]"), "{report}");

        trail::reset();
        assert!(trail::reason().contains("the frame ended before the layout did"));
    }

    #[test]
    fn a_component_past_the_seam_keeps_its_payload() {
        let native = DataComponentKind::MapId.to_u32() as i32;
        let wire = ids()
            .from_native
            .map(IdSpace::DataComponentType, native)
            .expect("26.2's map_id exists in 774");
        assert_ne!(wire, native, "pick a component that actually moved");

        let mut body = Vec::new();
        write_varint(&mut body, 1);
        write_varint(&mut body, 0);
        write_varint(&mut body, 1);
        write_varint(&mut body, 0);
        write_varint(&mut body, wire);
        write_varint(&mut body, 1234);

        let out = translated(|r, out| item_stack(ids(), r, out), &body);
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

        let out = entity_data(ids(), &body).expect("a whole packet");
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

        let out = entity_data(ids(), &body).unwrap();
        let mut r = Reader::new(&out);
        r.varint();
        r.u8();
        assert_eq!(r.varint(), Some(SERIALIZER_ITEM_STACK));
        assert_eq!(r.varint(), Some(1));
        assert_eq!(
            r.varint(),
            Some(ITEM_AT_THE_SEAM_NATIVE),
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
        write_varint(&mut body, 3);
        write_varint(&mut body, 2);
        write_varint(&mut body, 3);
        write_varint(&mut body, 0);
        write_varint(&mut body, 0);

        let out = update_recipes(ids(), &body).expect("rewrites");
        let mut expected = Vec::new();
        write_varint(&mut expected, 0);
        write_varint(&mut expected, 1);
        write_varint(&mut expected, 2);
        write_varint(&mut expected, 1);
        write_varint(&mut expected, SLOT_DISPLAY_ITEM_STACK);
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
        assert!(entity_data(ids(), &[]).is_none());
        assert!(entity_data(ids(), &[0x01, 0x00]).is_none());
        let mut body = Vec::new();
        write_varint(&mut body, 1);
        assert!(item_stack(ids(), &mut Reader::new(&body), &mut Vec::new()).is_none());
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
        write_varint(&mut frame, native_serverbound("attack") as i32);
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
            let native = ids()
                .to_native
                .map(IdSpace::ParticleType, wire)
                .unwrap_or_else(|| panic!("774 particle {wire} does not map"));
            let mut alone = Vec::new();
            write_varint(&mut alone, native);
            let mut cursor = std::io::Cursor::new(&alone[..]);
            let payload_free = azalea::entity::particle::Particle::azalea_read(&mut cursor).is_ok();
            assert_eq!(
                payload_free,
                !PAYLOAD_774.contains(&wire),
                "774 particle {wire} lands on 26.2's {native}, which has the other shape"
            );
        }
    }

    #[test]
    fn without_the_remap_24_ids_would_change_shape() {
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
            [
                7, 9, 10, 14, 16, 22, 23, 28, 29, 38, 42, 43, 45, 46, 47, 48, 53, 54, 55, 56, 103,
                109, 112, 113,
            ]
        );
    }

    #[test]
    fn an_item_particle_is_a_template_on_both_wires() {
        use azalea_registry::builtin::ParticleKind;
        let native = ParticleKind::Item.to_u32() as i32;
        let run = |ids: &Ids, wire: &[i32]| {
            let mut body = Vec::new();
            write_varint(
                &mut body,
                ids.from_native.map(IdSpace::ParticleType, native).unwrap(),
            );
            for value in wire {
                write_varint(&mut body, *value);
            }
            let (mut r, mut out, mut probe) = (Reader::new(&body), Vec::new(), Vec::new());
            particle(ids, &mut r, &mut out, &mut probe).expect("a whole particle");
            assert_eq!(r.remaining(), 0);
            out
        };
        let item_775 = 1000;
        let moved = ids775().to_native.map(IdSpace::Item, item_775).unwrap();
        assert_ne!(moved, item_775, "an id past 26.2's first item insertion");
        let want = |item: i32| {
            let mut out = Vec::new();
            for value in [native, item, 1, 0, 0] {
                write_varint(&mut out, value);
            }
            out
        };
        assert_eq!(run(ids775(), &[item_775, 1, 0, 0]), want(moved));
        assert_eq!(
            run(ids(), &[1, ITEM_AT_THE_SEAM, 0, 0]),
            want(ITEM_AT_THE_SEAM_NATIVE)
        );
    }

    #[test]
    fn a_ghost_recipe_moves_its_slot_displays() {
        let item_775 = 1000;
        let moved = ids775().to_native.map(IdSpace::Item, item_775).unwrap();
        let body = |item: i32| {
            let mut out = Vec::new();
            for value in [3, 3] {
                write_varint(&mut out, value);
            }
            for _ in 0..3 {
                write_varint(&mut out, SLOT_DISPLAY_ITEM);
                write_varint(&mut out, item);
            }
            out
        };
        assert_eq!(
            place_ghost_recipe(ids775(), &body(item_775)).as_deref(),
            Some(&body(moved)[..])
        );
        assert!(
            place_ghost_recipe(ids775(), &[3, 9]).is_none(),
            "no such display"
        );
    }

    #[test]
    fn a_block_particle_moves_its_id_and_its_state() {
        let mut body = Vec::new();
        write_varint(&mut body, 109);
        write_varint(&mut body, SECOND_RUN);

        let mut r = Reader::new(&body);
        let mut out = Vec::new();
        particle(ids(), &mut r, &mut out, &mut Vec::new()).expect("a whole particle");
        assert_eq!(r.remaining(), 0);

        let mut r = Reader::new(&out);
        assert_eq!(
            r.varint(),
            Some(DUST_PILLAR_NATIVE),
            "dust_pillar moved by both inserts"
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
        particle(ids(), &mut r, &mut out, &mut Vec::new()).unwrap();
        assert_eq!(r.remaining(), 0, "the delay was consumed");

        let mut r = Reader::new(&out);
        assert_eq!(r.varint(), Some(112), "26.2's shriek");
        assert_eq!(r.varint(), Some(40));
    }

    #[test]
    fn a_payload_free_particle_is_its_id_alone() {
        let mut body = Vec::new();
        write_varint(&mut body, 3);
        body.extend_from_slice(&[0xAA, 0xBB]);

        let mut r = Reader::new(&body);
        let mut out = Vec::new();
        particle(ids(), &mut r, &mut out, &mut Vec::new()).unwrap();
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

        let out = level_particles(ids(), &body).expect("a whole packet");
        assert_eq!(&out[..header], &body[..header]);
        let mut r = Reader::new(&out[header..]);
        assert_eq!(r.varint(), Some(DUST_PILLAR_NATIVE));
        assert_eq!(r.varint(), Some(moved(FIRST_RUN)));
    }

    const SOUND_774: i32 = 280;
    const DUST_PILLAR_NATIVE: i32 = 118;

    #[test]
    fn a_sound_moves_through_the_holder() {
        let native = ids()
            .to_native
            .map(IdSpace::SoundEvent, SOUND_774)
            .expect("774 sound 280 exists in 26.2");
        assert_ne!(native, SOUND_774, "pick a sound that actually moved");

        let mut body = Vec::new();
        write_varint(&mut body, SOUND_774 + 1);
        body.extend_from_slice(&[0xAA; 4]);

        let out = sound(ids(), &body).expect("a whole packet");
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

        let out = sound(ids(), &body).expect("a whole packet");
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

        let out = explode(ids(), &body).expect("a whole packet");
        let mut r = Reader::new(&out);
        assert_eq!(r.take(3 * 8 + 4 + 4), Some(&body[..32]));
        assert_eq!(r.u8(), Some(0));
        assert_eq!(r.varint(), Some(DUST_PILLAR_NATIVE));
        assert_eq!(r.varint(), Some(moved(FIRST_RUN)));
        assert_eq!(r.varint(), Some(293 + 1));
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

        let out = entity_data(ids(), &body).expect("a whole packet");
        let mut r = Reader::new(&out);
        r.varint();
        r.u8();
        assert_eq!(r.varint(), Some(SERIALIZER_BLOCK_STATE));
        assert_eq!(r.varint(), Some(moved(LAST_RUN)));
        r.u8();
        assert_eq!(r.varint(), Some(SERIALIZER_PARTICLES));
        assert_eq!(r.count(), Some(1));
        assert_eq!(r.varint(), Some(DUST_PILLAR_NATIVE));
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

        let out = block_event(ids(), &body).unwrap();
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
        write_varint(&mut body, STAT_KILLED);
        write_varint(&mut body, 40);
        write_varint(&mut body, 9);

        let out = award_stats(ids(), &body).unwrap();
        let mut r = Reader::new(&out);
        assert_eq!(r.count(), Some(3));
        assert_eq!(
            (r.varint(), r.varint(), r.varint()),
            (Some(0), Some(502), Some(7))
        );
        assert_eq!(
            (r.varint(), r.varint(), r.varint()),
            (Some(5), Some(ITEM_AT_THE_SEAM_NATIVE), Some(8))
        );
        assert_eq!(
            (r.varint(), r.varint(), r.varint()),
            (Some(6), Some(40), Some(9)),
            "an entity type below the seam keeps its id"
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
        let out = add_entity(ids(), &add_entity_bytes(falling, LAST_RUN)).expect("a falling block");
        let mut r = Reader::new(&out);
        r.varint();
        r.take(16);
        assert_eq!(r.varint(), Some(falling));
        r.take(3 * 8 + 1 + 3);
        assert_eq!(r.varint(), Some(moved(LAST_RUN)));

        let arrow = EntityKind::Arrow.to_u32() as i32;
        let body = add_entity_bytes(arrow, LAST_RUN);
        assert_eq!(
            add_entity(ids(), &body).as_deref(),
            Some(&body[..]),
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

            let out = level_event(ids(), &body).unwrap_or_else(|| panic!("event {kind}"));
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
        assert_eq!(level_event(ids(), &body).as_deref(), Some(&body[..]));
    }

    #[test]
    fn a_hashed_stack_moves_its_ids_backwards() {
        let mut body = vec![1];
        write_varint(&mut body, ITEM_AT_THE_SEAM_NATIVE);
        write_varint(&mut body, 2);
        write_varint(&mut body, 1);
        write_varint(&mut body, COMPONENT_AT_THE_SEAM + 1);
        body.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        write_varint(&mut body, 1);
        write_varint(&mut body, COMPONENT_AT_THE_SEAM + 3);

        let mut r = Reader::new(&body);
        let mut out = Vec::new();
        assert_eq!(hashed_stack(ids(), &mut r, &mut out), Some(true));
        assert_eq!(r.remaining(), 0);

        let mut r = Reader::new(&out);
        assert_eq!(r.u8(), Some(1));
        assert_eq!(r.varint(), Some(ITEM_AT_THE_SEAM));
        assert_eq!(r.varint(), Some(2), "the count is not an id");
        assert_eq!(r.count(), Some(1));
        assert_eq!(r.varint(), Some(COMPONENT_AT_THE_SEAM));
        assert_eq!(r.take(4), Some(&[0xDE, 0xAD, 0xBE, 0xEF][..]));
        assert_eq!(r.count(), Some(1));
        assert_eq!(r.varint(), Some(COMPONENT_AT_THE_SEAM + 1));
    }

    #[test]
    fn a_hashed_stack_from_azaleas_writer_walks_whole() {
        use azalea::protocol::packets::game::s_container_click::{HashedPatchMap, HashedStack};
        let stack = HashedStack(Some(
            azalea::protocol::packets::game::s_container_click::HashedActualItem {
                kind: azalea_registry::builtin::ItemKind::Stone,
                count: 1,
                components: HashedPatchMap {
                    added_components: vec![
                        (DataComponentKind::CustomName, Default::default()),
                        (DataComponentKind::Lore, Default::default()),
                    ],
                    removed_components: vec![DataComponentKind::Food],
                },
            },
        ));
        let mut body = Vec::new();
        stack.azalea_write(&mut body).unwrap();
        let mut r = Reader::new(&body);
        assert_eq!(hashed_stack(ids(), &mut r, &mut Vec::new()), Some(true));
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_775_only_item_empties_the_desync_check_rather_than_the_frame() {
        const GOLDEN_DANDELION: i32 = 257;
        assert_eq!(
            ids().from_native.map(IdSpace::Item, GOLDEN_DANDELION),
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

        let out = container_click(ids(), &body).expect("the click still goes out");
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
        write_varint(&mut body, ITEM_AT_THE_SEAM_NATIVE);
        write_varint(&mut body, 1);
        write_varint(&mut body, 0);
        write_varint(&mut body, COMPONENT_AT_THE_SEAM + 1);
        write_varint(&mut body, 3);
        body.extend_from_slice(&[1, 2, 3]);

        let out = creative_slot(ids(), &body).expect("a whole stack");
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

    fn container_ids() -> (i32, i32) {
        let native = DataComponentKind::Container.to_u32() as i32;
        let wire = ids()
            .from_native
            .map(IdSpace::DataComponentType, native)
            .expect("774 has container");
        (native, wire)
    }

    #[test]
    fn a_shulker_box_becomes_optional_templates_and_keeps_what_follows_aligned() {
        let (native, wire) = container_ids();
        let mut body = Vec::new();
        for v in [
            1,
            5,
            1,
            0,
            wire,
            2,
            0,
            3,
            ITEM_AT_THE_SEAM,
            0,
            1,
            COMPONENT_AT_THE_SEAM,
        ] {
            write_varint(&mut body, v);
        }
        for v in [1, 7, 0, 0] {
            write_varint(&mut body, v);
        }

        let out = translated(
            |r, out| {
                item_stack(ids(), r, out)?;
                item_stack(ids(), r, out)
            },
            &body,
        );
        let mut expected = Vec::new();
        for v in [
            1,
            5,
            1,
            0,
            native,
            2,
            0,
            1,
            ITEM_AT_THE_SEAM_NATIVE,
            3,
            0,
            1,
            COMPONENT_AT_THE_SEAM + 1,
            1,
            7,
            0,
            0,
        ] {
            write_varint(&mut expected, v);
        }
        assert_eq!(out, expected);
    }

    #[test]
    fn a_creative_shulker_box_goes_out_in_774s_layout() {
        let (native, wire) = container_ids();
        let mut payload = Vec::new();
        for v in [1, 1, ITEM_AT_THE_SEAM_NATIVE, 3, 0, 0] {
            write_varint(&mut payload, v);
        }
        let mut body = 9u16.to_be_bytes().to_vec();
        for v in [1, 5, 1, 0, native, payload.len() as i32] {
            write_varint(&mut body, v);
        }
        body.extend_from_slice(&payload);

        let out = creative_slot(ids(), &body).expect("a whole stack");
        let mut expected = 9u16.to_be_bytes().to_vec();
        let mut wire_payload = Vec::new();
        for v in [1, 3, ITEM_AT_THE_SEAM, 0, 0] {
            write_varint(&mut wire_payload, v);
        }
        for v in [1, 5, 1, 0, wire, wire_payload.len() as i32] {
            write_varint(&mut expected, v);
        }
        expected.extend_from_slice(&wire_payload);
        assert_eq!(out.as_ref(), &expected[..]);
    }

    #[test]
    fn a_creative_slot_of_a_26_2_only_item_is_cleared() {
        let mut body = 9u16.to_be_bytes().to_vec();
        write_varint(&mut body, 1);
        write_varint(&mut body, 26);
        write_varint(&mut body, 0);
        write_varint(&mut body, 0);

        let out = creative_slot(ids(), &body).expect("the slot is still set");
        let mut r = Reader::new(&out);
        assert_eq!(r.take(2), Some(&9u16.to_be_bytes()[..]));
        assert_eq!(r.varint(), Some(0), "an empty stack is a zero count");
        assert_eq!(r.remaining(), 0);
    }

    fn string(out: &mut Vec<u8>, s: &str) {
        write_varint(out, s.len() as i32);
        out.extend_from_slice(s.as_bytes());
    }

    fn profile_bytes() -> Vec<u8> {
        let mut body = 7u128.to_be_bytes().to_vec();
        string(&mut body, "Steve");
        write_varint(&mut body, 1);
        string(&mut body, "textures");
        string(&mut body, "e30=");
        body.push(1);
        string(&mut body, "c2ln");
        body
    }

    #[test]
    fn login_finished_gains_a_nil_session_id() {
        let body = profile_bytes();
        let out = login_finished(ids775(), &body).expect("a whole profile");
        assert_eq!(&out[..body.len()], &body[..]);
        assert_eq!(&out[body.len()..], &[0u8; 16]);

        let mut cursor = std::io::Cursor::new(&out[..]);
        let packet =
            azalea_protocol::packets::login::ClientboundLoginFinished::azalea_read(&mut cursor)
                .expect("decodes as 26.2");
        assert_eq!(cursor.position() as usize, out.len());
        assert!(packet.session_id.is_nil());
        assert_eq!(packet.game_profile.name, "Steve");

        assert!(login_finished(ids775(), &out).is_none());
    }

    #[test]
    fn the_login_phase_rewrites_login_finished() {
        let id = PacketTable::native()
            .id(Phase::Login, Direction::Clientbound, "login_finished")
            .expect("26.2 has login_finished");
        let body = profile_bytes();
        for h in [hop(), hop775()] {
            let out = h.inbound(ConnectionProtocol::Login, framed(id, &body));
            let mut want = framed(id, &body).into_vec();
            want.extend_from_slice(&[0; 16]);
            assert_eq!(&*out, &want[..]);
        }
    }

    fn login_head() -> Vec<u8> {
        let mut body = 42i32.to_be_bytes().to_vec();
        body.push(0);
        write_varint(&mut body, 1);
        string(&mut body, "minecraft:overworld");
        write_varint(&mut body, 20);
        write_varint(&mut body, 10);
        write_varint(&mut body, 8);
        body.extend_from_slice(&[0, 1, 0]);
        write_varint(&mut body, 0);
        string(&mut body, "minecraft:overworld");
        body.extend_from_slice(&123i64.to_be_bytes());
        body.push(0);
        body.push(0xFF);
        body.extend_from_slice(&[0, 0]);
        body.push(1);
        string(&mut body, "minecraft:the_nether");
        body.extend_from_slice(&5u64.to_be_bytes());
        write_varint(&mut body, 0);
        write_varint(&mut body, 63);
        body
    }

    #[test]
    fn login_gains_online_mode_before_secure_chat() {
        let head = login_head();
        let mut body = head.clone();
        body.push(1);
        let out = login(ids775(), &body).expect("a whole login");
        assert_eq!(&out[..head.len()], &head[..]);
        assert_eq!(&out[head.len()..], &[ONLINE_MODE, 1]);

        let mut cursor = std::io::Cursor::new(&out[..]);
        let packet =
            azalea_protocol::packets::game::c_login::ClientboundLogin::azalea_read(&mut cursor)
                .expect("decodes as 26.2");
        assert_eq!(cursor.position() as usize, out.len());
        assert!(!packet.online_mode);
        assert!(
            packet.enforces_secure_chat,
            "the old last byte is still last"
        );
        assert_eq!(packet.common.sea_level, 63);

        assert!(login(ids775(), &head).is_none());
        let mut long = body.clone();
        long.push(0);
        assert!(login(ids775(), &long).is_none());
    }

    struct Team {
        name: Vec<u8>,
        display: Vec<u8>,
        prefix: Vec<u8>,
        suffix: Vec<u8>,
        players: Vec<u8>,
    }

    fn team() -> Team {
        let text = |out: &mut Vec<u8>, extra: &[u8], value: &[u8]| {
            out.push(10);
            out.extend_from_slice(extra);
            out.extend_from_slice(&[8, 0, 4]);
            out.extend_from_slice(b"text");
            out.extend_from_slice(&(value.len() as u16).to_be_bytes());
            out.extend_from_slice(value);
            out.push(0);
        };
        let mut name = Vec::new();
        string(&mut name, "a818536d2b44");
        let mut display = Vec::new();
        text(
            &mut display,
            &[
                8, 0, 5, b'c', b'o', b'l', b'o', b'r', 0, 5, b'w', b'h', b'i', b't', b'e',
            ],
            b"a818536d2b44",
        );
        let mut prefix = Vec::new();
        text(&mut prefix, &[], "\u{a7}a[VIP\u{a7}6+\u{a7}a] ".as_bytes());
        let mut suffix = Vec::new();
        text(&mut suffix, &[], b"");
        let mut players = Vec::new();
        write_varint(&mut players, 3);
        for player in ["HamaKiller", "Zxy1oN", "tamalesfam"] {
            string(&mut players, player);
        }
        Team {
            name,
            display,
            prefix,
            suffix,
            players,
        }
    }

    fn team_775(t: &Team, method: u8, color: i32) -> Vec<u8> {
        let mut body = t.name.clone();
        body.push(method);
        body.extend_from_slice(&t.display);
        body.extend_from_slice(&[3, 0, 1]);
        write_varint(&mut body, color);
        body.extend_from_slice(&t.prefix);
        body.extend_from_slice(&t.suffix);
        body.extend_from_slice(&t.players);
        body
    }

    #[test]
    fn a_team_is_reordered_and_its_colour_becomes_optional() {
        let t = team();
        let out = set_player_team(ids775(), &team_775(&t, 0, 10)).expect("a whole team");
        let mut want = t.name.clone();
        want.push(0);
        want.extend_from_slice(&t.display);
        want.extend_from_slice(&t.prefix);
        want.extend_from_slice(&t.suffix);
        want.extend_from_slice(&[0, 1, 1, 10, 3]);
        want.extend_from_slice(&t.players);
        assert_eq!(&*out, &want[..]);

        let mut cursor = std::io::Cursor::new(&out[..]);
        let packet = azalea_protocol::packets::game::c_set_player_team::ClientboundSetPlayerTeam::azalea_read(
            &mut cursor,
        )
        .expect("decodes as 26.2");
        assert_eq!(cursor.position() as usize, out.len());
        let azalea_protocol::packets::game::c_set_player_team::Method::Add((parameters, players)) =
            packet.method
        else {
            panic!("method 0 is add");
        };
        assert_eq!(format!("{:?}", parameters.color), "Some(Green)");
        assert_eq!(parameters.options, 3);
        assert_eq!(players.len(), 3);
    }

    #[test]
    fn a_team_without_a_colour_and_the_other_methods() {
        let t = team();
        let mut change = team_775(&t, 2, 21);
        change.truncate(change.len() - t.players.len());
        let out = set_player_team(ids775(), &change).expect("a whole change");
        let mut want = t.name.clone();
        want.push(2);
        want.extend_from_slice(&t.display);
        want.extend_from_slice(&t.prefix);
        want.extend_from_slice(&t.suffix);
        want.extend_from_slice(&[0, 1, 0, 3]);
        assert_eq!(&*out, &want[..]);

        let mut join = t.name.clone();
        join.push(3);
        join.extend_from_slice(&t.players);
        assert_eq!(set_player_team(ids775(), &join).as_deref(), Some(&join[..]));
    }

    #[test]
    fn a_spectator_action_goes_out_as_spectate_entity() {
        let id = native_serverbound("spectator_action");
        let wire = PacketTable::v26_1_1()
            .id(Phase::Game, Direction::Serverbound, "spectate_entity")
            .expect("26.1.1 has spectate_entity");

        let out = hop775().outbound(ConnectionProtocol::Game, framed(id, &[43]));
        assert_eq!(&*out, &framed(wire, &[42])[..], "entity 42, sent as itself");

        let mut big = Vec::new();
        write_varint(&mut big, 300 + 1);
        let mut want = Vec::new();
        write_varint(&mut want, 300);
        let out = hop775().outbound(ConnectionProtocol::Game, framed(id, &big));
        assert_eq!(&*out, &framed(wire, &want)[..]);

        assert!(
            hop775()
                .outbound(ConnectionProtocol::Game, framed(id, &[0]))
                .is_empty()
        );
        assert!(
            hop()
                .outbound(ConnectionProtocol::Game, framed(id, &[43]))
                .is_empty()
        );
    }

    #[test]
    fn attributes_move_and_their_modifiers_are_copied() {
        let mut body = Vec::new();
        write_varint(&mut body, 5);
        write_varint(&mut body, 2);
        write_varint(&mut body, 0);
        body.extend_from_slice(&2f64.to_be_bytes());
        write_varint(&mut body, 1);
        string(&mut body, "minecraft:test");
        body.extend_from_slice(&1.5f64.to_be_bytes());
        write_varint(&mut body, 2);
        write_varint(&mut body, 14);
        body.extend_from_slice(&0.08f64.to_be_bytes());
        write_varint(&mut body, 0);

        let mut want = body.clone();
        want[2] = 1;
        let gravity_at = want.len() - 1 - 8 - 1;
        assert_eq!(want[gravity_at], 14);
        want[gravity_at] = 18;

        for ids in [ids(), ids775()] {
            assert_eq!(update_attributes(ids, &body).as_deref(), Some(&want[..]));
        }
        assert!(update_attributes(ids775(), &body[..body.len() - 1]).is_none());
    }

    #[test]
    fn a_block_entity_update_moves_its_type_and_a_beds_goes_nowhere() {
        use azalea_registry::builtin::BlockEntityKind;
        assert!(!BlockEntityKind::is_valid_id(NO_BLOCK_ENTITY_TYPE as u32));
        assert!(BlockEntityKind::is_valid_id(
            NO_BLOCK_ENTITY_TYPE as u32 - 1
        ));

        let update = |kind: i32| {
            let mut body = 0x0123_4567_89AB_CDEFu64.to_be_bytes().to_vec();
            write_varint(&mut body, kind);
            body.extend_from_slice(&[10, 0]);
            body
        };
        let conduit = BlockEntityKind::Conduit.to_u32() as i32;
        for ids in [ids(), ids775()] {
            assert_eq!(
                block_entity_data(ids, &update(26)).as_deref(),
                Some(&update(conduit)[..])
            );
            assert_eq!(
                block_entity_data(ids, &update(25)).as_deref(),
                Some(&update(NO_BLOCK_ENTITY_TYPE)[..])
            );
        }
    }

    #[test]
    fn a_775_section_keeps_its_length_and_moves_its_states() {
        let mut one = vec![0x01, 0x00, 0x00, 0x02];
        one.extend_from_slice(&container_bytes(0, &[24_687], &[]));
        one.extend_from_slice(&empty_biomes());
        let out = sections(ids775(), &one).expect("a whole section");
        assert_eq!(
            out.len(),
            one.len(),
            "no fluid count added; both ids take three bytes"
        );
        let mut r = Reader::new(&out);
        assert_eq!(
            r.take(4),
            Some(&[0x01, 0x00, 0x00, 0x02][..]),
            "both counts as sent"
        );
        assert_eq!(r.u8(), Some(0));
        assert_eq!(r.varint(), Some(27_160));
    }

    #[test]
    fn chunk_block_entities_move_and_beds_are_left_out() {
        let entry = |kind: i32, tag: &[u8]| {
            let mut out = vec![0x12, 0x00, 0x40];
            write_varint(&mut out, kind);
            out.extend_from_slice(tag);
            out
        };
        let empty_compound = [10u8, 0];
        let no_tag = [0u8];
        let mut wire = Vec::new();
        write_varint(&mut wire, 3);
        wire.extend_from_slice(&entry(26, &empty_compound));
        wire.extend_from_slice(&entry(25, &empty_compound));
        wire.extend_from_slice(&entry(27, &no_tag));
        let light = [0xAA, 0xBB];
        wire.extend_from_slice(&light);

        for ids in [ids(), ids775()] {
            let mut r = Reader::new(&wire);
            let mut out = Vec::new();
            chunk_block_entities(ids, &mut r, &mut out).expect("a whole list");
            assert_eq!(r.rest(), light, "the light is not walked");

            let mut want = Vec::new();
            write_varint(&mut want, 2);
            want.extend_from_slice(&entry(25, &empty_compound));
            want.extend_from_slice(&entry(26, &no_tag));
            assert_eq!(out, want);
        }
    }

    #[test]
    fn an_entity_type_past_the_seam_moves() {
        let out = add_entity(ids775(), &add_entity_bytes(130, 77)).expect("a whole packet");
        assert_eq!(
            &*out,
            &add_entity_bytes(131, 77)[..],
            "the data is not a state here"
        );

        let falling = EntityKind::FallingBlock.to_u32() as i32;
        let out = add_entity(ids775(), &add_entity_bytes(falling, 24_687)).unwrap();
        assert_eq!(&*out, &add_entity_bytes(falling, 27_160)[..]);
    }

    #[test]
    fn a_kill_stat_moves_its_entity_type() {
        let mut body = Vec::new();
        write_varint(&mut body, 1);
        write_varint(&mut body, STAT_KILLED_BY);
        write_varint(&mut body, 130);
        write_varint(&mut body, 2);
        let out = award_stats(ids775(), &body).unwrap();
        let mut r = Reader::new(&out);
        assert_eq!(r.count(), Some(1));
        assert_eq!(
            (r.varint(), r.varint(), r.varint()),
            (Some(STAT_KILLED_BY), Some(131), Some(2))
        );
    }

    #[test]
    fn a_775_container_keeps_its_layout_and_moves_its_ids() {
        let container = DataComponentKind::Container.to_u32() as i32;
        let stack = |item: i32| {
            let mut body = Vec::new();
            for v in [1, 1, 1, 0, container, 2, 1, item, 3, 0, 0, 0] {
                write_varint(&mut body, v);
            }
            body
        };
        let wire = stack(26);
        let out = translated(|r, out| item_stack(ids775(), r, out), &wire);
        assert_eq!(out, stack(53));

        let mut cursor = std::io::Cursor::new(&out[..]);
        azalea_inventory::ItemStack::azalea_read(&mut cursor).expect("decodes as 26.2");
        assert_eq!(cursor.position() as usize, out.len());
    }

    #[test]
    fn a_775_slot_display_keeps_its_type_and_moves_its_ids() {
        let mut wire = Vec::new();
        for v in [SLOT_DISPLAY_ONLY_WITH_COMPONENT, SLOT_DISPLAY_ITEM, 26, 78] {
            write_varint(&mut wire, v);
        }
        let out = translated(|r, out| slot_display(ids775(), r, out), &wire);
        let mut want = Vec::new();
        for v in [SLOT_DISPLAY_ONLY_WITH_COMPONENT, SLOT_DISPLAY_ITEM, 53, 79] {
            write_varint(&mut want, v);
        }
        assert_eq!(out, want);
    }

    #[test]
    fn an_attribute_modifier_component_moves_its_attribute() {
        use azalea_inventory::components::{
            AttributeModifier, AttributeModifierDisplay, AttributeModifiers,
            AttributeModifiersEntry, EquipmentSlotGroup,
        };
        use azalea_registry::builtin::Attribute;

        let wire_value = AttributeModifiers {
            modifiers: vec![AttributeModifiersEntry {
                kind: Attribute::from_u32(0).unwrap(),
                modifier: AttributeModifier {
                    id: "minecraft:test".into(),
                    amount: 2.0,
                    operation: azalea_core::attribute_modifier_operation::AttributeModifierOperation::AddValue,
                },
                slot: EquipmentSlotGroup::Any,
                display: AttributeModifierDisplay::Default,
            }],
        };
        let kind = DataComponentKind::AttributeModifiers.to_u32() as i32;
        let mut body = Vec::new();
        for v in [1, 1, 1, 0, kind] {
            write_varint(&mut body, v);
        }
        wire_value.azalea_write(&mut body).unwrap();

        let out = translated(|r, out| item_stack(ids775(), r, out), &body);
        let mut cursor = std::io::Cursor::new(&out[..]);
        let stack = azalea_inventory::ItemStack::azalea_read(&mut cursor).expect("decodes as 26.2");
        assert_eq!(cursor.position() as usize, out.len());
        let azalea_inventory::ItemStack::Present(data) = stack else {
            panic!("a present stack");
        };
        let got = data
            .component_patch
            .get::<AttributeModifiers>()
            .expect("kept");
        assert_eq!(got.modifiers[0].kind, Attribute::Armor);
        assert_eq!(got.modifiers[0].modifier.amount, 2.0);
    }

    fn cube_data(entity: i32, size_at: u8) -> Box<[u8]> {
        let mut body = Vec::new();
        write_varint(&mut body, entity);
        body.extend_from_slice(&[0, 0, 0x01]);
        body.push(size_at);
        write_varint(&mut body, 1);
        write_varint(&mut body, 3);
        body.push(0xFF);
        framed(native_id(Direction::Clientbound, "set_entity_data"), &body)
    }

    #[test]
    fn a_slimes_size_moves_past_ageable_mobs_fields() {
        use azalea::connection::PacketTranslator as _;
        let mut t = Translator::for_protocol(775).expect("775 has a hop");
        let add_entity = native_id(Direction::Clientbound, "add_entity");
        let slime = EntityKind::Slime.to_u32() as i32;
        let creeper = EntityKind::Creeper.to_u32() as i32;

        let _ = t.inbound(
            ConnectionProtocol::Game,
            framed(add_entity, &add_entity_bytes(slime, 0)),
        );
        assert_eq!(
            &*t.inbound(ConnectionProtocol::Game, cube_data(11, 16)),
            &*cube_data(11, 18),
            "the size is where 26.2's Slime reads it"
        );
        assert_eq!(
            &*t.inbound(ConnectionProtocol::Game, cube_data(11, 18)),
            &*cube_data(11, 20),
            "anything past 16 moves too"
        );

        let mut creeper_spawn = add_entity_bytes(creeper, 0);
        creeper_spawn[0] = 12;
        let _ = t.inbound(ConnectionProtocol::Game, framed(add_entity, &creeper_spawn));
        assert_eq!(
            &*t.inbound(ConnectionProtocol::Game, cube_data(12, 16)),
            &*cube_data(12, 16)
        );

        let mut removal = Vec::new();
        write_varint(&mut removal, 1);
        write_varint(&mut removal, 11);
        let _ = t.inbound(
            ConnectionProtocol::Game,
            framed(
                native_id(Direction::Clientbound, "remove_entities"),
                &removal,
            ),
        );
        assert_eq!(
            &*t.inbound(ConnectionProtocol::Game, cube_data(11, 16)),
            &*cube_data(11, 16)
        );
    }

    #[test]
    fn a_short_metadata_frame_is_left_alone() {
        let runs = [Run {
            from: 16,
            offset: 2,
        }];
        assert!(shifted_metadata(&[0x0B, 16], &runs).is_none());
        let mut body = vec![0x0B, 16];
        write_varint(&mut body, 1);
        assert!(
            shifted_metadata(&body, &runs).is_none(),
            "an int with no value"
        );
    }

    fn byte_field(index: u8, value: u8) -> Vec<u8> {
        let mut body = Vec::new();
        write_varint(&mut body, 11);
        body.extend_from_slice(&[index, 0, value]);
        body.push(0xFF);
        body
    }

    #[test]
    fn a_774_sheeps_colour_moves_past_age_locked() {
        use azalea::connection::PacketTranslator as _;
        let wire = PacketTable::v1_21_11();
        let wire_id = |name| wire.id(Phase::Game, Direction::Clientbound, name).unwrap();
        let sheep_native = EntityKind::Sheep.to_u32() as i32;
        let sheep_774 = ids()
            .from_native
            .map(IdSpace::EntityType, sheep_native)
            .expect("774 has sheep");
        let mut t = Translator::for_protocol(774).expect("774 has a hop");

        let _ = t.inbound(
            ConnectionProtocol::Game,
            framed(wire_id("add_entity"), &add_entity_bytes(sheep_774, 0)),
        );
        let set_entity_data = native_id(Direction::Clientbound, "set_entity_data");
        assert_eq!(
            &*t.inbound(
                ConnectionProtocol::Game,
                framed(wire_id("set_entity_data"), &byte_field(17, 14))
            ),
            &*framed(set_entity_data, &byte_field(18, 14)),
            "red wool, where 26.2's Sheep reads it"
        );
        assert_eq!(
            &*t.inbound(
                ConnectionProtocol::Game,
                framed(wire_id("set_entity_data"), &byte_field(0, 1))
            ),
            &*framed(set_entity_data, &byte_field(0, 1)),
            "Entity's own flags do not move"
        );

        let zombie_774 = ids()
            .from_native
            .map(IdSpace::EntityType, EntityKind::Zombie.to_u32() as i32)
            .unwrap();
        let _ = t.inbound(
            ConnectionProtocol::Game,
            framed(wire_id("add_entity"), &add_entity_bytes(zombie_774, 0)),
        );
        assert_eq!(
            &*t.inbound(
                ConnectionProtocol::Game,
                framed(wire_id("set_entity_data"), &byte_field(17, 1))
            ),
            &*framed(set_entity_data, &byte_field(17, 1))
        );
    }

    #[test]
    fn a_774_slimes_size_moves_two() {
        use azalea::connection::PacketTranslator as _;
        let wire = PacketTable::v1_21_11();
        let wire_id = |name| wire.id(Phase::Game, Direction::Clientbound, name).unwrap();
        let slime_774 = ids()
            .from_native
            .map(IdSpace::EntityType, EntityKind::Slime.to_u32() as i32)
            .unwrap();
        let mut t = Translator::for_protocol(774).unwrap();
        let _ = t.inbound(
            ConnectionProtocol::Game,
            framed(wire_id("add_entity"), &add_entity_bytes(slime_774, 0)),
        );
        let mut body = Vec::new();
        write_varint(&mut body, 11);
        body.push(16);
        write_varint(&mut body, 1);
        write_varint(&mut body, 3);
        body.push(0xFF);
        assert_eq!(
            &*t.inbound(
                ConnectionProtocol::Game,
                framed(wire_id("set_entity_data"), &body)
            ),
            &*cube_data(11, 18)
        );
    }

    fn tags_body(registries: &[(&str, &str, Vec<i32>)]) -> Vec<u8> {
        let mut body = Vec::new();
        write_varint(&mut body, registries.len() as i32);
        for (registry, tag, entries) in registries {
            string(&mut body, registry);
            write_varint(&mut body, 1);
            string(&mut body, tag);
            write_varint(&mut body, entries.len() as i32);
            for id in entries {
                write_varint(&mut body, *id);
            }
        }
        body
    }

    #[test]
    fn tags_move_with_their_registries() {
        let wire = tags_body(&[
            ("minecraft:item", "minecraft:logs", vec![1, 26]),
            ("minecraft:worldgen/biome", "minecraft:is_ocean", vec![26]),
            (
                "minecraft:block_entity_type",
                "minecraft:beds",
                vec![25, 26],
            ),
        ]);
        let want = tags_body(&[
            ("minecraft:item", "minecraft:logs", vec![1, 53]),
            ("minecraft:worldgen/biome", "minecraft:is_ocean", vec![26]),
            ("minecraft:block_entity_type", "minecraft:beds", vec![25]),
        ]);
        assert_eq!(update_tags(ids775(), &wire).as_deref(), Some(&want[..]));
        assert!(update_tags(ids775(), &wire[..wire.len() - 1]).is_none());
    }

    #[test]
    fn configuration_tags_are_rewritten_in_place() {
        let id = PacketTable::native()
            .id(Phase::Configuration, Direction::Clientbound, "update_tags")
            .expect("26.2 has update_tags");
        let wire = tags_body(&[("minecraft:item", "minecraft:logs", vec![26])]);
        let want = tags_body(&[("minecraft:item", "minecraft:logs", vec![53])]);
        let out = hop775().inbound(ConnectionProtocol::Configuration, framed(id, &wire));
        assert_eq!(&*out, &framed(id, &want)[..]);
    }

    #[test]
    fn a_registry_name_finds_its_space() {
        assert_eq!(IdSpace::of_registry("item"), Some(IdSpace::Item));
        assert_eq!(
            IdSpace::of_registry("entity_type"),
            Some(IdSpace::EntityType)
        );
        assert_eq!(IdSpace::of_registry("worldgen/biome"), None);
        assert_eq!(IdSpace::of_registry("block_state"), None, "not a registry");
    }
}
