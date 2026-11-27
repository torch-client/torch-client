use std::borrow::Cow;

use crate::gui::focus;
use crate::gui::menu;
use crate::gui::op_blocks::{cycle, on_off, step};
use crate::gui::options::generic_value;
use crate::gui::painter::Painter;
use crate::gui::tooltip::{label, translate};
use crate::gui::widgets::{
    self, Button, FOOTER_GAP, FOOTER_W, HeaderFooter, TAB_BAR_H, TextBox, WIDGET_HEIGHT,
    centered_row_x,
};
use crate::gui::worlds::back_to_list;
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::singleplayer::backed::{Control, is_backed};
use crate::singleplayer::backend;
use crate::singleplayer::store::{self, Difficulty, GameMode, WorldSettings, WorldSummary};
use crate::text::{LINE_HEIGHT, Span, Style};

const LAYOUT: HeaderFooter = HeaderFooter::new(TAB_BAR_H, HeaderFooter::DEFAULT_H);

const GAME_TAB: usize = 0;
const WORLD_TAB: usize = 1;

const TAB_COLUMN_W: f32 = 210.0;
const ROW_SPACING: f32 = 8.0;
const COLUMN_SPACING: f32 = 10.0;
const WORLD_COLUMN_W: f32 = 150.0;
const WORLD_TAB_W: f32 = 310.0;
const NAME_W: f32 = 208.0;
const SEED_W: f32 = 308.0;
const EDIT_MAX_LEN: usize = 32;
const LABEL_SPACING: f32 = 4.0;
const LABELED_H: f32 = LINE_HEIGHT + LABEL_SPACING + WIDGET_HEIGHT;
const SWITCH_W: f32 = 44.0;
const SWITCH_ROW_SPACING: f32 = 4.0;
const TAB_ALIGN_Y: f32 = 0.166_666_67;
const WORLD_TYPES: [&str; 5] = [
    "minecraft:normal",
    "minecraft:flat",
    "minecraft:large_biomes",
    "minecraft:amplified",
    "minecraft:single_biome_surface",
];

pub struct State {
    settings: WorldSettings,
    name: TextBox,
    seed: TextBox,
    tab: usize,
    target_folder: Option<(String, String)>,
    #[allow(
        dead_code,
        reason = "read by the experimental-features confirm, which is not ported"
    )]
    recreated: bool,
    error: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        State {
            settings: WorldSettings::default(),
            name: TextBox::bordered(EDIT_MAX_LEN, ""),
            seed: TextBox::bordered(EDIT_MAX_LEN, ""),
            tab: GAME_TAB,
            target_folder: None,
            recreated: false,
            error: None,
        }
    }
}

impl State {
    pub fn open_fresh(&mut self) {
        *self = State::default();
        self.settings.name = label!("selectWorld.newWorld").to_owned();
        self.name.set_text(&self.settings.name);
        self.name.focus();
    }

    pub(crate) fn open_from(&mut self, world: &WorldSummary) {
        self.open_fresh();
        self.settings = world.settings.clone();
        self.settings.allow_commands = Some(world.settings.effective_allow_commands());
        self.settings.seed =
            store::parse_seed(&world.settings.seed).map_or_else(String::new, |n| n.to_string());
        self.name.set_text(&self.settings.name);
        self.name.focus();
        self.seed.set_text(&self.settings.seed);
        self.recreated = true;
    }
}

#[derive(Clone, Copy)]
enum Sub {
    GameRules,
    Experiments,
    DataPacks,
    Customize,
}

fn take_results(state: &mut GuiState) {
    let w = &mut state.worlds;
    let s = &mut w.create.settings;
    if let Some(rules) = w.gamerules.result.take() {
        s.game_rules = rules;
    }
    if let Some(enabled) = w.experiments.result.take() {
        s.experiments = enabled;
    }
    if let Some(selected) = w.datapacks.result.take() {
        s.data_packs = selected;
    }
    if let Some(preset) = w.flat.result.take() {
        s.flat_preset = preset;
    }
    if let Some(biome) = w.buffet.result.take() {
        s.single_biome = biome;
    }
}

fn open_sub(state: &mut GuiState, sub: Sub) {
    let w = &mut state.worlds;
    let s = &w.create.settings;
    state.nav = Some(match sub {
        Sub::GameRules => {
            w.gamerules.open(&s.game_rules);
            Screen::WorldGameRules
        }
        Sub::Experiments => {
            w.experiments.open(&s.experiments);
            Screen::WorldExperiments
        }
        Sub::DataPacks => {
            w.datapacks.open(&s.data_packs);
            Screen::WorldDataPacks
        }
        Sub::Customize if s.world_type == "minecraft:flat" => {
            w.flat.open(&s.flat_preset);
            Screen::WorldFlat
        }
        Sub::Customize => {
            w.buffet.open(&s.single_biome);
            Screen::WorldBuffet
        }
    });
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    menu::background(p, ctx.vw, ctx.vh);
    take_results(state);
    let st = &mut state.worlds.create;
    let mut sub = None;

    let tabs = [
        label!("createWorld.tab.game.title"),
        label!("createWorld.tab.world.title"),
        label!("createWorld.tab.more.title"),
    ];
    if let Some(tab) = widgets::tab_bar(p, ctx, &tabs, st.tab) {
        st.tab = tab;
        focus::clear();
    }

    let mut tip: Option<Vec<Span>> = None;
    let pressed = match st.tab {
        GAME_TAB => game_tab(p, ctx, st, &mut tip),
        WORLD_TAB => world_tab(p, ctx, st, &mut tip, &mut sub),
        _ => more_tab(p, ctx, &mut tip, &mut sub),
    };

    p.sprite(
        "footer_separator",
        0.0,
        ctx.vh - LAYOUT.footer_h - 2.0,
        ctx.vw,
        2.0,
    );
    let footer_y = LAYOUT.footer_y(ctx.vh);
    let x = centered_row_x(ctx.vw, 2.0 * FOOTER_W + FOOTER_GAP);
    let hardcore_blocked = st.settings.is_hardcore() && !is_backed(Control::Hardcore);
    let create = Button {
        active: !hardcore_blocked,
        ..Button::new(
            x,
            footer_y,
            FOOTER_W,
            WIDGET_HEIGHT,
            label!("selectWorld.create"),
        )
    };
    let created = create.draw(p, ctx);
    if hardcore_blocked && ctx.hovering(create.x, create.y, create.w, create.h) {
        tip = Some(unsupported());
    }
    let cancel_x = x + FOOTER_W + FOOTER_GAP;
    let cancelled = Button::new(
        cancel_x,
        footer_y,
        FOOTER_W,
        WIDGET_HEIGHT,
        label!("gui.cancel"),
    )
    .draw(p, ctx)
        || ctx.input.escape;

    if let Some(error) = &st.error {
        let y = ctx.vh - LAYOUT.footer_h - 2.0 - LABEL_SPACING - LINE_HEIGHT;
        p.text_centered(error, ctx.vw / 2.0, y, crate::text::RED, true);
    }

    if let Some(spans) = tip {
        super::hover_spans(p, ctx, &spans);
    }

    let confirmed = ctx.input.enter && create.active && !pressed && !created && !cancelled;
    if cancelled {
        back_to_list(state);
    } else if let Some(sub) = sub {
        open_sub(state, sub);
    } else if created || confirmed {
        create_world(state);
    }
}

fn game_tab(p: &mut Painter, ctx: &ScreenCtx, st: &mut State, tip: &mut Option<Vec<Span>>) -> bool {
    let name_label = label!("selectWorld.enterName");
    let labeled_w = p.atlas.font.width_str(name_label).max(NAME_W);
    let col_w = TAB_COLUMN_W.max(labeled_w);
    let h = LABELED_H + 3.0 * (ROW_SPACING + WIDGET_HEIGHT);
    let (x, y) = place(ctx, col_w, h);
    let mut pressed = false;

    let lx = x + ((col_w - labeled_w) / 2.0).trunc();
    p.text_plain(name_label, lx, y, crate::text::WHITE, true);
    let box_y = y + LINE_HEIGHT + LABEL_SPACING;
    st.name.update(p, ctx, lx, box_y, NAME_W, WIDGET_HEIGHT);
    if st.settings.name != st.name.text {
        st.settings.name.clone_from(&st.name.text);
    }
    if ctx.hovering(lx, box_y, NAME_W, WIDGET_HEIGHT) {
        *tip = Some(target_folder_tip(st));
    }

    let s = &mut st.settings;
    let mut row = y + LABELED_H + ROW_SPACING;

    let backed = is_backed(Control::GameMode);
    let caption = generic_value(label!("selectWorld.gameMode"), game_mode_name(s.game_mode));
    let button = Button {
        active: backed,
        ..Button::new(x, row, TAB_COLUMN_W, WIDGET_HEIGHT, &caption)
    };
    let delta = cycle(p, ctx, &button);
    if delta != 0 {
        s.game_mode = step(&GameMode::ALL, s.game_mode, delta);
        pressed = true;
    }
    if ctx.hovering(x, row, TAB_COLUMN_W, WIDGET_HEIGHT) {
        *tip = Some(if backed {
            line(game_mode_info(s.game_mode))
        } else {
            unsupported()
        });
    }
    row += WIDGET_HEIGHT + ROW_SPACING;

    let backed = is_backed(Control::Difficulty);
    let difficulty = s.effective_difficulty();
    let caption = generic_value(label!("options.difficulty"), difficulty_name(difficulty));
    let button = Button {
        active: backed && !s.is_hardcore(),
        ..Button::new(x, row, TAB_COLUMN_W, WIDGET_HEIGHT, &caption)
    };
    let delta = cycle(p, ctx, &button);
    if delta != 0 {
        s.difficulty = step(&Difficulty::ALL, difficulty, delta);
        pressed = true;
    }
    if ctx.hovering(x, row, TAB_COLUMN_W, WIDGET_HEIGHT) {
        *tip = Some(if backed {
            line(difficulty_info(difficulty))
        } else {
            unsupported()
        });
    }
    row += WIDGET_HEIGHT + ROW_SPACING;

    let backed = is_backed(Control::AllowCommands);
    let allow = s.effective_allow_commands();
    let caption = generic_value(label!("selectWorld.allowCommands"), on_off(allow));
    let button = Button {
        active: backed && !s.is_hardcore(),
        ..Button::new(x, row, TAB_COLUMN_W, WIDGET_HEIGHT, &caption)
    };
    if cycle(p, ctx, &button) != 0 {
        s.allow_commands = Some(!allow);
        pressed = true;
    }
    if ctx.hovering(x, row, TAB_COLUMN_W, WIDGET_HEIGHT) {
        *tip = Some(if backed {
            line(label!("selectWorld.allowCommands.info"))
        } else {
            unsupported()
        });
    }
    pressed
}

fn world_tab(
    p: &mut Painter,
    ctx: &ScreenCtx,
    st: &mut State,
    tip: &mut Option<Vec<Span>>,
    sub: &mut Option<Sub>,
) -> bool {
    let switch_grid_h = 2.0 * (SWITCH_ROW_SPACING + WIDGET_HEIGHT);
    let h = WIDGET_HEIGHT + ROW_SPACING + LABELED_H + ROW_SPACING + switch_grid_h;
    let (x, y) = place(ctx, WORLD_TAB_W, h);
    let mut pressed = false;

    {
        let s = &mut st.settings;
        let known = WORLD_TYPES.contains(&s.world_type.as_str());
        let backed = is_backed(Control::WorldType);
        let caption = generic_value(
            label!("selectWorld.mapType"),
            &world_type_name(&s.world_type),
        );
        let button = Button {
            active: backed && known,
            ..Button::new(x, y, WORLD_COLUMN_W, WIDGET_HEIGHT, &caption)
        };
        let delta = cycle(p, ctx, &button);
        if delta != 0 {
            s.world_type = step(&WORLD_TYPES, s.world_type.as_str(), delta).to_owned();
            pressed = true;
        }
        if ctx.hovering(x, y, WORLD_COLUMN_W, WIDGET_HEIGHT) {
            if !backed {
                *tip = Some(unsupported());
            } else if s.world_type == "minecraft:amplified" {
                *tip = Some(line(label!("generator.minecraft.amplified.info")));
            }
        }

        let backed = is_backed(Control::CustomizeType);
        let has_editor = matches!(
            s.world_type.as_str(),
            "minecraft:flat" | "minecraft:single_biome_surface"
        );
        let cx = x + WORLD_COLUMN_W + COLUMN_SPACING;
        let customize = Button {
            active: backed && has_editor,
            ..Button::new(
                cx,
                y,
                WORLD_COLUMN_W,
                WIDGET_HEIGHT,
                label!("selectWorld.customizeType"),
            )
        };
        if customize.draw(p, ctx) {
            *sub = Some(Sub::Customize);
            pressed = true;
        }
        if !backed && ctx.hovering(cx, y, WORLD_COLUMN_W, WIDGET_HEIGHT) {
            *tip = Some(unsupported());
        }
    }

    let label_y = y + WIDGET_HEIGHT + ROW_SPACING;
    p.text_plain(
        label!("selectWorld.enterSeed"),
        x,
        label_y,
        crate::text::WHITE,
        true,
    );
    let hint = label!("selectWorld.seedInfo");
    if st.seed.placeholder != hint {
        st.seed.placeholder = hint.to_owned();
    }
    let seed_y = label_y + LINE_HEIGHT + LABEL_SPACING;
    st.seed.update(p, ctx, x, seed_y, SEED_W, WIDGET_HEIGHT);
    if st.settings.seed != st.seed.text {
        st.settings.seed.clone_from(&st.seed.text);
    }
    if !is_backed(Control::Seed) && ctx.hovering(x, seed_y, SEED_W, WIDGET_HEIGHT) {
        *tip = Some(unsupported());
    }

    let s = &mut st.settings;
    let grid_y = seed_y + WIDGET_HEIGHT + ROW_SPACING;
    let row = grid_y + SWITCH_ROW_SPACING;
    let backed = is_backed(Control::GenerateStructures);
    if switch_row(
        p,
        ctx,
        x,
        row,
        label!("selectWorld.mapFeatures"),
        s.generate_structures,
        backed,
    ) {
        s.generate_structures = !s.generate_structures;
        pressed = true;
    }
    if ctx.hovering(x + WORLD_TAB_W - SWITCH_W, row, SWITCH_W, WIDGET_HEIGHT) {
        *tip = Some(if backed {
            line(label!("selectWorld.mapFeatures.info"))
        } else {
            unsupported()
        });
    }

    let row = row + WIDGET_HEIGHT + SWITCH_ROW_SPACING;
    let backed = is_backed(Control::BonusChest);
    let bonus = s.effective_bonus_chest();
    if switch_row(
        p,
        ctx,
        x,
        row,
        label!("selectWorld.bonusItems"),
        bonus,
        backed && !s.is_hardcore(),
    ) {
        s.bonus_chest = !bonus;
        pressed = true;
    }
    if !backed && ctx.hovering(x + WORLD_TAB_W - SWITCH_W, row, SWITCH_W, WIDGET_HEIGHT) {
        *tip = Some(unsupported());
    }
    pressed
}

fn more_tab(
    p: &mut Painter,
    ctx: &ScreenCtx,
    tip: &mut Option<Vec<Span>>,
    sub: &mut Option<Sub>,
) -> bool {
    let h = 3.0 * WIDGET_HEIGHT + 2.0 * ROW_SPACING;
    let (x, mut y) = place(ctx, TAB_COLUMN_W, h);
    let mut pressed = false;
    let rows = [
        (
            label!("selectWorld.gameRules"),
            Sub::GameRules,
            is_backed(Control::GameRules),
        ),
        (label!("selectWorld.experiments"), Sub::Experiments, true),
        (label!("selectWorld.dataPacks"), Sub::DataPacks, true),
    ];
    for (text, opens, active) in rows {
        if (Button {
            active,
            ..Button::new(x, y, TAB_COLUMN_W, WIDGET_HEIGHT, text)
        })
        .draw(p, ctx)
        {
            *sub = Some(opens);
            pressed = true;
        }
        if !active && ctx.hovering(x, y, TAB_COLUMN_W, WIDGET_HEIGHT) {
            *tip = Some(unsupported());
        }
        y += WIDGET_HEIGHT + ROW_SPACING;
    }
    pressed
}

fn switch_row(
    p: &mut Painter,
    ctx: &ScreenCtx,
    x: f32,
    y: f32,
    text: &str,
    value: bool,
    active: bool,
) -> bool {
    let label_y = y + ((WIDGET_HEIGHT - LINE_HEIGHT) / 2.0).round();
    p.text_plain(text, x, label_y, crate::text::WHITE, true);
    let button = Button {
        active,
        ..Button::new(
            x + WORLD_TAB_W - SWITCH_W,
            y,
            SWITCH_W,
            WIDGET_HEIGHT,
            on_off(value),
        )
    };
    cycle(p, ctx, &button) != 0
}

fn place(ctx: &ScreenCtx, w: f32, h: f32) -> (f32, f32) {
    let area_h = ctx.vh - LAYOUT.footer_h - TAB_BAR_H;
    let x = ((ctx.vw - w) * 0.5).trunc();
    let y = TAB_BAR_H + ((area_h - h) * TAB_ALIGN_Y).trunc();
    (x, y)
}

fn target_folder_tip(st: &mut State) -> Vec<Span> {
    let name = &st.settings.name;
    if st
        .target_folder
        .as_ref()
        .is_none_or(|(for_name, _)| for_name != name)
    {
        st.target_folder = Some((name.clone(), store::available_folder(folder_base(name))));
    }
    let folder = st.target_folder.as_ref().map_or("", |(_, f)| f.as_str());
    const MARK: &str = "\u{E000}";
    let text = translate("selectWorld.targetFolder", &[MARK.to_owned()]);
    let italic = Style {
        italic: true,
        ..Style::default()
    };
    match text.split_once(MARK) {
        Some((before, after)) => vec![
            Span::new(before),
            Span {
                text: folder.to_owned(),
                style: italic,
            },
            Span::new(after),
        ],
        None => vec![Span::new(text)],
    }
}

fn create_world(state: &mut GuiState) {
    let st = &mut state.worlds.create;
    let mut settings = st.settings.clone();
    settings.name = store::java_trim(&st.name.text).to_owned();
    settings.seed.clone_from(&st.seed.text);
    let summary = match store::create(&settings, folder_base(&st.name.text)) {
        Ok(summary) => summary,
        Err(e) => {
            crate::log_warn!("worlds", "could not create {:?}: {e}", settings.name);
            st.error = Some(e);
            return;
        }
    };
    st.error = None;
    if let Err(e) = backend::open(&summary.folder) {
        crate::log_warn!("worlds", "could not open {}: {e}", summary.folder);
    }
    back_to_list(state);
}

fn folder_base(name: &str) -> &str {
    let trimmed = store::java_trim(name);
    if trimmed.is_empty() {
        label!("selectWorld.newWorld")
    } else {
        trimmed
    }
}

fn unsupported() -> Vec<Span> {
    line(&super::unsupported_text())
}

fn line(text: &str) -> Vec<Span> {
    vec![Span::new(text)]
}

fn game_mode_name(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Survival => label!("selectWorld.gameMode.survival"),
        GameMode::Hardcore => label!("selectWorld.gameMode.hardcore"),
        GameMode::Creative => label!("selectWorld.gameMode.creative"),
    }
}

fn game_mode_info(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Survival => label!("selectWorld.gameMode.survival.info"),
        GameMode::Hardcore => label!("selectWorld.gameMode.hardcore.info"),
        GameMode::Creative => label!("selectWorld.gameMode.creative.info"),
    }
}

fn difficulty_name(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Peaceful => label!("options.difficulty.peaceful"),
        Difficulty::Easy => label!("options.difficulty.easy"),
        Difficulty::Normal => label!("options.difficulty.normal"),
        Difficulty::Hard => label!("options.difficulty.hard"),
    }
}

fn difficulty_info(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Peaceful => label!("options.difficulty.peaceful.info"),
        Difficulty::Easy => label!("options.difficulty.easy.info"),
        Difficulty::Normal => label!("options.difficulty.normal.info"),
        Difficulty::Hard => label!("options.difficulty.hard.info"),
    }
}

fn world_type_name(id: &str) -> Cow<'static, str> {
    Cow::Borrowed(match id {
        "minecraft:normal" => label!("generator.minecraft.normal"),
        "minecraft:flat" => label!("generator.minecraft.flat"),
        "minecraft:large_biomes" => label!("generator.minecraft.large_biomes"),
        "minecraft:amplified" => label!("generator.minecraft.amplified"),
        "minecraft:single_biome_surface" => label!("generator.minecraft.single_biome_surface"),
        other => {
            return Cow::Owned(translate(
                &format!("generator.{}", other.replacen(':', ".", 1)),
                &[],
            ));
        }
    })
}
