use std::sync::atomic::{AtomicU8, Ordering};

use bevy::math::Mat4;

use super::icons::{Dispatch, parse_transformation};
use super::json::Json;
use super::model::strip_namespace;
use crate::util::mth::positive_modulo;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ItemUse {
    pub using: bool,
    pub use_ticks: f32,
    pub charge: ChargeType,
    pub trim: u8,
    pub has_lodestone: bool,
    pub lodestone: Option<[i32; 3]>,
    pub bundle_selected: bool,
    pub broken: bool,
    pub fishing_cast: bool,
    pub use_max: i32,
    pub quick_charge: u8,
    pub block_state: u8,
    pub owner: Option<Owner>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Owner {
    pub pos: [f64; 3],
    pub yaw: f32,
    pub local: bool,
    pub seed: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChargeType {
    #[default]
    None,
    Arrow,
    Rocket,
}

impl ChargeType {
    fn name(self) -> &'static str {
        match self {
            ChargeType::None => "none",
            ChargeType::Arrow => "arrow",
            ChargeType::Rocket => "rocket",
        }
    }
}

impl ItemUse {
    pub const NONE: ItemUse = ItemUse {
        using: false,
        use_ticks: 0.0,
        charge: ChargeType::None,
        trim: 0,
        has_lodestone: false,
        lodestone: None,
        bundle_selected: false,
        broken: false,
        fishing_cast: false,
        use_max: 0,
        quick_charge: 0,
        block_state: 0,
        owner: None,
    };

    fn use_duration(&self) -> f32 {
        self.use_ticks.floor()
    }

    fn use_remaining(&self) -> Option<f32> {
        (self.use_max > 0).then(|| self.use_max as f32 - self.use_duration())
    }
}

#[rustfmt::skip]
pub const BLOCK_STATE_VALUES: [(&str, &str); 20] = [
    ("level", "0"), ("level", "1"), ("level", "2"), ("level", "3"), ("level", "4"),
    ("level", "5"), ("level", "6"), ("level", "7"), ("level", "8"), ("level", "9"),
    ("level", "10"), ("level", "11"), ("level", "12"), ("level", "13"), ("level", "14"),
    ("level", "15"), ("mode", "start"), ("mode", "log"), ("mode", "fail"), ("mode", "accept"),
];

pub fn block_state_index(property: &str, value: &str) -> u8 {
    BLOCK_STATE_VALUES
        .iter()
        .position(|&(p, v)| p == property && v == value)
        .map_or(0, |i| i as u8 + 1)
}

pub fn block_state_of(data: &azalea_inventory::ItemStackData) -> u8 {
    super::component::<azalea_inventory::components::BlockState>(data).map_or(0, |c| {
        c.properties
            .iter()
            .map(|(p, v)| block_state_index(p, v))
            .find(|&i| i != 0)
            .unwrap_or(0)
    })
}

pub fn block_state_of_codec(state: &simdnbt::owned::NbtCompound) -> u8 {
    state
        .iter()
        .filter_map(|(p, v)| Some(block_state_index(&p.to_str(), &v.string()?.to_str())))
        .find(|&i| i != 0)
        .unwrap_or(0)
}

pub const TRIM_MATERIALS: [&str; 11] = [
    "quartz",
    "iron",
    "netherite",
    "redstone",
    "copper",
    "gold",
    "emerald",
    "diamond",
    "lapis",
    "amethyst",
    "resin",
];

pub fn trim_index(path: &str) -> u8 {
    TRIM_MATERIALS
        .iter()
        .position(|m| *m == path)
        .map_or(0, |i| i as u8 + 1)
}

static TRIM_BY_ID: [AtomicU8; 64] = [const { AtomicU8::new(0) }; 64];

pub fn refresh_trims(registries: &azalea_core::registry_holder::RegistryHolder) {
    static KEY: std::sync::LazyLock<azalea::Identifier> =
        std::sync::LazyLock::new(|| azalea::Identifier::from("trim_material"));
    let names = registries.extra.get(&*KEY).map(|registry| &registry.map);
    for (id, slot) in TRIM_BY_ID.iter().enumerate() {
        let trim = names
            .and_then(|names| names.get_index(id))
            .filter(|(name, _)| name.namespace() == "minecraft")
            .map_or(0, |(name, _)| trim_index(name.path()));
        slot.store(trim, Ordering::Relaxed);
    }
}

pub fn trim_of(data: &azalea_inventory::ItemStackData) -> u8 {
    use azalea_registry::{DataRegistry, Holder};
    match super::component::<azalea_inventory::components::Trim>(data).as_deref() {
        Some(azalea_inventory::components::Trim {
            material: Holder::Reference(material),
            ..
        }) => TRIM_BY_ID
            .get(material.protocol_id() as usize)
            .map_or(0, |slot| slot.load(Ordering::Relaxed)),
        _ => 0,
    }
}

pub fn lodestone_of(data: &azalea_inventory::ItemStackData) -> (bool, Option<[i32; 3]>) {
    let Some(tracker) = super::component::<azalea_inventory::components::LodestoneTracker>(data)
    else {
        return (false, None);
    };
    let target = tracker
        .target
        .as_ref()
        .filter(|at| crate::renderer::dimension::is_current_level(&at.dimension))
        .map(|at| [at.pos.x, at.pos.y, at.pos.z]);
    (true, target)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ItemWorld {
    pub spawn: Option<[i32; 3]>,
    pub death: Option<[i32; 3]>,
    pub level: &'static str,
    pub sun: f32,
    pub random: f32,
    pub needle: f32,
    pub spin: f32,
    pub daytime: f32,
    pub clock: f32,
    pub moon: f32,
    pub moon_wobble: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Wobbler {
    rotation: f32,
    delta: f32,
}

impl Wobbler {
    fn update(&mut self, target: f32, factor: f32) -> f32 {
        let step = positive_modulo(target - self.rotation + 0.5, 1.0) - 0.5;
        self.delta += step * 0.1;
        self.delta *= factor;
        self.rotation = positive_modulo(self.rotation + self.delta, 1.0);
        self.rotation
    }
}

pub struct Needles {
    tick: Option<u64>,
    random: crate::util::javarandom::JavaRandom,
    needle: Wobbler,
    spin: Wobbler,
    daytime: Wobbler,
    clock: Wobbler,
    moon: Wobbler,
    world: ItemWorld,
}

impl Default for Needles {
    fn default() -> Self {
        Needles {
            tick: None,
            random: crate::util::javarandom::JavaRandom::new(0),
            needle: Wobbler::default(),
            spin: Wobbler::default(),
            daytime: Wobbler::default(),
            clock: Wobbler::default(),
            moon: Wobbler::default(),
            world: ItemWorld::default(),
        }
    }
}

impl Needles {
    #[allow(clippy::too_many_arguments, reason = "one parameter per level input")]
    pub fn step(
        &mut self,
        game_time: u64,
        yaw: f32,
        sun_angle: f32,
        moon_phase: usize,
        spawn: Option<[i32; 3]>,
        death: Option<[i32; 3]>,
        level: &'static str,
    ) -> ItemWorld {
        let sun = positive_modulo(sun_angle / std::f32::consts::TAU, 1.0);
        let moon = moon_phase as f32 / MOON_PHASE_COUNT;
        if self.tick != Some(game_time) {
            self.tick = Some(game_time);
            let random = self.random.next_f32();
            let clock_random = self.random.next_f32();
            let yaw01 = positive_modulo(yaw / 360.0, 1.0);
            self.world.random = random;
            self.world.needle = self.needle.update(0.5 - (yaw01 - 0.25), 0.8);
            self.world.spin = self.spin.update(random, 0.8);
            self.world.daytime = self.daytime.update(sun, 0.9);
            self.world.clock = self.clock.update(clock_random, 0.9);
            self.world.moon_wobble = self.moon.update(moon, 0.9);
        }
        self.world.spawn = spawn;
        self.world.death = death;
        self.world.level = level;
        self.world.sun = sun;
        self.world.moon = moon;
        self.world
    }
}

const MOON_PHASE_COUNT: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum CondProp {
    UsingItem,
    HasLodestone,
    HasSelectedItem,
    Broken,
    FishingRodCast,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SelectProp {
    DisplayContext,
    ChargeType,
    TrimMaterial,
    ContextDimension,
    BlockState { property: &'static str },
    LocalTime(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    None,
    Lodestone,
    Spawn,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TimeSource {
    Daytime,
    Random,
    MoonPhase,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RangeProp {
    UseDuration { remaining: bool },
    UseCycle { period: f32 },
    CrossbowPull,
    Compass { target: Target, wobble: bool },
    Time { source: TimeSource, wobble: bool },
}

fn property(node: &Json) -> &str {
    node.get("property")
        .and_then(Json::as_str)
        .map(strip_namespace)
        .unwrap_or("")
}

fn wobble(node: &Json) -> bool {
    node.get("wobble").and_then(Json::as_bool).unwrap_or(true)
}

impl CondProp {
    fn of(node: &Json) -> Option<CondProp> {
        match property(node) {
            "using_item" => Some(CondProp::UsingItem),
            "bundle/has_selected_item" => Some(CondProp::HasSelectedItem),
            "broken" => Some(CondProp::Broken),
            "fishing_rod/cast" => Some(CondProp::FishingRodCast),
            "has_component"
                if node
                    .get("component")
                    .and_then(Json::as_str)
                    .map(strip_namespace)
                    == Some("lodestone_tracker") =>
            {
                Some(CondProp::HasLodestone)
            }
            _ => None,
        }
    }
}

impl SelectProp {
    fn of(node: &Json) -> Option<SelectProp> {
        match property(node) {
            "display_context" => Some(SelectProp::DisplayContext),
            "charge_type" => Some(SelectProp::ChargeType),
            "trim_material" => Some(SelectProp::TrimMaterial),
            "context_dimension" => Some(SelectProp::ContextDimension),
            "block_state" => {
                let name = node.get("block_state_property").and_then(Json::as_str)?;
                let &(property, _) = BLOCK_STATE_VALUES.iter().find(|(p, _)| *p == name)?;
                Some(SelectProp::BlockState { property })
            }
            "local_time" => {
                let field = |key: &str| node.get(key).and_then(Json::as_str);
                let index = super::local_time::intern(
                    field("pattern")?,
                    field("locale").unwrap_or(""),
                    field("time_zone"),
                )?;
                Some(SelectProp::LocalTime(index))
            }
            _ => None,
        }
    }
}

impl RangeProp {
    fn of(node: &Json) -> Option<RangeProp> {
        let field = |key: &str| node.get(key).and_then(Json::as_str).unwrap_or("");
        Some(match property(node) {
            "use_duration" => RangeProp::UseDuration {
                remaining: node
                    .get("remaining")
                    .and_then(Json::as_bool)
                    .unwrap_or(false),
            },
            "use_cycle" => RangeProp::UseCycle {
                period: match node.get("period") {
                    None => 1.0,
                    Some(period) => period.as_f32().filter(|p| *p > 0.0)?,
                },
            },
            "crossbow/pull" => RangeProp::CrossbowPull,
            "compass" => RangeProp::Compass {
                target: match field("target") {
                    "none" => Target::None,
                    "lodestone" => Target::Lodestone,
                    "spawn" => Target::Spawn,
                    "recovery" => Target::Recovery,
                    _ => return None,
                },
                wobble: wobble(node),
            },
            "time" => RangeProp::Time {
                source: match field("source") {
                    "daytime" => TimeSource::Daytime,
                    "random" => TimeSource::Random,
                    "moon_phase" => TimeSource::MoonPhase,
                    _ => return None,
                },
                wobble: wobble(node),
            },
            _ => return None,
        })
    }

    fn get(self, use_: &ItemUse, world: &ItemWorld) -> f32 {
        match self {
            RangeProp::UseDuration { .. } | RangeProp::UseCycle { .. } if !use_.using => 0.0,
            RangeProp::UseDuration { remaining: true } => use_.use_remaining().unwrap_or(f32::NAN),
            RangeProp::UseDuration { remaining: false } => use_.use_duration(),
            RangeProp::UseCycle { period } => use_.use_remaining().map_or(f32::NAN, |r| r % period),
            RangeProp::CrossbowPull if use_.charge != ChargeType::None || !use_.using => 0.0,
            RangeProp::CrossbowPull => {
                use_.use_duration() / crossbow_charge_ticks(use_.quick_charge)
            }
            RangeProp::Compass { target, wobble } => compass(target, wobble, use_, world),
            RangeProp::Time { source, wobble } => match (use_.owner, source, wobble) {
                (None, ..) => 0.0,
                (_, TimeSource::Daytime, true) => world.daytime,
                (_, TimeSource::Daytime, false) => world.sun,
                (_, TimeSource::Random, true) => world.clock,
                (_, TimeSource::Random, false) => world.random,
                (_, TimeSource::MoonPhase, true) => world.moon_wobble,
                (_, TimeSource::MoonPhase, false) => world.moon,
            },
        }
    }
}

use super::kinetic::crossbow_charge_ticks;

fn compass(target: Target, wobble: bool, use_: &ItemUse, world: &ItemWorld) -> f32 {
    let Some(owner) = use_.owner else {
        return 0.0;
    };
    let pos = match target {
        Target::None => None,
        Target::Lodestone => use_.lodestone,
        Target::Spawn => world.spawn,
        Target::Recovery => world.death.filter(|_| owner.local),
    };
    let [ox, oy, oz] = owner.pos;
    let centre = pos.map(|[x, y, z]| [x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5]);
    let Some([cx, _, cz]) = centre.filter(|[cx, cy, cz]| {
        (cx - ox).powi(2) + (cy - oy).powi(2) + (cz - oz).powi(2) >= 9.999999747378752E-6
    }) else {
        let spin = if wobble { world.spin } else { world.random };
        let offset = owner.seed.wrapping_mul(1327217883) as f32 / 2.1474836E9;
        return positive_modulo(spin + offset, 1.0);
    };
    let angle = ((cz - oz).atan2(cx - ox) / 6.2831854820251465) as f32;
    let yaw01 = positive_modulo(owner.yaw / 360.0, 1.0);
    if owner.local {
        let needle = if wobble {
            world.needle
        } else {
            0.5 - (yaw01 - 0.25)
        };
        return positive_modulo(angle + needle, 1.0);
    }
    positive_modulo(0.5 - (yaw01 - 0.25 - angle), 1.0)
}

fn select_index(thresholds: &[f32], value: f32) -> Option<usize> {
    if value.is_nan() {
        return None;
    }
    let found = if thresholds.len() < 16 {
        thresholds
            .iter()
            .position(|&t| t > value)
            .unwrap_or(thresholds.len()) as isize
            - 1
    } else {
        let (mut low, mut high) = (0isize, thresholds.len() as isize - 1);
        let key_bits = value.to_bits() as i32;
        loop {
            if low > high {
                break low - 1;
            }
            let mid = (low + high) >> 1;
            let mid_val = thresholds[mid as usize];
            let mid_bits = mid_val.to_bits() as i32;
            if mid_val < value || (mid_val == value && mid_bits < key_bits) {
                low = mid + 1;
            } else if mid_val > value || mid_bits != key_bits {
                high = mid - 1;
            } else {
                break mid;
            }
        }
    };
    usize::try_from(found).ok()
}

type NodeId = u16;

#[derive(Clone, Debug)]
enum Node {
    Leaf(u16),
    Cond {
        prop: CondProp,
        on_false: NodeId,
        on_true: NodeId,
    },
    Select {
        prop: SelectProp,
        cases: Box<[(Box<str>, NodeId)]>,
        fallback: NodeId,
    },
    Range {
        prop: RangeProp,
        scale: f32,
        thresholds: Box<[f32]>,
        entries: Box<[NodeId]>,
        fallback: NodeId,
    },
}

#[derive(Clone, Debug)]
pub struct Leaf {
    pub json: Json,
    pub transform: Mat4,
    pub icon: Option<Box<str>>,
    pub bundle_open: Option<(Box<str>, Box<str>)>,
}

#[derive(Clone, Debug)]
pub struct ModelTree {
    nodes: Box<[Node]>,
    leaves: Box<[Leaf]>,
}

impl ModelTree {
    pub fn compile(root: &Json) -> Option<ModelTree> {
        let mut builder = Builder::default();
        builder.node(root, Mat4::IDENTITY, 0);
        if !builder.dynamic {
            return None;
        }
        Some(ModelTree {
            nodes: builder.nodes.into_boxed_slice(),
            leaves: builder.leaves.into_boxed_slice(),
        })
    }

    pub fn leaves(&self) -> &[Leaf] {
        &self.leaves
    }

    pub fn leaf(&self, index: u16) -> &Leaf {
        &self.leaves[index as usize]
    }

    pub fn eval(&self, ctx: &str, use_: &ItemUse, world: &ItemWorld) -> u16 {
        let mut at = 0usize;
        loop {
            at = usize::from(match &self.nodes[at] {
                Node::Leaf(leaf) => return *leaf,
                Node::Cond {
                    prop,
                    on_false,
                    on_true,
                } => {
                    let holds = match prop {
                        CondProp::UsingItem => use_.using,
                        CondProp::HasLodestone => use_.has_lodestone,
                        CondProp::HasSelectedItem => use_.bundle_selected,
                        CondProp::Broken => use_.broken,
                        CondProp::FishingRodCast => use_.fishing_cast,
                    };
                    if holds { *on_true } else { *on_false }
                }
                Node::Select {
                    prop: SelectProp::LocalTime(format),
                    cases,
                    fallback,
                } => super::local_time::pick(*format, |value| {
                    cases
                        .iter()
                        .find(|(when, _)| **when == *value)
                        .map_or(*fallback, |(_, node)| *node)
                }),
                Node::Select {
                    prop,
                    cases,
                    fallback,
                } => {
                    let value = match prop {
                        SelectProp::DisplayContext => Some(ctx),
                        SelectProp::ChargeType => Some(use_.charge.name()),
                        SelectProp::TrimMaterial => TRIM_MATERIALS
                            .get((use_.trim as usize).wrapping_sub(1))
                            .copied(),
                        SelectProp::ContextDimension => {
                            Some(world.level).filter(|level| !level.is_empty())
                        }
                        SelectProp::BlockState { property } => BLOCK_STATE_VALUES
                            .get((use_.block_state as usize).wrapping_sub(1))
                            .filter(|&&(p, _)| p == *property)
                            .map(|&(_, value)| value),
                        SelectProp::LocalTime(_) => None,
                    };
                    value
                        .and_then(|value| cases.iter().find(|(when, _)| **when == *value))
                        .map_or(*fallback, |(_, node)| *node)
                }
                Node::Range {
                    prop,
                    scale,
                    thresholds,
                    entries,
                    fallback,
                } => match select_index(thresholds, prop.get(use_, world) * scale) {
                    Some(i) => entries[i],
                    None => *fallback,
                },
            });
        }
    }
}

#[derive(Default)]
struct Builder {
    nodes: Vec<Node>,
    leaves: Vec<Leaf>,
    dynamic: bool,
}

impl Builder {
    fn node(&mut self, node: &Json, parent: Mat4, depth: u32) -> NodeId {
        let at = self.nodes.len();
        self.nodes.push(Node::Leaf(0));
        let local = parent * parse_transformation(node);
        let child = |builder: &mut Builder, json: Option<&Json>| match json {
            Some(json) => builder.node(json, local, depth + 1),
            None => builder.leaf(&Json::Null, local),
        };
        let compiled = if depth > 8 {
            None
        } else {
            match Dispatch::of(node) {
                Dispatch::Condition => CondProp::of(node).map(|prop| {
                    let on_false = child(self, node.get("on_false"));
                    let on_true = child(self, node.get("on_true"));
                    Node::Cond {
                        prop,
                        on_false,
                        on_true,
                    }
                }),
                Dispatch::Select => SelectProp::of(node).map(|prop| {
                    let mut cases: Vec<(Box<str>, NodeId)> = Vec::new();
                    for case in node.get("cases").map(Json::arr).unwrap_or_default() {
                        let target = child(self, case.get("model"));
                        let whens = match case.get("when") {
                            Some(Json::Arr(values)) => values.as_slice(),
                            Some(value) => std::slice::from_ref(value),
                            None => &[],
                        };
                        cases.extend(
                            whens
                                .iter()
                                .filter_map(Json::as_str)
                                .map(|when| (Box::from(strip_namespace(when)), target)),
                        );
                    }
                    let fallback = child(self, node.get("fallback"));
                    Node::Select {
                        prop,
                        cases: cases.into_boxed_slice(),
                        fallback,
                    }
                }),
                Dispatch::RangeDispatch => RangeProp::of(node).map(|prop| {
                    let mut entries: Vec<(f32, Option<&Json>)> = node
                        .get("entries")
                        .map(Json::arr)
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|e| Some((e.get("threshold")?.as_f32()?, e.get("model"))))
                        .collect();
                    entries.sort_by(|a, b| a.0.total_cmp(&b.0));
                    let thresholds = entries.iter().map(|e| e.0).collect();
                    let entries = entries.iter().map(|e| child(self, e.1)).collect();
                    let fallback = child(self, node.get("fallback"));
                    Node::Range {
                        prop,
                        scale: node.get("scale").and_then(Json::as_f32).unwrap_or(1.0),
                        thresholds,
                        entries,
                        fallback,
                    }
                }),
                _ => None,
            }
        };
        self.nodes[at] = match compiled {
            Some(compiled) => {
                self.dynamic = true;
                compiled
            }
            None => Node::Leaf(self.leaf(node, parent)),
        };
        at as NodeId
    }

    fn leaf(&mut self, json: &Json, transform: Mat4) -> u16 {
        let plain = Dispatch::of(json) == Dispatch::Model
            && json.get("transformation").is_none()
            && transform == Mat4::IDENTITY;
        let icon = plain
            .then(|| json.get("model").and_then(Json::as_str))
            .flatten()
            .map(|model| Box::from(strip_namespace(model)));
        let bundle_open = bundle_open_layers(json);
        self.leaves.push(Leaf {
            json: json.clone(),
            transform,
            icon,
            bundle_open,
        });
        (self.leaves.len() - 1) as u16
    }
}

fn bundle_open_layers(json: &Json) -> Option<(Box<str>, Box<str>)> {
    if Dispatch::of(json) != Dispatch::Composite {
        return None;
    }
    let models = json.get("models").map(Json::arr)?;
    let path = |node: Option<&Json>| {
        let node = node.filter(|n| Dispatch::of(n) == Dispatch::Model)?;
        Some(Box::from(strip_namespace(node.get("model")?.as_str()?)))
    };
    let at = models
        .iter()
        .position(|m| Dispatch::of(m) == Dispatch::Bundle)?;
    Some((
        path(models.get(at.checked_sub(1)?))?,
        path(models.get(at + 1))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(src: &str) -> ModelTree {
        ModelTree::compile(&Json::parse(src).unwrap()).expect("dynamic tree")
    }

    fn icon(tree: &ModelTree, ctx: &str, use_: &ItemUse, world: &ItemWorld) -> String {
        tree.leaf(tree.eval(ctx, use_, world))
            .icon
            .as_deref()
            .unwrap_or("")
            .to_string()
    }

    #[test]
    fn a_bow_pulls_through_its_three_stages() {
        let bow = tree(
            r#"{"type":"minecraft:condition","property":"minecraft:using_item",
            "on_false":{"type":"minecraft:model","model":"minecraft:item/bow"},
            "on_true":{"type":"minecraft:range_dispatch","property":"minecraft:use_duration",
              "scale":0.05,"fallback":{"type":"minecraft:model","model":"minecraft:item/bow_pulling_0"},
              "entries":[{"threshold":0.65,"model":{"type":"minecraft:model","model":"minecraft:item/bow_pulling_1"}},
                         {"threshold":0.9,"model":{"type":"minecraft:model","model":"minecraft:item/bow_pulling_2"}}]}}"#,
        );
        let world = ItemWorld::default();
        let at = |ticks: f32| ItemUse {
            using: true,
            use_ticks: ticks,
            ..ItemUse::NONE
        };
        assert_eq!(icon(&bow, "gui", &ItemUse::NONE, &world), "item/bow");
        assert_eq!(icon(&bow, "gui", &at(5.0), &world), "item/bow_pulling_0");
        assert_eq!(icon(&bow, "gui", &at(13.0), &world), "item/bow_pulling_1");
        assert_eq!(icon(&bow, "gui", &at(18.0), &world), "item/bow_pulling_2");
    }

    #[test]
    fn a_brush_cycles_on_its_remaining_ticks() {
        let brush = tree(
            r#"{"type":"minecraft:range_dispatch","property":"minecraft:use_cycle","period":10.0,"scale":0.1,
            "fallback":{"type":"minecraft:model","model":"minecraft:item/brush"},
            "entries":[{"threshold":0.25,"model":{"type":"minecraft:model","model":"minecraft:item/brush_brushing_0"}},
                       {"threshold":0.5,"model":{"type":"minecraft:model","model":"minecraft:item/brush_brushing_1"}},
                       {"threshold":0.75,"model":{"type":"minecraft:model","model":"minecraft:item/brush_brushing_2"}}]}"#,
        );
        let world = ItemWorld::default();
        let at = |ticks: f32| ItemUse {
            using: true,
            use_ticks: ticks,
            use_max: 200,
            ..ItemUse::NONE
        };
        assert_eq!(icon(&brush, "gui", &ItemUse::NONE, &world), "item/brush");
        assert_eq!(
            icon(&brush, "gui", &at(3.5), &world),
            "item/brush_brushing_1"
        );
        assert_eq!(
            icon(&brush, "gui", &at(7.0), &world),
            "item/brush_brushing_0"
        );
        assert_eq!(icon(&brush, "gui", &at(8.0), &world), "item/brush");
        let unknown = ItemUse {
            use_max: 0,
            ..at(3.0)
        };
        assert_eq!(icon(&brush, "gui", &unknown, &world), "item/brush");
    }

    #[test]
    fn broken_and_cast_pick_their_models() {
        let elytra = tree(
            r#"{"type":"minecraft:condition","property":"minecraft:broken",
            "on_false":{"type":"minecraft:model","model":"minecraft:item/elytra"},
            "on_true":{"type":"minecraft:model","model":"minecraft:item/elytra_broken"}}"#,
        );
        let rod = tree(
            r#"{"type":"minecraft:condition","property":"minecraft:fishing_rod/cast",
            "on_false":{"type":"minecraft:model","model":"minecraft:item/fishing_rod"},
            "on_true":{"type":"minecraft:model","model":"minecraft:item/fishing_rod_cast"}}"#,
        );
        let world = ItemWorld::default();
        assert_eq!(
            icon(
                &elytra,
                "gui",
                &ItemUse {
                    broken: true,
                    ..ItemUse::NONE
                },
                &world
            ),
            "item/elytra_broken"
        );
        assert_eq!(icon(&elytra, "gui", &ItemUse::NONE, &world), "item/elytra");
        assert_eq!(
            icon(
                &rod,
                "gui",
                &ItemUse {
                    fishing_cast: true,
                    ..ItemUse::NONE
                },
                &world
            ),
            "item/fishing_rod_cast"
        );
    }

    #[test]
    fn a_trim_select_matches_its_material() {
        let armour = tree(
            r#"{"type":"minecraft:select","property":"minecraft:trim_material",
            "cases":[{"when":"minecraft:quartz","model":{"type":"minecraft:model","model":"minecraft:item/x_quartz_trim"}}],
            "fallback":{"type":"minecraft:model","model":"minecraft:item/x"}}"#,
        );
        let world = ItemWorld::default();
        let quartz = ItemUse {
            trim: trim_index("quartz"),
            ..ItemUse::NONE
        };
        assert_eq!(icon(&armour, "gui", &quartz, &world), "item/x_quartz_trim");
        assert_eq!(icon(&armour, "gui", &ItemUse::NONE, &world), "item/x");
        let wolf = Json::parse(
            r#"{"type":"minecraft:condition","property":"minecraft:has_component",
            "component":"minecraft:dyed_color","on_false":{"type":"minecraft:model","model":"a"},
            "on_true":{"type":"minecraft:model","model":"b"}}"#,
        )
        .unwrap();
        assert!(ModelTree::compile(&wolf).is_none());
    }

    #[test]
    fn a_block_state_select_matches_its_property() {
        let light = tree(
            r#"{"type":"minecraft:select","property":"minecraft:block_state","block_state_property":"level",
            "cases":[{"when":"7","model":{"type":"minecraft:model","model":"minecraft:item/light_07"}}],
            "fallback":{"type":"minecraft:model","model":"minecraft:item/light"}}"#,
        );
        let world = ItemWorld::default();
        let at = |p, v| ItemUse {
            block_state: block_state_index(p, v),
            ..ItemUse::NONE
        };
        assert_eq!(
            icon(&light, "gui", &at("level", "7"), &world),
            "item/light_07"
        );
        assert_eq!(icon(&light, "gui", &ItemUse::NONE, &world), "item/light");
        assert_eq!(
            icon(&light, "gui", &at("mode", "log"), &world),
            "item/light"
        );
    }

    #[test]
    fn a_compass_points_at_spawn() {
        let use_ = ItemUse {
            owner: Some(Owner {
                pos: [0.5, 0.5, 0.5],
                yaw: 0.0,
                local: false,
                seed: 0,
            }),
            ..ItemUse::NONE
        };
        let world = ItemWorld {
            spawn: Some([10, 0, 0]),
            ..ItemWorld::default()
        };
        let value = compass(Target::Spawn, true, &use_, &world);
        assert!((value - 0.75).abs() < 1e-6, "{value}");
        assert_eq!(compass(Target::Spawn, true, &ItemUse::NONE, &world), 0.0);
    }

    #[test]
    fn the_wobbler_settles_on_its_target() {
        let mut w = Wobbler::default();
        for _ in 0..400 {
            w.update(0.3, 0.8);
        }
        assert!((w.rotation - 0.3).abs() < 1e-3, "{}", w.rotation);
    }
}
