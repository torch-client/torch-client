use crate::gui::painter::Painter;
use crate::gui::render::GuiInput;
use crate::gui::widgets::{self, Button, Slider};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::shaderpack::lang::{self, Lang};
use crate::shaderpack::options::{Kind, Opt, Options, Values};
use crate::shaderpack::properties::{self, Element, Properties, ROOT_SCREEN};
use crate::shaderpack::{discover, features, source::Source};

use super::options::{
    BIG_W, COLUMN_STEP, COLUMN_W, GRID_LEFT_OFFSET, LAYOUT, ROW_H, ROW_INSET, draw_done,
    draw_title, scroll_input,
};
use super::widgets::WIDGET_HEIGHT;

const PACKS_TITLE: &str = "Shader Packs";
pub(in crate::gui) const SHADER_PACKS: &str = "Shader Packs...";
const NONE_ROW: &str = "(none)";
const NO_PACKS: &str = "No packs in the shaders folder";
const SETTINGS: &str = "Settings...";
const RESET: &str = "Reset";
const PROFILE: &str = "Profile";
const CUSTOM: &str = "Custom";
const ON: &str = "ON";
const OFF: &str = "OFF";

const LIST_ROW_H: f32 = 20.0;

const SLIDER_ID_BASE: u32 = 100;

pub struct Loaded {
    pub name: String,
    options: Options,
    values: Values,
    props: Properties,
    lang: Lang,
    profile: Option<usize>,
}

impl Loaded {
    fn option_label(&self, name: &str) -> String {
        self.lang
            .option(name)
            .map_or_else(|| lang::humanise(name), str::to_owned)
    }

    fn value_label(&self, name: &str, value: &str) -> String {
        self.lang.value(name, value).unwrap_or(value).to_owned()
    }
}

#[derive(Default)]
pub struct State {
    packs: Vec<discover::Pack>,
    pub loaded: Option<Loaded>,
    nav: Vec<String>,
    scroll: f32,
    scroll_drag: Option<f32>,
    list_scroll: f32,
    list_scroll_drag: Option<f32>,
    error: Option<String>,
    revision: u64,
}

impl State {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn refresh(&mut self) {
        self.packs = discover::list();
        self.list_scroll = 0.0;
    }

    pub fn open_options(&mut self) {
        self.nav.clear();
        self.scroll = 0.0;
    }

    fn current_screen(&self) -> &str {
        self.nav.last().map_or(ROOT_SCREEN, String::as_str)
    }

    pub fn pop_screen(&mut self) -> bool {
        self.scroll = 0.0;
        self.nav.pop().is_some()
    }
}

fn load(name: &str) -> Result<Loaded, String> {
    let source = Source::open(&discover::dir().join(name))?;

    let mut defines = features::base_defines();
    let props = match source.read_text("/shaders.properties") {
        Some(text) => properties::parse(&text, &mut defines),
        None => Properties::default(),
    };

    let unmet = features::unmet(props.features("iris.features.required"));
    if !unmet.is_empty() {
        return Err(format!("needs {}", unmet.join(", ")));
    }

    let texts: Vec<String> = source
        .files()
        .into_iter()
        .filter(|f| {
            [".glsl", ".vsh", ".fsh", ".gsh", ".csh"]
                .iter()
                .any(|e| f.ends_with(e))
        })
        .filter_map(|f| source.read_text(&f))
        .collect();
    let options = crate::shaderpack::options::discover(texts.iter().map(String::as_str));

    let values = match std::fs::read_to_string(values_path(name)) {
        Ok(text) => Values::load(&text, &options),
        Err(_) => Values::default(),
    };

    let mut loaded = Loaded {
        name: name.to_owned(),
        options,
        values,
        props,
        lang: Lang::load(&source),
        profile: None,
    };
    loaded.profile = matching_profile(&loaded);
    Ok(loaded)
}

fn values_path(name: &str) -> std::path::PathBuf {
    discover::dir().join(format!("{name}.txt"))
}

fn save(loaded: &Loaded) {
    let at = values_path(&loaded.name);
    if loaded.values.is_empty() {
        let _ = std::fs::remove_file(&at);
        return;
    }
    if let Err(e) = std::fs::write(&at, loaded.values.save()) {
        crate::log_warn!("shaders", "could not save {}: {e}", at.display());
    }
}

fn matching_profile(loaded: &Loaded) -> Option<usize> {
    (0..loaded.props.profiles.len()).find(|at| profile_matches(loaded, *at))
}

fn profile_matches(loaded: &Loaded, at: usize) -> bool {
    let mut wanted = Values::default();
    if !apply_profile(loaded, at, &mut wanted) {
        return false;
    }
    loaded
        .options
        .iter()
        .all(|opt| wanted.get(opt) == loaded.values.get(opt))
}

fn apply_profile(loaded: &Loaded, at: usize, into: &mut Values) -> bool {
    apply_profile_depth(loaded, at, into, 0)
}

fn apply_profile_depth(loaded: &Loaded, at: usize, into: &mut Values, depth: u32) -> bool {
    if depth > 8 {
        return false;
    }
    let Some((_, profile)) = loaded.props.profiles.get(at) else {
        return false;
    };
    for token in &profile.tokens {
        if let Some(name) = token.strip_prefix("profile.") {
            match loaded.props.profiles.iter().position(|(n, _)| n == name) {
                Some(other) => {
                    apply_profile_depth(loaded, other, into, depth + 1);
                }
                None => return false,
            }
            continue;
        }
        let (name, value) = match token.split_once('=') {
            Some((name, value)) => (name, value.to_owned()),
            None => match token.strip_prefix('!') {
                Some(name) => (name, OFF_VALUE.to_owned()),
                None => (token.as_str(), ON_VALUE.to_owned()),
            },
        };
        if let Some(opt) = loaded.options.get(name) {
            into.set(opt, &value);
        }
    }
    true
}

const ON_VALUE: &str = "true";
const OFF_VALUE: &str = "false";

pub fn draw_packs(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    draw_title(p, ctx, PACKS_TITLE);

    let (list_x, list_y, list_w, list_h) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let left = (ctx.vw / 2.0 - GRID_LEFT_OFFSET).floor();
    let rows = state.shaderpacks.packs.len() + 1;
    let content_h = rows as f32 * LIST_ROW_H;

    scroll_input(
        &mut state.shaderpacks.list_scroll,
        &mut state.shaderpacks.list_scroll_drag,
        ctx,
        left,
        BIG_W,
        list_y,
        list_h,
        content_h,
    );
    let top = (list_y - state.shaderpacks.list_scroll).floor();

    let input = masked_input(ctx, (list_x, list_y, list_w, list_h), false);
    let list_ctx = &ScreenCtx {
        input: &input,
        vw: ctx.vw,
        vh: ctx.vh,
    };

    let clip = p.push_clip(list_x, list_y, list_w, list_h);

    let loaded_name = state.shaderpacks.loaded.as_ref().map(|l| l.name.clone());
    let mut pick: Option<Option<String>> = None;

    let selected = loaded_name.is_none();
    if row(p, list_ctx, left, top, 0, NONE_ROW, selected) {
        pick = Some(None);
    }
    for (index, pack) in state.shaderpacks.packs.iter().enumerate() {
        let selected = loaded_name.as_deref() == Some(pack.name.as_str());
        if row(p, list_ctx, left, top, index + 1, &pack.name, selected) {
            pick = Some(Some(pack.name.clone()));
        }
    }
    if state.shaderpacks.packs.is_empty() {
        text_center(p, NO_PACKS, ctx.vw / 2.0, top + LIST_ROW_H + 6.0, 0xA0_A0A0);
    }
    p.pop_clip(clip);

    if let Some(choice) = pick {
        state.shaderpacks.error = None;
        match choice {
            None => state.shaderpacks.loaded = None,
            Some(name) => match load(&name) {
                Ok(loaded) => state.shaderpacks.loaded = Some(loaded),
                Err(e) => {
                    state.shaderpacks.error = Some(format!("{name}: {e}"));
                    state.shaderpacks.loaded = None;
                }
            },
        }
    }

    if let Some(error) = state.shaderpacks.error.clone() {
        text_center(p, &error, ctx.vw / 2.0, list_y + list_h + 4.0, 0xFF_5555);
    }

    let footer_y = LAYOUT.footer_y(ctx.vh);
    let has_options = state
        .shaderpacks
        .loaded
        .as_ref()
        .is_some_and(|l| !l.options.is_empty());
    if has_options {
        let settings = Button::new(left, footer_y, COLUMN_W, WIDGET_HEIGHT, SETTINGS);
        if settings.draw(p, ctx) {
            state.shaderpacks.open_options();
            state.nav = Some(Screen::ShaderOptions);
        }
        let done = Button::new(
            left + COLUMN_STEP,
            footer_y,
            COLUMN_W,
            WIDGET_HEIGHT,
            "Done",
        );
        if done.draw(p, ctx) {
            state.nav = Some(Screen::VideoSettings);
        }
    } else if draw_done(p, ctx) {
        state.nav = Some(Screen::VideoSettings);
    }
}

fn row(
    p: &mut Painter,
    ctx: &ScreenCtx,
    left: f32,
    top: f32,
    index: usize,
    label: &str,
    selected: bool,
) -> bool {
    let y = top + index as f32 * LIST_ROW_H;
    let caption = if selected {
        format!("> {label}")
    } else {
        label.to_owned()
    };
    Button::new(left, y, BIG_W, LIST_ROW_H - 2.0, &caption).draw(p, ctx)
}

pub fn draw_options(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    let Some(loaded) = state.shaderpacks.loaded.as_ref() else {
        state.nav = Some(Screen::ShaderPacks);
        return;
    };

    let screen_id = state.shaderpacks.current_screen().to_owned();
    let title = screen_title(loaded, &screen_id);
    draw_title(p, ctx, &title);

    let elements = layout(loaded, &screen_id);
    let columns = loaded
        .props
        .columns(&screen_id)
        .unwrap_or(if elements.len() > 18 { 3 } else { 2 })
        .max(1);

    let (list_x, list_y, list_w, list_h) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let rows = elements.len().div_ceil(columns as usize);
    let content_h = rows as f32 * ROW_H;
    let grid_w = columns as f32 * COLUMN_STEP - (COLUMN_STEP - COLUMN_W);
    let left = (ctx.vw / 2.0 - grid_w / 2.0).floor();
    let cell_w = COLUMN_W.min(grid_w / columns as f32 - 4.0);

    scroll_input(
        &mut state.shaderpacks.scroll,
        &mut state.shaderpacks.scroll_drag,
        ctx,
        left,
        grid_w,
        list_y,
        list_h,
        content_h,
    );
    let top = (list_y - state.shaderpacks.scroll).floor();

    let dragging = state.shaderpacks.scroll_drag.is_some();
    let input = masked_input(ctx, (list_x, list_y, list_w, list_h), dragging);
    let masked = ScreenCtx {
        input: &input,
        vw: ctx.vw,
        vh: ctx.vh,
    };
    crate::gui::focus::suspend();
    let clip = p.push_clip(list_x, list_y, list_w, list_h);

    let mut action = None;
    let mut hovered: Option<&str> = None;

    for (index, element) in elements.iter().enumerate() {
        let column = index % columns as usize;
        let row_index = index / columns as usize;
        let x = left + column as f32 * COLUMN_STEP;
        let y = top + row_index as f32 * ROW_H + ROW_INSET;
        if y + ROW_H < list_y || y > list_y + list_h {
            continue;
        }
        if let Element::Option(name) = element.as_ref()
            && masked.hovering(x, y, cell_w, WIDGET_HEIGHT)
        {
            hovered = loaded.lang.comment(name);
        }
        if let Some(next) = element_widget(p, &masked, loaded, element, x, y, cell_w, index) {
            action = Some(next);
        }
    }
    p.pop_clip(clip);
    crate::gui::focus::resume();

    if let Some(text) = hovered {
        draw_comment(p, ctx, text);
    }

    if draw_done(p, ctx) {
        if state.shaderpacks.nav.pop().is_none() {
            state.nav = Some(Screen::ShaderPacks);
        }
        state.shaderpacks.scroll = 0.0;
        return;
    }

    if let Some(action) = action {
        apply_action(state, action);
    }
}

enum Action {
    Enter(String),
    Set(String, String),
    Reset(String),
    Profile(usize),
}

fn apply_action(state: &mut GuiState, action: Action) {
    let Some(loaded) = state.shaderpacks.loaded.as_mut() else {
        return;
    };
    match action {
        Action::Enter(id) => {
            state.shaderpacks.nav.push(id);
            state.shaderpacks.scroll = 0.0;
            return;
        }
        Action::Set(name, value) => {
            if let Some(opt) = loaded.options.get(&name) {
                let opt = opt.clone();
                loaded.values.set(&opt, &value);
            }
        }
        Action::Reset(name) => {
            if let Some(opt) = loaded.options.get(&name) {
                let opt = opt.clone();
                loaded.values.reset(&opt);
            }
        }
        Action::Profile(at) => {
            let mut values = Values::default();
            if apply_profile(loaded, at, &mut values) {
                loaded.values = values;
            }
        }
    }
    loaded.profile = matching_profile(loaded);
    save(loaded);
    state.shaderpacks.revision += 1;
}

fn layout<'a>(loaded: &'a Loaded, screen: &str) -> Vec<std::borrow::Cow<'a, Element>> {
    use std::borrow::Cow;
    let declared = loaded.props.screens.get(screen);

    let Some(declared) = declared else {
        return loaded
            .options
            .iter()
            .map(|opt| Cow::Owned(Element::Option(opt.name.clone())))
            .collect();
    };

    if !declared.contains(&Element::Rest) {
        return declared.iter().map(Cow::Borrowed).collect();
    }

    let placed: std::collections::HashSet<&str> = loaded
        .props
        .screens
        .values()
        .flatten()
        .filter_map(|e| match e {
            Element::Option(name) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    let rest: Vec<Element> = loaded
        .options
        .iter()
        .filter(|opt| !placed.contains(opt.name.as_str()))
        .map(|opt| Element::Option(opt.name.clone()))
        .collect();

    declared
        .iter()
        .flat_map(|element| match element {
            Element::Rest => rest.iter().cloned().map(Cow::Owned).collect::<Vec<_>>(),
            other => vec![Cow::Borrowed(other)],
        })
        .collect()
}

fn screen_title(loaded: &Loaded, screen: &str) -> String {
    if screen == ROOT_SCREEN {
        return loaded.name.clone();
    }
    loaded
        .lang
        .screen(screen)
        .map_or_else(|| lang::humanise(screen), str::to_owned)
}

fn element_widget(
    p: &mut Painter,
    ctx: &ScreenCtx,
    loaded: &Loaded,
    element: &Element,
    x: f32,
    y: f32,
    w: f32,
    index: usize,
) -> Option<Action> {
    match element {
        Element::Empty => None,
        Element::Rest => None,
        Element::Link(id) => {
            let label = format!("{}...", screen_title(loaded, id));
            Button::new(x, y, w, WIDGET_HEIGHT, &label)
                .draw(p, ctx)
                .then(|| Action::Enter(id.clone()))
        }
        Element::Profile => {
            let name = match loaded.profile {
                Some(at) => loaded.props.profiles[at].0.as_str(),
                None => CUSTOM,
            };
            let named = loaded
                .lang
                .profile(name)
                .map_or_else(|| lang::humanise(name), str::to_owned);
            let label = format!("{PROFILE}: {named}");
            if loaded.props.profiles.is_empty() {
                return None;
            }
            Button::new(x, y, w, WIDGET_HEIGHT, &label)
                .draw(p, ctx)
                .then(|| {
                    let next = loaded
                        .profile
                        .map_or(0, |at| (at + 1) % loaded.props.profiles.len());
                    Action::Profile(next)
                })
        }
        Element::Option(name) => {
            let opt = loaded.options.get(name)?;
            option_widget(p, ctx, loaded, opt, x, y, w, index)
        }
    }
}

fn option_widget(
    p: &mut Painter,
    ctx: &ScreenCtx,
    loaded: &Loaded,
    opt: &Opt,
    x: f32,
    y: f32,
    w: f32,
    index: usize,
) -> Option<Action> {
    let current = loaded.values.get(opt);
    let changed = !loaded.values.is_default(opt);
    let label = loaded.option_label(&opt.name);

    let reset = ctx.input.shift;

    match &opt.kind {
        Kind::Boolean { .. } => {
            let on = current == ON_VALUE;
            let state = loaded
                .lang
                .value(&opt.name, current)
                .unwrap_or(if on { ON } else { OFF });
            let caption = tag(&label, state, changed);
            Button::new(x, y, w, WIDGET_HEIGHT, &caption)
                .draw(p, ctx)
                .then(|| {
                    if reset {
                        Action::Reset(opt.name.clone())
                    } else {
                        Action::Set(
                            opt.name.clone(),
                            if on { OFF_VALUE } else { ON_VALUE }.to_owned(),
                        )
                    }
                })
        }
        Kind::Value { values, .. } => {
            let at = values.iter().position(|v| v == current).unwrap_or(0);
            if loaded.props.is_slider(&opt.name) && values.len() > 1 {
                let picked = Slider {
                    id: SLIDER_ID_BASE + index as u32,
                    x,
                    y,
                    w,
                    h: WIDGET_HEIGHT,
                    min: 0,
                    max: values.len() as i32 - 1,
                    value: at as i32,
                    label: |i: i32| {
                        let value = &values[i.clamp(0, values.len() as i32 - 1) as usize];
                        tag(&label, &loaded.value_label(&opt.name, value), changed)
                    },
                }
                .draw(p, ctx);
                return (picked != at as i32)
                    .then(|| Action::Set(opt.name.clone(), values[picked as usize].clone()));
            }
            let caption = tag(&label, &loaded.value_label(&opt.name, current), changed);
            Button::new(x, y, w, WIDGET_HEIGHT, &caption)
                .draw(p, ctx)
                .then(|| {
                    if reset {
                        Action::Reset(opt.name.clone())
                    } else {
                        Action::Set(opt.name.clone(), values[(at + 1) % values.len()].clone())
                    }
                })
        }
    }
}

fn tag(label: &str, value: &str, changed: bool) -> String {
    let mark = if changed { "*" } else { "" };
    format!("{mark}{label}: {value}")
}

fn masked_input(ctx: &ScreenCtx, rect: (f32, f32, f32, f32), dragging: bool) -> GuiInput {
    let (x, y, w, h) = rect;
    let over = ctx.hovering(x, y, w, h);
    let mut input = ctx.input.clone();
    if dragging || (!over && !widgets::slider_dragging()) {
        input.mouse = None;
    }
    if dragging || !over {
        input.left_click = false;
    }
    input
}

fn draw_comment(p: &mut Painter, ctx: &ScreenCtx, text: &str) {
    let Some(mouse) = ctx.input.mouse else { return };
    let spans = [crate::text::Span {
        text: text.to_owned(),
        style: crate::text::Style::default(),
    }];
    let lines: Vec<String> = p
        .atlas
        .font
        .wrap(&spans, (ctx.vw / 3.0).max(120.0))
        .iter()
        .map(|line| line.iter().map(|span| span.text.as_str()).collect())
        .collect();
    if !lines.is_empty() {
        super::options::draw_bind_tooltip(p, &lines, mouse.x, mouse.y, ctx.vw);
    }
}

fn text_center(p: &mut Painter, s: &str, cx: f32, y: f32, color: u32) {
    let x = (cx - p.atlas.font.width_str(s) / 2.0).floor();
    p.text_str(s, x, y, color, true);
}
