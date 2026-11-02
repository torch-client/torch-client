use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use azalea::block::{BlockState, BlockTrait};
use azalea_core::direction::Direction;
use azalea_core::position::BlockPos;
use azalea_protocol::packets::game::c_block_entity_data::ClientboundBlockEntityData;
use azalea_protocol::packets::game::c_block_event::ClientboundBlockEvent;
use azalea_protocol::packets::game::c_level_chunk_with_light::BlockEntity as PacketBlockEntity;
use azalea_registry::builtin::BlockEntityKind;
use simdnbt::owned::{Nbt, NbtCompound, NbtTag};

use super::sim::{self, Anim, TickCtx};
use crate::text::{Span, Style};
use crate::util::dye::{DYES, id_by_name};

#[derive(Debug)]
pub struct BlockStateInfo {
    pub state_id: u16,
    pub block: String,
    pub props: HashMap<String, String>,
    pub facing: Option<Direction>,
    pub rotation: u32,
    pub chest_half: ChestHalf,
    pub powered: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChestHalf {
    Single,
    Left,
    Right,
}

impl BlockStateInfo {
    pub fn new(state_id: u16, block: String, props: HashMap<String, String>) -> Self {
        let prop = |name: &str| props.get(name).map(String::as_str).unwrap_or("");
        let facing = crate::blocks::facing::from_name(prop("facing"));
        let rotation = prop("rotation").parse().unwrap_or(0);
        let chest_half = match prop("type") {
            "left" => ChestHalf::Left,
            "right" => ChestHalf::Right,
            _ => ChestHalf::Single,
        };
        let powered = prop("powered") == "true";
        BlockStateInfo {
            state_id,
            block,
            props,
            facing,
            rotation,
            chest_half,
            powered,
        }
    }

    pub fn prop(&self, name: &str) -> &str {
        self.props.get(name).map(String::as_str).unwrap_or("")
    }

    pub fn rotation_degrees(&self) -> f32 {
        self.rotation as f32 * (360.0 / 16.0)
    }
}

#[derive(Clone, Debug)]
pub struct BlockEntityInfo {
    pub kind: BlockEntityKind,
    pub state: Arc<BlockStateInfo>,
    pub data: Arc<BlockEntityData>,
    pub anim: Anim,
    pub partner: Option<[i32; 3]>,
    pub rev: u64,
}

impl PartialEq for BlockEntityInfo {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.state.state_id == other.state.state_id
            && (Arc::ptr_eq(&self.data, &other.data) || self.data == other.data)
            && self.anim == other.anim
            && self.partner == other.partner
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum BlockEntityData {
    #[default]
    None,
    Sign(SignData),
    Banner(super::banner::BannerData),
    #[cfg(feature = "skins")]
    Skull(SkullData),
    Pot(sim::pot::Decorations),
    Conduit(sim::conduit::Data),
    Spawner(sim::spawner::Data),
    Vault(sim::vault::Data),
    Gateway(sim::gateway::Data),
    Campfire(sim::items::Campfire),
    Shelf(sim::items::Shelf),
    Brushable(sim::items::Brushable),
    Piston(sim::piston::Moving),
    Bounds(sim::bounds::Data),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg(feature = "skins")]
pub struct SkullData {
    pub skin: Option<std::sync::Arc<str>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SignData {
    pub front: SignFace,
    pub back: SignFace,
    pub waxed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SignFace {
    pub lines: [Vec<Span>; 4],
    pub color: u32,
    pub glowing: bool,
}

impl Default for SignFace {
    fn default() -> SignFace {
        SignFace {
            lines: [const { Vec::new() }; 4],
            color: dye_text_color("black"),
            glowing: false,
        }
    }
}

fn dye_text_color(name: &str) -> u32 {
    let black = DYES.len() - 1;
    DYES[id_by_name(name).unwrap_or(black)].text_color
}

struct Entry {
    kind: BlockEntityKind,
    data: Arc<BlockEntityData>,
    anim: Anim,
    private: sim::Private,
    #[cfg(lua_mods)]
    raw: Option<Arc<Nbt>>,
}

impl Entry {
    fn new(kind: BlockEntityKind, data: Arc<BlockEntityData>) -> Entry {
        Entry {
            kind,
            anim: Anim::new(kind, &data),
            data,
            private: sim::Private::None,
            #[cfg(lua_mods)]
            raw: None,
        }
    }
}

#[cfg(lua_mods)]
pub fn raw_at(pos: [i32; 3]) -> Option<(String, Arc<Nbt>)> {
    let store = store();
    let entry = store.entries.get(&pos)?;
    Some((entry.kind.to_string(), entry.raw.clone()?))
}

#[derive(Default)]
struct Store {
    entries: HashMap<[i32; 3], Entry>,
    states: HashMap<u16, Arc<BlockStateInfo>>,
    kinds: HashMap<u16, Option<BlockEntityKind>>,
    published: Arc<HashMap<[i32; 3], BlockEntityInfo>>,
    next_rev: u64,
    game_time: u64,
}

static STORE: OnceLock<Mutex<Store>> = OnceLock::new();

fn store() -> MutexGuard<'static, Store> {
    match STORE.get_or_init(Default::default).lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn on_chunk(chunk_x: i32, chunk_z: i32, entities: &[PacketBlockEntity]) {
    let mut store = store();
    for entity in entities {
        let x = chunk_x * 16 + ((entity.packed_xz >> 4) & 15) as i32;
        let z = chunk_z * 16 + (entity.packed_xz & 15) as i32;
        let y = entity.y as i16 as i32;
        store.insert([x, y, z], entity.kind, &entity.data);
    }
}

pub fn on_block_entity_data(packet: &ClientboundBlockEntityData) {
    let pos = [packet.pos.x, packet.pos.y, packet.pos.z];
    store().insert(pos, packet.block_entity_type, &packet.tag);
}

pub fn on_block_event(packet: &ClientboundBlockEvent, world: &azalea_world::World) {
    let pos = [packet.pos.x, packet.pos.y, packet.pos.z];
    let mut store = store();
    let game_time = store.game_time;
    if let Some(entry) = store.entries.get_mut(&pos) {
        entry
            .anim
            .event(packet.action_id, packet.action_parameter, game_time);
        return;
    }
    for (pos, moving) in
        sim::piston::block_event(world, pos, packet.action_id, packet.action_parameter)
    {
        let data = Arc::new(BlockEntityData::Piston(moving));
        store
            .entries
            .insert(pos, Entry::new(BlockEntityKind::Piston, data));
    }
}

pub fn on_block_change(pos: BlockPos, new: BlockState) {
    let key = [pos.x, pos.y, pos.z];
    let mut store = store();
    match store.kind_of(new) {
        Some(kind) => match store.entries.get_mut(&key) {
            Some(entry) if entry.kind == kind => {}
            _ => {
                store
                    .entries
                    .insert(key, Entry::new(kind, Arc::new(BlockEntityData::None)));
            }
        },
        None => {
            store.entries.remove(&key);
        }
    }
}

pub fn on_forget_chunk(chunk_x: i32, chunk_z: i32) {
    let mut store = store();
    store
        .entries
        .retain(|pos, _| pos[0] >> 4 != chunk_x || pos[2] >> 4 != chunk_z);
}

pub fn reset() {
    let mut store = store();
    store.entries.clear();
    store.published = Default::default();
}

impl Store {
    fn kind_of(&mut self, state: BlockState) -> Option<BlockEntityKind> {
        *self
            .kinds
            .entry(state.id())
            .or_insert_with(|| kind_for_block(Box::<dyn BlockTrait>::from(state).id()))
    }

    fn insert(&mut self, pos: [i32; 3], kind: BlockEntityKind, tag: &Nbt) {
        self.put(pos, kind, decode(kind, tag));
        #[cfg(lua_mods)]
        if let Some(entry) = self.entries.get_mut(&pos) {
            entry.raw = Some(Arc::new(tag.clone()));
        }
    }

    fn put(&mut self, pos: [i32; 3], kind: BlockEntityKind, data: BlockEntityData) {
        match self.entries.get_mut(&pos) {
            Some(entry) if entry.kind == kind => {
                if *entry.data != data {
                    entry.anim.on_data(&data);
                    entry.data = Arc::new(data);
                }
            }
            _ => {
                self.entries.insert(pos, Entry::new(kind, Arc::new(data)));
            }
        }
    }
}

pub(crate) fn kind_for_block(id: &str) -> Option<BlockEntityKind> {
    let kind = match id {
        "skeleton_skull"
        | "skeleton_wall_skull"
        | "wither_skeleton_skull"
        | "wither_skeleton_wall_skull"
        | "player_head"
        | "player_wall_head"
        | "zombie_head"
        | "zombie_wall_head"
        | "creeper_head"
        | "creeper_wall_head"
        | "dragon_head"
        | "dragon_wall_head"
        | "piglin_head"
        | "piglin_wall_head" => BlockEntityKind::Skull,
        "ender_chest" => BlockEntityKind::EnderChest,
        "trapped_chest" => BlockEntityKind::TrappedChest,
        "chest" => BlockEntityKind::Chest,
        "lectern" => BlockEntityKind::Lectern,
        "spawner" => BlockEntityKind::MobSpawner,
        "trial_spawner" => BlockEntityKind::TrialSpawner,
        "vault" => BlockEntityKind::Vault,
        "moving_piston" => BlockEntityKind::Piston,
        "enchanting_table" => BlockEntityKind::EnchantingTable,
        "end_portal" => BlockEntityKind::EndPortal,
        "end_gateway" => BlockEntityKind::EndGateway,
        "beacon" => BlockEntityKind::Beacon,
        "structure_block" => BlockEntityKind::StructureBlock,
        "test_instance_block" => BlockEntityKind::TestInstanceBlock,
        "conduit" => BlockEntityKind::Conduit,
        "bell" => BlockEntityKind::Bell,
        "decorated_pot" => BlockEntityKind::DecoratedPot,
        "suspicious_sand" | "suspicious_gravel" => BlockEntityKind::BrushableBlock,
        _ if id.ends_with("copper_chest") => BlockEntityKind::Chest,
        _ if id.ends_with("_hanging_sign") => BlockEntityKind::HangingSign,
        _ if id.ends_with("_sign") => BlockEntityKind::Sign,
        _ if id.ends_with("_bed") => BlockEntityKind::Bed,
        _ if id.ends_with("_banner") => BlockEntityKind::Banner,
        _ if id.ends_with("shulker_box") => BlockEntityKind::ShulkerBox,
        _ if id.ends_with("campfire") => BlockEntityKind::Campfire,
        _ if id.ends_with("_shelf") => BlockEntityKind::Shelf,
        _ if id.ends_with("copper_golem_statue") => BlockEntityKind::CopperGolemStatue,
        _ => return None,
    };
    Some(kind)
}

fn decode(kind: BlockEntityKind, tag: &Nbt) -> BlockEntityData {
    match kind {
        BlockEntityKind::Sign | BlockEntityKind::HangingSign => {
            BlockEntityData::Sign(sign_data(tag))
        }
        BlockEntityKind::Banner => BlockEntityData::Banner(super::banner::BannerData {
            layers: banner_layers(tag),
        }),
        #[cfg(feature = "skins")]
        BlockEntityKind::Skull => BlockEntityData::Skull(skull_data(tag)),
        BlockEntityKind::DecoratedPot => BlockEntityData::Pot(sim::pot::decode(tag)),
        BlockEntityKind::Conduit => BlockEntityData::Conduit(sim::conduit::decode(tag)),
        BlockEntityKind::MobSpawner | BlockEntityKind::TrialSpawner => {
            BlockEntityData::Spawner(sim::spawner::decode(kind, tag))
        }
        BlockEntityKind::Vault => BlockEntityData::Vault(sim::vault::decode(tag)),
        BlockEntityKind::EndGateway => BlockEntityData::Gateway(sim::gateway::decode(tag)),
        BlockEntityKind::Campfire => BlockEntityData::Campfire(sim::items::campfire(tag)),
        BlockEntityKind::Shelf => BlockEntityData::Shelf(sim::items::shelf(tag)),
        BlockEntityKind::BrushableBlock => BlockEntityData::Brushable(sim::items::brushable(tag)),
        BlockEntityKind::StructureBlock | BlockEntityKind::TestInstanceBlock => {
            BlockEntityData::Bounds(sim::bounds::decode(kind, tag))
        }
        _ => BlockEntityData::None,
    }
}

#[cfg(feature = "skins")]
fn skull_data(tag: &Nbt) -> SkullData {
    let Some(profile) = tag.compound("profile") else {
        return SkullData::default();
    };
    let Some(properties) = profile.get("properties") else {
        return SkullData::default();
    };

    let packed = match properties {
        NbtTag::Compound(map) => map
            .list("textures")
            .and_then(|list| list.strings())
            .and_then(<[_]>::first)
            .map(|value| value.as_str().to_string()),
        NbtTag::List(list) => list.compounds().and_then(|entries| {
            entries
                .iter()
                .find(|entry| {
                    entry
                        .string("name")
                        .is_some_and(|name| name.to_string() == "textures")
                })
                .and_then(|entry| entry.string("value"))
                .map(|value| value.to_string())
        }),
        _ => None,
    };

    let skin = packed
        .as_deref()
        .map(crate::client::skins::parse_textures)
        .and_then(|refs| refs.body);
    if let Some(url) = &skin {
        crate::client::skins::request(url, crate::client::skins::Kind::Body);
    }
    SkullData { skin }
}

fn sign_data(tag: &Nbt) -> SignData {
    SignData {
        front: tag
            .compound("front_text")
            .map(sign_face)
            .unwrap_or_default(),
        back: tag.compound("back_text").map(sign_face).unwrap_or_default(),
        waxed: tag.byte("is_waxed").unwrap_or(0) != 0,
    }
}

fn sign_face(tag: &NbtCompound) -> SignFace {
    let mut face = SignFace {
        color: tag
            .string("color")
            .map(|s| dye_text_color(&s.to_string()))
            .unwrap_or_else(|| dye_text_color("black")),
        glowing: tag.byte("has_glowing_text").unwrap_or(0) != 0,
        ..SignFace::default()
    };
    if let Some(messages) = tag.list("messages") {
        for (i, message) in messages.as_nbt_tags().into_iter().take(4).enumerate() {
            face.lines[i] = component_spans(&message);
        }
    }
    face
}

fn banner_layers(tag: &Nbt) -> Vec<super::banner::BannerLayer> {
    let Some(patterns) = tag.list("patterns") else {
        return Vec::new();
    };
    patterns
        .as_nbt_tags()
        .into_iter()
        .filter_map(|tag| {
            let entry = tag.compound()?;
            let asset = match entry.string("pattern") {
                Some(id) => {
                    let id = id.to_string();
                    super::banner::asset_for(id.strip_prefix("minecraft:").unwrap_or(&id))
                }
                None => {
                    let inline = entry.compound("pattern")?.string("asset_id")?.to_string();
                    Box::from(inline.strip_prefix("minecraft:").unwrap_or(&inline))
                }
            };
            Some(super::banner::BannerLayer {
                asset,
                color: id_by_name(&entry.string("color")?.to_string()).unwrap_or(0) as u8,
            })
        })
        .take(super::banner::MAX_PATTERNS)
        .collect()
}

fn component_spans(tag: &NbtTag) -> Vec<Span> {
    let Some(text) = component(tag) else {
        return Vec::new();
    };
    let mut spans = crate::client::chat_text::to_spans(&text);
    for span in &mut spans {
        if span.style.color == Style::default().color {
            span.style.color = INHERIT_COLOR;
        }
    }
    spans
}

pub const INHERIT_COLOR: u32 = 0x0100_0000;

fn component(tag: &NbtTag) -> Option<azalea_chat::FormattedText> {
    crate::client::chat_text::from_nbt_tag(tag)
}

pub fn publish(
    world: &azalea_world::World,
    game_time: u64,
    players: &[[f64; 3]],
) -> Option<Arc<HashMap<[i32; 3], BlockEntityInfo>>> {
    let mut store = store();
    let store = &mut *store;
    store.game_time = game_time;

    let mut snapshot: HashMap<[i32; 3], BlockEntityInfo> =
        HashMap::with_capacity(store.entries.len());
    let mut changed = false;
    let states = &mut store.states;
    let kinds = &mut store.kinds;
    let published = &store.published;
    let next_rev = &mut store.next_rev;
    store.entries.retain(|pos, entry| {
        let block_pos = BlockPos::new(pos[0], pos[1], pos[2]);
        let Some(state) = world.get_block_state(block_pos) else {
            changed = true;
            return false;
        };
        if kinds
            .entry(state.id())
            .or_insert_with(|| kind_for_block(Box::<dyn BlockTrait>::from(state).id()))
            .is_none()
        {
            changed = true;
            return false;
        }
        let resolved_state = resolve(states, state);
        entry.anim.tick(
            &TickCtx {
                world,
                pos: *pos,
                state: &resolved_state,
                data: &entry.data,
                game_time,
                players,
            },
            &mut entry.private,
        );
        let info = BlockEntityInfo {
            kind: entry.kind,
            state: resolved_state,
            data: entry.data.clone(),
            anim: entry.anim.clone(),
            partner: None,
            rev: 0,
        };
        snapshot.insert(*pos, info);
        true
    });

    let halves: Vec<[i32; 3]> = snapshot
        .iter()
        .filter(|(_, info)| info.state.chest_half != ChestHalf::Single)
        .map(|(pos, _)| *pos)
        .collect();
    for pos in halves {
        let Some(info) = snapshot.get(&pos) else {
            continue;
        };
        let partner = chest_partner(pos, info, &snapshot);
        if let Some(info) = snapshot.get_mut(&pos) {
            info.partner = partner;
        }
    }

    for (pos, info) in snapshot.iter_mut() {
        match published.get(pos) {
            Some(old) if *old == *info => info.rev = old.rev,
            _ => {
                *next_rev = next_rev.wrapping_add(1);
                info.rev = *next_rev;
                changed = true;
            }
        }
    }
    changed |= snapshot.len() != published.len();

    if !changed {
        return None;
    }
    let snapshot = Arc::new(snapshot);
    store.published = snapshot.clone();
    Some(snapshot)
}

fn resolve(
    cache: &mut HashMap<u16, Arc<BlockStateInfo>>,
    state: BlockState,
) -> Arc<BlockStateInfo> {
    cache
        .entry(state.id())
        .or_insert_with(|| {
            let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
            Arc::new(BlockStateInfo::new(
                state.id(),
                block.id().to_string(),
                block
                    .property_map()
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ))
        })
        .clone()
}

fn chest_partner(
    pos: [i32; 3],
    info: &BlockEntityInfo,
    all: &HashMap<[i32; 3], BlockEntityInfo>,
) -> Option<[i32; 3]> {
    let left = match info.state.chest_half {
        ChestHalf::Left => true,
        ChestHalf::Right => false,
        ChestHalf::Single => return None,
    };
    let facing = info.state.facing?;
    let toward = if left {
        crate::blocks::facing::clockwise(facing)
    } else {
        crate::blocks::facing::counter_clockwise(facing)
    };
    if matches!(toward, Direction::Up | Direction::Down) {
        return None;
    }
    let step = toward.normal();
    let neighbour = [pos[0] + step.x, pos[1], pos[2] + step.z];
    let other = all.get(&neighbour)?;
    let opposite = if left {
        ChestHalf::Right
    } else {
        ChestHalf::Left
    };
    (other.state.block == info.state.block
        && other.state.chest_half == opposite
        && other.state.facing == Some(facing))
    .then_some(neighbour)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockentities::sim::test_nbt::{compound, string};

    #[test]
    fn the_two_halves_of_a_double_chest_find_each_other() {
        let left = Arc::new(BlockStateInfo::new(
            1,
            "chest".to_string(),
            HashMap::from([
                ("type".to_string(), "left".to_string()),
                ("facing".to_string(), "south".to_string()),
            ]),
        ));
        let right = Arc::new(BlockStateInfo::new(
            2,
            "chest".to_string(),
            HashMap::from([
                ("type".to_string(), "right".to_string()),
                ("facing".to_string(), "south".to_string()),
            ]),
        ));
        let info = |state: Arc<BlockStateInfo>| BlockEntityInfo {
            kind: BlockEntityKind::Chest,
            state,
            data: Arc::new(BlockEntityData::None),
            anim: Anim::None,
            partner: None,
            rev: 0,
        };
        let all = HashMap::from([([0, 64, 0], info(left)), ([-1, 64, 0], info(right))]);
        assert_eq!(
            chest_partner([0, 64, 0], &all[&[0, 64, 0]], &all),
            Some([-1, 64, 0])
        );
        assert_eq!(
            chest_partner([-1, 64, 0], &all[&[-1, 64, 0]], &all),
            Some([0, 64, 0])
        );
    }

    #[test]
    fn a_single_chest_stands_alone() {
        let single = Arc::new(BlockStateInfo::new(
            3,
            "chest".to_string(),
            HashMap::from([
                ("type".to_string(), "single".to_string()),
                ("facing".to_string(), "north".to_string()),
            ]),
        ));
        let info = BlockEntityInfo {
            kind: BlockEntityKind::Chest,
            state: single,
            data: Arc::new(BlockEntityData::None),
            anim: Anim::None,
            partner: None,
            rev: 0,
        };
        let all = HashMap::from([([0, 64, 0], info.clone())]);
        assert_eq!(chest_partner([0, 64, 0], &info, &all), None);
    }

    #[test]
    fn dye_colours_match_the_vanilla_table() {
        assert_eq!(dye_text_color("black"), 0);
        assert_eq!(dye_text_color("white"), 0xFF_FFFF);
        assert_eq!(dye_text_color("blue"), 0x0000_FF);
        assert_eq!(dye_text_color("not a colour"), 0);
    }

    #[test]
    fn a_plain_sign_line_decodes_to_one_span() {
        let tag = string("hello");
        let spans = component_spans(&tag);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "hello");
        assert_eq!(spans[0].style.color, INHERIT_COLOR);
    }

    #[test]
    fn a_coloured_sign_line_keeps_its_colour() {
        let tag = NbtTag::Compound(compound(vec![
            ("text", string("hi")),
            ("color", string("red")),
        ]));
        let spans = component_spans(&tag);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].style.color, 0xFF_5555);
    }
}
