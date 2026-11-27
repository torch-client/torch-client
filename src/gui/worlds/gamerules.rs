use std::collections::BTreeMap;

use crate::gui::menu;
use crate::gui::options::{draw_scrollbar, row_visible, scroll_masked};
use crate::gui::painter::Painter;
use crate::gui::tooltip::{label, translate};
use crate::gui::widgets::{
    Button, FOOTER_GAP, FOOTER_W, HeaderFooter, TextBox, WIDGET_HEIGHT, centered_row_x,
};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::singleplayer::gamerules::{self, Kind, RULES};
use crate::text::{GRAY, LINE_HEIGHT, Span, Style, YELLOW};

const SEARCH_H: f32 = 15.0;
const SEARCH_W: f32 = 200.0;
const HEADER_H: f32 = 12.0 + LINE_HEIGHT + SEARCH_H;
const LAYOUT: HeaderFooter = HeaderFooter::new(HEADER_H, HeaderFooter::DEFAULT_H);
const HEADER_GAP: f32 = 4.0;
const TITLE_Y: f32 = (HEADER_H - (LINE_HEIGHT + HEADER_GAP + SEARCH_H)) / 2.0;
const SEARCH_Y: f32 = TITLE_Y + LINE_HEIGHT + HEADER_GAP;
const MAX_LEN: usize = 32;

const ROW_H: f32 = 24.0;
const ROW_W: f32 = 220.0;
const PAD: f32 = 2.0;
const WIDGET_W: f32 = 44.0;
const WIDGET_FROM_RIGHT: f32 = 45.0;
const LABEL_W: f32 = 170.0;
const DESCRIPTION_W: f32 = 150.0;
const VALID: u32 = 0xE0_E0E0;
const INVALID: u32 = 0xFF_0000;

struct Texts {
    label: Vec<Vec<Span>>,
    description: Option<String>,
    search: [String; 4],
}

#[derive(Default)]
pub struct State {
    search: TextBox,
    last_search: String,
    values: Vec<String>,
    boxes: Vec<TextBox>,
    texts: Vec<Texts>,
    scroll: f32,
    scroll_drag: Option<f32>,
    pub result: Option<BTreeMap<String, String>>,
}

impl State {
    pub fn open(&mut self, changed: &BTreeMap<String, String>) {
        let mut search = TextBox::bordered(MAX_LEN, &translate("gui.game_rule.search", &[]));
        search.focus();
        let values: Vec<String> = RULES
            .iter()
            .map(|rule| {
                changed
                    .get(rule.id)
                    .and_then(|v| gamerules::parse(rule, v))
                    .unwrap_or_else(|| rule.default.to_owned())
            })
            .collect();
        let boxes = RULES
            .iter()
            .zip(&values)
            .map(|(rule, value)| {
                let mut b = TextBox::bordered(MAX_LEN, "");
                if matches!(rule.kind, Kind::Int { .. }) {
                    b.set_text(value);
                }
                b
            })
            .collect();
        *self = State {
            search,
            last_search: String::new(),
            values,
            boxes,
            texts: Vec::new(),
            scroll: 0.0,
            scroll_drag: None,
            result: None,
        };
    }
}

enum Row {
    Category(&'static str),
    Rule(usize),
}

enum Action {
    Done,
    Cancel,
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    menu::background(p, ctx.vw, ctx.vh);
    let st = &mut state.worlds.gamerules;
    if st.values.len() != RULES.len() {
        st.open(&BTreeMap::new());
    }
    if st.texts.len() != RULES.len() {
        st.texts = RULES.iter().map(|rule| texts(p, rule)).collect();
    }

    p.text_centered(
        label!("editGamerule.title"),
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
    if st.search.text != st.last_search {
        st.last_search.clone_from(&st.search.text);
        st.scroll = 0.0;
    }

    let rows = filtered_rows(&st.texts, &st.search.text.to_lowercase());
    let (lx, ly, lw, lh) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let left = (ctx.vw / 2.0).floor() - ROW_W / 2.0;
    let content_h = rows.len() as f32 * ROW_H + 2.0 * PAD;
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

    let content_x = left + PAD;
    let content_w = ROW_W - 2.0 * PAD;
    let widget_x = content_x + content_w - WIDGET_FROM_RIGHT;
    let top = (ly + PAD - st.scroll).floor();
    let mut hovered = None;
    let clip = p.push_clip(lx, ly, lw, lh);
    for (n, row) in rows.iter().enumerate() {
        let y = top + n as f32 * ROW_H;
        let content_y = y + PAD;
        let visible = row_visible(y, ROW_H, ly, lh);
        match *row {
            Row::Category(key) => {
                if visible {
                    let spans = [Span {
                        text: translate(key, &[]),
                        style: Style {
                            color: YELLOW,
                            bold: true,
                            ..Style::default()
                        },
                    }];
                    let w = p.atlas.font.width(&spans);
                    let x = content_x + (content_w / 2.0).floor() - (w / 2.0).floor();
                    p.text(&spans, x, content_y + 5.0, true);
                }
            }
            Row::Rule(i) => {
                if visible {
                    draw_label(p, &st.texts[i].label, content_x, content_y);
                    if list_ctx.hovering(left, y, ROW_W, ROW_H) {
                        hovered = Some(i);
                    }
                }
                match RULES[i].kind {
                    Kind::Bool => {
                        let on = st.values[i] == "true";
                        let text = if on {
                            label!("options.on")
                        } else {
                            label!("options.off")
                        };
                        if Button::new(widget_x, content_y, WIDGET_W, WIDGET_HEIGHT, text)
                            .draw(p, &list_ctx)
                        {
                            st.values[i] = (!on).to_string();
                        }
                    }
                    Kind::Int { .. } => {
                        st.boxes[i].update(
                            p,
                            &list_ctx,
                            widget_x,
                            content_y,
                            WIDGET_W,
                            WIDGET_HEIGHT,
                        );
                    }
                }
            }
        }
    }
    p.pop_clip(clip);
    draw_scrollbar(p, st.scroll, left, ROW_W, ly, lh, content_h);

    let mut invalid = false;
    for (i, rule) in RULES.iter().enumerate() {
        if !matches!(rule.kind, Kind::Int { .. }) {
            continue;
        }
        let b = &mut st.boxes[i];
        match gamerules::parse(rule, &b.text) {
            Some(value) => {
                b.color = VALID;
                st.values[i] = value;
            }
            None => {
                b.color = INVALID;
                invalid = true;
            }
        }
    }

    let footer_y = LAYOUT.footer_y(ctx.vh);
    let x0 = centered_row_x(ctx.vw, 2.0 * FOOTER_W + FOOTER_GAP);
    let mut action = None;
    let done = Button {
        active: !invalid,
        ..Button::new(x0, footer_y, FOOTER_W, WIDGET_HEIGHT, label!("gui.done"))
    };
    if done.draw(p, ctx) {
        action = Some(Action::Done);
    }
    let cancel = Button::new(
        x0 + FOOTER_W + FOOTER_GAP,
        footer_y,
        FOOTER_W,
        WIDGET_HEIGHT,
        label!("gui.cancel"),
    );
    if cancel.draw(p, ctx) || ctx.input.escape {
        action = Some(Action::Cancel);
    }

    if let (Some(i), Some(m)) = (hovered, ctx.mouse()) {
        let lines = tooltip(p, i, &st.texts[i]);
        crate::gui::tooltip::draw_lines(p, &lines, m.x, m.y, ctx.vw, ctx.vh);
    }

    match action {
        Some(Action::Done) => {
            st.result = Some(
                RULES
                    .iter()
                    .zip(&st.values)
                    .filter(|(rule, value)| value.as_str() != rule.default)
                    .map(|(rule, value)| (rule.id.to_owned(), value.clone()))
                    .collect(),
            );
            state.nav = Some(Screen::CreateWorld);
        }
        Some(Action::Cancel) => state.nav = Some(Screen::CreateWorld),
        None => {}
    }
}

fn texts(p: &Painter, rule: &gamerules::Rule) -> Texts {
    let name = translate(rule.description_id, &[]);
    let key = format!("{}.description", rule.description_id);
    let description = Some(translate(&key, &[])).filter(|d| *d != key);
    let label = p.atlas.font.wrap(&[Span::new(name.clone())], LABEL_W);
    let search = [
        rule.id.to_lowercase(),
        name.to_lowercase(),
        translate(rule.category, &[]).to_lowercase(),
        description
            .as_deref()
            .map(str::to_lowercase)
            .unwrap_or_default(),
    ];
    Texts {
        label,
        description,
        search,
    }
}

fn filtered_rows(texts: &[Texts], needle: &str) -> Vec<Row> {
    let mut shown: Vec<usize> = (0..RULES.len())
        .filter(|&i| texts[i].search.iter().any(|s| s.contains(needle)))
        .collect();
    shown.sort_by_key(|&i| (RULES[i].category, RULES[i].description_id));
    let mut rows = Vec::with_capacity(shown.len() + 7);
    let mut category = None;
    for i in shown {
        if category != Some(RULES[i].category) {
            category = Some(RULES[i].category);
            rows.push(Row::Category(RULES[i].category));
        }
        rows.push(Row::Rule(i));
    }
    rows
}

fn draw_label(p: &mut Painter, label: &[Vec<Span>], x: f32, y: f32) {
    match label {
        [] => {}
        [line] => {
            p.text(line, x, y + 5.0, true);
        }
        [first, second, ..] => {
            p.text(first, x, y, true);
            p.text(second, x, y + 10.0, true);
        }
    }
}

fn tooltip(p: &Painter, i: usize, texts: &Texts) -> Vec<Vec<Span>> {
    let rule = &RULES[i];
    let mut lines = vec![vec![Span::colored(rule.id, YELLOW)]];
    if let Some(description) = &texts.description {
        lines.extend(
            p.atlas
                .font
                .wrap(&[Span::new(description.clone())], DESCRIPTION_W),
        );
    }
    let default = translate("editGamerule.default", &[rule.default.to_owned()]);
    lines.push(vec![Span::colored(default, GRAY)]);
    lines
}
