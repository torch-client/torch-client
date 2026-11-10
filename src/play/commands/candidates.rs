use azalea_protocol::packets::game::c_commands::BrigadierParser;
use azalea_registry::Registry;
use azalea_registry::builtin::{BlockKind, EntityKind, ItemKind, ParticleKind, SoundEvent};
use azalea_registry::identifier::Identifier;

use super::parse::{Context, Parse, SELECTORS, parse_argument};
use super::suggestion::{Range, Suggestion, Suggestions};
use super::tree::{CommandTree, NodeKind, Provider};

macro_rules! registry_ids {
    ($ty:ty) => {
        (0u32..)
            .map_while(<$ty as Registry>::from_u32)
            .map(|value| value.to_str())
    };
}

#[derive(Default, Clone)]
pub struct Source {
    pub player_names: Vec<String>,
    pub custom_completions: Vec<String>,
}

impl Source {
    pub(super) fn custom_tab_suggestions(&self) -> Vec<String> {
        let mut out = self.player_names.clone();
        for e in &self.custom_completions {
            if !out.contains(e) {
                out.push(e.clone());
            }
        }
        out
    }
}

fn is_splitter(c: char) -> bool {
    matches!(c, '.' | '_' | '/')
}

pub fn matches_sub_str(pattern: &str, input: &str) -> bool {
    let mut index = 0usize;
    loop {
        if input[index..].starts_with(pattern) {
            return true;
        }
        match input[index..].find(is_splitter) {
            Some(off) => index = index + off + 1,
            None => return false,
        }
    }
}

const TEAM_COLORS: [&str; 16] = [
    "black",
    "dark_blue",
    "dark_green",
    "dark_aqua",
    "dark_red",
    "dark_purple",
    "gold",
    "gray",
    "dark_gray",
    "blue",
    "green",
    "aqua",
    "red",
    "light_purple",
    "yellow",
    "white",
];

fn display_slots() -> Vec<String> {
    let mut out = vec![
        "list".to_string(),
        "sidebar".to_string(),
        "below_name".to_string(),
    ];
    for color in &TEAM_COLORS {
        out.push(format!("sidebar.team.{color}"));
    }
    out
}

enum Candidates {
    Values(Vec<String>),
    AskServer,
    None,
}

fn candidates(tree: &CommandTree, node: usize, remaining: &str, source: &Source) -> Candidates {
    let lower = remaining.to_lowercase();
    match &tree.node(node).kind {
        NodeKind::Root => Candidates::None,
        NodeKind::Literal(name) => {
            if name.to_lowercase().starts_with(&lower) {
                Candidates::Values(vec![name.clone()])
            } else {
                Candidates::Values(Vec::new())
            }
        }
        NodeKind::Argument {
            parser, provider, ..
        } => match provider {
            Provider::Argument => argument_candidates(parser, remaining, &lower, source),
            Provider::AskServer => Candidates::AskServer,
            Provider::AvailableSounds => {
                Candidates::Values(suggest_resource(&lower, registry_ids!(SoundEvent)))
            }
            Provider::SummonableEntities => {
                Candidates::Values(suggest_resource(&lower, registry_ids!(EntityKind)))
            }
        },
    }
}

fn argument_candidates(
    parser: &BrigadierParser,
    remaining: &str,
    lower: &str,
    source: &Source,
) -> Candidates {
    match parser {
        BrigadierParser::Bool => Candidates::Values(
            ["false", "true"]
                .into_iter()
                .filter(|v| v.starts_with(lower))
                .map(str::to_string)
                .collect(),
        ),
        BrigadierParser::GameMode => {
            suggest(lower, ["survival", "creative", "adventure", "spectator"])
        }
        BrigadierParser::EntityAnchor => suggest(lower, ["eyes", "feet"]),
        BrigadierParser::Operation => {
            suggest(lower, ["=", "+=", "-=", "*=", "/=", "%=", "><", "<", ">"])
        }
        BrigadierParser::TemplateMirror => suggest(lower, ["none", "left_right", "front_back"]),
        BrigadierParser::TemplateRotation => suggest(
            lower,
            ["none", "clockwise_90", "180", "counterclockwise_90"],
        ),
        BrigadierParser::Heightmap => suggest(
            lower,
            [
                "world_surface",
                "motion_blocking",
                "motion_blocking_no_leaves",
                "ocean_floor",
            ],
        ),
        BrigadierParser::TeamColor => suggest(lower, TEAM_COLORS),
        BrigadierParser::ScoreboardSlot => suggest(lower, display_slots()),
        BrigadierParser::Entity(_)
        | BrigadierParser::GameProfile
        | BrigadierParser::ScoreHolder { .. } => {
            if remaining.starts_with('@') && remaining.contains('[') {
                Candidates::AskServer
            } else {
                let players = source.player_names.iter().map(String::as_str);
                suggest(lower, players.chain(SELECTORS))
            }
        }
        BrigadierParser::Vec3 | BrigadierParser::BlockPos => {
            suggest(lower, coordinate_candidates(parser, remaining, 3))
        }
        BrigadierParser::ColumnPos | BrigadierParser::Vec2 => {
            suggest(lower, coordinate_candidates(parser, remaining, 2))
        }
        BrigadierParser::ItemStack => Candidates::Values(suggest_resource(lower, item_ids())),
        BrigadierParser::ItemPredicate => tag_or(lower, remaining, item_ids()),
        BrigadierParser::BlockState => Candidates::Values(suggest_resource(lower, block_ids())),
        BrigadierParser::BlockPredicate => tag_or(lower, remaining, block_ids()),
        BrigadierParser::Particle => {
            Candidates::Values(suggest_resource(lower, registry_ids!(ParticleKind)))
        }
        BrigadierParser::ItemSlot => suggest(lower, slot_names(false)),
        BrigadierParser::ItemSlots => suggest(lower, slot_names(true)),
        BrigadierParser::Time { .. } => {
            let number = !remaining.is_empty()
                && remaining
                    .chars()
                    .all(|c| matches!(c, '0'..='9' | '.' | '-'));
            if number {
                suggest(
                    lower,
                    ["d", "s", "t"].map(|unit| format!("{remaining}{unit}")),
                )
            } else {
                Candidates::None
            }
        }
        BrigadierParser::Resource { registry_key }
        | BrigadierParser::ResourceKey { registry_key }
        | BrigadierParser::ResourceSelector { registry_key } => {
            registry_candidates(registry_key, lower)
        }
        BrigadierParser::ResourceOrTag { registry_key }
        | BrigadierParser::ResourceOrTagKey { registry_key } => {
            if remaining.starts_with('#') {
                Candidates::AskServer
            } else {
                registry_candidates(registry_key, lower)
            }
        }
        BrigadierParser::String(_)
        | BrigadierParser::Message
        | BrigadierParser::Integer(_)
        | BrigadierParser::Long(_)
        | BrigadierParser::Float(_)
        | BrigadierParser::Double(_)
        | BrigadierParser::Uuid
        | BrigadierParser::Angle
        | BrigadierParser::Rotation
        | BrigadierParser::Swizzle
        | BrigadierParser::IntRange
        | BrigadierParser::FloatRange
        | BrigadierParser::HexColor => Candidates::None,
        _ => Candidates::AskServer,
    }
}

fn suggest<S: AsRef<str> + Into<String>>(
    lower: &str,
    values: impl IntoIterator<Item = S>,
) -> Candidates {
    Candidates::Values(
        values
            .into_iter()
            .filter(|v| matches_sub_str(lower, &v.as_ref().to_lowercase()))
            .map(Into::into)
            .collect(),
    )
}

fn block_ids() -> impl Iterator<Item = &'static str> {
    registry_ids!(BlockKind)
}

fn item_ids() -> impl Iterator<Item = &'static str> {
    registry_ids!(ItemKind)
}

fn tag_or<'a>(lower: &str, remaining: &str, ids: impl Iterator<Item = &'a str>) -> Candidates {
    if remaining.starts_with('#') {
        Candidates::AskServer
    } else {
        Candidates::Values(suggest_resource(lower, ids))
    }
}

fn registry_candidates(registry_key: &Identifier, lower: &str) -> Candidates {
    use azalea_registry::builtin::{
        Attribute, BlockEntityKind, CustomStat, Fluid, GameEvent, MenuKind, MobEffect,
        PointOfInterestKind, Potion, RecipeKind, StatKind, VillagerKind, VillagerProfession,
    };
    if registry_key.namespace() != "minecraft" {
        return Candidates::AskServer;
    }
    let values = match registry_key.path() {
        "block" => suggest_resource(lower, block_ids()),
        "item" => suggest_resource(lower, item_ids()),
        "entity_type" => suggest_resource(lower, registry_ids!(EntityKind)),
        "particle_type" => suggest_resource(lower, registry_ids!(ParticleKind)),
        "sound_event" => suggest_resource(lower, registry_ids!(SoundEvent)),
        "mob_effect" => suggest_resource(lower, registry_ids!(MobEffect)),
        "potion" => suggest_resource(lower, registry_ids!(Potion)),
        "attribute" => suggest_resource(lower, registry_ids!(Attribute)),
        "block_entity_type" => suggest_resource(lower, registry_ids!(BlockEntityKind)),
        "fluid" => suggest_resource(lower, registry_ids!(Fluid)),
        "game_event" => suggest_resource(lower, registry_ids!(GameEvent)),
        "menu" => suggest_resource(lower, registry_ids!(MenuKind)),
        "point_of_interest_type" => suggest_resource(lower, registry_ids!(PointOfInterestKind)),
        "recipe_type" => suggest_resource(lower, registry_ids!(RecipeKind)),
        "custom_stat" => suggest_resource(lower, registry_ids!(CustomStat)),
        "stat_type" => suggest_resource(lower, registry_ids!(StatKind)),
        "villager_profession" => suggest_resource(lower, registry_ids!(VillagerProfession)),
        "villager_type" => suggest_resource(lower, registry_ids!(VillagerKind)),
        _ => return Candidates::AskServer,
    };
    Candidates::Values(values)
}

fn slot_names(ranges: bool) -> Vec<String> {
    let mut out = vec!["contents".to_string()];
    let mut numbered = |prefix: &str, count: usize| {
        for i in 0..count {
            out.push(format!("{prefix}.{i}"));
        }
        if ranges {
            out.push(format!("{prefix}.*"));
        }
    };
    numbered("container", 54);
    numbered("hotbar", 9);
    numbered("inventory", 27);
    numbered("enderchest", 27);
    numbered("mob.inventory", 8);
    numbered("horse", 15);
    numbered("player.crafting", 4);
    out.extend(
        [
            "weapon",
            "weapon.mainhand",
            "weapon.offhand",
            "armor.head",
            "armor.chest",
            "armor.legs",
            "armor.feet",
            "armor.body",
            "saddle",
            "horse.chest",
            "player.cursor",
        ]
        .map(str::to_string),
    );
    if ranges {
        out.extend(["weapon.*", "armor.*"].map(str::to_string));
    }
    out
}

fn suggest_resource<'a>(lower: &str, ids: impl Iterator<Item = &'a str>) -> Vec<String> {
    let has_namespace = lower.contains(':');
    ids.filter(|id| {
        if has_namespace {
            matches_sub_str(lower, id)
        } else {
            let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
            matches_sub_str(lower, namespace) || matches_sub_str(lower, path)
        }
    })
    .map(str::to_string)
    .collect()
}

fn coordinate_candidates(parser: &BrigadierParser, current: &str, axes: usize) -> Vec<String> {
    let axis = if current.starts_with('^') { "^" } else { "~" };
    let mut out = Vec::new();
    if current.is_empty() {
        for n in 1..=axes {
            out.push(vec![axis; n].join(" "));
        }
    } else {
        let fields: Vec<&str> = current.split(' ').collect();
        if fields.len() < axes {
            let mut acc = fields.join(" ");
            for _ in fields.len()..axes {
                acc.push(' ');
                acc.push_str(axis);
                out.push(acc.clone());
            }
        }
    }
    let full: Vec<char> = out.last().map(|s| s.chars().collect()).unwrap_or_default();
    let mut cursor = 0;
    if !parse_argument(parser, &full, &mut cursor).is_ok_and(|()| cursor == full.len()) {
        return Vec::new();
    }
    out
}

pub fn find_suggestion_context(parse: &Parse, cursor: usize) -> (usize, usize) {
    find_in(&parse.chain, 0, cursor)
}

fn find_in(chain: &[Context], i: usize, cursor: usize) -> (usize, usize) {
    let c = &chain[i];
    if c.range.end < cursor {
        if i + 1 < chain.len() {
            return find_in(chain, i + 1, cursor);
        }
        return match c.nodes.last() {
            Some((node, range)) => (*node, range.end + 1),
            None => (c.root, c.range.start),
        };
    }
    let mut prev = c.root;
    for (node, range) in &c.nodes {
        if range.start <= cursor && cursor <= range.end {
            return (prev, range.start);
        }
        prev = *node;
    }
    (prev, c.range.start)
}

pub(super) fn local_suggestions(
    values: impl IntoIterator<Item = String>,
    remaining: &str,
    range: Range,
) -> impl Iterator<Item = Suggestion> {
    values
        .into_iter()
        .filter(move |text| text != remaining)
        .map(move |text| Suggestion {
            range,
            text,
            tooltip: None,
        })
}

pub fn completion_suggestions(
    tree: &CommandTree,
    parent: usize,
    start_pos: usize,
    input: &[char],
    cursor: usize,
    source: &Source,
) -> (Suggestions, bool) {
    let start = start_pos.min(cursor);
    let remaining: String = input[start..cursor].iter().collect();
    let range = Range::between(start, cursor);

    let mut all: Vec<Suggestion> = Vec::new();
    let mut ask_server = false;
    for &child in &tree.node(parent).children {
        match candidates(tree, child, &remaining, source) {
            Candidates::AskServer => ask_server = true,
            Candidates::None => {}
            Candidates::Values(values) => {
                all.extend(local_suggestions(values, &remaining, range));
            }
        }
    }
    (Suggestions::create(input, all), ask_server)
}
