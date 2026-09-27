use crate::client::tablist::TabList;
use crate::gui::painter::Painter;
use crate::gui::tablist::DEFAULT_SKINS;
use crate::gui::{ScreenCtx, screens::HotbarGeom};
use crate::text::{Span, Style, styled};

const FADE_OUT_DELAY: f32 = 5.0;
const FADE_OUT_TIME: f32 = 2.0;
const ITEMS_PER_PAGE: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
enum Category {
    #[default]
    Root,
    Players {
        team: Option<String>,
    },
    Teams,
}

enum Item {
    Category(Category),
    Player {
        uuid: u128,
        name: Vec<Span>,
        skin: u8,
        show_hat: bool,
        upside_down: bool,
    },
}

fn category_prompt(category: &Category) -> Vec<Span> {
    let text = match category {
        Category::Root => "Teleport to a player or team",
        Category::Players { .. } => "Select a player",
        Category::Teams => "Select a team",
    };
    styled(text, Style::default())
}

fn item_name(item: &Item) -> Vec<Span> {
    match item {
        Item::Category(Category::Players { .. }) => styled("Teleport to Player", Style::default()),
        Item::Category(Category::Teams) => styled("Teleport to Team", Style::default()),
        Item::Category(Category::Root) => Vec::new(),
        Item::Player { name, .. } => name.clone(),
    }
}

fn items_for(category: &Category, tab_list: &TabList) -> Vec<Item> {
    match category {
        Category::Root => vec![
            Item::Category(Category::Players { team: None }),
            Item::Category(Category::Teams),
        ],
        Category::Players { team } => tab_list
            .rows
            .iter()
            .filter(|r| !r.spectator)
            .filter(|r| team.is_none() || r.team == *team)
            .map(|r| Item::Player {
                uuid: r.uuid,
                name: r.name.clone(),
                skin: r.skin,
                show_hat: r.show_hat,
                upside_down: r.upside_down,
            })
            .collect(),
        Category::Teams => {
            let mut seen: Vec<String> = Vec::new();
            for row in tab_list.rows.iter().filter(|r| !r.spectator) {
                if let Some(team) = &row.team
                    && !seen.iter().any(|t| t == team)
                {
                    seen.push(team.clone());
                }
            }
            seen.into_iter()
                .map(|team| Item::Category(Category::Players { team: Some(team) }))
                .collect()
        }
    }
}

#[derive(Default)]
pub struct SpectatorMenuState {
    open: bool,
    category: Category,
    selected: Option<u8>,
    page: usize,
    last_selection: f32,
}

impl SpectatorMenuState {
    const CLOSE_SLOT: u8 = 8;

    fn page_items<'a>(&self, items: &'a [Item]) -> &'a [Item] {
        let start = (self.page * ITEMS_PER_PAGE).min(items.len());
        let end = (start + ITEMS_PER_PAGE).min(items.len());
        &items[start..end]
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    fn open_at_root(&mut self, now: f32) {
        self.open = true;
        self.category = Category::Root;
        self.page = 0;
        self.selected = None;
        self.last_selection = now;
    }

    fn select_slot(&mut self, slot: u8, tab_list: &TabList) -> Option<u128> {
        let items = items_for(&self.category, tab_list);
        let activate = self.selected == Some(slot);
        self.selected = Some(slot);
        if !activate {
            return None;
        }
        match slot {
            0 if self.page > 0 => {
                self.page -= 1;
                self.selected = None;
                None
            }
            7 if (self.page + 1) * ITEMS_PER_PAGE < items.len() => {
                self.page += 1;
                self.selected = None;
                None
            }
            Self::CLOSE_SLOT => {
                self.open = false;
                None
            }
            1..=6 => match self.page_items(&items).get(slot as usize - 1) {
                Some(Item::Category(category)) => {
                    self.category = category.clone();
                    self.page = 0;
                    self.selected = None;
                    None
                }
                Some(Item::Player { uuid, .. }) => Some(*uuid),
                None => None,
            },
            _ => None,
        }
    }

    pub fn on_hotbar_selected(&mut self, slot: u8, tab_list: &TabList, now: f32) -> Option<u128> {
        if !self.open {
            self.open_at_root(now);
            return None;
        }
        self.last_selection = now;
        self.select_slot(slot, tab_list)
    }

    pub fn on_action_key(&mut self, tab_list: &TabList, now: f32) -> Option<u128> {
        if !self.open {
            self.open_at_root(now);
            return None;
        }
        self.last_selection = now;
        match self.selected {
            Some(slot) => self.select_slot(slot, tab_list),
            None => None,
        }
    }

    pub fn on_scroll(&mut self, notches: i32, tab_list: &TabList, now: f32) {
        if !self.open || notches == 0 {
            return;
        }
        self.last_selection = now;
        let items = items_for(&self.category, tab_list);
        let page = self.page_items(&items);
        let can_scroll_left = self.page > 0;
        let can_scroll_right = (self.page + 1) * ITEMS_PER_PAGE < items.len();
        let enabled = |slot: i32| -> bool {
            match slot {
                0 => can_scroll_left,
                7 => can_scroll_right,
                8 => true,
                1..=6 => page.get(slot as usize - 1).is_some(),
                _ => false,
            }
        };
        let step = notches.signum();
        let mut next = self.selected.map_or(-1, |s| s as i32);
        for _ in 0..9 {
            next += step;
            if !(0..=8).contains(&next) {
                break;
            }
            if enabled(next) {
                self.selected = Some(next as u8);
                return;
            }
        }
    }

    fn alpha(&mut self, now: f32) -> f32 {
        if !self.open {
            return 0.0;
        }
        let alpha = ((self.last_selection + FADE_OUT_DELAY + FADE_OUT_TIME - now) / FADE_OUT_TIME)
            .clamp(0.0, 1.0);
        if alpha <= 0.0 {
            self.open = false;
        }
        alpha
    }
}

pub fn draw(
    p: &mut Painter,
    ctx: &ScreenCtx,
    state: &mut SpectatorMenuState,
    tab_list: &TabList,
    #[cfg(feature = "skins")] faces: &mut crate::gui::player_faces::PlayerFaces,
) {
    let now = ctx.input.time;
    let alpha = state.alpha(now);
    if alpha <= 0.0 {
        return;
    }
    let (vw, vh) = (ctx.vw, ctx.vh);
    let geom = HotbarGeom::new(vw, vh);
    let top = geom.top() + (1.0 - alpha) * 22.0;

    p.sprite("hud/hotbar", geom.left(), top, 182.0, 22.0);

    let items = items_for(&state.category, tab_list);
    let page = state.page_items(&items);
    let can_scroll_left = state.page > 0;
    let can_scroll_right = (state.page + 1) * ITEMS_PER_PAGE < items.len();

    if let Some(slot) = state.selected {
        p.sprite(
            "hud/hotbar_selection",
            geom.item_x(slot as usize) - 4.0,
            top - 1.0,
            24.0,
            23.0,
        );
    }

    let icon_y = top + 3.0;
    for slot in 0..9usize {
        let x = geom.item_x(slot);
        match slot {
            0 => {
                if can_scroll_left {
                    p.sprite("spectator/scroll_left", x, icon_y, 16.0, 16.0);
                }
            }
            7 => {
                if can_scroll_right {
                    p.sprite("spectator/scroll_right", x, icon_y, 16.0, 16.0);
                }
            }
            8 => p.sprite("spectator/close", x, icon_y, 16.0, 16.0),
            _ => match page.get(slot - 1) {
                Some(Item::Player {
                    uuid,
                    skin,
                    show_hat,
                    upside_down,
                    ..
                }) => {
                    #[cfg(feature = "skins")]
                    let drawn = match faces
                        .slot_of(*uuid)
                        .and_then(|slot| p.atlas.player_face(slot))
                    {
                        Some((face, hat)) => {
                            p.atlas_region_flipped(face, x, icon_y, 16.0, 16.0, *upside_down);
                            if *show_hat {
                                p.atlas_region_flipped(hat, x, icon_y, 16.0, 16.0, *upside_down);
                            }
                            true
                        }
                        None => false,
                    };
                    #[cfg(not(feature = "skins"))]
                    let drawn = false;
                    if !drawn {
                        let skin = &DEFAULT_SKINS[(*skin as usize).min(DEFAULT_SKINS.len() - 1)];
                        p.skin_face(
                            skin.face,
                            show_hat.then_some(skin.hat),
                            x,
                            icon_y,
                            16.0,
                            *upside_down,
                        );
                    }
                }
                Some(Item::Category(Category::Players { .. })) => {
                    p.sprite("spectator/teleport_to_player", x, icon_y, 16.0, 16.0);
                }
                Some(Item::Category(Category::Teams)) => {
                    p.sprite("spectator/teleport_to_team", x, icon_y, 16.0, 16.0);
                }
                Some(Item::Category(Category::Root)) | None => {}
            },
        }
    }

    let text = match state.selected.and_then(|s| {
        (1..=6)
            .contains(&s)
            .then(|| page.get(s as usize - 1))
            .flatten()
    }) {
        Some(item) => item_name(item),
        None => category_prompt(&state.category),
    };
    if !text.is_empty() {
        let width = p.atlas.font.width(&text).ceil();
        let x = ((vw - width) / 2.0).floor();
        p.text_faded(&text, x, vh - 35.0, true, alpha);
    }
}
