use std::sync::{Arc, Mutex, OnceLock};

use azalea_chat::style::ChatFormatting;
use azalea_core::objectives::ObjectiveCriteria;
use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_protocol::packets::game::c_set_objective::Method as ObjectiveMethod;
use azalea_protocol::packets::game::c_set_player_team::{Method as TeamMethod, NameTagVisibility};
use bevy::platform::collections::HashMap;

use crate::client::chat_text::{to_spans, to_spans_styled};
use crate::session::{Gamemode, SharedMutex};
use crate::text::{Span, Style};

#[derive(Default)]
pub struct TabList {
    pub header: Vec<Span>,
    pub footer: Vec<Span>,
    pub rows: Vec<TabRow>,
    pub objective: Option<TabObjective>,
}

#[derive(Clone, Default)]
pub struct TabRow {
    pub uuid: u128,
    pub name: Vec<Span>,
    pub spectator: bool,
    pub latency: i32,
    pub skin: u8,
    pub show_hat: bool,
    pub upside_down: bool,
    pub score: i32,
    pub team: Option<String>,
    pub score_text: Vec<Span>,
}

#[derive(Default)]
pub struct Sidebar {
    pub active: bool,
    pub title: Vec<Span>,
    pub rows: Vec<SidebarRow>,
}

#[derive(Clone)]
pub struct SidebarRow {
    pub name: Vec<Span>,
    pub score: Vec<Span>,
}

#[derive(Clone, Copy, Default)]
pub struct TabObjective {
    pub hearts: bool,
}

struct Entry {
    name: String,
    display_name: Option<Vec<Span>>,
    gamemode: Gamemode,
    latency: i32,
    listed: bool,
    list_order: i32,
    show_hat: bool,
    default_skin: u8,
}

impl Entry {
    fn new(uuid: u128, name: String) -> Entry {
        Entry {
            default_skin: default_skin(uuid),
            name,
            display_name: None,
            gamemode: Gamemode::Survival,
            latency: 0,
            listed: false,
            list_order: 0,
            show_hat: true,
        }
    }
}

struct Team {
    color: Option<u32>,
    slot: Option<usize>,
    prefix: Vec<Span>,
    suffix: Vec<Span>,
    members: Vec<String>,
    nametag_visibility: NameTagVisibility,
}

struct Objective {
    hearts: bool,
    format: Option<NumberFormat>,
    display_name: Vec<Span>,
}

#[derive(Clone)]
enum NumberFormat {
    Blank,
    Styled(Style),
    Fixed(Vec<Span>),
}

impl NumberFormat {
    fn player_list_default() -> NumberFormat {
        NumberFormat::Styled(Style::colored(crate::text::FORMAT_COLORS[14]))
    }

    fn sidebar_default() -> NumberFormat {
        NumberFormat::Styled(Style::colored(crate::text::FORMAT_COLORS[12]))
    }

    fn no_style() -> NumberFormat {
        NumberFormat::Styled(Style::default())
    }

    fn format(&self, value: i32) -> Vec<Span> {
        match self {
            NumberFormat::Blank => Vec::new(),
            NumberFormat::Styled(style) => vec![Span {
                text: value.to_string(),
                style: *style,
            }],
            NumberFormat::Fixed(spans) => spans.clone(),
        }
    }
}

struct Score {
    value: i32,
    format: Option<NumberFormat>,
    display: Option<Vec<Span>>,
}

#[derive(Default)]
struct State {
    players: HashMap<u128, Entry>,
    header: Vec<Span>,
    footer: Vec<Span>,
    teams: HashMap<String, Team>,
    team_of: HashMap<String, String>,
    objectives: HashMap<String, Objective>,
    scores: HashMap<String, HashMap<String, Score>>,
    slots: [Option<String>; SLOT_COUNT],
}

const LIST: usize = 0;
const SIDEBAR: usize = 1;
const BELOW_NAME: usize = 2;
const TEAM_SLOT_BASE: usize = 3;
const SLOT_COUNT: usize = TEAM_SLOT_BASE + 16;

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(Default::default)
}

fn player_names(s: &State) -> Vec<String> {
    let mut names: Vec<String> = s
        .players
        .values()
        .filter(|e| !e.name.is_empty())
        .map(|e| e.name.clone())
        .collect();
    names.sort();
    names
}

pub(crate) fn reset() {
    *state().lock().unwrap() = State::default();
    #[cfg(feature = "skins")]
    crate::client::skins::reset();
}

pub(crate) fn packet(shared: &Arc<SharedMutex>, packet: &ClientboundGamePacket) {
    let mut s = match packet {
        ClientboundGamePacket::PlayerInfoUpdate(_)
        | ClientboundGamePacket::PlayerInfoRemove(_)
        | ClientboundGamePacket::TabList(_)
        | ClientboundGamePacket::SetObjective(_)
        | ClientboundGamePacket::SetDisplayObjective(_)
        | ClientboundGamePacket::SetScore(_)
        | ClientboundGamePacket::ResetScore(_)
        | ClientboundGamePacket::SetPlayerTeam(_)
        | ClientboundGamePacket::StartConfiguration(_) => state().lock().unwrap(),
        _ => return,
    };

    let mut names_changed = false;
    let mut list_dirty = true;
    let mut sidebar_dirty = true;

    match packet {
        ClientboundGamePacket::PlayerInfoUpdate(p) => {
            sidebar_dirty = false;
            let a = &p.actions;
            for entry in &p.entries {
                let uuid = entry.profile.uuid.as_u128();
                let e = s
                    .players
                    .entry(uuid)
                    .or_insert_with(|| Entry::new(uuid, String::new()));
                if a.add_player {
                    e.name = entry.profile.name.clone();
                    names_changed = true;
                    #[cfg(feature = "skins")]
                    {
                        let refs = entry
                            .profile
                            .properties
                            .map
                            .get("textures")
                            .map(|p| crate::client::skins::parse_textures(&p.value))
                            .unwrap_or_default();
                        crate::client::skins::remember(uuid, refs);
                    }
                }
                if a.update_game_mode {
                    e.gamemode = Gamemode::from_azalea(entry.game_mode);
                }
                if a.update_listed {
                    e.listed = entry.listed;
                }
                if a.update_latency {
                    e.latency = entry.latency;
                }
                if a.update_display_name {
                    e.display_name = entry.display_name.as_ref().map(|d| to_spans(d));
                }
                if a.update_list_order {
                    e.list_order = entry.list_order;
                }
                if a.update_hat {
                    e.show_hat = entry.update_hat;
                }
            }
        }
        ClientboundGamePacket::PlayerInfoRemove(p) => {
            sidebar_dirty = false;
            for id in &p.profile_ids {
                s.players.remove(&id.as_u128());
            }
            names_changed = true;
        }
        ClientboundGamePacket::TabList(p) => {
            sidebar_dirty = false;
            s.header = to_spans(&p.header);
            s.footer = to_spans(&p.footer);
        }
        ClientboundGamePacket::SetObjective(p) => match &p.method {
            ObjectiveMethod::Add {
                render_type,
                number_format,
                display_name,
            }
            | ObjectiveMethod::Change {
                render_type,
                number_format,
                display_name,
            } => {
                s.objectives.insert(
                    p.objective_name.clone(),
                    Objective {
                        hearts: *render_type == ObjectiveCriteria::Hearts,
                        format: number_format.as_ref().map(read_number_format),
                        display_name: to_spans(display_name),
                    },
                );
            }
            ObjectiveMethod::Remove => {
                s.objectives.remove(&p.objective_name);
                s.scores.remove(&p.objective_name);
                for slot in &mut s.slots {
                    if slot.as_deref() == Some(p.objective_name.as_str()) {
                        *slot = None;
                    }
                }
            }
        },
        ClientboundGamePacket::SetDisplayObjective(p) => {
            let name = (!p.objective_name.is_empty()).then(|| p.objective_name.clone());
            s.slots[p.slot as usize] = name;
        }
        ClientboundGamePacket::SetScore(p) => {
            let touched = Some(p.objective_name.as_str());
            list_dirty = s.slots[LIST].as_deref() == touched;
            sidebar_dirty = sidebar_objective(&s).map(String::as_str) == touched;
            let value = p.score as i32;
            s.scores
                .entry(p.objective_name.clone())
                .or_default()
                .insert(
                    p.owner.clone(),
                    Score {
                        value,
                        format: p.number_format.as_ref().map(read_number_format),
                        display: p.display.as_ref().map(to_spans),
                    },
                );
        }
        ClientboundGamePacket::ResetScore(p) => match &p.objective_name {
            Some(name) => {
                let touched = Some(name.as_str());
                list_dirty = s.slots[LIST].as_deref() == touched;
                sidebar_dirty = sidebar_objective(&s).map(String::as_str) == touched;
                if let Some(scores) = s.scores.get_mut(name) {
                    scores.remove(&p.owner);
                }
            }
            None => {
                for scores in s.scores.values_mut() {
                    scores.remove(&p.owner);
                }
            }
        },
        ClientboundGamePacket::SetPlayerTeam(p) => {
            apply_team(&mut s, &p.name, &p.method);
        }
        ClientboundGamePacket::StartConfiguration(_) => {
            *s = State::default();
            names_changed = true;
        }
        _ => return,
    }

    let list = list_dirty.then(|| build(&s));
    let sidebar = sidebar_dirty.then(|| build_sidebar(&s));
    let names = names_changed.then(|| player_names(&s));
    drop(s);

    let mut shared = shared.lock().unwrap();
    if let Some(list) = list {
        shared.session.tab_list = Arc::new(list);
    }
    if let Some(sidebar) = sidebar {
        shared.session.sidebar = Arc::new(sidebar);
    }
    if let Some(names) = names {
        shared.session.player_names = names;
    }
}

fn apply_team(s: &mut State, name: &str, method: &TeamMethod) {
    match method {
        TeamMethod::Add((params, members)) => {
            let color = team_color(params.color);
            let root = color.map(Style::colored).unwrap_or_default();
            let team = Team {
                color,
                slot: team_slot(params.color),
                prefix: to_spans_styled(&params.player_prefix, root),
                suffix: to_spans_styled(&params.player_suffix, root),
                members: members.clone(),
                nametag_visibility: params.nametag_visibility,
            };
            for m in &team.members {
                s.team_of.insert(m.clone(), name.to_string());
            }
            s.teams.insert(name.to_string(), team);
        }
        TeamMethod::Change(params) => {
            let color = team_color(params.color);
            let root = color.map(Style::colored).unwrap_or_default();
            if let Some(team) = s.teams.get_mut(name) {
                team.color = color;
                team.slot = team_slot(params.color);
                team.prefix = to_spans_styled(&params.player_prefix, root);
                team.suffix = to_spans_styled(&params.player_suffix, root);
                team.nametag_visibility = params.nametag_visibility;
            }
        }
        TeamMethod::Remove => {
            if let Some(team) = s.teams.remove(name) {
                for m in &team.members {
                    if s.team_of.get(m).map(String::as_str) == Some(name) {
                        s.team_of.remove(m);
                    }
                }
            }
        }
        TeamMethod::Join(members) => {
            if let Some(team) = s.teams.get_mut(name) {
                for m in members {
                    if !team.members.contains(m) {
                        team.members.push(m.clone());
                    }
                }
            }
            for m in members {
                s.team_of.insert(m.clone(), name.to_string());
            }
        }
        TeamMethod::Leave(members) => {
            if let Some(team) = s.teams.get_mut(name) {
                team.members.retain(|m| !members.contains(m));
            }
            for m in members {
                if s.team_of.get(m).map(String::as_str) == Some(name) {
                    s.team_of.remove(m);
                }
            }
        }
    }
}

fn team_color(color: ChatFormatting) -> Option<u32> {
    crate::text::named_color(color.name())
}

fn team_slot(color: ChatFormatting) -> Option<usize> {
    let ord = color as usize;
    (ord < 16).then_some(TEAM_SLOT_BASE + ord)
}

fn read_number_format(format: &azalea_chat::numbers::NumberFormat) -> NumberFormat {
    use azalea_chat::numbers::NumberFormat as Az;
    match format {
        Az::Blank => NumberFormat::Blank,
        Az::Styled { style } => NumberFormat::Styled(styled_format(style)),
        Az::Fixed { value } => NumberFormat::Fixed(to_spans(value)),
    }
}

fn styled_format(style: &simdnbt::owned::Nbt) -> Style {
    let mut out = Style::default();
    let simdnbt::owned::Nbt::Some(nbt) = style else {
        return out;
    };
    if let Some(color) = nbt.string("color") {
        let color = color.to_str();
        let rgb = match color.strip_prefix('#') {
            Some(hex) => u32::from_str_radix(hex, 16).ok(),
            None => crate::text::named_color(&color),
        };
        if let Some(rgb) = rgb {
            out.color = rgb & 0xFF_FFFF;
        }
    }
    let flag = |name: &str| nbt.byte(name).map(|b| b != 0);
    out.bold = flag("bold").unwrap_or(false);
    out.italic = flag("italic").unwrap_or(false);
    out.underline = flag("underlined").unwrap_or(false);
    out.strikethrough = flag("strikethrough").unwrap_or(false);
    out.obfuscated = flag("obfuscated").unwrap_or(false);
    out
}

const MAX_ROWS: usize = 80;

fn build(s: &State) -> TabList {
    let mut listed: Vec<(&u128, &Entry)> = s.players.iter().filter(|(_, e)| e.listed).collect();

    let key = |e: &Entry| {
        (
            -e.list_order,
            (e.gamemode == Gamemode::Spectator) as u8,
            s.team_of.get(&e.name).cloned().unwrap_or_default(),
            e.name.to_lowercase(),
        )
    };
    if listed.len() > MAX_ROWS {
        listed.select_nth_unstable_by_key(MAX_ROWS, |&(_, e)| key(e));
        listed.truncate(MAX_ROWS);
    }
    listed.sort_by_cached_key(|&(_, e)| key(e));

    let objective = s.slots[LIST]
        .as_ref()
        .and_then(|name| s.objectives.get(name).map(|o| (name, o)));
    let scores = objective.and_then(|(name, _)| s.scores.get(name.as_str()));

    let rows: Vec<TabRow> = listed
        .iter()
        .map(|&(uuid, e)| {
            let spectator = e.gamemode == Gamemode::Spectator;
            let score = scores.and_then(|m| m.get(&e.name));
            let (value, score_text) = match objective {
                Some((_, obj)) if !obj.hearts => {
                    let value = score.map(|s| s.value).unwrap_or(0);
                    let format = score
                        .and_then(|s| s.format.as_ref())
                        .or(obj.format.as_ref())
                        .cloned()
                        .unwrap_or_else(NumberFormat::player_list_default);
                    (value, format.format(value))
                }
                Some((_, _)) => (score.map(|s| s.value).unwrap_or(0), Vec::new()),
                None => (0, Vec::new()),
            };
            TabRow {
                uuid: *uuid,
                name: display_name(s, e),
                spectator,
                latency: e.latency,
                skin: e.default_skin,
                show_hat: e.show_hat,
                upside_down: is_upside_down_name(&e.name),
                score: value,
                score_text,
                team: s.team_of.get(&e.name).cloned(),
            }
        })
        .collect();

    TabList {
        header: s.header.clone(),
        footer: s.footer.clone(),
        rows,
        objective: objective.map(|(_, o)| TabObjective { hearts: o.hearts }),
    }
}

pub const MAX_SIDEBAR_ROWS: usize = 15;

fn sidebar_objective(s: &State) -> Option<&String> {
    let team_objective = s.slots[TEAM_SLOT_BASE..]
        .iter()
        .any(Option::is_some)
        .then(|| {
            let team = s
                .team_of
                .get(&crate::client::bot::username())
                .and_then(|t| s.teams.get(t))?;
            s.slots[team.slot?].as_ref()
        })
        .flatten();
    team_objective.or(s.slots[SIDEBAR].as_ref())
}

fn build_sidebar(s: &State) -> Sidebar {
    let Some((objective, scores)) = sidebar_objective(s).and_then(|name| {
        let objective = s.objectives.get(name)?;
        Some((objective, s.scores.get(name)))
    }) else {
        return Sidebar::default();
    };

    let mut entries: Vec<(&String, &Score)> = scores
        .into_iter()
        .flatten()
        .filter(|(owner, _)| !owner.starts_with('#'))
        .collect();
    entries.sort_by_cached_key(|(owner, score)| {
        (std::cmp::Reverse(score.value), owner.to_lowercase())
    });
    entries.truncate(MAX_SIDEBAR_ROWS);

    let rows = entries
        .into_iter()
        .map(|(owner, score)| {
            let team = s.team_of.get(owner).and_then(|t| s.teams.get(t));
            let name = match &score.display {
                Some(display) => display.clone(),
                None => crate::text::parse_formatted(owner),
            };
            let format = score
                .format
                .as_ref()
                .or(objective.format.as_ref())
                .cloned()
                .unwrap_or_else(NumberFormat::sidebar_default);
            SidebarRow {
                name: format_for_team(team, name),
                score: format.format(score.value),
            }
        })
        .collect();

    Sidebar {
        active: true,
        title: objective.display_name.clone(),
        rows,
    }
}

fn display_name(s: &State, e: &Entry) -> Vec<Span> {
    let mut spans = match &e.display_name {
        Some(name) => name.clone(),
        None => {
            let team = s.team_of.get(&e.name).and_then(|t| s.teams.get(t));
            format_for_team(team, crate::text::styled(&e.name, Style::default()))
        }
    };
    if e.gamemode == Gamemode::Spectator {
        for span in &mut spans {
            span.style.italic = true;
        }
    }
    spans
}

fn format_for_team(team: Option<&Team>, name: Vec<Span>) -> Vec<Span> {
    let Some(team) = team else {
        return name;
    };
    let mut out = Vec::with_capacity(team.prefix.len() + name.len() + team.suffix.len());
    out.extend(team.prefix.iter().cloned());
    out.extend(name.into_iter().map(|mut span| {
        if let Some(color) = team.color
            && span.style.color == Style::default().color
        {
            span.style.color = color;
        }
        span
    }));
    out.extend(team.suffix.iter().cloned());
    out
}

#[derive(Clone, Default)]
pub(crate) struct NameTagText {
    pub name: Vec<Span>,
    pub below: Vec<Span>,
    pub hidden_by_team: bool,
}

pub(crate) struct Lookup<'a>(&'a State);

pub(crate) fn with_lookup<R>(f: impl FnOnce(&Lookup) -> R) -> R {
    let s = state().lock().unwrap();
    f(&Lookup(&*s))
}

impl Lookup<'_> {
    pub(crate) fn gamemode(&self, uuid: u128) -> Option<crate::session::Gamemode> {
        self.0.players.get(&uuid).map(|e| e.gamemode)
    }

    #[cfg(feature = "skins")]
    pub(crate) fn skin(&self, uuid: u128) -> (crate::client::skins::SkinRef, u8) {
        let default = match self.0.players.get(&uuid) {
            Some(e) => e.default_skin,
            None => default_skin(uuid),
        };
        (crate::client::skins::of(uuid), default)
    }

    pub(crate) fn name_tag(
        &self,
        scoreboard_name: &str,
        base: Vec<Span>,
        viewer: &str,
    ) -> NameTagText {
        name_tag(self.0, scoreboard_name, base, viewer)
    }
}

fn name_tag(s: &State, scoreboard_name: &str, base: Vec<Span>, viewer: &str) -> NameTagText {
    let mut hidden_by_team = false;
    let team_name = s.team_of.get(scoreboard_name);
    let team = team_name.and_then(|t| s.teams.get(t));
    if let Some(team) = team {
        let viewer_team = s.team_of.get(viewer);
        let allied = viewer_team.is_some() && viewer_team == team_name;
        match team.nametag_visibility {
            NameTagVisibility::Always => {}
            NameTagVisibility::Never => hidden_by_team = true,
            NameTagVisibility::HideForOtherTeams => {
                hidden_by_team = viewer_team.is_some() && !allied;
            }
            NameTagVisibility::HideForOwnTeam => hidden_by_team = allied,
        }
    }

    let below = match (&s.slots[BELOW_NAME], hidden_by_team) {
        (Some(name), false) => match s.objectives.get(name) {
            Some(objective) => {
                let score = s.scores.get(name).and_then(|m| m.get(scoreboard_name));
                let value = score.map(|s| s.value).unwrap_or(0);
                let format = score
                    .and_then(|s| s.format.as_ref())
                    .or(objective.format.as_ref())
                    .cloned()
                    .unwrap_or_else(NumberFormat::no_style);
                let mut below = format.format(value);
                below.push(Span {
                    text: " ".to_string(),
                    style: Style::default(),
                });
                below.extend(objective.display_name.iter().cloned());
                below
            }
            None => Vec::new(),
        },
        _ => Vec::new(),
    };

    NameTagText {
        name: format_for_team(team, base),
        below,
        hidden_by_team,
    }
}

fn is_upside_down_name(name: &str) -> bool {
    name == "Dinnerbone" || name == "Grumm"
}

fn default_skin(uuid: u128) -> u8 {
    let (hi, lo) = ((uuid >> 64) as u64, uuid as u64);
    let hilo = hi ^ lo;
    let hash = ((hilo >> 32) as u32 ^ hilo as u32) as i32;
    hash.rem_euclid(crate::gui::tablist::DEFAULT_SKINS.len() as i32) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_skin_matches_java_uuid_hash() {
        let uuid = 0x8667_ba71_b85a_4004_af54_457a_9734_eed7u128;
        let (hi, lo) = ((uuid >> 64) as u64, uuid as u64);
        let hilo = hi ^ lo;
        let expected = (((hilo >> 32) as u32 ^ hilo as u32) as i32).rem_euclid(18) as u8;
        assert_eq!(default_skin(uuid), expected);
        assert!((default_skin(uuid) as usize) < crate::gui::tablist::DEFAULT_SKINS.len());
    }

    #[test]
    fn team_formatting_wraps_the_name() {
        let mut s = State::default();
        s.teams.insert(
            "red".into(),
            Team {
                color: Some(0xFF5555),
                slot: team_slot(ChatFormatting::Red),
                prefix: vec![Span {
                    text: "[R] ".into(),
                    style: Style::colored(0xFF5555),
                }],
                suffix: Vec::new(),
                members: vec!["Alice".into()],
                nametag_visibility: NameTagVisibility::Always,
            },
        );
        s.team_of.insert("Alice".into(), "red".into());
        let e = Entry::new(1, "Alice".into());
        let spans = display_name(&s, &e);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[1].text, "Alice");
        assert_eq!(spans[1].style.color, 0xFF5555);

        let team = s.teams.get("red");
        let mixed = format_for_team(
            team,
            vec![
                Span {
                    text: "plain".into(),
                    style: Style::default(),
                },
                Span {
                    text: "gold".into(),
                    style: Style::colored(0xFFAA00),
                },
            ],
        );
        assert_eq!(mixed.len(), 3);
        assert_eq!(mixed[1].style.color, 0xFF5555);
        assert_eq!(mixed[2].style.color, 0xFFAA00);
    }

    #[test]
    fn spectators_are_italic() {
        let s = State::default();
        let mut e = Entry::new(1, "Bob".into());
        e.gamemode = Gamemode::Spectator;
        assert!(display_name(&s, &e).iter().all(|s| s.style.italic));
    }

    fn objective(s: &mut State, name: &str, format: Option<NumberFormat>) {
        s.objectives.insert(
            name.to_string(),
            Objective {
                hearts: false,
                format,
                display_name: crate::text::styled(name, Style::default()),
            },
        );
    }

    fn score(s: &mut State, objective: &str, owner: &str, value: i32) {
        s.scores.entry(objective.to_string()).or_default().insert(
            owner.to_string(),
            Score {
                value,
                format: None,
                display: None,
            },
        );
    }

    fn text_of(spans: &[Span]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn sidebar_sorts_filters_and_caps() {
        let mut s = State::default();
        objective(&mut s, "board", None);
        s.slots[SIDEBAR] = Some("board".to_string());
        for i in 0..16 {
            score(&mut s, "board", &format!("player{i}"), i);
        }
        score(&mut s, "board", "#bookkeeping", 999);

        let sidebar = build_sidebar(&s);
        assert_eq!(sidebar.rows.len(), MAX_SIDEBAR_ROWS);
        assert_eq!(text_of(&sidebar.rows[0].name), "player15");
        assert_eq!(text_of(&sidebar.rows[14].name), "player1");
        assert!(
            !sidebar
                .rows
                .iter()
                .any(|r| r.name.iter().any(|s| s.text.starts_with('#')))
        );
    }

    #[test]
    fn sidebar_breaks_ties_case_insensitively() {
        let mut s = State::default();
        objective(&mut s, "board", None);
        s.slots[SIDEBAR] = Some("board".to_string());
        score(&mut s, "board", "bob", 5);
        score(&mut s, "board", "Alice", 5);

        let rows = build_sidebar(&s).rows;
        assert_eq!(text_of(&rows[0].name), "Alice");
        assert_eq!(text_of(&rows[1].name), "bob");
    }

    #[test]
    fn a_team_colour_slot_beats_the_plain_sidebar_slot() {
        let mut s = State::default();
        objective(&mut s, "plain", None);
        objective(&mut s, "red_team_board", None);
        s.slots[SIDEBAR] = Some("plain".to_string());
        assert_eq!(sidebar_objective(&s), Some(&"plain".to_string()));

        s.slots[TEAM_SLOT_BASE + 12] = Some("red_team_board".to_string());
        assert_eq!(sidebar_objective(&s), Some(&"plain".to_string()));

        s.teams.insert(
            "reds".to_string(),
            Team {
                color: team_color(ChatFormatting::Red),
                slot: team_slot(ChatFormatting::Red),
                prefix: Vec::new(),
                suffix: Vec::new(),
                members: Vec::new(),
                nametag_visibility: NameTagVisibility::Always,
            },
        );
        s.team_of
            .insert(crate::client::bot::username(), "reds".to_string());
        assert_eq!(sidebar_objective(&s), Some(&"red_team_board".to_string()));
    }

    #[test]
    fn an_uncoloured_team_has_no_slot() {
        assert_eq!(team_slot(ChatFormatting::Reset), None);
        assert_eq!(team_slot(ChatFormatting::Bold), None);
        assert_eq!(team_slot(ChatFormatting::Black), Some(TEAM_SLOT_BASE));
        assert_eq!(team_slot(ChatFormatting::White), Some(TEAM_SLOT_BASE + 15));
    }

    #[test]
    fn sidebar_number_format_precedence() {
        let mut s = State::default();
        objective(&mut s, "board", None);
        s.slots[SIDEBAR] = Some("board".to_string());
        score(&mut s, "board", "player", 7);

        let red = crate::text::FORMAT_COLORS[12];
        let rows = build_sidebar(&s).rows;
        assert_eq!(text_of(&rows[0].score), "7");
        assert_eq!(rows[0].score[0].style.color, red);

        objective(
            &mut s,
            "board",
            Some(NumberFormat::Fixed(crate::text::styled(
                "obj",
                Style::default(),
            ))),
        );
        assert_eq!(text_of(&build_sidebar(&s).rows[0].score), "obj");

        s.scores
            .get_mut("board")
            .unwrap()
            .get_mut("player")
            .unwrap()
            .format = Some(NumberFormat::Blank);
        assert!(build_sidebar(&s).rows[0].score.is_empty());
    }

    #[test]
    fn a_score_display_name_replaces_the_holder() {
        let mut s = State::default();
        objective(&mut s, "board", None);
        s.slots[SIDEBAR] = Some("board".to_string());
        score(&mut s, "board", "player", 1);
        s.scores
            .get_mut("board")
            .unwrap()
            .get_mut("player")
            .unwrap()
            .display = Some(crate::text::styled("Shown", Style::default()));

        assert_eq!(text_of(&build_sidebar(&s).rows[0].name), "Shown");
    }

    #[test]
    fn no_sidebar_objective_is_inactive() {
        let s = State::default();
        assert!(!build_sidebar(&s).active);
    }

    #[test]
    fn a_backend_switch_empties_everything() {
        use azalea_protocol::packets::game::c_start_configuration::ClientboundStartConfiguration;

        let mut before = State::default();
        let mut lobby = Entry::new(7, "Lobbygoer".to_string());
        lobby.listed = true;
        before.players.insert(7, lobby);
        objective(&mut before, "board", None);
        before.slots[SIDEBAR] = Some("board".to_string());
        score(&mut before, "board", "Lobbygoer", 3);
        before
            .team_of
            .insert("Lobbygoer".to_string(), "lobby".to_string());
        before.header = crate::text::styled("Lobby", Style::default());

        assert_eq!(build(&before).rows.len(), 1);
        assert!(build_sidebar(&before).active);
        *state().lock().unwrap() = before;

        let shared: Arc<SharedMutex> = Arc::default();
        packet(
            &shared,
            &ClientboundGamePacket::StartConfiguration(ClientboundStartConfiguration),
        );

        let s = state().lock().unwrap();
        assert!(s.players.is_empty(), "the old server's players survived");
        assert!(s.objectives.is_empty() && s.scores.is_empty());
        assert!(s.team_of.is_empty() && s.teams.is_empty());
        assert!(s.header.is_empty() && s.footer.is_empty());
        assert!(s.slots.iter().all(Option::is_none));

        let published = shared.lock().unwrap();
        assert!(published.session.tab_list.rows.is_empty());
        assert!(!published.session.sidebar.active);
        assert!(published.session.player_names.is_empty());
    }
}
