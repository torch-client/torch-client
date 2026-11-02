use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::gui::packdrop;
use crate::gui::painter::{ClipGuard, Painter};
use crate::gui::render::GuiInput;
use crate::gui::widgets::{
    self, Button, FOOTER_GAP, FOOTER_W, HeaderFooter, TAB_BAR_H, TextBox, WIDGET_HEIGHT,
    centered_row_x,
};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::platform::time::Instant;
use crate::resourcepacks::modrinth::{self, Hit, Query, Sort, Tags};
use crate::resourcepacks::{self, Compat, Info};
use crate::text::{LINE_HEIGHT, Span};
use crate::util::bytes::Bytes;

use super::options::{SCROLLBAR_W, draw_scrollbar, row_visible, scroll_input_in, scroll_masked};
use super::resourcepacks::{
    BAR_ROW_W, DESCRIPTION, HEADER_H, ICON, LIST_GAP, LIST_W, PADDING, ROW_H, ROW_W, TEXT_W,
    entry_text, header, icon_key, plain,
};

pub(in crate::gui) const EXPLORE: &str = "Explore Packs";
const TABS: [&str; 2] = ["Installed", "Browse"];
const INSTALLED_TAB: usize = 0;
const BROWSE_TAB: usize = 1;

const PACKS: &str = "Packs Folder";
const RESULTS: &str = "Modrinth";
const DETAILS: &str = "Details";
const DELETE: &str = "Delete";
const DOWNLOAD: &str = "Download";
const DOWNLOADING: &str = "Downloading...";
const INSTALLED: &str = "Installed";
const RETRY: &str = "Failed, Retry";
const DONE: &str = "Done";
const SCANNING: &str = "Reading the packs folder...";
const NO_PACKS: &str = "The packs folder is empty";
const SEARCHING: &str = "Searching...";
const NO_RESULTS: &str = "No packs found";
const NOTHING_SELECTED: &str = "Select a pack";
const LOADING_BODY: &str = "Loading description...";

const BROWSE_LAYOUT: HeaderFooter =
    HeaderFooter::new(FILTER_Y + WIDGET_HEIGHT + 4.0, HeaderFooter::DEFAULT_H);
const INSTALLED_LAYOUT: HeaderFooter = HeaderFooter::new(TAB_BAR_H + 6.0, HeaderFooter::DEFAULT_H);
const SEARCH_Y: f32 = TAB_BAR_H + 6.0;
const FILTER_Y: f32 = SEARCH_Y + packdrop::SEARCH_H + 4.0;
const FULL_W: f32 = 2.0 * (LIST_W + LIST_GAP);
const FILTER_GAP: f32 = 5.0;
const DETAIL_TEXT_W: f32 = ROW_W - 2.0 * PADDING - SCROLLBAR_W;
const SIZE_GAP: f32 = 4.0;

const DEBOUNCE: Duration = Duration::from_millis(300);
const PREFETCH_ROWS: f32 = 3.0;
const ICON_KEYS: usize = 12;
const SELECTED_OUTLINE: u32 = 0xFF80_8080;
const DOWNLOADED_FILL: u32 = 0x6020_8020;
const BY_HAND_FILL: u32 = 0x6050_5050;
const WHITE: u32 = 0xFF_FFFF;

const PREVIEW_KEY: &str = "\u{0}modrinth/preview";

fn pack_icon_key(pack: &LocalPack) -> String {
    let key = icon_key(&pack.name);
    if let Some(pixels) = &pack.icon {
        crate::gui::ping::ensure_icon(&key, pixels);
    }
    key
}

fn row_key(i: usize) -> String {
    format!("\u{0}modrinth/{i}")
}

struct LocalPack {
    name: String,
    info: Info,
    icon: Option<Arc<Vec<u8>>>,
    size: u64,
    downloaded: bool,
}

#[derive(Default)]
struct Inbox {
    tags: Option<Result<Tags, String>>,
    pages: Vec<(u64, Result<(Vec<Hit>, usize), String>)>,
    bodies: Vec<(String, Result<String, String>)>,
    icons: Vec<(String, Arc<Vec<u8>>)>,
    downloads: Vec<(String, Result<String, String>)>,
    installed: Option<Vec<String>>,
    packs: Option<Vec<LocalPack>>,
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn<F>(inbox: &Arc<Mutex<Inbox>>, job: impl FnOnce(Arc<Mutex<Inbox>>) -> F + Send + 'static)
where
    F: Future<Output = ()>,
{
    let inbox = inbox.clone();
    let spawned = std::thread::Builder::new()
        .name("modrinth".into())
        .spawn(move || crate::platform::executor::block_on(job(inbox)));
    if let Err(e) = spawned {
        crate::log_warn!("modrinth", "could not start a request thread: {e}");
    }
}

#[cfg(target_arch = "wasm32")]
fn spawn<F>(inbox: &Arc<Mutex<Inbox>>, job: impl FnOnce(Arc<Mutex<Inbox>>) -> F + 'static)
where
    F: Future<Output = ()> + 'static,
{
    crate::platform::executor::spawn_detached(job(inbox.clone()));
}

#[derive(Clone, PartialEq, Default)]
struct Filters {
    text: String,
    sort: Sort,
    any_version: bool,
    resolution: Option<usize>,
}

enum Download {
    Running,
    Done,
    Failed,
}

#[derive(Default)]
struct Panes {
    scroll: [f32; 2],
    drag: [Option<f32>; 2],
}

pub struct State {
    tab: usize,
    packs: Vec<LocalPack>,
    scanning: bool,
    pack_selected: Option<usize>,
    pack_detail: Option<Vec<Vec<Span>>>,
    installed_panes: Panes,

    search: TextBox,
    filters: Filters,
    seen: Filters,
    changed_at: Instant,
    sent: Option<(Filters, Query)>,
    generation: u64,
    tags: Option<Tags>,
    tags_requested: bool,
    hits: Vec<Hit>,
    total: usize,
    loading: bool,
    error: Option<String>,
    selected: Option<usize>,
    bodies: HashMap<String, Option<String>>,
    preview: Option<(String, Vec<Vec<Span>>)>,
    icons: HashMap<String, Arc<Vec<u8>>>,
    keyed: [Option<String>; ICON_KEYS],
    preview_icon: Option<String>,
    downloads: HashMap<String, Download>,
    installed_projects: HashSet<String>,
    browse_panes: Panes,

    changed: bool,
    inbox: Arc<Mutex<Inbox>>,
}

impl Default for State {
    fn default() -> State {
        State {
            tab: INSTALLED_TAB,
            packs: Vec::new(),
            scanning: false,
            pack_selected: None,
            pack_detail: None,
            installed_panes: Panes::default(),
            search: packdrop::search_box(),
            filters: Filters::default(),
            seen: Filters::default(),
            changed_at: Instant::now(),
            sent: None,
            generation: 0,
            tags: None,
            tags_requested: false,
            hits: Vec::new(),
            total: 0,
            loading: false,
            error: None,
            selected: None,
            bodies: HashMap::new(),
            preview: None,
            icons: HashMap::new(),
            keyed: Default::default(),
            preview_icon: None,
            downloads: HashMap::new(),
            installed_projects: HashSet::new(),
            browse_panes: Panes::default(),
            changed: false,
            inbox: Arc::default(),
        }
    }
}

impl State {
    pub fn open(&mut self) {
        if !self.tags_requested {
            self.tags_requested = true;
            spawn(&self.inbox, |inbox| async move {
                let tags = modrinth::tags().await;
                inbox.lock().unwrap().tags = Some(tags);
            });
            spawn(&self.inbox, |inbox| async move {
                let installed = modrinth::installed_projects().await;
                inbox.lock().unwrap().installed = Some(installed);
            });
        }
        self.scan();
        if self.tab == BROWSE_TAB {
            self.search.focus();
        }
    }

    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    fn scan(&mut self) {
        self.scanning = true;
        spawn(&self.inbox, |inbox| async move {
            let downloaded = modrinth::downloaded();
            let packs = resourcepacks::list()
                .into_iter()
                .map(|pack| LocalPack {
                    info: resourcepacks::info(&pack),
                    icon: None,
                    size: resourcepacks::size(&pack.name),
                    downloaded: downloaded.contains_key(&pack.name),
                    name: pack.name,
                })
                .collect();
            inbox.lock().unwrap().packs = Some(packs);
        });
    }

    fn drain(&mut self) {
        let inbox = std::mem::take(&mut *self.inbox.lock().unwrap());
        if let Some(tags) = inbox.tags {
            self.tags = Some(tags.unwrap_or_else(|e| {
                crate::log_warn!("modrinth", "no tags: {e}");
                Tags {
                    recent: vec![modrinth::OLDEST.to_owned()],
                    resolutions: Vec::new(),
                }
            }));
        }
        for (generation, page) in inbox.pages {
            if generation != self.generation {
                continue;
            }
            self.loading = false;
            match page {
                Ok((hits, total)) => {
                    self.total = total;
                    self.fetch_icons(&hits);
                    self.hits.extend(hits);
                }
                Err(e) => {
                    crate::log_warn!("modrinth", "search failed: {e}");
                    self.error = Some(e);
                }
            }
        }
        for (id, body) in inbox.bodies {
            let text = body.map_or_else(
                |e| format!("Could not load the description: {e}"),
                |md| plain_text(&md),
            );
            if self.preview.as_ref().is_some_and(|(shown, _)| *shown == id) {
                self.preview = None;
            }
            self.bodies.insert(id, Some(text));
        }
        self.icons.extend(inbox.icons);
        if let Some(installed) = inbox.installed {
            self.installed_projects.extend(installed);
        }
        for (id, result) in inbox.downloads {
            let state = match result {
                Ok(_) => {
                    self.changed = true;
                    self.installed_projects.insert(id.clone());
                    self.scan();
                    Download::Done
                }
                Err(e) => {
                    crate::log_warn!("modrinth", "download of {id} failed: {e}");
                    Download::Failed
                }
            };
            self.downloads.insert(id, state);
        }
        if let Some(mut packs) = inbox.packs {
            for pack in &mut packs {
                pack.icon = pack.info.icon.take().map(Arc::new);
            }
            let selected = self.pack_selected.map(|i| &self.packs[i].name);
            self.pack_selected =
                selected.and_then(|name| packs.iter().position(|pack| pack.name == *name));
            self.pack_detail = None;
            self.packs = packs;
            self.scanning = false;
        }
    }

    fn fetch_icons(&self, hits: &[Hit]) {
        let wanted: Vec<(String, String)> = hits
            .iter()
            .filter(|h| !self.icons.contains_key(&h.project_id))
            .filter_map(|h| Some((h.project_id.clone(), h.icon_url.clone()?)))
            .collect();
        if wanted.is_empty() {
            return;
        }
        spawn(&self.inbox, move |inbox| async move {
            for (id, url) in wanted {
                if let Some(pixels) = modrinth::icon(&url).await {
                    inbox.lock().unwrap().icons.push((id, Arc::new(pixels)));
                }
            }
        });
    }

    fn schedule(&mut self) {
        if self.filters != self.seen {
            self.seen = self.filters.clone();
            self.changed_at = Instant::now();
        }
        let Some(tags) = &self.tags else { return };
        if self
            .sent
            .as_ref()
            .is_some_and(|(sent, _)| *sent == self.seen)
            || self.changed_at.elapsed() < DEBOUNCE
        {
            return;
        }
        let query = Query {
            text: self.seen.text.trim().to_owned(),
            sort: self.seen.sort,
            versions: if self.seen.any_version {
                Vec::new()
            } else {
                tags.recent.clone()
            },
            resolution: self
                .seen
                .resolution
                .and_then(|i| tags.resolutions.get(i).cloned()),
        };
        self.sent = Some((self.seen.clone(), query));
        self.generation += 1;
        self.hits.clear();
        self.total = 0;
        self.error = None;
        self.selected = None;
        self.preview = None;
        self.browse_panes.scroll = [0.0; 2];
        self.request_page();
    }

    fn request_page(&mut self) {
        let Some((_, query)) = &self.sent else { return };
        self.loading = true;
        let (query, offset, generation) = (query.clone(), self.hits.len(), self.generation);
        spawn(&self.inbox, move |inbox| async move {
            let page = modrinth::search(&query, offset).await;
            inbox.lock().unwrap().pages.push((generation, page));
        });
    }

    fn select(&mut self, index: usize) {
        self.selected = Some(index);
        self.preview = None;
        self.browse_panes.scroll[1] = 0.0;
        let id = self.hits[index].project_id.clone();
        if self.bodies.contains_key(&id) {
            return;
        }
        self.bodies.insert(id.clone(), None);
        spawn(&self.inbox, move |inbox| async move {
            let body = modrinth::body(&id).await;
            inbox.lock().unwrap().bodies.push((id, body));
        });
    }

    fn download(&mut self, id: String) {
        let versions = self
            .sent
            .as_ref()
            .map(|(_, q)| q.versions.clone())
            .unwrap_or_default();
        self.downloads.insert(id.clone(), Download::Running);
        spawn(&self.inbox, move |inbox| async move {
            let result = modrinth::download(&id, &versions).await;
            inbox.lock().unwrap().downloads.push((id, result));
        });
    }

    fn delete(&mut self, index: usize) {
        let name = &self.packs[index].name;
        match modrinth::delete(name) {
            Ok(project) => {
                self.packs.remove(index);
                self.pack_selected = None;
                self.pack_detail = None;
                self.installed_projects.remove(&project);
                self.downloads.remove(&project);
                self.changed = true;
            }
            Err(e) => crate::log_warn!("modrinth", "could not delete {name}: {e}"),
        }
    }

    fn assign_icons(&mut self, rows: Range<usize>) {
        let visible: Vec<&str> = self.hits[rows]
            .iter()
            .map(|h| h.project_id.as_str())
            .collect();
        for id in &visible {
            let Some(pixels) = self.icons.get(*id) else {
                continue;
            };
            if let Some(held) = self
                .keyed
                .iter()
                .position(|held| held.as_deref() == Some(*id))
            {
                if crate::gui::ping::icon_slot(&row_key(held)).is_none() {
                    crate::gui::ping::note_icon(&row_key(held), pixels.clone());
                }
                continue;
            }
            let free = self
                .keyed
                .iter()
                .position(|held| held.as_deref().is_none_or(|held| !visible.contains(&held)));
            let Some(free) = free else { break };
            self.keyed[free] = Some((*id).to_owned());
            crate::gui::ping::note_icon(&row_key(free), pixels.clone());
        }
    }

    fn row_icon_key(&self, id: &str) -> Option<String> {
        self.keyed
            .iter()
            .position(|held| held.as_deref() == Some(id))
            .map(row_key)
    }

    fn preview_icon_key(&mut self, id: &str) -> Option<String> {
        if self.preview_icon.as_deref() != Some(id)
            || crate::gui::ping::icon_slot(PREVIEW_KEY).is_none()
        {
            let pixels = self.icons.get(id)?;
            crate::gui::ping::note_icon(PREVIEW_KEY, pixels.clone());
            self.preview_icon = Some(id.to_owned());
        }
        Some(PREVIEW_KEY.to_owned())
    }
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    let st = &mut state.explore_packs;
    st.drain();

    if let Some(tab) = widgets::tab_bar(p, ctx, &TABS, st.tab) {
        st.tab = tab;
        if tab == BROWSE_TAB {
            st.search.focus();
        }
    }
    let layout = if st.tab == BROWSE_TAB {
        BROWSE_LAYOUT
    } else {
        INSTALLED_LAYOUT
    };
    let (_, list_y, _, list_h) = layout.content_rect(ctx.vw, ctx.vh);
    let lefts = [
        (ctx.vw / 2.0 - LIST_GAP - LIST_W).floor(),
        (ctx.vw / 2.0 + LIST_GAP).floor(),
    ];
    let footer_y = layout.footer_y(ctx.vh);
    let x = centered_row_x(ctx.vw, 2.0 * FOOTER_W + FOOTER_GAP);
    let action = |label, active| Button {
        active,
        ..Button::new(x, footer_y, FOOTER_W, WIDGET_HEIGHT, label)
    };

    if st.tab == BROWSE_TAB {
        let x0 = (ctx.vw / 2.0 - FULL_W / 2.0).floor();
        let search_x = centered_row_x(ctx.vw, packdrop::SEARCH_W);
        st.search.update(
            p,
            ctx,
            search_x,
            SEARCH_Y,
            packdrop::SEARCH_W,
            packdrop::SEARCH_H,
        );
        st.filters.text.clone_from(&st.search.text);
        filter_row(p, ctx, st, x0);
        st.schedule();
        results(p, ctx, st, lefts[0], list_y, list_h);
        preview(p, ctx, st, lefts[1], list_y, list_h);

        let id = st.selected.map(|i| st.hits[i].project_id.clone());
        let (label, active) = match id.as_ref().and_then(|id| st.downloads.get(id)) {
            _ if id.is_none() => (DOWNLOAD, false),
            None => (DOWNLOAD, true),
            Some(Download::Running) => (DOWNLOADING, false),
            Some(Download::Done) => (INSTALLED, false),
            Some(Download::Failed) => (RETRY, true),
        };
        if action(label, active).draw(p, ctx)
            && let Some(id) = id
        {
            st.download(id);
        }
    } else {
        installed(p, ctx, st, lefts[0], list_y, list_h);
        pack_details(p, ctx, st, lefts[1], list_y, list_h);

        let deletable = st.pack_selected.filter(|&i| st.packs[i].downloaded);
        if action(DELETE, deletable.is_some()).draw(p, ctx)
            && let Some(i) = deletable
        {
            st.delete(i);
        }
    }

    if Button::new(
        x + FOOTER_W + FOOTER_GAP,
        footer_y,
        FOOTER_W,
        WIDGET_HEIGHT,
        DONE,
    )
    .draw(p, ctx)
    {
        state.nav = Some(Screen::ResourcePacks);
    }
    p.sprite(
        "footer_separator",
        0.0,
        ctx.vh - layout.footer_h - 2.0,
        ctx.vw,
        2.0,
    );
}

fn filter_row(p: &mut Painter, ctx: &ScreenCtx, st: &mut State, x0: f32) {
    let w = ((FULL_W - 2.0 * FILTER_GAP) / 3.0).floor();
    let f = &mut st.filters;

    let version = if f.any_version {
        "Any".to_owned()
    } else {
        format!("{}+", modrinth::OLDEST)
    };
    if Button::new(
        x0,
        FILTER_Y,
        w,
        WIDGET_HEIGHT,
        &format!("Version: {version}"),
    )
    .draw(p, ctx)
    {
        f.any_version = !f.any_version;
    }

    let sort = format!("Sort: {}", f.sort.label());
    if Button::new(x0 + w + FILTER_GAP, FILTER_Y, w, WIDGET_HEIGHT, &sort).draw(p, ctx) {
        let at = Sort::ALL.iter().position(|s| *s == f.sort).unwrap_or(0);
        f.sort = Sort::ALL[(at + 1) % Sort::ALL.len()];
    }

    let resolutions = st.tags.as_ref().map_or(&[][..], |t| &t.resolutions[..]);
    let name = f
        .resolution
        .and_then(|i| resolutions.get(i))
        .map_or("Any", String::as_str);
    let label = format!("Resolution: {name}");
    let resolution = Button {
        active: !resolutions.is_empty(),
        ..Button::new(
            x0 + 2.0 * (w + FILTER_GAP),
            FILTER_Y,
            w,
            WIDGET_HEIGHT,
            &label,
        )
    };
    if resolution.draw(p, ctx) {
        f.resolution = match f.resolution {
            None => Some(0),
            Some(i) if i + 1 < resolutions.len() => Some(i + 1),
            Some(_) => None,
        };
    }
}

struct ListView {
    input: GuiInput,
    clip: ClipGuard,
    scroll: f32,
    top: f32,
    left: f32,
    list_y: f32,
    list_h: f32,
    content_h: f32,
    row_x: f32,
    text_w: f32,
}

impl ListView {
    fn open(
        p: &mut Painter,
        ctx: &ScreenCtx,
        panes: &mut Panes,
        (left, list_y, list_h): (f32, f32, f32),
        title: &str,
        count: usize,
    ) -> ListView {
        let content_h = HEADER_H + count as f32 * ROW_H;
        let input = scroll_masked(
            &mut panes.scroll[0],
            &mut panes.drag[0],
            ctx,
            left,
            BAR_ROW_W,
            (left, list_y, LIST_W, list_h),
            content_h,
        );
        let clip = p.push_clip(left, list_y, LIST_W, list_h);
        let scroll = panes.scroll[0];
        let top = (list_y - scroll).floor();
        header(p, title, left, top, LIST_W);
        ListView {
            input,
            clip,
            scroll,
            top,
            left,
            list_y,
            list_h,
            content_h,
            row_x: left + (LIST_W - ROW_W) / 2.0,
            text_w: TEXT_W - if content_h > list_h { SCROLLBAR_W } else { 0.0 },
        }
    }

    fn row_y(&self, i: usize) -> f32 {
        self.top + HEADER_H + i as f32 * ROW_H
    }

    fn visible(&self, count: usize) -> Range<usize> {
        let first = ((self.list_y - self.top - HEADER_H) / ROW_H)
            .floor()
            .max(0.0) as usize;
        let last = (((self.list_y + self.list_h - self.top - HEADER_H) / ROW_H)
            .ceil()
            .max(0.0) as usize)
            .min(count);
        first.min(last)..last
    }

    fn row(
        &self,
        p: &mut Painter,
        ctx: &ScreenCtx,
        i: usize,
        fill: Option<u32>,
        selected: bool,
    ) -> bool {
        let y = self.row_y(i);
        if let Some(fill) = fill {
            p.fill(self.row_x, y, ROW_W, ROW_H, fill);
        }
        if selected {
            outline(p, self.row_x, y, ROW_W, ROW_H);
        }
        self.input.left_click
            && ctx
                .with_input(&self.input)
                .hovering(self.row_x, y, ROW_W, ROW_H)
    }

    fn close(self, p: &mut Painter, empty: Option<&str>) {
        if let Some(text) = empty {
            centred(p, text, self.left, self.top + HEADER_H + PADDING);
        }
        p.pop_clip(self.clip);
        draw_scrollbar(
            p,
            self.scroll,
            self.left,
            BAR_ROW_W,
            self.list_y,
            self.list_h,
            self.content_h,
        );
    }
}

fn installed(
    p: &mut Painter,
    ctx: &ScreenCtx,
    st: &mut State,
    left: f32,
    list_y: f32,
    list_h: f32,
) {
    let view = ListView::open(
        p,
        ctx,
        &mut st.installed_panes,
        (left, list_y, list_h),
        PACKS,
        st.packs.len(),
    );
    let mut clicked = None;
    for i in view.visible(st.packs.len()) {
        let pack = &st.packs[i];
        let fill = if pack.downloaded {
            DOWNLOADED_FILL
        } else {
            BY_HAND_FILL
        };
        if view.row(p, ctx, i, Some(fill), st.pack_selected == Some(i)) {
            clicked = Some(i);
        }
        let (cx, cy) = (view.row_x + PADDING, view.row_y(i) + PADDING);
        icon(p, Some(pack_icon_key(pack)), cx, cy);
        let size = plain(&Bytes(pack.size).to_string());
        let size_w = p.atlas.font.width(&size);
        p.text(
            &size,
            cx + ICON + PADDING + view.text_w - size_w,
            cy + 1.0,
            true,
        );
        let name = p
            .atlas
            .font
            .trim_to_width(&plain(&pack.name), view.text_w - size_w - SIZE_GAP);
        let name: String = name.iter().map(|span| span.text.as_str()).collect();
        entry_text(p, &name, &pack.info.description, cx, cy, view.text_w);
    }
    let empty = if st.scanning { SCANNING } else { NO_PACKS };
    view.close(p, st.packs.is_empty().then_some(empty));

    if let Some(i) = clicked {
        st.pack_selected = Some(i);
        st.pack_detail = None;
        st.installed_panes.scroll[1] = 0.0;
    }
}

fn pack_details(
    p: &mut Painter,
    ctx: &ScreenCtx,
    st: &mut State,
    left: f32,
    list_y: f32,
    list_h: f32,
) {
    let Some(index) = st.pack_selected else {
        detail(p, ctx, &mut st.installed_panes, left, list_y, list_h, None);
        return;
    };
    let pack = &st.packs[index];
    let lines = st.pack_detail.get_or_insert_with(|| pack_lines(p, pack));
    let shown = Detail {
        icon: Some(pack_icon_key(pack)),
        name: &pack.name,
        summary: &pack.info.description,
        lines: &lines[..],
    };
    detail(
        p,
        ctx,
        &mut st.installed_panes,
        left,
        list_y,
        list_h,
        Some(shown),
    );
}

fn pack_lines(p: &Painter, pack: &LocalPack) -> Vec<Vec<Span>> {
    let source = if pack.downloaded {
        "Downloaded from Modrinth"
    } else {
        "Added to the folder by hand"
    };
    let compat = match pack.info.compat {
        Compat::Compatible => "Compatible",
        Compat::TooOld => "Made for an older version",
        Compat::TooNew => "Made for a newer version",
        Compat::Unknown => "Broken or incompatible",
    };
    field_lines(
        p,
        vec![
            ("Size", Bytes(pack.size).to_string()),
            ("Source", source.to_owned()),
            ("Compatibility", compat.to_owned()),
        ],
    )
}

fn results(p: &mut Painter, ctx: &ScreenCtx, st: &mut State, left: f32, list_y: f32, list_h: f32) {
    let count = st.hits.len();
    let view = ListView::open(
        p,
        ctx,
        &mut st.browse_panes,
        (left, list_y, list_h),
        RESULTS,
        count,
    );
    let near_end = view.scroll + list_h + PREFETCH_ROWS * ROW_H >= view.content_h;
    if near_end && !st.loading && st.error.is_none() && count < st.total {
        st.request_page();
    }

    let rows = view.visible(count);
    st.assign_icons(rows.clone());
    let mut clicked = None;
    for i in rows {
        let hit = &st.hits[i];
        let fill = st
            .installed_projects
            .contains(&hit.project_id)
            .then_some(DOWNLOADED_FILL);
        if view.row(p, ctx, i, fill, st.selected == Some(i)) {
            clicked = Some(i);
        }
        let (cx, cy) = (view.row_x + PADDING, view.row_y(i) + PADDING);
        icon(p, st.row_icon_key(&hit.project_id), cx, cy);
        entry_text(p, &hit.title, &plain(&hit.description), cx, cy, view.text_w);
    }
    let status = match &st.error {
        Some(e) => e.as_str(),
        None if st.loading || st.sent.is_none() => SEARCHING,
        None => NO_RESULTS,
    };
    view.close(p, (count == 0).then_some(status));

    if let Some(i) = clicked {
        st.select(i);
    }
}

fn preview(p: &mut Painter, ctx: &ScreenCtx, st: &mut State, left: f32, list_y: f32, list_h: f32) {
    let Some(index) = st.selected else {
        detail(p, ctx, &mut st.browse_panes, left, list_y, list_h, None);
        return;
    };
    let id = st.hits[index].project_id.clone();
    if st.preview.as_ref().is_none_or(|(shown, _)| *shown != id) {
        let body = st.bodies.get(&id).and_then(Option::as_deref);
        let lines = preview_lines(p, &st.hits[index], body);
        st.preview = Some((id.clone(), lines));
    }
    let icon = st.preview_icon_key(&id);
    let hit = &st.hits[index];
    let summary = plain(&hit.description);
    let lines = st.preview.as_ref().map_or(&[][..], |(_, lines)| &lines[..]);
    let shown = Detail {
        icon,
        name: &hit.title,
        summary: &summary,
        lines,
    };
    detail(
        p,
        ctx,
        &mut st.browse_panes,
        left,
        list_y,
        list_h,
        Some(shown),
    );
}

struct Detail<'a> {
    icon: Option<String>,
    name: &'a str,
    summary: &'a [Span],
    lines: &'a [Vec<Span>],
}

fn detail(
    p: &mut Painter,
    ctx: &ScreenCtx,
    panes: &mut Panes,
    left: f32,
    list_y: f32,
    list_h: f32,
    shown: Option<Detail<'_>>,
) {
    let Some(shown) = shown else {
        let clip = p.push_clip(left, list_y, LIST_W, list_h);
        header(p, DETAILS, left, list_y, LIST_W);
        centred(p, NOTHING_SELECTED, left, list_y + HEADER_H + PADDING);
        p.pop_clip(clip);
        return;
    };
    let content_h = HEADER_H + ROW_H + shown.lines.len() as f32 * LINE_HEIGHT + PADDING;
    scroll_input_in(
        &mut panes.scroll[1],
        &mut panes.drag[1],
        ctx,
        left,
        BAR_ROW_W,
        (left, LIST_W),
        list_y,
        list_h,
        content_h,
    );
    let clip = p.push_clip(left, list_y, LIST_W, list_h);
    let top = (list_y - panes.scroll[1]).floor();
    header(p, DETAILS, left, top, LIST_W);

    let (cx, cy) = (
        left + (LIST_W - ROW_W) / 2.0 + PADDING,
        top + HEADER_H + PADDING,
    );
    icon(p, shown.icon, cx, cy);
    entry_text(
        p,
        shown.name,
        shown.summary,
        cx,
        cy,
        DETAIL_TEXT_W - ICON - PADDING,
    );
    let text_top = top + HEADER_H + ROW_H;
    for (i, line) in shown.lines.iter().enumerate() {
        let y = text_top + i as f32 * LINE_HEIGHT;
        if !row_visible(y, LINE_HEIGHT, list_y, list_h) {
            continue;
        }
        p.text(line, cx, y, true);
    }
    p.pop_clip(clip);
    draw_scrollbar(
        p,
        panes.scroll[1],
        left,
        BAR_ROW_W,
        list_y,
        list_h,
        content_h,
    );
}

fn preview_lines(p: &Painter, hit: &Hit, body: Option<&str>) -> Vec<Vec<Span>> {
    let mut fields = vec![
        ("Author", hit.author.clone()),
        ("Downloads", compact(hit.downloads)),
        ("Follows", compact(hit.follows)),
        (
            "Updated",
            hit.date_modified
                .get(..10)
                .unwrap_or(hit.date_modified.as_str())
                .to_owned(),
        ),
    ];
    if let (Some(first), Some(last)) = (hit.versions.first(), hit.versions.last()) {
        fields.push((
            "Versions",
            if first == last {
                first.clone()
            } else {
                format!("{first} to {last}")
            },
        ));
    }
    let tags: Vec<&str> = hit
        .categories
        .iter()
        .map(String::as_str)
        .filter(|c| *c != "minecraft")
        .collect();
    if !tags.is_empty() {
        fields.push(("Tags", tags.join(", ")));
    }

    let mut lines = field_lines(p, fields);
    lines.push(Vec::new());
    match body {
        Some(body) => {
            for paragraph in body.lines() {
                if paragraph.is_empty() {
                    lines.push(Vec::new());
                } else {
                    let spans = [Span::colored(paragraph, WHITE)];
                    lines.extend(p.atlas.font.wrap(&spans, DETAIL_TEXT_W));
                }
            }
        }
        None => lines.push(plain(LOADING_BODY)),
    }
    lines
}

fn field_lines(p: &Painter, fields: Vec<(&str, String)>) -> Vec<Vec<Span>> {
    let mut lines = Vec::new();
    for (label, value) in fields {
        let spans = [
            Span::colored(format!("{label}: "), DESCRIPTION),
            Span::colored(value, WHITE),
        ];
        lines.extend(p.atlas.font.wrap(&spans, DETAIL_TEXT_W));
    }
    lines
}

fn icon(p: &mut Painter, key: Option<String>, x: f32, y: f32) {
    let region = key
        .and_then(|key| crate::gui::ping::icon_slot(&key))
        .and_then(|slot| p.atlas.server_icon(slot));
    match region {
        Some(region) => p.region(region, x, y, ICON, ICON),
        None => p.sprite("unknown_pack", x, y, ICON, ICON),
    }
}

fn outline(p: &mut Painter, x: f32, y: f32, w: f32, h: f32) {
    p.fill(x, y, w, 1.0, SELECTED_OUTLINE);
    p.fill(x, y + h - 1.0, w, 1.0, SELECTED_OUTLINE);
    p.fill(x, y, 1.0, h, SELECTED_OUTLINE);
    p.fill(x + w - 1.0, y, 1.0, h, SELECTED_OUTLINE);
}

fn centred(p: &mut Painter, text: &str, left: f32, y: f32) {
    let line = p.atlas.font.trim_to_width(&plain(text), ROW_W);
    let w = p.atlas.font.width(&line);
    p.text(&line, (left + (LIST_W - w) / 2.0).floor(), y, true);
}

fn compact(n: u64) -> String {
    match n {
        1_000_000.. => format!("{:.1}M", n as f64 / 1e6),
        1_000.. => format!("{:.1}k", n as f64 / 1e3),
        _ => n.to_string(),
    }
}

fn plain_text(markdown: &str) -> String {
    let text = strip_tags(markdown);
    let mut out = String::with_capacity(text.len());
    let mut blank = true;
    for line in text.lines() {
        let line = strip_links(&strip_images(line.trim()));
        let line = line.trim_start_matches('#').trim_start_matches('>').trim();
        let rule = line.len() >= 3 && line.chars().all(|c| matches!(c, '-' | '*' | '_' | ' '));
        let line = if rule {
            String::new()
        } else {
            line.replace("**", "")
                .replace("__", "")
                .replace("~~", "")
                .replace('`', "")
        };
        let line = decode_entities(&line);
        let line = line.trim();
        if line.is_empty() {
            if !blank {
                out.push('\n');
            }
            blank = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
        blank = false;
    }
    out.trim_end().to_owned()
}

fn strip_tags(s: &str) -> String {
    const BREAKS: [&str; 15] = [
        "br", "p", "div", "h1", "h2", "h3", "h4", "h5", "h6", "li", "hr", "ul", "ol", "tr",
        "center",
    ];
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let is_tag = after.starts_with(|c: char| c.is_ascii_alphabetic() || c == '/' || c == '!');
        let Some(close) = after.find('>').filter(|_| is_tag) else {
            out.push('<');
            rest = after;
            continue;
        };
        let name: String = after[..close]
            .trim_start_matches('/')
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        if BREAKS.contains(&name.as_str()) {
            out.push('\n');
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

fn strip_images(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find("![") {
        let Some((_, end)) = link_at(&rest[open + 1..]) else {
            break;
        };
        out.push_str(&rest[..open]);
        rest = &rest[open + 1 + end..];
    }
    out.push_str(rest);
    out
}

fn strip_links(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        match link_at(&rest[open..]) {
            Some((text, end)) => {
                out.push_str(text);
                rest = &rest[open + end..];
            }
            None => {
                out.push('[');
                rest = &rest[open + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn link_at(s: &str) -> Option<(&str, usize)> {
    let close = s.find(']')?;
    let url = s[close + 1..].strip_prefix('(')?;
    let end = url.find(')')?;
    Some((&s[1..close], close + 2 + end + 1))
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    s.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::plain_text;

    #[test]
    fn markdown_to_plain() {
        let md = "# Title\n\n<p align=\"center\"><img src=\"x.png\"></p>\n\n\n**Bold** and [a link](https://example.com) and [![badge](b.svg)](https://example.com)\n---\n- item &amp; more";
        assert_eq!(
            plain_text(md),
            "Title\n\nBold and a link and\n\n- item & more"
        );
    }
}
