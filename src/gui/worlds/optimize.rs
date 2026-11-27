use crate::gui::menu;
use crate::gui::painter::Painter;
use crate::gui::tooltip::{label, translate};
use crate::gui::widgets::{self, Button, WIDGET_HEIGHT, click_in};
use crate::gui::{GuiState, Screen, ScreenCtx, focus};
use crate::singleplayer::backed::{Control, is_backed};
use crate::text::LINE_HEIGHT;

const CONFIRM_TITLE_Y: f32 = 50.0;
const MESSAGE_Y: f32 = 70.0;
const MESSAGE_INSET: f32 = 50.0;
const CHECKBOX_Y: f32 = 76.0;
const PROCEED_Y: f32 = 100.0;
const CANCEL_Y: f32 = 124.0;
const BUTTON_W: f32 = 150.0;
const CHECKBOX_SIZE: f32 = LINE_HEIGHT + 8.0;
const CHECKBOX_SPACING: f32 = 4.0;
const CHECKBOX_LABEL_COLOR: u32 = 0xE0E0E0;

const PROGRESS_TITLE_Y: f32 = 20.0;
const INFO_Y: f32 = 40.0;
const INFO_STEP: f32 = LINE_HEIGHT + 3.0;
const BAR_HALF_W: f32 = 150.0;
const BAR_H: f32 = 10.0;
const BAR_Y: f32 = 100.0;
const PROGRESS_CANCEL_Y: f32 = 150.0;
const PROGRESS_CANCEL_W: f32 = 200.0;
const PROGRESS_TEXT_COLOR: u32 = 0xA0A0A0;
const OVERWORLD_COLOR: u32 = 0xFF33_6622;
const NETHER_COLOR: u32 = 0xFF66_4433;
const END_COLOR: u32 = 0xFF77_8855;
const OTHER_DIMENSION_COLOR: u32 = 0xFFDD_DDDD;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OptimizeChoice {
    pub backup: bool,
    pub erase_cache: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Page {
    #[default]
    Confirm,
    Progress,
}

pub struct Progress {
    pub status: &'static str,
    pub converted: u32,
    pub skipped: u32,
    pub total: u32,
    pub levels: Vec<(String, f32)>,
    pub total_progress: f32,
    pub finished: bool,
}

impl Default for Progress {
    fn default() -> Progress {
        Progress {
            status: "optimizeWorld.stage.counting",
            converted: 0,
            skipped: 0,
            total: 0,
            levels: Vec::new(),
            total_progress: 0.0,
            finished: false,
        }
    }
}

#[derive(Default)]
pub struct State {
    folder: String,
    world_name: String,
    page: Page,
    erase_cache: bool,
    pub progress: Progress,
    pub result: Option<OptimizeChoice>,
}

impl State {
    pub fn open(&mut self, folder: &str, world_name: &str) {
        self.folder = folder.to_owned();
        self.world_name = world_name.to_owned();
        self.page = Page::Confirm;
        self.erase_cache = false;
        self.progress = Progress::default();
        self.result = None;
    }

    #[allow(
        dead_code,
        reason = "called by the singleplayer backend, which does not exist yet"
    )]
    pub fn folder(&self) -> &str {
        &self.folder
    }
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    menu::background(p, ctx.vw, ctx.vh);
    match state.worlds.optimize.page {
        Page::Confirm => draw_confirm(p, state, ctx),
        Page::Progress => draw_progress(p, state, ctx),
    }
}

fn draw_confirm(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    let cx = (ctx.vw / 2.0).floor();
    let opt = &mut state.worlds.optimize;

    let lines = widgets::wrap(
        p,
        &[label!("optimizeWorld.confirm.description")],
        ctx.vw - MESSAGE_INSET + 4.0,
    );
    let text_size = (lines.len() as f32 + 1.0) * LINE_HEIGHT;

    checkbox(
        p,
        ctx,
        cx - 155.0 + 80.0,
        CHECKBOX_Y + text_size,
        label!("selectWorld.backupEraseCache"),
        &mut opt.erase_cache,
    );

    let backed = is_backed(Control::Optimize);
    let row_y = PROCEED_Y + text_size;
    let mut proceed = None;
    let mut unsupported_hover = false;
    let proceed_buttons = [
        (cx - 155.0, label!("optimizeWorld.confirm.proceed"), true),
        (
            cx - 155.0 + 160.0,
            label!("selectWorld.backupJoinSkipButton"),
            false,
        ),
    ];
    for (x, text, backup) in proceed_buttons {
        let button = Button {
            active: backed,
            ..Button::new(x, row_y, BUTTON_W, WIDGET_HEIGHT, text)
        };
        if button.draw(p, ctx) {
            proceed = Some(backup);
        }
        if !backed && ctx.hovering(x, row_y, BUTTON_W, WIDGET_HEIGHT) {
            unsupported_hover = true;
        }
    }

    let cancel = Button::new(
        cx - 155.0 + 80.0,
        CANCEL_Y + text_size,
        BUTTON_W,
        WIDGET_HEIGHT,
        label!("gui.cancel"),
    );
    let cancelled = cancel.draw(p, ctx) || ctx.input.escape;

    p.text_centered(
        label!("optimizeWorld.confirm.title"),
        cx,
        CONFIRM_TITLE_Y,
        0xFFFFFF,
        true,
    );
    for (i, line) in lines.iter().enumerate() {
        p.text_centered(line, cx, MESSAGE_Y + i as f32 * LINE_HEIGHT, 0xFFFFFF, true);
    }

    if unsupported_hover {
        super::unsupported_tip(p, ctx);
    }

    if let Some(backup) = proceed {
        opt.result = Some(OptimizeChoice {
            backup,
            erase_cache: opt.erase_cache,
        });
        opt.progress = Progress::default();
        opt.page = Page::Progress;
    } else if cancelled {
        state.nav = Some(Screen::EditWorld);
    }
}

fn draw_progress(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    let cx = (ctx.vw / 2.0).floor();
    let quarter = (ctx.vh / 4.0).floor();
    let opt = &mut state.worlds.optimize;

    let cancel_x = cx - PROGRESS_CANCEL_W / 2.0;
    let cancel = Button::new(
        cancel_x,
        quarter + PROGRESS_CANCEL_Y,
        PROGRESS_CANCEL_W,
        WIDGET_HEIGHT,
        label!("gui.cancel"),
    );
    let cancelled = cancel.draw(p, ctx);

    let title = translate("optimizeWorld.title", std::slice::from_ref(&opt.world_name));
    p.text_centered(&title, cx, PROGRESS_TITLE_Y, 0xFFFFFF, true);

    let x0 = cx - BAR_HALF_W;
    let x1 = cx + BAR_HALF_W;
    let y0 = quarter + BAR_Y;
    let y1 = y0 + BAR_H;
    let progress = &opt.progress;
    let status = translate(progress.status, &[]);
    p.text_centered(
        &status,
        cx,
        y0 - LINE_HEIGHT - 2.0,
        PROGRESS_TEXT_COLOR,
        true,
    );

    if progress.total > 0 {
        p.fill(
            x0 - 1.0,
            y0 - 1.0,
            x1 - x0 + 2.0,
            y1 - y0 + 2.0,
            0xFF00_0000,
        );
        let info = [
            ("optimizeWorld.info.converted", progress.converted),
            ("optimizeWorld.info.skipped", progress.skipped),
            ("optimizeWorld.info.total", progress.total),
        ];
        for (i, (key, value)) in info.into_iter().enumerate() {
            let text = translate(key, &[value.to_string()]);
            p.text_plain(
                &text,
                x0,
                INFO_Y + i as f32 * INFO_STEP,
                PROGRESS_TEXT_COLOR,
                true,
            );
        }

        let mut offset = 0.0;
        for (dimension, fraction) in &progress.levels {
            let length = (fraction * (x1 - x0)).floor();
            p.fill(x0 + offset, y0, length, y1 - y0, dimension_color(dimension));
            offset += length;
        }

        let done = progress.converted + progress.skipped;
        let counter = translate(
            "optimizeWorld.progress.counter",
            &[done.to_string(), progress.total.to_string()],
        );
        let percent = (progress.total_progress * 100.0).floor() as i32;
        let percentage = translate("optimizeWorld.progress.percentage", &[percent.to_string()]);
        p.text_centered(
            &counter,
            cx,
            y0 + 2.0 * LINE_HEIGHT + 2.0,
            PROGRESS_TEXT_COLOR,
            true,
        );
        let percent_y = y0 + ((y1 - y0) / 2.0).floor() - (LINE_HEIGHT / 2.0).floor();
        p.text_centered(&percentage, cx, percent_y, PROGRESS_TEXT_COLOR, true);
    }

    if cancelled || opt.progress.finished {
        super::back_to_list(state);
    }
}

fn dimension_color(dimension: &str) -> u32 {
    match dimension {
        "minecraft:overworld" => OVERWORLD_COLOR,
        "minecraft:the_nether" => NETHER_COLOR,
        "minecraft:the_end" => END_COLOR,
        _ => OTHER_DIMENSION_COLOR,
    }
}

fn checkbox(p: &mut Painter, ctx: &ScreenCtx, x: f32, y: f32, text: &str, selected: &mut bool) {
    let w = CHECKBOX_SIZE + CHECKBOX_SPACING + p.atlas.font.width_str(text);
    let h = CHECKBOX_SIZE;
    let focused = focus::next(true);
    let sprite = match (*selected, focused) {
        (true, true) => "widget/checkbox_selected_highlighted",
        (true, false) => "widget/checkbox_selected",
        (false, true) => "widget/checkbox_highlighted",
        (false, false) => "widget/checkbox",
    };
    p.sprite(sprite, x, y, CHECKBOX_SIZE, CHECKBOX_SIZE);
    let text_y = y + (CHECKBOX_SIZE / 2.0).floor() - (LINE_HEIGHT / 2.0).floor();
    p.text_plain(
        text,
        x + CHECKBOX_SIZE + CHECKBOX_SPACING,
        text_y,
        CHECKBOX_LABEL_COLOR,
        true,
    );

    let clicked = click_in(ctx, x, y, w, h);
    #[cfg(not(feature = "mobile_ui"))]
    if clicked {
        focus::claim();
    }
    if clicked || (focused && ctx.input.select) {
        *selected = !*selected;
        #[cfg(feature = "audio")]
        widgets::play_click();
    }
}
