use std::str::FromStr;

use azalea_registry::builtin::BlockKind;

use crate::gui::focus;
use crate::gui::menu;
use crate::gui::multiplayer::arrow_select;
use crate::gui::options::{draw_scrollbar, row_visible, scroll_masked};
use crate::gui::painter::Painter;
use crate::gui::tooltip::{label, styled_hover_name, translate};
use crate::gui::widgets::{
    Button, FOOTER_GAP, FOOTER_W, HeaderFooter, TextBox, WIDGET_HEIGHT, centered_row_x,
};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::items::placed::block_item;
use crate::session::SlotStack;
use crate::text::{LINE_HEIGHT, Span, Style};

use super::buffet::biome_by_id;

const Y_SIZE: i32 = (1 << 12) - 32;
const DEFAULT_BIOME: &str = "plains";

struct Preset {
    id: &'static str,
    icon: &'static str,
    layers: &'static [(i32, &'static str)],
    biome: &'static str,
    features: bool,
    lakes: bool,
    structure_overrides: &'static [&'static str],
}

const PRESETS: [Preset; 9] = [
    Preset {
        id: "classic_flat",
        icon: "grass_block",
        layers: &[(1, "bedrock"), (2, "dirt"), (1, "grass_block")],
        biome: "plains",
        features: false,
        lakes: false,
        structure_overrides: &["minecraft:villages"],
    },
    Preset {
        id: "tunnelers_dream",
        icon: "stone",
        layers: &[
            (1, "bedrock"),
            (230, "stone"),
            (5, "dirt"),
            (1, "grass_block"),
        ],
        biome: "windswept_hills",
        features: true,
        lakes: false,
        structure_overrides: &["minecraft:mineshafts", "minecraft:strongholds"],
    },
    Preset {
        id: "water_world",
        icon: "water_bucket",
        layers: &[
            (1, "bedrock"),
            (64, "deepslate"),
            (5, "stone"),
            (5, "dirt"),
            (5, "gravel"),
            (90, "water"),
        ],
        biome: "deep_ocean",
        features: false,
        lakes: false,
        structure_overrides: &[
            "minecraft:ocean_ruins",
            "minecraft:shipwrecks",
            "minecraft:ocean_monuments",
        ],
    },
    Preset {
        id: "overworld",
        icon: "short_grass",
        layers: &[
            (1, "bedrock"),
            (59, "stone"),
            (3, "dirt"),
            (1, "grass_block"),
        ],
        biome: "plains",
        features: true,
        lakes: true,
        structure_overrides: &[
            "minecraft:villages",
            "minecraft:mineshafts",
            "minecraft:pillager_outposts",
            "minecraft:ruined_portals",
            "minecraft:strongholds",
        ],
    },
    Preset {
        id: "snowy_kingdom",
        icon: "snow",
        layers: &[
            (1, "bedrock"),
            (59, "stone"),
            (3, "dirt"),
            (1, "grass_block"),
            (1, "snow"),
        ],
        biome: "snowy_plains",
        features: false,
        lakes: false,
        structure_overrides: &["minecraft:villages", "minecraft:igloos"],
    },
    Preset {
        id: "bottomless_pit",
        icon: "feather",
        layers: &[(2, "cobblestone"), (3, "dirt"), (1, "grass_block")],
        biome: "plains",
        features: false,
        lakes: false,
        structure_overrides: &["minecraft:villages"],
    },
    Preset {
        id: "desert",
        icon: "sand",
        layers: &[(1, "bedrock"), (3, "stone"), (52, "sandstone"), (8, "sand")],
        biome: "desert",
        features: true,
        lakes: false,
        structure_overrides: &[
            "minecraft:villages",
            "minecraft:desert_pyramids",
            "minecraft:mineshafts",
            "minecraft:strongholds",
        ],
    },
    Preset {
        id: "redstone_ready",
        icon: "redstone",
        layers: &[(1, "bedrock"), (3, "stone"), (116, "sandstone")],
        biome: "desert",
        features: false,
        lakes: false,
        structure_overrides: &[],
    },
    Preset {
        id: "the_void",
        icon: "barrier",
        layers: &[(1, "air")],
        biome: "the_void",
        features: true,
        lakes: false,
        structure_overrides: &[],
    },
];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extras {
    pub structure_overrides: Option<Vec<&'static str>>,
    pub features: bool,
    pub lakes: bool,
}

#[derive(Clone, Copy)]
struct Layer {
    height: i32,
    block: BlockKind,
}

impl Layer {
    fn display_item(self) -> Option<&'static str> {
        let bare = |id: &'static str| id.strip_prefix("minecraft:").unwrap_or(id);
        match block_item(self.block) {
            Some(item) => Some(bare(item.to_str())),
            None if self.block == BlockKind::Water => Some("water_bucket"),
            None if self.block == BlockKind::Lava => Some("lava_bucket"),
            None => None,
        }
    }
}

#[derive(Clone)]
struct Settings {
    layers: Vec<Layer>,
    biome: &'static str,
    extras: Extras,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            layers: vec![
                Layer {
                    height: 1,
                    block: BlockKind::Bedrock,
                },
                Layer {
                    height: 2,
                    block: BlockKind::Dirt,
                },
                Layer {
                    height: 1,
                    block: BlockKind::GrassBlock,
                },
            ],
            biome: DEFAULT_BIOME,
            extras: Extras {
                structure_overrides: Some(vec!["minecraft:strongholds", "minecraft:villages"]),
                features: false,
                lakes: false,
            },
        }
    }
}

impl Settings {
    fn of_preset(preset: &Preset) -> Settings {
        Settings {
            layers: preset
                .layers
                .iter()
                .filter_map(|&(height, id)| {
                    BlockKind::from_str(id)
                        .ok()
                        .map(|block| Layer { height, block })
                })
                .collect(),
            biome: biome_by_id(preset.biome).unwrap_or(DEFAULT_BIOME),
            extras: Extras {
                structure_overrides: Some(preset.structure_overrides.to_vec()),
                features: preset.features,
                lakes: preset.lakes,
            },
        }
    }

    fn save(&self) -> String {
        let mut out = String::new();
        for (i, layer) in self.layers.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            if layer.height != 1 {
                out.push_str(&format!("{}*", layer.height));
            }
            out.push_str(layer.block.to_str());
        }
        out.push_str(";minecraft:");
        out.push_str(self.biome);
        out
    }

    fn read_share(&self, definition: &str) -> Settings {
        let mut parts = definition.split(';');
        let layers = layers_from_string(parts.next().unwrap_or(""));
        if layers.is_empty() {
            return Settings::default();
        }
        let biome = match parts.next() {
            None => DEFAULT_BIOME,
            Some(name) => biome_by_id(name).unwrap_or_else(|| {
                crate::log_warn!("worlds", "Invalid biome: {name}");
                DEFAULT_BIOME
            }),
        };
        Settings {
            layers,
            biome,
            extras: self.extras.clone(),
        }
    }
}

fn layers_from_string(input: &str) -> Vec<Layer> {
    let mut depths: Vec<&str> = input.split(',').collect();
    while depths.last() == Some(&"") {
        depths.pop();
    }
    let mut result = Vec::new();
    let mut first_free = 0i32;
    for depth in depths {
        let Some(layer) = layer_from_string(depth, first_free) else {
            return Vec::new();
        };
        let max_height = Y_SIZE - first_free;
        if max_height > 0 {
            result.push(Layer {
                height: layer.height.min(max_height),
                ..layer
            });
            first_free = first_free.wrapping_add(layer.height);
        }
    }
    result
}

fn layer_from_string(input: &str, first_free: i32) -> Option<Layer> {
    let (height, block_id) = match input.split_once('*') {
        Some((n, id)) => (n.parse::<i32>().ok()?.max(0), id),
        None => (1, input),
    };
    let first_above = first_free.wrapping_add(height).min(Y_SIZE);
    let block = block_by_id(block_id)?;
    Some(Layer {
        height: first_above - first_free,
        block,
    })
}

fn block_by_id(id: &str) -> Option<BlockKind> {
    let path = match id.split_once(':') {
        Some(("" | "minecraft", path)) => path,
        Some(_) => return None,
        None => id,
    };
    if path.contains(':') {
        return None;
    }
    BlockKind::from_str(path).ok()
}

const LAYOUT: HeaderFooter = HeaderFooter::new(HeaderFooter::DEFAULT_H, 64.0);
const FOOTER_ROW_GAP: f32 = 4.0;
const ROW_W: f32 = 220.0;
const LIST_PAD: f32 = 2.0;
const ROW_H: f32 = 24.0;
const HEADER_ROW_H: f32 = 13.0;
const SLOT: f32 = 18.0;
const NAME_X: f32 = SLOT + 5.0;

const PRESETS_TITLE_Y: f32 = 8.0;
const SHARE_LABEL: (f32, f32) = (51.0, 30.0);
const LIST_LABEL: (f32, f32) = (51.0, 68.0);
const EXPORT_X: f32 = 50.0;
const EXPORT_Y: f32 = 40.0;
const EXPORT_MAX_LEN: usize = 1230;
const PRESETS_LIST_Y: f32 = 80.0;
const PRESETS_LIST_INSET: f32 = 117.0;
const LABEL_GRAY: u32 = 0xA0_A0A0;
const PRESETS_BUTTONS_FROM_BOTTOM: f32 = 28.0;

struct Presets {
    export: TextBox,
    settings: Settings,
    selected: Option<usize>,
    select_active: bool,
    scroll: f32,
    scroll_drag: Option<f32>,
}

#[derive(Default)]
pub struct State {
    settings: Settings,
    selected: Option<usize>,
    scroll: f32,
    scroll_drag: Option<f32>,
    presets: Option<Presets>,
    pub result: Option<String>,
    pub extras: Extras,
}

impl State {
    pub fn open(&mut self, current: &str) {
        *self = State {
            settings: Settings::default().read_share(current),
            ..State::default()
        };
    }
}

pub(super) fn scroll_to_entry(scroll: &mut f32, rel: f32, h: f32, list_h: f32, content_h: f32) {
    let max = (content_h - list_h).max(0.0);
    let top_delta = rel - scroll.trunc();
    if top_delta < 0.0 {
        *scroll = (*scroll + top_delta).clamp(0.0, max);
    }
    let bottom_delta = list_h - (LIST_PAD + rel - scroll.trunc()) - h - LIST_PAD;
    if bottom_delta < 0.0 {
        *scroll = (*scroll - bottom_delta).clamp(0.0, max);
    }
}

pub(super) fn scroll_on_select(
    scroll: &mut f32,
    rel: f32,
    h: f32,
    list_h: f32,
    content_h: f32,
    keyboard: bool,
) {
    let y = LIST_PAD + rel - scroll.trunc();
    let clipped = y + LIST_PAD < 0.0 || y + h - LIST_PAD > list_h;
    if keyboard || clipped {
        scroll_to_entry(scroll, rel, h, list_h, content_h);
    }
}

fn draw_selection(p: &mut Painter, x: f32, y: f32, h: f32, focused: bool) {
    let border = if focused { 0xFFFF_FFFF } else { 0xFF80_8080 };
    p.fill(x, y, ROW_W, h, border);
    p.fill(x + 1.0, y + 1.0, ROW_W - 2.0, h - 2.0, 0xFF00_0000);
}

fn draw_slot(p: &mut Painter, cx: f32, cy: f32, item: Option<&str>) {
    p.sprite("container/slot", cx + 1.0, cy + 1.0, SLOT, SLOT);
    if let Some(item) = item {
        p.item_icon(item, cx + 2.0, cy + 2.0);
    }
}

fn hover_name(item: &'static str) -> String {
    let stack = SlotStack {
        item,
        ..SlotStack::default()
    };
    styled_hover_name(&stack)
        .into_iter()
        .map(|span| span.text)
        .collect()
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    menu::background(p, ctx.vw, ctx.vh);
    let st = &mut state.worlds.flat;
    if st.presets.is_some() {
        draw_presets(p, st, ctx);
        return;
    }

    p.text_centered(
        label!("createWorld.customize.flat.title"),
        ctx.vw / 2.0,
        LAYOUT.title_y(),
        0xFF_FFFF,
        true,
    );

    let count = st.settings.layers.len();
    let row_top = |i: usize| {
        if i == 0 {
            0.0
        } else {
            HEADER_ROW_H + (i - 1) as f32 * ROW_H
        }
    };
    let row_h = |i: usize| if i == 0 { HEADER_ROW_H } else { ROW_H };
    let (lx, ly, lw, lh) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let left = ((ctx.vw / 2.0).floor() - ROW_W / 2.0).floor();
    let content_h = HEADER_ROW_H + count as f32 * ROW_H + 2.0 * LIST_PAD;

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

    let list_focused = focus::next(true);
    let mut clicked = None;
    let top = (ly + LIST_PAD - st.scroll.trunc()).floor();
    let (cx, content_right) = (left + LIST_PAD, left + ROW_W - LIST_PAD);
    let clip = p.push_clip(lx, ly, lw, lh);
    for i in 0..=count {
        let (y, h) = (top + row_top(i), row_h(i));
        if !row_visible(y, h, ly, lh) {
            continue;
        }
        if st.selected == Some(i) {
            draw_selection(p, left, y, h, list_focused);
        }
        let cy = y + LIST_PAD;
        if i == 0 {
            let underlined = |key: &str| Span {
                text: translate(key, &[]),
                style: Style {
                    underline: true,
                    ..Style::default()
                },
            };
            let tile = [underlined("createWorld.customize.flat.tile")];
            let height = [underlined("createWorld.customize.flat.height")];
            p.text(&tile, cx, cy, true);
            let w = p.atlas.font.width(&height);
            p.text(&height, content_right - w, cy, true);
        } else {
            let layer = st.settings.layers[count - i];
            let item = layer.display_item();
            draw_slot(p, cx, cy, item);
            let text_y =
                cy + ((ROW_H - 2.0 * LIST_PAD) / 2.0).floor() - (LINE_HEIGHT / 2.0).floor();
            p.text_plain(
                &hover_name(item.unwrap_or("air")),
                cx + NAME_X,
                text_y,
                0xFF_FFFF,
                true,
            );
            let key = if i == 1 {
                "createWorld.customize.flat.layer.top"
            } else if i == count {
                "createWorld.customize.flat.layer.bottom"
            } else {
                "createWorld.customize.flat.layer"
            };
            let height = translate(key, &[layer.height.to_string()]);
            let w = p.atlas.font.width_str(&height);
            p.text_plain(&height, content_right - w, text_y, 0xFF_FFFF, true);
        }
        if list_ctx.input.left_click && list_ctx.hovering(left, y, ROW_W, h) {
            clicked = Some(i);
            focus::claim();
        }
    }
    p.pop_clip(clip);
    draw_scrollbar(p, st.scroll, left, ROW_W, ly, lh, content_h);

    let mut keyboard = false;
    if clicked.is_some() {
        st.selected = clicked;
    } else if list_focused && (ctx.input.up_arrow || ctx.input.down_arrow) {
        arrow_select(
            &mut st.selected,
            count + 1,
            ctx.input.up_arrow,
            ctx.input.down_arrow,
        );
        keyboard = true;
    }
    if (clicked.is_some() || keyboard)
        && let Some(i) = st.selected
    {
        scroll_on_select(
            &mut st.scroll,
            row_top(i),
            row_h(i),
            lh,
            content_h,
            keyboard,
        );
    }

    let rows_h = 2.0 * WIDGET_HEIGHT + FOOTER_ROW_GAP;
    let y0 = ctx.vh - LAYOUT.footer_h + ((LAYOUT.footer_h - rows_h) / 2.0).round();
    let y1 = y0 + WIDGET_HEIGHT + FOOTER_ROW_GAP;
    let x0 = centered_row_x(ctx.vw, 2.0 * FOOTER_W + FOOTER_GAP);
    let x1 = x0 + FOOTER_W + FOOTER_GAP;
    let remove = Button {
        active: st.selected.is_some_and(|i| i > 0),
        ..Button::new(
            x0,
            y0,
            FOOTER_W,
            WIDGET_HEIGHT,
            label!("createWorld.customize.flat.removeLayer"),
        )
    }
    .draw(p, ctx);
    let presets = Button::new(
        x1,
        y0,
        FOOTER_W,
        WIDGET_HEIGHT,
        label!("createWorld.customize.presets"),
    )
    .draw(p, ctx);
    let done = Button::new(x0, y1, FOOTER_W, WIDGET_HEIGHT, label!("gui.done")).draw(p, ctx);
    let cancel = Button::new(x1, y1, FOOTER_W, WIDGET_HEIGHT, label!("gui.cancel")).draw(p, ctx);

    if remove && let Some(i) = st.selected.filter(|i| *i > 0) {
        st.settings.layers.remove(count - i);
        let left_over = st.settings.layers.len();
        st.selected = (left_over > 0).then(|| i.min(left_over));
    } else if presets {
        let mut export = TextBox::bordered(EXPORT_MAX_LEN, "");
        export.set_text(&st.settings.save());
        st.presets = Some(Presets {
            select_active: export.text.chars().count() > 1,
            export,
            settings: st.settings.clone(),
            selected: None,
            scroll: 0.0,
            scroll_drag: None,
        });
        focus::clear();
    } else if done {
        st.result = Some(st.settings.save());
        st.extras = st.settings.extras.clone();
        state.nav = Some(Screen::CreateWorld);
    } else if cancel || ctx.input.escape {
        st.result = None;
        state.nav = Some(Screen::CreateWorld);
    }
}

fn draw_presets(p: &mut Painter, st: &mut State, ctx: &ScreenCtx) {
    let Some(ps) = st.presets.as_mut() else {
        return;
    };
    p.text_centered(
        label!("createWorld.customize.presets.title"),
        ctx.vw / 2.0,
        PRESETS_TITLE_Y,
        0xFF_FFFF,
        true,
    );
    p.text_plain(
        label!("createWorld.customize.presets.share"),
        SHARE_LABEL.0,
        SHARE_LABEL.1,
        LABEL_GRAY,
        true,
    );
    p.text_plain(
        label!("createWorld.customize.presets.list"),
        LIST_LABEL.0,
        LIST_LABEL.1,
        LABEL_GRAY,
        true,
    );
    ps.export.update(
        p,
        ctx,
        EXPORT_X,
        EXPORT_Y,
        ctx.vw - 2.0 * EXPORT_X,
        WIDGET_HEIGHT,
    );

    let (lx, ly, lw, lh) = (
        0.0,
        PRESETS_LIST_Y,
        ctx.vw,
        (ctx.vh - PRESETS_LIST_INSET).max(0.0),
    );
    let left = ((ctx.vw / 2.0).floor() - ROW_W / 2.0).floor();
    let content_h = PRESETS.len() as f32 * ROW_H + 2.0 * LIST_PAD;
    let input = scroll_masked(
        &mut ps.scroll,
        &mut ps.scroll_drag,
        ctx,
        left,
        ROW_W,
        (lx, ly, lw, lh),
        content_h,
    );
    menu::list_background(p, lx, ly, lw, lh, ps.scroll);
    let list_ctx = ctx.with_input(&input);

    let list_focused = focus::next(true);
    let mut clicked = None;
    let top = (ly + LIST_PAD - ps.scroll.trunc()).floor();
    let clip = p.push_clip(lx, ly, lw, lh);
    for (i, preset) in PRESETS.iter().enumerate() {
        let y = top + i as f32 * ROW_H;
        if !row_visible(y, ROW_H, ly, lh) {
            continue;
        }
        if ps.selected == Some(i) {
            draw_selection(p, left, y, ROW_H, list_focused);
        }
        let (cx, cy) = (left + LIST_PAD, y + LIST_PAD);
        draw_slot(p, cx, cy, Some(preset.icon));
        let name = translate(&format!("flat_world_preset.minecraft.{}", preset.id), &[]);
        p.text_plain(&name, cx + NAME_X, cy + 6.0, 0xFF_FFFF, true);
        if list_ctx.input.left_click && list_ctx.hovering(left, y, ROW_W, ROW_H) {
            clicked = Some(i);
            focus::claim();
        }
    }
    p.pop_clip(clip);
    draw_scrollbar(p, ps.scroll, left, ROW_W, ly, lh, content_h);

    let mut select = clicked;
    if clicked.is_none() && list_focused {
        if ctx.input.up_arrow || ctx.input.down_arrow {
            arrow_select(
                &mut ps.selected,
                PRESETS.len(),
                ctx.input.up_arrow,
                ctx.input.down_arrow,
            );
            ps.select_active = ps.selected.is_some() || ps.export.text.chars().count() > 1;
            if let Some(i) = ps.selected {
                scroll_on_select(&mut ps.scroll, i as f32 * ROW_H, ROW_H, lh, content_h, true);
            }
        } else if ctx.input.select {
            select = ps.selected;
        }
    }
    if let Some(i) = select {
        ps.selected = Some(i);
        ps.select_active = true;
        ps.settings = Settings::of_preset(&PRESETS[i]);
        ps.export.set_text(&ps.settings.save());
        ps.export.cursor = 0;
        ps.export.anchor = 0;
        ps.export.scroll = 0;
        if clicked.is_some() {
            scroll_on_select(
                &mut ps.scroll,
                i as f32 * ROW_H,
                ROW_H,
                lh,
                content_h,
                false,
            );
        }
    }

    let y = ctx.vh - PRESETS_BUTTONS_FROM_BOTTOM;
    let half = (ctx.vw / 2.0).floor();
    let select_pressed = Button {
        active: ps.select_active,
        ..Button::new(
            half - 155.0,
            y,
            FOOTER_W,
            WIDGET_HEIGHT,
            label!("createWorld.customize.presets.select"),
        )
    }
    .draw(p, ctx);
    let cancel =
        Button::new(half + 5.0, y, FOOTER_W, WIDGET_HEIGHT, label!("gui.cancel")).draw(p, ctx);

    if select_pressed {
        let settings = ps.settings.read_share(&ps.export.text);
        let rows = settings.layers.len() + 1;
        st.settings = settings;
        st.selected = st.selected.filter(|i| *i < rows);
        st.presets = None;
        focus::clear();
    } else if cancel || ctx.input.escape {
        st.presets = None;
        focus::clear();
    }
}
