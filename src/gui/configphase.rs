use crate::gui::painter::Painter;
use crate::gui::tooltip::{label, translate};
use crate::gui::widgets::{Button, WIDGET_HEIGHT};
use crate::gui::{GuiState, Screen, ScreenCtx, options};
use crate::text::{LINE_HEIGHT, Span, Style};

const MESSAGE_PADDING: f32 = 100.0;
const MESSAGE_CELL_PADDING: f32 = 12.0;
pub(in crate::gui) const INNER_PADDING: f32 = 4.0;
const SPACING: f32 = 8.0;
const BUTTON_W: f32 = 150.0;
const RECONFIG_CELL_PADDING: f32 = 10.0;
const RECONFIG_DISCONNECT_SECS: f32 = 600.0 / 20.0;

#[derive(Default)]
pub struct State {
    text: Vec<Span>,
    parent: Screen,
    scroll: f32,
    scroll_drag: Option<f32>,
    reconfig_since: Option<f32>,
}

impl State {
    pub fn open_code_of_conduct(&mut self, text: &str, parent: Screen) {
        self.text = crate::text::parse_formatted(text);
        self.parent = parent;
        self.scroll = 0.0;
        self.scroll_drag = None;
    }

    pub fn open_reconfig(&mut self) {
        self.reconfig_since = None;
    }
}

pub fn draw_code_of_conduct(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    crate::gui::menu::background(p, ctx.vw, ctx.vh);
    let st = &mut state.config_phase;

    let box_w = (ctx.vw - MESSAGE_PADDING).max(80.0);
    let lines = p.atlas.font.wrap(&st.text, box_w - 2.0 * INNER_PADDING);
    let content_h = lines.len() as f32 * LINE_HEIGHT;
    let box_h = (content_h + 2.0 * INNER_PADDING).min((ctx.vh - MESSAGE_PADDING).max(20.0));
    let total =
        LINE_HEIGHT + SPACING + 2.0 * MESSAGE_CELL_PADDING + box_h + SPACING + WIDGET_HEIGHT;
    let top = ((ctx.vh - total) / 2.0).floor().max(0.0);

    let title = crate::text::styled(
        label!("multiplayer.codeOfConduct.title"),
        Style {
            bold: true,
            ..Style::default()
        },
    );
    let title_w = p.atlas.font.width(&title);
    p.text(&title, ((ctx.vw - title_w) / 2.0).floor(), top, true);

    let box_x = ((ctx.vw - box_w) / 2.0).floor();
    let box_y = top + LINE_HEIGHT + SPACING + MESSAGE_CELL_PADDING;
    text_area(
        p,
        ctx,
        &lines,
        box_x,
        box_y,
        box_w,
        box_h,
        &mut st.scroll,
        &mut st.scroll_drag,
    );

    let row_y = box_y + box_h + MESSAGE_CELL_PADDING + SPACING;
    let row_x = ((ctx.vw - (2.0 * BUTTON_W + SPACING)) / 2.0).floor();
    if Button::new(
        row_x,
        row_y,
        BUTTON_W,
        WIDGET_HEIGHT,
        label!("gui.acknowledge"),
    )
    .draw(p, ctx)
    {
        crate::client::server_screens::accept_code_of_conduct();
        state.nav = Some(st.parent);
    } else if Button::new(
        row_x + BUTTON_W + SPACING,
        row_y,
        BUTTON_W,
        WIDGET_HEIGHT,
        label!("menu.disconnect"),
    )
    .draw(p, ctx)
    {
        state.disconnect = true;
        state.disconnect_reason = Some(crate::text::parse_formatted(&translate(
            "multiplayer.disconnect.code_of_conduct",
            &[],
        )));
        state.nav = Some(Screen::Disconnected);
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::gui) fn text_area(
    p: &mut Painter,
    ctx: &ScreenCtx,
    lines: &[Vec<Span>],
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scroll: &mut f32,
    drag: &mut Option<f32>,
) {
    p.fill(x, y, w, h, 0xFFA0_A0A0);
    p.fill(x + 1.0, y + 1.0, w - 2.0, h - 2.0, 0xFF00_0000);

    let content_h = lines.len() as f32 * LINE_HEIGHT;
    let (list_x, list_y) = (x + INNER_PADDING, y + INNER_PADDING);
    let (list_w, list_h) = (w - 2.0 * INNER_PADDING, h - 2.0 * INNER_PADDING);
    options::scroll_input(scroll, drag, ctx, list_x, list_w, list_y, list_h, content_h);
    let clip = p.push_clip(list_x, list_y, list_w, list_h);
    let mut row_y = (list_y - *scroll).floor();
    for line in lines {
        if row_y + LINE_HEIGHT > list_y && row_y < list_y + list_h {
            p.text(line, list_x, row_y, true);
        }
        row_y += LINE_HEIGHT;
    }
    p.pop_clip(clip);
    options::draw_scrollbar(p, *scroll, list_x, list_w, list_y, list_h, content_h);
}

pub fn draw_reconfig(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    crate::gui::menu::background(p, ctx.vw, ctx.vh);
    let since = *state
        .config_phase
        .reconfig_since
        .get_or_insert(ctx.input.time);

    let total = LINE_HEIGHT + WIDGET_HEIGHT + 4.0 * RECONFIG_CELL_PADDING;
    let top = ((ctx.vh - total) / 2.0).floor();
    let text = label!("connect.reconfiguring");
    p.text_centered(
        text,
        ctx.vw / 2.0,
        top + RECONFIG_CELL_PADDING,
        0xFFFFFF,
        true,
    );

    let mut button = Button::new(
        ((ctx.vw - BUTTON_W) / 2.0).floor(),
        top + LINE_HEIGHT + 3.0 * RECONFIG_CELL_PADDING,
        BUTTON_W,
        WIDGET_HEIGHT,
        label!("menu.disconnect"),
    );
    button.active = ctx.input.time - since >= RECONFIG_DISCONNECT_SECS;
    if button.draw(p, ctx) {
        state.disconnect = true;
        state.nav = Some(Screen::Title);
    }
}
