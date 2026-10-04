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

use crate::text::{Span, Style};

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
}

#[derive(Clone, Debug)]
pub struct BlockEntityInfo {
    pub kind: BlockEntityKind,
    pub state: Arc<BlockStateInfo>,
    pub data: Arc<BlockEntityData>,
    pub open: f32,
    pub open_prev: f32,
    pub partner: Option<[i32; 3]>,
    pub animation_tick: u32,
    pub animating: bool,
    pub rev: u64,
}

impl PartialEq for BlockEntityInfo {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.state.state_id == other.state.state_id
            && (Arc::ptr_eq(&self.data, &other.data) || self.data == other.data)
            && self.open == other.open
            && self.open_prev == other.open_prev
            && self.partner == other.partner
            && self.animation_tick == other.animation_tick
            && self.animating == other.animating
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlockEntityData {
    #[default]
    None,
    Sign(SignData),
    Banner(super::banner::BannerData),
    #[cfg(feature = "skins")]
    Skull(SkullData),
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

pub fn dye_text_color(name: &str) -> u32 {
    match name {
        "white" => 16777215,
        "orange" => 16738335,
        "magenta" => 16711935,
        "light_blue" => 10141901,
        "yellow" => 16776960,
        "lime" => 12582656,
        "pink" => 16738740,
        "gray" => 8421504,
        "light_gray" => 13882323,
        "cyan" => 65535,
        "purple" => 10494192,
        "blue" => 255,
        "brown" => 9127187,
        "green" => 65280,
        "red" => 16711680,
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct LidController {
    should_be_open: bool,
    openness: f32,
    o_openness: f32,
}

impl LidController {
    fn tick(&mut self) {
        self.o_openness = self.openness;
        if !self.should_be_open && self.openness > 0.0 {
            self.openness = (self.openness - 0.1).max(0.0);
        } else if self.should_be_open && self.openness < 1.0 {
            self.openness = (self.openness + 0.1).min(1.0);
        }
    }
}

struct Entry {
    kind: BlockEntityKind,
    data: Arc<BlockEntityData>,
    lid: LidController,
    animation_tick: u32,
    animating: bool,
}

#[derive(Default)]
struct Store {
    entries: HashMap<[i32; 3], Entry>,
    states: HashMap<u16, Arc<BlockStateInfo>>,
    kinds: HashMap<u16, Option<BlockEntityKind>>,
    published: Arc<HashMap<[i32; 3], BlockEntityInfo>>,
    next_rev: u64,
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

pub fn on_block_event(packet: &ClientboundBlockEvent) {
    if packet.action_id != 1 {
        return;
    }
    let pos = [packet.pos.x, packet.pos.y, packet.pos.z];
    let mut store = store();
    if let Some(entry) = store.entries.get_mut(&pos) {
        entry.lid.should_be_open = packet.action_parameter > 0;
    }
}

pub fn on_block_change(pos: BlockPos, new: BlockState) {
    let key = [pos.x, pos.y, pos.z];
    let mut store = store();
    match store.kind_of(new) {
        Some(kind) => match store.entries.get_mut(&key) {
            Some(entry) if entry.kind == kind => {}
            _ => {
                store.entries.insert(
                    key,
                    Entry {
                        kind,
                        data: Arc::new(BlockEntityData::None),
                        lid: LidController::default(),
                        animation_tick: 0,
                        animating: false,
                    },
                );
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
        let data = Arc::new(decode(kind, tag));
        match self.entries.get_mut(&pos) {
            Some(entry) if entry.kind == kind => {
                if *entry.data != *data {
                    entry.data = data;
                }
            }
            _ => {
                self.entries.insert(
                    pos,
                    Entry {
                        kind,
                        data,
                        lid: LidController::default(),
                        animation_tick: 0,
                        animating: false,
                    },
                );
            }
        }
    }
}

pub(crate) fn kind_for_block(id: &str) -> Option<BlockEntityKind> {
    if matches!(
        id,
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
            | "piglin_wall_head"
    ) {
        return Some(BlockEntityKind::Skull);
    }
    if id == "ender_chest" {
        return Some(BlockEntityKind::EnderChest);
    }
    if id == "trapped_chest" {
        return Some(BlockEntityKind::TrappedChest);
    }
    if id == "chest" || id.ends_with("copper_chest") {
        return Some(BlockEntityKind::Chest);
    }
    if id.ends_with("_hanging_sign") {
        return Some(BlockEntityKind::HangingSign);
    }
    if id.ends_with("_sign") {
        return Some(BlockEntityKind::Sign);
    }
    if id.ends_with("_bed") {
        return Some(BlockEntityKind::Bed);
    }
    if id == "lectern" {
        return Some(BlockEntityKind::Lectern);
    }
    if id.ends_with("_banner") {
        return Some(BlockEntityKind::Banner);
    }
    None
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
                color: super::banner::color_by_name(&entry.string("color")?.to_string()),
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

pub fn publish(world: &azalea_world::World) -> Option<Arc<HashMap<[i32; 3], BlockEntityInfo>>> {
    let mut store = store();
    let store = &mut *store;

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
        entry.lid.tick();
        let resolved_state = resolve(states, state);
        if entry.kind == BlockEntityKind::Skull {
            entry.animating = resolved_state.powered;
            if entry.animating {
                entry.animation_tick = entry.animation_tick.wrapping_add(1);
            }
        }
        let info = BlockEntityInfo {
            kind: entry.kind,
            state: resolved_state,
            data: entry.data.clone(),
            open: entry.lid.openness,
            open_prev: entry.lid.o_openness,
            partner: None,
            animation_tick: entry.animation_tick,
            animating: entry.animating,
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
    use simdnbt::Mutf8String;

    use super::*;

    #[test]
    fn the_lid_takes_ten_ticks_to_open() {
        let mut lid = LidController {
            should_be_open: true,
            ..Default::default()
        };
        for _ in 0..10 {
            lid.tick();
        }
        assert!((lid.openness - 1.0).abs() < 1e-6);
        lid.tick();
        assert_eq!(lid.openness, 1.0);
        lid.should_be_open = false;
        for _ in 0..10 {
            lid.tick();
        }
        assert_eq!(lid.openness, 0.0);
    }

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
            open: 0.0,
            open_prev: 0.0,
            partner: None,
            animation_tick: 0,
            animating: false,
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
            open: 0.0,
            open_prev: 0.0,
            partner: None,
            animation_tick: 0,
            animating: false,
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
        let tag = NbtTag::String(Mutf8String::from("hello"));
        let spans = component_spans(&tag);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "hello");
        assert_eq!(spans[0].style.color, INHERIT_COLOR);
    }

    #[test]
    fn a_coloured_sign_line_keeps_its_colour() {
        let tag = NbtTag::Compound(NbtCompound::from_values(vec![
            (
                Mutf8String::from("text"),
                NbtTag::String(Mutf8String::from("hi")),
            ),
            (
                Mutf8String::from("color"),
                NbtTag::String(Mutf8String::from("red")),
            ),
        ]));
        let spans = component_spans(&tag);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].style.color, 0xFF_5555);
    }
}
