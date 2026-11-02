use std::sync::Arc;

use crate::gui::packdrop::{self, Drops, Search};
use crate::gui::painter::Painter;
use crate::gui::widgets::{Button, Slider, TextBox};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::platform::{packfs, storage};
use crate::session::SharedMutex;
use crate::shaderpack::lang::{self, Lang};
use crate::shaderpack::options::{Kind, Opt, Options, Values};
use crate::shaderpack::properties::{self, Element, Properties, ROOT_SCREEN};
use crate::shaderpack::{discover, features};
use crate::util::pack::{Pack, Source};

use super::options::{
    BIG_W, COLUMN_STEP, COLUMN_W, GRID_LEFT_OFFSET, LAYOUT, ROW_H, ROW_INSET, draw_title,
    row_visible, scroll_masked,
};
use super::widgets::{FOOTER_GAP, WIDGET_HEIGHT};

const PACKS_TITLE: &str = "Shader Packs";
pub(in crate::gui) const SHADER_PACKS: &str = "Shader Packs...";
const NONE_ROW: &str = "(none)";
#[cfg(not(target_arch = "wasm32"))]
const NO_PACKS: &str = "No packs in the shaders folder";
#[cfg(target_arch = "wasm32")]
const NO_PACKS: &str = "Drop a pack on the page, or choose one below";
const SETTINGS_ICON: char = '🛠';
const SETTINGS_ICON_STR: &str = "🛠";
const SETTINGS_FALLBACK: &str = "...";
const SETTINGS_GAP: f32 = 2.0;
const RESET: &str = "Reset";
const DONE: &str = "Done";
const OPEN_FOLDER: &str = "Open Shader Pack Folder";
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
    properties_text: Option<String>,
    lang: Lang,
    profile: Option<usize>,
    menu: Menu,
    unmet: Vec<String>,
}

#[derive(Default)]
struct Menu {
    every: Vec<Element>,
    unplaced: Vec<Element>,
}

impl Menu {
    fn of(options: &Options, props: &Properties) -> Menu {
        let placed: std::collections::HashSet<&str> = props
            .screens
            .values()
            .flatten()
            .filter_map(|e| match e {
                Element::Option(name) => Some(name.as_str()),
                _ => None,
            })
            .collect();
        let every: Vec<Element> = options
            .iter()
            .map(|opt| Element::Option(opt.name.clone()))
            .collect();
        let unplaced = every
            .iter()
            .filter(|e| matches!(e, Element::Option(name) if !placed.contains(name.as_str())))
            .cloned()
            .collect();
        Menu { every, unplaced }
    }
}

impl Loaded {
    pub(crate) fn values(&self) -> &Values {
        &self.values
    }

    fn option_label(&self, name: &str) -> String {
        self.lang
            .option(name)
            .map_or_else(|| lang::humanise(name), str::to_owned)
    }

    fn value_label(&self, name: &str, value: &str) -> String {
        self.lang.value(name, value).unwrap_or(value).to_owned()
    }
}

pub struct State {
    packs: Vec<Pack>,
    pub loaded: Option<Loaded>,
    nav: Vec<String>,
    scroll: f32,
    scroll_drag: Option<f32>,
    list_scroll: f32,
    list_scroll_drag: Option<f32>,
    error: Option<String>,
    search: TextBox,
    pub(in crate::gui) drops: Drops,
    revision: u64,
}

impl Default for State {
    fn default() -> Self {
        let saved = storage::SHADER_PACK
            .load()
            .map(|t| t.trim().to_owned())
            .filter(|n| !n.is_empty());
        let (loaded, error) = match saved {
            None => (None, None),
            Some(name) => match load(&name) {
                Ok(loaded) => (Some(loaded), None),
                Err(e) => (None, Some(format!("{name}: {e}"))),
            },
        };
        State {
            packs: Vec::new(),
            loaded,
            nav: Vec::new(),
            scroll: 0.0,
            scroll_drag: None,
            list_scroll: 0.0,
            list_scroll_drag: None,
            error,
            search: TextBox::default(),
            drops: Drops::default(),
            revision: 0,
        }
    }
}

impl State {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn refresh(&mut self) {
        self.rescan();
        self.list_scroll = 0.0;
        self.search = packdrop::search_box();
        self.drops.clear();
    }

    fn rescan(&mut self) {
        self.packs = discover::list();
    }

    pub fn cancel_prompt(&mut self) -> bool {
        self.drops.cancel()
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
    let source = Source::open(&discover::dir().join(name), crate::shaderpack::ROOT)?;

    let texts: Vec<String> = crate::shaderpack::include::option_texts(&source);
    let options = crate::shaderpack::options::discover(texts.iter().map(String::as_str));

    let values = match packfs::read_to_string(values_path(name)) {
        Ok(text) => Values::load(&text, &options),
        Err(_) => Values::default(),
    };

    let properties_text = source.read_text("/shaders.properties");
    let props = parse_properties(name, properties_text.as_deref(), &options, &values);

    let unmet = features::unmet(props.features("iris.features.required"));
    if !unmet.is_empty() {
        crate::log_warn!(
            "shaders",
            "{name} asks for {}; loading without",
            unmet.join(", ")
        );
    }

    let mut loaded = Loaded {
        name: name.to_owned(),
        options,
        values,
        props,
        properties_text,
        lang: Lang::load(&source),
        profile: None,
        menu: Menu::default(),
        unmet,
    };
    loaded.menu = Menu::of(&loaded.options, &loaded.props);
    loaded.profile = matching_profile(&loaded);
    Ok(loaded)
}

fn parse_properties(
    pack: &str,
    text: Option<&str>,
    options: &Options,
    values: &Values,
) -> Properties {
    let Some(text) = text else {
        return Properties::default();
    };
    let props = properties::parse_with_options(text, options, values);
    if !props.unknown.is_empty() {
        crate::log_info!(
            "shaders",
            "{pack}: unreadable in shaders.properties: {}",
            props.unknown.join(", ")
        );
    }
    props
}

fn values_path(name: &str) -> std::path::PathBuf {
    discover::dir().join(format!("{name}.txt"))
}

fn save(loaded: &Loaded) {
    let at = values_path(&loaded.name);
    if loaded.values.is_empty() {
        let _ = packfs::remove_file(&at);
        return;
    }
    if let Err(e) = packfs::write(&at, loaded.values.save()) {
        crate::log_warn!("shaders", "could not save {}: {e}", at.display());
    }
}

fn matching_profile(loaded: &Loaded) -> Option<usize> {
    sorted_profiles(loaded).into_iter().find(|at| {
        profile_settings(loaded, *at).is_some_and(|settings| {
            settings.iter().all(|(name, value)| {
                loaded
                    .options
                    .get(name)
                    .is_some_and(|opt| loaded.values.get(opt) == value)
            })
        })
    })
}

fn sorted_profiles(loaded: &Loaded) -> Vec<usize> {
    let mut order: Vec<(usize, usize)> = (0..loaded.props.profiles.len())
        .map(|at| (at, profile_settings(loaded, at).map_or(0, |s| s.len())))
        .collect();
    order.sort_by(|a, b| b.1.cmp(&a.1));
    order.into_iter().map(|(at, _)| at).collect()
}

fn profile_settings(loaded: &Loaded, at: usize) -> Option<Vec<(String, String)>> {
    let mut settings = Vec::new();
    collect_profile(loaded, at, &mut settings, 0).then_some(settings)
}

fn collect_profile(
    loaded: &Loaded,
    at: usize,
    into: &mut Vec<(String, String)>,
    depth: u32,
) -> bool {
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
                    if !collect_profile(loaded, other, into, depth + 1) {
                        return false;
                    }
                }
                None => return false,
            }
            continue;
        }
        let (name, value) = match token.split_once('=') {
            Some((name, value)) => (name, value),
            None => match token.strip_prefix('!') {
                Some(name) => (name, OFF_VALUE),
                None => (token.as_str(), ON_VALUE),
            },
        };
        if loaded
            .options
            .get(name)
            .is_some_and(|opt| opt.accepts(value))
        {
            into.retain(|(n, _)| n != name);
            into.push((name.to_owned(), value.to_owned()));
        }
    }
    true
}

const ON_VALUE: &str = "true";
const OFF_VALUE: &str = "false";

pub fn draw_packs(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    shared: &Arc<SharedMutex>,
) {
    let packs = &mut state.shaderpacks;
    if let Some(copied) = packs
        .drops
        .draw(p, ctx, shared, &discover::dir(), discover::is_pack_dir)
    {
        if copied {
            packs.rescan();
        }
        return;
    }
    packdrop::header(p, ctx, PACKS_TITLE, &mut packs.search);

    let (list_x, list_y, list_w, full_h) = packdrop::LAYOUT.content_rect(ctx.vw, ctx.vh);
    let left = (ctx.vw / 2.0 - GRID_LEFT_OFFSET).floor();

    let failed: Vec<String> = crate::shaderpack::FAILED
        .get()
        .into_iter()
        .filter(|name| {
            state
                .shaderpacks
                .loaded
                .as_ref()
                .is_some_and(|l| &l.name == name)
        })
        .collect();
    let message = match (&state.shaderpacks.error, &state.shaderpacks.loaded) {
        (Some(error), _) => Some((error.clone(), packdrop::MESSAGE_ERROR)),
        _ if !failed.is_empty() => {
            packdrop::failed_message(&failed).map(|m| (m, packdrop::MESSAGE_ERROR))
        }
        (None, Some(loaded)) if !loaded.unmet.is_empty() => Some((
            format!("{}: runs without {}", loaded.name, loaded.unmet.join(", ")),
            packdrop::MESSAGE_WARNING,
        )),
        _ => None,
    };
    let message = message
        .as_ref()
        .map(|(text, color)| (text.as_str(), *color));
    let list_h = packdrop::message(
        p,
        ctx,
        (list_x, list_y, list_w, full_h),
        LIST_ROW_H,
        message,
    );

    let search = Search::of(&state.shaderpacks.search);
    let show_none = search.matches(NONE_ROW);
    let shown: Vec<&Pack> = state
        .shaderpacks
        .packs
        .iter()
        .filter(|pack| search.matches(&pack.name.to_lowercase()))
        .collect();
    let rows = shown.len() + usize::from(show_none);
    let content_h = rows as f32 * LIST_ROW_H;

    let input = scroll_masked(
        &mut state.shaderpacks.list_scroll,
        &mut state.shaderpacks.list_scroll_drag,
        ctx,
        left,
        BIG_W,
        (list_x, list_y, list_w, list_h),
        content_h,
    );
    let top = (list_y - state.shaderpacks.list_scroll).floor();
    let list_ctx = &ctx.with_input(&input);
    #[cfg(feature = "tutorial")]
    crate::gui::tutorial::mark(
        crate::gui::tutorial::Spot::ShaderList,
        crate::gui::hud_layout::Rect::new(list_x, list_y, list_w, list_h),
    );

    let clip = p.push_clip(list_x, list_y, list_w, list_h);

    let loaded = state.shaderpacks.loaded.as_ref();
    let loaded_name = loaded.map(|l| l.name.clone());
    let loaded_bare = loaded.is_some_and(|l| l.options.is_empty());
    let mut pick: Option<Option<String>> = None;
    let mut open_settings = false;

    if show_none
        && row(
            p,
            list_ctx,
            left,
            top,
            0,
            NONE_ROW,
            loaded_name.is_none(),
            None,
        )
        .is_some()
    {
        pick = Some(None);
    }
    for (index, pack) in shown.iter().enumerate() {
        let selected = loaded_name.as_deref() == Some(pack.name.as_str());
        let settings = Some(!(selected && loaded_bare));
        let at = index + usize::from(show_none);
        if let Some(click) = row(p, list_ctx, left, top, at, &pack.name, selected, settings) {
            open_settings = click == RowClick::Settings;
            if !(open_settings && selected) {
                pick = Some(Some(pack.name.clone()));
            }
        }
    }
    if state.shaderpacks.packs.is_empty() {
        p.text_centered(
            NO_PACKS,
            ctx.vw / 2.0,
            top + LIST_ROW_H + 6.0,
            0xA0_A0A0,
            true,
        );
    }
    p.pop_clip(clip);

    if let Some(choice) = pick {
        state.shaderpacks.error = None;
        storage::SHADER_PACK.store(choice.as_deref().unwrap_or(""));
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
    if open_settings
        && state
            .shaderpacks
            .loaded
            .as_ref()
            .is_some_and(|l| !l.options.is_empty())
    {
        state.shaderpacks.open_options();
        state.nav = Some(Screen::ShaderOptions);
    }

    let footer = packdrop::footer(p, ctx, 2);
    if footer.reload {
        state.shaderpacks.rescan();
    }
    let mut x = footer.x;
    #[cfg(feature = "tutorial")]
    {
        let reload_x = x - WIDGET_HEIGHT - FOOTER_GAP;
        crate::gui::tutorial::mark(
            crate::gui::tutorial::Spot::ShaderFolder,
            crate::gui::hud_layout::Rect::new(
                reload_x,
                footer.y,
                x + footer.w - reload_x,
                WIDGET_HEIGHT,
            ),
        );
    }
    let folder_hovered = packdrop::open_folder(p, ctx, &footer, x, OPEN_FOLDER, &discover::dir());
    x += footer.w + FOOTER_GAP;
    if Button::new(x, footer.y, footer.w, WIDGET_HEIGHT, DONE).draw(p, ctx) {
        state.nav = Some(Screen::VideoSettings);
    }
    if folder_hovered {
        packdrop::folder_tooltip(p, ctx);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowClick {
    Pick,
    Settings,
}

#[allow(clippy::too_many_arguments)]
fn row(
    p: &mut Painter,
    ctx: &ScreenCtx,
    left: f32,
    top: f32,
    index: usize,
    label: &str,
    selected: bool,
    settings: Option<bool>,
) -> Option<RowClick> {
    let y = top + index as f32 * LIST_ROW_H;
    let h = LIST_ROW_H - 2.0;
    let caption = if selected {
        format!("> {label}")
    } else {
        label.to_owned()
    };
    let Some(active) = settings else {
        return Button::new(left, y, BIG_W, h, &caption)
            .draw(p, ctx)
            .then_some(RowClick::Pick);
    };
    let pick = Button::new(left, y, BIG_W - h - SETTINGS_GAP, h, &caption).draw(p, ctx);
    let icon = if p.atlas.font.glyph(SETTINGS_ICON).is_some() {
        SETTINGS_ICON_STR
    } else {
        SETTINGS_FALLBACK
    };
    let open = Button {
        active,
        ..Button::new(left + BIG_W - h, y, h, h, icon)
    }
    .draw(p, ctx);
    match (pick, open) {
        (true, _) => Some(RowClick::Pick),
        (_, true) => Some(RowClick::Settings),
        _ => None,
    }
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

    let input = scroll_masked(
        &mut state.shaderpacks.scroll,
        &mut state.shaderpacks.scroll_drag,
        ctx,
        left,
        grid_w,
        (list_x, list_y, list_w, list_h),
        content_h,
    );
    let top = (list_y - state.shaderpacks.scroll).floor();

    let masked = ctx.with_input(&input);
    crate::gui::focus::suspend();
    let clip = p.push_clip(list_x, list_y, list_w, list_h);

    let mut action = None;
    let mut hovered: Option<&str> = None;

    for (index, element) in elements.iter().enumerate() {
        let column = index % columns as usize;
        let row_index = index / columns as usize;
        let x = left + column as f32 * COLUMN_STEP;
        let y = top + row_index as f32 * ROW_H + ROW_INSET;
        if !row_visible(y, ROW_H, list_y, list_h) {
            continue;
        }
        if let Element::Option(name) = element
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

    let footer_y = LAYOUT.footer_y(ctx.vh);
    let footer_left = (ctx.vw / 2.0 - GRID_LEFT_OFFSET).floor();
    if Button::new(footer_left, footer_y, COLUMN_W, WIDGET_HEIGHT, RESET).draw(p, ctx) {
        action = Some(Action::ResetAll);
    }
    if Button::new(
        footer_left + COLUMN_STEP,
        footer_y,
        COLUMN_W,
        WIDGET_HEIGHT,
        DONE,
    )
    .draw(p, ctx)
    {
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
    ResetAll,
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
        Action::ResetAll => loaded.values.reset_all(),
        Action::Profile(at) => {
            if let Some(settings) = profile_settings(loaded, at) {
                for (name, value) in settings {
                    if let Some(opt) = loaded.options.get(&name).cloned() {
                        loaded.values.set(&opt, &value);
                    }
                }
            }
        }
    }
    loaded.props = parse_properties(
        &loaded.name,
        loaded.properties_text.as_deref(),
        &loaded.options,
        &loaded.values,
    );
    loaded.menu = Menu::of(&loaded.options, &loaded.props);
    loaded.profile = matching_profile(loaded);
    save(loaded);
    state.shaderpacks.revision += 1;
}

fn layout<'a>(loaded: &'a Loaded, screen: &str) -> Vec<&'a Element> {
    let Some(declared) = loaded.props.screens.get(screen) else {
        return loaded.menu.every.iter().collect();
    };
    declared
        .iter()
        .flat_map(|element| match element {
            Element::Rest => loaded.menu.unplaced.iter().collect::<Vec<_>>(),
            other => vec![other],
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
                    let order = sorted_profiles(loaded);
                    let next = loaded
                        .profile
                        .and_then(|current| order.iter().position(|at| *at == current))
                        .map_or(order[0], |i| order[(i + 1) % order.len()]);
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

fn draw_comment(p: &mut Painter, ctx: &ScreenCtx, text: &str) {
    let Some(mouse) = ctx.input.mouse else { return };
    let spans = [crate::text::Span::new(text)];
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
