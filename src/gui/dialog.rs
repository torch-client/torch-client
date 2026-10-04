use std::collections::HashMap;
use std::sync::Arc;

use crate::gui::clickevent::{self, Followup};
use crate::gui::confirmlink;
use crate::gui::focus;
use crate::gui::painter::Painter;
use crate::gui::widgets::{self, HeaderFooter, TextBox, WIDGET_HEIGHT};
use crate::gui::{Screen, ScreenCtx, options, tooltip};
use crate::play::dialog::{
    Action, ActionButton, AfterAction, Body, Click, Control, Dialog, Kind, RangeInfo, Value,
};
use crate::session::{ServerLink, SharedMutex};
use crate::text::{LINE_HEIGHT, Span, Style};

const FOOTER_MARGIN: f32 = 5.0;

const ELEMENT_SPACING: f32 = 10.0;

const LABEL_SPACING: f32 = 4.0;

const GRID_SPACING: f32 = 2.0;

const FOOTER_SPACING: f32 = 8.0;

const WARNING_BUTTON_SIZE: f32 = 20.0;
const TITLE_SPACING: f32 = 10.0;

const CONTROL_HEIGHT: f32 = WIDGET_HEIGHT;

const CHECKBOX_SIZE: f32 = LINE_HEIGHT + 8.0;
const CHECKBOX_SPACING: f32 = 4.0;

const HANDLE_WIDTH: f32 = 8.0;

const ITEM_DESCRIPTION_SPACING: f32 = 2.0;

const SLIDER_ID_BASE: u32 = 0x4000;

const WAIT_BUTTON_VISIBLE_SECS: f32 = 1.0;
const WAIT_BUTTON_ACTIVE_SECS: f32 = 5.0;
const WAIT_BUTTON_WIDTH: f32 = 200.0;

const CONFIRM_BUTTON_WIDTH: f32 = 150.0;

#[derive(Default)]
pub struct DialogState {
    phase: Phase,
    parent: Screen,
    inputs: Vec<InputState>,
    scroll: f32,
    scroll_drag: Option<f32>,
    layout: Option<Layout>,
    overlay: Overlay,
}

#[derive(Default)]
enum Phase {
    #[default]
    Closed,
    Showing(Arc<Dialog>),
    Waiting {
        since: Option<f32>,
        active: bool,
    },
}

enum InputState {
    Text(TextBox),
    Boolean(bool),
    Choice { index: usize, label: Vec<Span> },
    Number { slider: f32, label: Vec<Span> },
}

#[derive(Default, PartialEq)]
enum Overlay {
    #[default]
    None,
    Warning,
    Link(String),
}

#[derive(Default)]
pub struct Outcome {
    pub nav: Option<Screen>,
    pub chat: Option<String>,
    pub disconnect: bool,
}

impl DialogState {
    pub fn show(&mut self, dialog: Arc<Dialog>, current: Screen) {
        if current != Screen::Dialog {
            self.parent = current;
        }
        self.open(dialog);
    }

    fn open(&mut self, dialog: Arc<Dialog>) {
        self.inputs = dialog.inputs.iter().map(initial_state).collect();
        self.phase = Phase::Showing(dialog);
        self.scroll = 0.0;
        self.scroll_drag = None;
        self.overlay = Overlay::None;
    }

    fn showing(&self) -> Option<Arc<Dialog>> {
        match &self.phase {
            Phase::Showing(dialog) => Some(dialog.clone()),
            Phase::Closed | Phase::Waiting { .. } => None,
        }
    }

    pub fn clear(&mut self) -> Screen {
        self.phase = Phase::Closed;
        self.overlay = Overlay::None;
        self.parent
    }

    pub fn parent(&self) -> Screen {
        self.parent
    }

    pub fn is_open(&self) -> bool {
        !matches!(self.phase, Phase::Closed)
    }

    pub fn pauses(&self) -> bool {
        matches!(&self.phase, Phase::Showing(dialog) if dialog.pause)
    }

    pub fn on_escape(&mut self, shared: &Arc<SharedMutex>) -> Outcome {
        match &self.overlay {
            Overlay::Warning | Overlay::Link(_) => {
                self.overlay = Overlay::None;
                Outcome::default()
            }
            Overlay::None => {
                let Some(dialog) = self.showing() else {
                    let mut out = Outcome::default();
                    if matches!(self.phase, Phase::Waiting { active: true, .. }) {
                        out.nav = Some(self.clear());
                    }
                    return out;
                };
                if !dialog.can_close_with_escape {
                    return Outcome::default();
                }
                let cancel = cancel_action(&dialog);
                self.press(&dialog, cancel, shared)
            }
        }
    }

    fn values(&self, dialog: &Dialog) -> HashMap<String, Value> {
        dialog
            .inputs
            .iter()
            .zip(&self.inputs)
            .filter_map(|(input, state)| Some((input.key.clone(), value(&input.control, state)?)))
            .collect()
    }

    fn press(
        &mut self,
        dialog: &Dialog,
        action: Option<&Action>,
        shared: &Arc<SharedMutex>,
    ) -> Outcome {
        let values = self.values(dialog);
        let click = action.and_then(|action| action.resolve(&values));
        let mut out = Outcome::default();
        match dialog.after_action {
            AfterAction::None => {}
            AfterAction::Close => out.nav = Some(self.parent),
            AfterAction::WaitForResponse => {
                self.phase = Phase::Waiting {
                    since: None,
                    active: false,
                };
            }
        }
        let Some(click) = click else { return out };
        self.dispatch(click, shared, &mut out);
        out
    }

    fn dispatch(&mut self, click: Click, shared: &Arc<SharedMutex>, out: &mut Outcome) {
        match clickevent::dispatch(click, shared) {
            Followup::None => {}
            Followup::Suggest(command) => out.chat = Some(command),
            Followup::Link(url) => {
                self.overlay = Overlay::Link(url);
                out.nav = None;
            }
            Followup::Dialog(dialog) => {
                self.open(dialog);
                out.nav = None;
            }
        }
    }
}

fn cancel_action(dialog: &Dialog) -> Option<&Action> {
    match &dialog.kind {
        Kind::Simple { actions } => actions.last()?.action.as_ref(),
        Kind::ButtonList { exit, .. } | Kind::ServerLinks { exit, .. } => {
            exit.as_ref()?.action.as_ref()
        }
    }
}

fn initial_state(input: &crate::play::dialog::Input) -> InputState {
    match &input.control {
        Control::Text {
            initial,
            max_length,
            ..
        } => {
            let mut field = TextBox::bordered(*max_length, "");
            field.set_text(initial);
            InputState::Text(field)
        }
        Control::Boolean { initial, .. } => InputState::Boolean(*initial),
        Control::SingleOption { initial, .. } => InputState::Choice {
            index: *initial,
            label: choice_label(&input.control, *initial),
        },
        Control::NumberRange { range, .. } => {
            let slider = range.initial_slider();
            InputState::Number {
                slider,
                label: slider_label(&input.control, slider),
            }
        }
    }
}

fn choice_label(control: &Control, index: usize) -> Vec<Span> {
    let Control::SingleOption {
        label,
        label_visible,
        options,
        ..
    } = control
    else {
        return Vec::new();
    };
    let chosen = options
        .get(index)
        .map(|option| option.display.clone())
        .unwrap_or_default();
    if !*label_visible {
        return chosen;
    }
    let mut spans = label.clone();
    spans.push(Span {
        text: ": ".to_string(),
        style: Style::default(),
    });
    spans.extend(chosen);
    spans
}

fn slider_label(control: &Control, slider: f32) -> Vec<Span> {
    let Control::NumberRange {
        label,
        label_format,
        range,
        ..
    } = control
    else {
        return Vec::new();
    };
    let plain: String = label.iter().map(|span| span.text.as_str()).collect();
    let value = RangeInfo::value_string(range.scaled(slider));
    vec![Span {
        text: tooltip::translate(label_format, &[plain, value]),
        style: label.first().map(|span| span.style).unwrap_or_default(),
    }]
}

fn value(control: &Control, state: &InputState) -> Option<Value> {
    Some(match (control, state) {
        (Control::Text { .. }, InputState::Text(field)) => Value::Text(field.text.clone()),
        (
            Control::Boolean {
                on_true, on_false, ..
            },
            InputState::Boolean(selected),
        ) => Value::Boolean {
            selected: *selected,
            on_true: on_true.clone(),
            on_false: on_false.clone(),
        },
        (Control::SingleOption { options, .. }, InputState::Choice { index, .. }) => {
            Value::Choice(options.get(*index)?.id.clone())
        }
        (Control::NumberRange { range, .. }, InputState::Number { slider, .. }) => {
            Value::Number(range.scaled(*slider))
        }
        _ => return None,
    })
}

enum ListButton<'a> {
    Data(&'a ActionButton),
    Link { link: &'a ServerLink, width: f32 },
}

impl ListButton<'_> {
    fn label(&self) -> &[Span] {
        match self {
            ListButton::Data(button) => &button.label,
            ListButton::Link { link, .. } => &link.label,
        }
    }

    fn tooltip(&self) -> Option<&[Span]> {
        match self {
            ListButton::Data(button) => button.tooltip.as_deref(),
            ListButton::Link { .. } => None,
        }
    }

    fn width(&self) -> f32 {
        match self {
            ListButton::Data(button) => button.width,
            ListButton::Link { width, .. } => *width,
        }
    }

    fn pressed(&self) -> Pressed<'_> {
        match self {
            ListButton::Data(button) => Pressed::Action(button.action.as_ref()),
            ListButton::Link { link, .. } => Pressed::Link(link.url.clone()),
        }
    }
}

enum Pressed<'a> {
    Action(Option<&'a Action>),
    Link(String),
}

pub fn draw(
    p: &mut Painter,
    state: &mut DialogState,
    ctx: &ScreenCtx,
    shared: &Arc<SharedMutex>,
) -> Outcome {
    let masked_input;
    let masked_ctx;
    let dialog_ctx = if state.overlay == Overlay::None {
        ctx
    } else {
        masked_input = confirmlink::masked(ctx.input);
        masked_ctx = ScreenCtx {
            input: &masked_input,
            vw: ctx.vw,
            vh: ctx.vh,
        };
        &masked_ctx
    };

    let mut out = match state.showing() {
        Some(dialog) => draw_dialog(p, state, dialog_ctx, shared, &dialog),
        None => draw_waiting(p, state, dialog_ctx),
    };
    if state.overlay != Overlay::None {
        draw_overlay(p, state, ctx, &mut out);
    }
    out
}

fn draw_dialog(
    p: &mut Painter,
    state: &mut DialogState,
    ctx: &ScreenCtx,
    shared: &Arc<SharedMutex>,
    dialog: &Arc<Dialog>,
) -> Outcome {
    let mut out = Outcome::default();

    let cached = match state.layout.take() {
        Some(layout) if layout.matches(dialog, ctx) => layout,
        _ => Layout::measure(p, dialog, ctx, shared),
    };
    let layout = cached.frame;
    let buttons = cached.buttons(dialog);

    if draw_header(p, ctx, dialog, layout) {
        state.overlay = Overlay::Warning;
    }

    let (list_x, list_y, list_w, list_h) = layout.content_rect(ctx.vw, ctx.vh);
    let rows = &cached.rows;
    let (content_w, content_h) = (cached.content_w, cached.content_h);
    let content_x = ((ctx.vw - content_w) / 2.0).floor();

    options::scroll_input(
        &mut state.scroll,
        &mut state.scroll_drag,
        ctx,
        content_x,
        content_w,
        list_y,
        list_h,
        content_h,
    );

    let over_list = ctx.hovering(list_x, list_y, list_w, list_h);
    let dragging_bar = state.scroll_drag.is_some();
    let mut list_input = ctx.input.clone();
    if dragging_bar || (!over_list && !widgets::slider_dragging()) {
        list_input.mouse = None;
    }
    if dragging_bar || !over_list {
        list_input.left_click = false;
    }
    let list_ctx = ScreenCtx {
        input: &list_input,
        vw: ctx.vw,
        vh: ctx.vh,
    };

    let top = if content_h <= list_h {
        layout.content_y(ctx.vh, content_h).floor()
    } else {
        (list_y - state.scroll).floor()
    };

    if ctx.input.left_click {
        for input in &mut state.inputs {
            if let InputState::Text(field) = input {
                field.focused = false;
            }
        }
    }

    let clip = p.push_clip(list_x, list_y, list_w, list_h);
    let mut y = top;
    let mut pressed: Option<Pressed> = None;
    let mut hovered_tooltip: Option<Vec<Span>> = None;
    let mut hovered_item: Option<crate::session::SlotStack> = None;
    for row in rows {
        match &row.kind {
            RowKind::Body(i) => {
                if let Some(stack) = draw_body(p, &list_ctx, &dialog.body[*i], y, row) {
                    hovered_item = Some(stack);
                }
            }
            RowKind::Input(i) => {
                draw_input(p, &list_ctx, dialog, state, *i, y, row);
            }
            RowKind::Buttons(range) => {
                let row_buttons = &buttons[range.clone()];
                let pitch = (row.w + GRID_SPACING) / row_buttons.len() as f32;
                let cell = pitch - GRID_SPACING;
                let mut x = ((ctx.vw - row.w) / 2.0).floor();
                for button in row_buttons {
                    let w = button.width().min(cell).max(1.0);
                    let bx = (x + (cell - w) / 2.0).floor();
                    if widgets::draw_button_spans(
                        p,
                        &list_ctx,
                        bx,
                        y,
                        w,
                        CONTROL_HEIGHT,
                        button.label(),
                        true,
                    ) {
                        pressed = Some(button.pressed());
                    }
                    if let Some(tip) = button.tooltip()
                        && list_ctx.hovering(bx, y, w, CONTROL_HEIGHT)
                    {
                        hovered_tooltip = Some(tip.to_vec());
                    }
                    x += pitch;
                }
            }
        }
        y += row.h + ELEMENT_SPACING;
    }
    p.pop_clip(clip);
    options::draw_scrollbar(
        p,
        state.scroll,
        content_x,
        content_w,
        list_y,
        list_h,
        content_h,
    );

    let footer: &[ActionButton] = match &dialog.kind {
        Kind::Simple { actions } => actions,
        Kind::ButtonList { exit, .. } | Kind::ServerLinks { exit, .. } => exit.as_slice(),
    };
    let footer_y = layout.footer_y(ctx.vh);
    let total: f32 = footer.iter().map(|button| button.width).sum::<f32>()
        + FOOTER_SPACING * footer.len().saturating_sub(1) as f32;
    let mut x = ((ctx.vw - total) / 2.0).floor();
    for button in footer {
        if widgets::draw_button_spans(
            p,
            ctx,
            x,
            footer_y,
            button.width,
            CONTROL_HEIGHT,
            &button.label,
            true,
        ) {
            pressed = Some(Pressed::Action(button.action.as_ref()));
        }
        if let Some(tip) = &button.tooltip
            && ctx.hovering(x, footer_y, button.width, CONTROL_HEIGHT)
        {
            hovered_tooltip = Some(tip.clone());
        }
        x += button.width + FOOTER_SPACING;
    }

    match pressed {
        Some(Pressed::Action(action)) => out = state.press(dialog, action, shared),
        Some(Pressed::Link(url)) => {
            let action = Action::Static(Click::OpenUrl(url));
            out = state.press(dialog, Some(&action), shared);
        }
        None => {
            if let Some(tip) = hovered_tooltip
                && let Some(m) = ctx.mouse()
            {
                tooltip::draw_lines(p, &[tip], m.x, m.y, ctx.vw, ctx.vh);
            }
            if let Some(stack) = hovered_item
                && let Some(m) = ctx.mouse()
            {
                tooltip::draw(p, &stack, m.x, m.y, ctx.vw, ctx.vh, false);
            }
        }
    }
    state.layout = Some(cached);
    out
}

fn draw_header(p: &mut Painter, ctx: &ScreenCtx, dialog: &Dialog, layout: HeaderFooter) -> bool {
    let title_w = p.atlas.font.width(&dialog.title);
    let group_w = title_w + TITLE_SPACING + WARNING_BUTTON_SIZE;
    let group_x = ((ctx.vw - group_w) / 2.0).floor();
    p.text(&dialog.title, group_x, layout.title_y(), true);

    let mut bx = group_x + title_w + TITLE_SPACING;
    let mut by = ((HeaderFooter::DEFAULT_H - WARNING_BUTTON_SIZE) / 2.0).round();
    if bx < 0.0
        || bx > ctx.vw - WARNING_BUTTON_SIZE
        || by < 0.0
        || by > ctx.vh - WARNING_BUTTON_SIZE
    {
        bx = (ctx.vw - 2.0 * WARNING_BUTTON_SIZE).max(0.0);
        by = 5.0f32.min(ctx.vh);
    }

    let hovered = ctx.hovering(bx, by, WARNING_BUTTON_SIZE, WARNING_BUTTON_SIZE);
    let sprite = if hovered {
        "dialog/warning_button_highlighted"
    } else {
        "dialog/warning_button"
    };
    p.sprite(sprite, bx, by, WARNING_BUTTON_SIZE, WARNING_BUTTON_SIZE);
    hovered && ctx.input.left_click
}

struct Layout {
    dialog: usize,
    vw: f32,
    vh: f32,
    links: Vec<ServerLink>,
    rows: Vec<Row>,
    frame: HeaderFooter,
    content_w: f32,
    content_h: f32,
}

impl Layout {
    fn matches(&self, dialog: &Arc<Dialog>, ctx: &ScreenCtx) -> bool {
        self.dialog == Arc::as_ptr(dialog) as usize && self.vw == ctx.vw && self.vh == ctx.vh
    }

    fn measure(
        p: &Painter,
        dialog: &Arc<Dialog>,
        ctx: &ScreenCtx,
        shared: &Arc<SharedMutex>,
    ) -> Layout {
        let links = match dialog.kind {
            Kind::ServerLinks { .. } => shared.lock().unwrap().session.server_links.clone(),
            _ => Vec::new(),
        };
        let columns = match dialog.kind {
            Kind::Simple { .. } => 1,
            Kind::ButtonList { columns, .. } | Kind::ServerLinks { columns, .. } => columns.max(1),
        };
        let footer_h = match &dialog.kind {
            Kind::Simple { .. } => HeaderFooter::DEFAULT_H,
            Kind::ButtonList { exit, .. } | Kind::ServerLinks { exit, .. } => {
                if exit.is_some() {
                    HeaderFooter::DEFAULT_H
                } else {
                    FOOTER_MARGIN
                }
            }
        };

        let rows = measure_rows(p, dialog, &list_buttons(dialog, &links), columns);
        let content_h = rows
            .iter()
            .map(|row| row.h + ELEMENT_SPACING)
            .sum::<f32>()
            .max(ELEMENT_SPACING)
            - ELEMENT_SPACING;
        let content_w = rows.iter().map(|row| row.w).fold(0.0f32, f32::max);

        Layout {
            dialog: Arc::as_ptr(dialog) as usize,
            vw: ctx.vw,
            vh: ctx.vh,
            links,
            rows,
            frame: HeaderFooter::new(HeaderFooter::DEFAULT_H, footer_h),
            content_w,
            content_h,
        }
    }

    fn buttons<'a>(&'a self, dialog: &'a Dialog) -> Vec<ListButton<'a>> {
        list_buttons(dialog, &self.links)
    }
}

fn list_buttons<'a>(dialog: &'a Dialog, links: &'a [ServerLink]) -> Vec<ListButton<'a>> {
    match &dialog.kind {
        Kind::Simple { .. } => Vec::new(),
        Kind::ButtonList { buttons, .. } => buttons.iter().map(ListButton::Data).collect(),
        Kind::ServerLinks { button_width, .. } => links
            .iter()
            .map(|link| ListButton::Link {
                link,
                width: *button_width,
            })
            .collect(),
    }
}

enum RowKind {
    Body(usize),
    Input(usize),
    Buttons(std::ops::Range<usize>),
}

struct Row {
    kind: RowKind,
    w: f32,
    h: f32,
    lines: Vec<Line>,
}

struct Line {
    spans: Vec<Span>,
    width: f32,
}

fn wrap(font: &crate::text::Font, spans: &[Span], max_width: f32) -> Vec<Line> {
    font.wrap(spans, max_width)
        .into_iter()
        .map(|spans| Line {
            width: font.width(&spans),
            spans,
        })
        .collect()
}

fn measure_rows(p: &Painter, dialog: &Dialog, buttons: &[ListButton], columns: u32) -> Vec<Row> {
    let font = &p.atlas.font;
    let mut rows = Vec::new();

    for (i, body) in dialog.body.iter().enumerate() {
        match body {
            Body::Plain { contents, width } => {
                let lines = wrap(font, contents, *width);
                rows.push(Row {
                    kind: RowKind::Body(i),
                    w: *width,
                    h: (lines.len().max(1) as f32) * LINE_HEIGHT,
                    lines,
                });
            }
            Body::Item {
                description,
                width,
                height,
                ..
            } => {
                let (lines, text_w) = match description {
                    Some((spans, max)) => {
                        let lines = wrap(font, spans, *max);
                        (lines, *max)
                    }
                    None => (Vec::new(), 0.0),
                };
                let text_h = lines.len() as f32 * LINE_HEIGHT;
                let w = if lines.is_empty() {
                    *width
                } else {
                    width + ITEM_DESCRIPTION_SPACING + text_w
                };
                rows.push(Row {
                    kind: RowKind::Body(i),
                    w,
                    h: height.max(text_h),
                    lines,
                });
            }
        }
    }

    for (i, input) in dialog.inputs.iter().enumerate() {
        let (w, control_h, label) = match &input.control {
            Control::Text {
                width,
                label,
                label_visible,
                multiline,
                ..
            } => (
                *width,
                multiline
                    .as_ref()
                    .map(|m| m.height)
                    .unwrap_or(CONTROL_HEIGHT),
                label_visible.then(|| label.clone()),
            ),
            Control::SingleOption { width, .. } => (*width, CONTROL_HEIGHT, None),
            Control::NumberRange { width, .. } => (*width, CONTROL_HEIGHT, None),
            Control::Boolean { label, .. } => (
                CHECKBOX_SIZE + CHECKBOX_SPACING + font.width(label),
                CHECKBOX_SIZE,
                None,
            ),
        };
        let label_h = label
            .as_ref()
            .map(|_| LINE_HEIGHT + LABEL_SPACING)
            .unwrap_or(0.0);
        rows.push(Row {
            kind: RowKind::Input(i),
            w,
            h: label_h + control_h,
            lines: label
                .map(|spans| Line {
                    width: font.width(&spans),
                    spans,
                })
                .into_iter()
                .collect(),
        });
    }

    let cell = buttons.iter().map(|b| b.width()).fold(0.0f32, f32::max);
    for start in (0..buttons.len()).step_by(columns as usize) {
        let end = (start + columns as usize).min(buttons.len());
        let count = (end - start) as f32;
        rows.push(Row {
            kind: RowKind::Buttons(start..end),
            w: cell * count + GRID_SPACING * (count - 1.0),
            h: CONTROL_HEIGHT,
            lines: Vec::new(),
        });
    }
    rows
}

fn draw_body(
    p: &mut Painter,
    ctx: &ScreenCtx,
    body: &Body,
    y: f32,
    row: &Row,
) -> Option<crate::session::SlotStack> {
    match body {
        Body::Plain { .. } => {
            for (i, line) in row.lines.iter().enumerate() {
                p.text(
                    &line.spans,
                    ((ctx.vw - line.width) / 2.0).floor(),
                    y + i as f32 * LINE_HEIGHT,
                    true,
                );
            }
        }
        Body::Item {
            stack,
            show_decorations,
            show_tooltip,
            width,
            height,
            ..
        } => {
            let x = ((ctx.vw - row.w) / 2.0).floor();
            let scale = (width / 16.0).min(height / 16.0);
            let iy = (y + (row.h - height) / 2.0).floor();
            p.scaled(scale, x, iy, |p| {
                p.item_icon(&stack.model_key(), 0.0, 0.0);
                if *show_decorations {
                    p.item_decorations(0.0, 0.0, stack.count, stack.damage, stack.max_damage);
                }
            });
            if !row.lines.is_empty() {
                let tx = x + width + ITEM_DESCRIPTION_SPACING;
                let text_h = row.lines.len() as f32 * LINE_HEIGHT;
                let mut ty = (y + (row.h - text_h) / 2.0).floor();
                for line in &row.lines {
                    p.text(&line.spans, tx, ty, true);
                    ty += LINE_HEIGHT;
                }
            }
            if *show_tooltip && ctx.hovering(x, iy, *width, *height) {
                return Some(stack.clone());
            }
        }
    }
    None
}

fn draw_input(
    p: &mut Painter,
    ctx: &ScreenCtx,
    dialog: &Dialog,
    state: &mut DialogState,
    i: usize,
    y: f32,
    row: &Row,
) {
    let control = &dialog.inputs[i].control;
    let x = ((ctx.vw - row.w) / 2.0).floor();
    let mut cy = y;
    if let Some(label) = row.lines.first() {
        p.text(
            &label.spans,
            ((ctx.vw - label.width) / 2.0).floor(),
            cy,
            true,
        );
        cy += LINE_HEIGHT + LABEL_SPACING;
    }
    let control_h = row.h - (cy - y);

    match (control, state.inputs.get_mut(i)) {
        (Control::Text { .. }, Some(InputState::Text(field))) => {
            field.update(p, ctx, x, cy, row.w, control_h);
        }
        (Control::Boolean { label, .. }, Some(InputState::Boolean(selected))) => {
            let hovered = ctx.hovering(x, cy, row.w, control_h);
            let focused = focus::next(true);
            let sprite = match (*selected, hovered || focused) {
                (true, true) => "widget/checkbox_selected_highlighted",
                (true, false) => "widget/checkbox_selected",
                (false, true) => "widget/checkbox_highlighted",
                (false, false) => "widget/checkbox",
            };
            p.sprite(sprite, x, cy, CHECKBOX_SIZE, CHECKBOX_SIZE);
            let ty = (cy + CHECKBOX_SIZE / 2.0 - LINE_HEIGHT / 2.0).floor();
            p.text(label, x + CHECKBOX_SIZE + CHECKBOX_SPACING, ty, true);
            let clicked = hovered && ctx.input.left_click;
            if clicked {
                focus::claim();
            }
            if clicked || (focused && ctx.input.select) {
                *selected = !*selected;
            }
        }
        (Control::SingleOption { options, .. }, Some(InputState::Choice { index, label })) => {
            if widgets::draw_button_spans(p, ctx, x, cy, row.w, control_h, label, true) {
                *index = (*index + 1) % options.len().max(1);
                *label = choice_label(control, *index);
            }
        }
        (Control::NumberRange { .. }, Some(InputState::Number { slider, label })) => {
            let hovered = ctx.hovering(x, cy, row.w, control_h);
            let focused = focus::next(true);
            let id = SLIDER_ID_BASE + i as u32;
            let tracking = widgets::slider_track_cursor(id, ctx.input.scale, hovered, ctx.input);
            if tracking {
                focus::claim();
            }
            if tracking && let Some(m) = ctx.mouse() {
                let travel = (row.w - HANDLE_WIDTH).max(1.0);
                let moved = ((m.x - (x + HANDLE_WIDTH / 2.0)) / travel).clamp(0.0, 1.0);
                if moved != *slider {
                    *slider = moved;
                    *label = slider_label(control, moved);
                }
            }
            p.sprite(
                if focused {
                    "widget/slider_highlighted"
                } else {
                    "widget/slider"
                },
                x,
                cy,
                row.w,
                control_h,
            );
            let handle = if hovered || tracking {
                "widget/slider_handle_highlighted"
            } else {
                "widget/slider_handle"
            };
            p.sprite(
                handle,
                x + (*slider * (row.w - HANDLE_WIDTH)).floor(),
                cy,
                HANDLE_WIDTH,
                control_h,
            );
            widgets::draw_label_spans(p, label, x, cy, row.w, control_h);
        }
        _ => {}
    }
}

fn draw_waiting(p: &mut Painter, state: &mut DialogState, ctx: &ScreenCtx) -> Outcome {
    let mut out = Outcome::default();
    let since = match state.phase {
        Phase::Waiting {
            since: Some(since), ..
        } => since,
        _ => {
            state.phase = Phase::Waiting {
                since: Some(ctx.input.time),
                active: false,
            };
            ctx.input.time
        }
    };
    let elapsed = (ctx.input.time - since).max(0.0);

    let layout = HeaderFooter::new(HeaderFooter::DEFAULT_H, 0.0);
    widgets::draw_title(
        p,
        ctx.vw,
        layout.title_y(),
        &tooltip::translate("gui.waitingForResponse.title", &[]),
    );

    let active = elapsed >= WAIT_BUTTON_ACTIVE_SECS;
    state.phase = Phase::Waiting {
        since: Some(since),
        active,
    };
    if elapsed < WAIT_BUTTON_VISIBLE_SECS {
        return out;
    }
    let label = if active {
        tooltip::translate("gui.back", &[])
    } else {
        let remaining = (WAIT_BUTTON_ACTIVE_SECS - elapsed).ceil().max(1.0) as i32;
        tooltip::translate(
            "gui.waitingForResponse.button.inactive",
            &[remaining.to_string()],
        )
    };
    let x = ((ctx.vw - WAIT_BUTTON_WIDTH) / 2.0).floor();
    let y = layout.content_y(ctx.vh, CONTROL_HEIGHT).floor();
    let spans = [Span {
        text: label,
        style: Style::default(),
    }];
    if widgets::draw_button_spans(
        p,
        ctx,
        x,
        y,
        WAIT_BUTTON_WIDTH,
        CONTROL_HEIGHT,
        &spans,
        active,
    ) {
        out.nav = Some(state.clear());
    }
    out
}

fn draw_overlay(p: &mut Painter, state: &mut DialogState, ctx: &ScreenCtx, out: &mut Outcome) {
    match &state.overlay {
        Overlay::None => (),
        Overlay::Link(url) => {
            let url = url.clone();
            if confirmlink::draw(p, ctx, &url) {
                state.overlay = Overlay::None;
            }
        }
        Overlay::Warning => {
            crate::gui::screens::dim_background(p, ctx.vw, ctx.vh);
            let buttons = [
                tooltip::translate("menu.disconnect", &[]),
                tooltip::translate("gui.back", &[]),
            ];
            let chosen = confirmlink::draw_box(
                p,
                ctx,
                &tooltip::translate("menu.custom_screen_info.title", &[]),
                &tooltip::translate("menu.custom_screen_info.contents", &[]),
                None,
                &buttons,
                CONFIRM_BUTTON_WIDTH,
            );
            let Some(chosen) = chosen else { return };
            state.overlay = Overlay::None;
            if chosen == 0 {
                out.disconnect = true;
                out.nav = Some(Screen::Title);
            }
        }
    }
}
