use crate::client::language::{self, ENGLISH, Fetch, Language, ListFetch};
use crate::gui::focus;
use crate::gui::menu;
use crate::gui::options::{draw_scrollbar, row_visible, scroll_masked};
use crate::gui::painter::Painter;
use crate::gui::tooltip::{label, translate};
use crate::gui::widgets::{
    Button, FOOTER_GAP, FOOTER_W, HeaderFooter, TextBox, WIDGET_HEIGHT, centered_row_x,
};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::text::LINE_HEIGHT;

const HEADER_H: f32 = 12.0 + LINE_HEIGHT + SEARCH_H;
const FOOTER_H: f32 = 53.0;
const LAYOUT: HeaderFooter = HeaderFooter::new(HEADER_H, FOOTER_H);
const HEADER_GAP: f32 = 4.0;
const SEARCH_H: f32 = 15.0;
const SEARCH_W: f32 = 200.0;
const SEARCH_MAX_LEN: usize = 32;
const TITLE_Y: f32 = (HEADER_H - (LINE_HEIGHT + HEADER_GAP + SEARCH_H)) / 2.0;
const SEARCH_Y: f32 = TITLE_Y + LINE_HEIGHT + HEADER_GAP;

const ROW_H: f32 = 18.0;
const ROW_W: f32 = 270.0;
const LIST_PAD: f32 = 2.0;

const WARNING: u32 = 0xBA_BABA;
const ERROR: u32 = 0xFF_5555;
const PROGRESS: u32 = crate::text::GRAY;

const APPLY: &str = "Apply";
const DOWNLOADING: &str = "Downloading...";
const LIST_LOADING: &str = "Downloading the language list...";
const LIST_FAILED: &str = "No language list";

pub struct State {
    search: TextBox,
    selected: String,
    scroll: f32,
    scroll_drag: Option<f32>,
    center_pending: bool,
}

impl Default for State {
    fn default() -> State {
        State {
            search: TextBox::bordered(SEARCH_MAX_LEN, ""),
            selected: ENGLISH.to_owned(),
            scroll: 0.0,
            scroll_drag: None,
            center_pending: false,
        }
    }
}

impl State {
    pub fn open(&mut self) {
        language::retry();
        language::request_list();
        let mut search = TextBox::bordered(SEARCH_MAX_LEN, &translate("gui.language.search", &[]));
        search.focus();
        *self = State {
            search,
            selected: language::current(),
            scroll: 0.0,
            scroll_drag: None,
            center_pending: true,
        };
    }
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    menu::background(p, ctx.vw, ctx.vh);
    let st = &mut state.language;

    p.text_centered(
        label!("options.language.title"),
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

    let list_fetch = language::list_fetch();
    let languages = language::list().unwrap_or_else(language::english_only);
    let needle = st.search.text.to_lowercase();
    let shown: Vec<&Language> = languages
        .iter()
        .filter(|l| {
            needle.is_empty()
                || l.name.to_lowercase().contains(&needle)
                || l.region.to_lowercase().contains(&needle)
        })
        .collect();

    let (lx, ly, lw, lh) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let left = ((ctx.vw / 2.0).floor() - ROW_W / 2.0).floor();
    let content_h = shown.len() as f32 * ROW_H + 2.0 * LIST_PAD;
    let max_scroll = (content_h - lh).max(0.0);
    if st.center_pending {
        st.center_pending = list_fetch == ListFetch::Fetching;
        if let Some(i) = shown.iter().position(|l| l.code == st.selected) {
            st.scroll = (i as f32 * ROW_H + ROW_H / 2.0 - lh / 2.0).clamp(0.0, max_scroll);
        }
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
    let mut confirm = false;
    let top = (ly + LIST_PAD - st.scroll).floor();
    let clip = p.push_clip(lx, ly, lw, lh);
    for (i, lang) in shown.iter().enumerate() {
        let y = top + i as f32 * ROW_H;
        if !row_visible(y, ROW_H, ly, lh) {
            continue;
        }
        if lang.code == st.selected {
            let border = if list_focused {
                0xFFFF_FFFF
            } else {
                0xFF80_8080
            };
            p.fill(left, y, ROW_W, ROW_H, border);
            p.fill(left + 1.0, y + 1.0, ROW_W - 2.0, ROW_H - 2.0, 0xFF00_0000);
        }
        let text_y =
            y + LIST_PAD + ((ROW_H - 2.0 * LIST_PAD) / 2.0).floor() - (LINE_HEIGHT / 2.0).floor();
        p.text_centered(
            &lang.label(),
            (ctx.vw / 2.0).floor(),
            text_y,
            0xFF_FFFF,
            true,
        );
        if list_ctx.input.left_click && list_ctx.hovering(left, y, ROW_W, ROW_H) {
            st.selected = lang.code.clone();
            focus::claim();
            confirm |= list_ctx.input.double_click;
        }
    }
    p.pop_clip(clip);
    draw_scrollbar(p, st.scroll, left, ROW_W, ly, lh, content_h);
    confirm |= list_focused && focus::by_keyboard() && ctx.input.select;

    let footer_top =
        ctx.vh - FOOTER_H + ((FOOTER_H - (LINE_HEIGHT + FOOTER_GAP + WIDGET_HEIGHT)) / 2.0).floor();
    let fetch = language::fetch();
    let name_of = |code: &str| {
        languages
            .iter()
            .find(|l| l.code == code)
            .map_or_else(|| code.to_owned(), |l| l.name.clone())
    };
    let (line, color) = match &fetch {
        Fetch::Fetching(code) => (format!("Downloading {}...", name_of(code)), PROGRESS),
        Fetch::Failed(code, why) if *code == st.selected => {
            (format!("{}: {why}", name_of(code)), ERROR)
        }
        _ => match list_fetch {
            ListFetch::Fetching => (LIST_LOADING.to_owned(), PROGRESS),
            ListFetch::Failed(why) => (format!("{LIST_FAILED}: {why}"), ERROR),
            ListFetch::Idle => (
                label!("options.languageAccuracyWarning").to_owned(),
                WARNING,
            ),
        },
    };
    p.text_centered(&line, ctx.vw / 2.0, footer_top, color, true);

    let buttons_y = footer_top + LINE_HEIGHT + FOOTER_GAP;
    let x0 = centered_row_x(ctx.vw, 2.0 * FOOTER_W + FOOTER_GAP);
    Button {
        active: false,
        ..Button::new(
            x0,
            buttons_y,
            FOOTER_W,
            WIDGET_HEIGHT,
            label!("options.font"),
        )
    }
    .draw(p, ctx);

    let fetching = matches!(fetch, Fetch::Fetching(_));
    let changed = st.selected != language::current();
    let done_label = if fetching {
        DOWNLOADING
    } else if changed {
        APPLY
    } else {
        label!("gui.done")
    };
    let pressed = Button {
        active: !fetching,
        ..Button::new(
            x0 + FOOTER_W + FOOTER_GAP,
            buttons_y,
            FOOTER_W,
            WIDGET_HEIGHT,
            done_label,
        )
    }
    .draw(p, ctx);

    if (pressed || confirm) && !fetching {
        if changed {
            if let Some(lang) = languages.iter().find(|l| l.code == st.selected) {
                language::apply(lang);
            }
        } else {
            state.nav = Some(Screen::Title);
        }
    }
    if ctx.input.escape {
        state.nav = Some(Screen::Title);
    }
}
