use crate::gui::focus;
use crate::gui::menu;
use crate::gui::multiplayer::arrow_select;
use crate::gui::options::{draw_scrollbar, row_visible, scroll_masked};
use crate::gui::painter::Painter;
use crate::gui::tooltip::{label, translate};
use crate::gui::widgets::{
    Button, FOOTER_GAP, FOOTER_W, HeaderFooter, TextBox, WIDGET_HEIGHT, centered_row_x,
};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::text::LINE_HEIGHT;

use super::flat::scroll_on_select;

pub(super) const BIOMES: [&str; 66] = [
    "badlands",
    "bamboo_jungle",
    "basalt_deltas",
    "beach",
    "birch_forest",
    "cherry_grove",
    "cold_ocean",
    "crimson_forest",
    "dark_forest",
    "deep_cold_ocean",
    "deep_dark",
    "deep_frozen_ocean",
    "deep_lukewarm_ocean",
    "deep_ocean",
    "desert",
    "dripstone_caves",
    "end_barrens",
    "end_highlands",
    "end_midlands",
    "eroded_badlands",
    "flower_forest",
    "forest",
    "frozen_ocean",
    "frozen_peaks",
    "frozen_river",
    "grove",
    "ice_spikes",
    "jagged_peaks",
    "jungle",
    "lukewarm_ocean",
    "lush_caves",
    "mangrove_swamp",
    "meadow",
    "mushroom_fields",
    "nether_wastes",
    "ocean",
    "old_growth_birch_forest",
    "old_growth_pine_taiga",
    "old_growth_spruce_taiga",
    "pale_garden",
    "plains",
    "river",
    "savanna",
    "savanna_plateau",
    "small_end_islands",
    "snowy_beach",
    "snowy_plains",
    "snowy_slopes",
    "snowy_taiga",
    "soul_sand_valley",
    "sparse_jungle",
    "stony_peaks",
    "stony_shore",
    "sulfur_caves",
    "sunflower_plains",
    "swamp",
    "taiga",
    "the_end",
    "the_void",
    "warm_ocean",
    "warped_forest",
    "windswept_forest",
    "windswept_gravelly_hills",
    "windswept_hills",
    "windswept_savanna",
    "wooded_badlands",
];

pub(super) fn biome_by_id(id: &str) -> Option<&'static str> {
    let path = match id.split_once(':') {
        Some(("" | "minecraft", path)) => path,
        Some(_) => return None,
        None => id,
    };
    BIOMES.iter().copied().find(|b| *b == path)
}

const SPACING: f32 = 3.0;
const SEARCH_H: f32 = 15.0;
const SEARCH_W: f32 = 200.0;
const SEARCH_MAX_LEN: usize = 32;
const HEADER_H: f32 = 13.0 + LINE_HEIGHT + SPACING + SEARCH_H;
const LAYOUT: HeaderFooter = HeaderFooter::new(HEADER_H, HeaderFooter::DEFAULT_H);
const TITLE_Y: f32 = ((HEADER_H - (LINE_HEIGHT + SPACING + SEARCH_H)) / 2.0).round();
const SEARCH_Y: f32 = TITLE_Y + LINE_HEIGHT + SPACING;

const ROW_H: f32 = 15.0;
const ROW_W: f32 = 220.0;
const LIST_PAD: f32 = 2.0;

#[derive(Default)]
pub struct State {
    search: TextBox,
    filter: String,
    rows: Vec<(&'static str, String)>,
    biome: String,
    selected: Option<&'static str>,
    done_active: bool,
    scroll: f32,
    scroll_drag: Option<f32>,
    scroll_pending: bool,
    pub result: Option<String>,
}

impl State {
    pub fn open(&mut self, current: &str) {
        let biome = if current.is_empty() {
            "minecraft:plains".to_owned()
        } else {
            biome_by_id(current).map_or_else(|| current.to_owned(), |b| format!("minecraft:{b}"))
        };
        let rows = sorted_rows();
        let selected = biome_by_id(&biome).filter(|b| rows.iter().any(|(id, _)| id == b));
        *self = State {
            search: TextBox::bordered(
                SEARCH_MAX_LEN,
                &translate("createWorld.customize.buffet.search", &[]),
            ),
            filter: String::new(),
            rows,
            biome,
            selected,
            done_active: selected.is_some(),
            scroll: 0.0,
            scroll_drag: None,
            scroll_pending: true,
            result: None,
        };
    }
}

fn sorted_rows() -> Vec<(&'static str, String)> {
    let mut rows: Vec<(&'static str, String)> = BIOMES
        .iter()
        .map(|id| {
            let key = format!("biome.minecraft.{id}");
            let name = translate(&key, &[]);
            let name = if name == key {
                format!("minecraft:{id}")
            } else {
                name
            };
            (*id, name)
        })
        .collect();
    rows.sort_by_cached_key(|(_, name)| (name.to_lowercase(), name.clone()));
    rows
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    menu::background(p, ctx.vw, ctx.vh);
    let st = &mut state.worlds.buffet;

    p.text_centered(
        label!("createWorld.customize.buffet.title"),
        ctx.vw / 2.0,
        TITLE_Y,
        0xFF_FFFF,
        true,
    );
    st.search.update(
        p,
        ctx,
        centered_row_x(ctx.vw, SEARCH_W),
        SEARCH_Y,
        SEARCH_W,
        SEARCH_H,
    );
    if st.search.text != st.filter {
        st.filter.clone_from(&st.search.text);
        st.selected = None;
    }

    let needle = st.filter.to_lowercase();
    let shown: Vec<(&'static str, &str)> = st
        .rows
        .iter()
        .filter(|(_, name)| needle.is_empty() || name.to_lowercase().contains(&needle))
        .map(|(id, name)| (*id, name.as_str()))
        .collect();

    let (lx, ly, lw, lh) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let left = ((ctx.vw / 2.0).floor() - ROW_W / 2.0).floor();
    let content_h = shown.len() as f32 * ROW_H + 2.0 * LIST_PAD;
    let selected_index =
        |sel: Option<&'static str>| sel.and_then(|s| shown.iter().position(|(id, _)| *id == s));
    if std::mem::take(&mut st.scroll_pending)
        && let Some(i) = selected_index(st.selected)
    {
        scroll_on_select(
            &mut st.scroll,
            i as f32 * ROW_H,
            ROW_H,
            lh,
            content_h,
            false,
        );
    }

    let input = scroll_masked(
        &mut st.scroll,
        &mut st.scroll_drag,
        ctx,
        left,
        ROW_W,
        (lx, ly, lw, lh),
        content_h,
    );
    menu::list_background(p, lx, ly, lw, lh, st.scroll);
    let list_ctx = ctx.with_input(&input);

    let list_focused = focus::next(!shown.is_empty());
    let mut clicked = None;
    let top = (ly + LIST_PAD - st.scroll.trunc()).floor();
    let clip = p.push_clip(lx, ly, lw, lh);
    for (i, (id, name)) in shown.iter().enumerate() {
        let y = top + i as f32 * ROW_H;
        if !row_visible(y, ROW_H, ly, lh) {
            continue;
        }
        if st.selected == Some(*id) {
            let border = if list_focused {
                0xFFFF_FFFF
            } else {
                0xFF80_8080
            };
            p.fill(left, y, ROW_W, ROW_H, border);
            p.fill(left + 1.0, y + 1.0, ROW_W - 2.0, ROW_H - 2.0, 0xFF00_0000);
        }
        p.text_plain(
            name,
            left + LIST_PAD + 5.0,
            y + LIST_PAD + 2.0,
            0xFF_FFFF,
            true,
        );
        if list_ctx.input.left_click && list_ctx.hovering(left, y, ROW_W, ROW_H) {
            clicked = Some(i);
            focus::claim();
        }
    }
    p.pop_clip(clip);
    draw_scrollbar(p, st.scroll, left, ROW_W, ly, lh, content_h);

    let mut keyboard = false;
    let mut pick = clicked;
    if pick.is_none() && list_focused && (ctx.input.up_arrow || ctx.input.down_arrow) {
        let mut index = selected_index(st.selected);
        arrow_select(
            &mut index,
            shown.len(),
            ctx.input.up_arrow,
            ctx.input.down_arrow,
        );
        pick = index;
        keyboard = true;
    }
    if let Some(i) = pick
        && let Some((id, _)) = shown.get(i)
    {
        st.selected = Some(*id);
        st.biome = format!("minecraft:{id}");
        st.done_active = true;
        scroll_on_select(
            &mut st.scroll,
            i as f32 * ROW_H,
            ROW_H,
            lh,
            content_h,
            keyboard,
        );
    }

    let y = LAYOUT.footer_y(ctx.vh);
    let x0 = centered_row_x(ctx.vw, 2.0 * FOOTER_W + FOOTER_GAP);
    let done = Button {
        active: st.done_active,
        ..Button::new(x0, y, FOOTER_W, WIDGET_HEIGHT, label!("gui.done"))
    }
    .draw(p, ctx);
    let cancel = Button::new(
        x0 + FOOTER_W + FOOTER_GAP,
        y,
        FOOTER_W,
        WIDGET_HEIGHT,
        label!("gui.cancel"),
    )
    .draw(p, ctx);

    if done {
        st.result = Some(st.biome.clone());
        state.nav = Some(Screen::CreateWorld);
    } else if cancel || ctx.input.escape {
        st.result = None;
        state.nav = Some(Screen::CreateWorld);
    }
}
