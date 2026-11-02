use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::gui::dialog::CONFIRM_BUTTON_WIDTH;
use crate::gui::painter::Painter;
use crate::gui::tooltip::translate;
use crate::gui::widgets::{
    Button, FOOTER_GAP, FOOTER_W, HeaderFooter, TextBox, WIDGET_HEIGHT, centered_row_x,
};
use crate::gui::{GuiState, Screen, ScreenCtx, confirmlink};
use crate::session::SharedMutex;
use crate::text::{LINE_HEIGHT, Span, Style};

const RELOAD: &str = "R";
const FOOTER3_W: f32 = 120.0;

pub(in crate::gui) const SEARCH_H: f32 = 15.0;
const TITLE_Y: f32 = 4.0;
const DROP_INFO_Y: f32 = TITLE_Y + LINE_HEIGHT + 4.0;
const SEARCH_Y: f32 = DROP_INFO_Y + LINE_HEIGHT + 4.0;
pub(in crate::gui) const LAYOUT: HeaderFooter =
    HeaderFooter::new(SEARCH_Y + SEARCH_H + 4.0, HeaderFooter::DEFAULT_H);
pub(in crate::gui) const SEARCH_W: f32 = 200.0;
const SEARCH_MAX_LEN: usize = 32;
pub(in crate::gui) const GRAY: u32 = 0xAA_AAAA;

pub(in crate::gui) fn search_box() -> TextBox {
    let mut search = TextBox::bordered(SEARCH_MAX_LEN, &translate("gui.packSelection.search", &[]));
    search.focus();
    search
}

pub(in crate::gui) fn header(p: &mut Painter, ctx: &ScreenCtx, title: &str, search: &mut TextBox) {
    p.text_centered(title, ctx.vw / 2.0, TITLE_Y, 0xFFFFFF, true);
    p.text_centered(
        &translate("pack.dropInfo", &[]),
        ctx.vw / 2.0,
        DROP_INFO_Y,
        GRAY,
        true,
    );
    search.update(
        p,
        ctx,
        centered_row_x(ctx.vw, SEARCH_W),
        SEARCH_Y,
        SEARCH_W,
        SEARCH_H,
    );
}

pub(in crate::gui) struct Footer {
    pub(in crate::gui) reload: bool,
    pub(in crate::gui) x: f32,
    pub(in crate::gui) y: f32,
    pub(in crate::gui) w: f32,
}

pub(in crate::gui) fn footer(p: &mut Painter, ctx: &ScreenCtx, wide: usize) -> Footer {
    let w = if wide > 2 { FOOTER3_W } else { FOOTER_W };
    let y = LAYOUT.footer_y(ctx.vh);
    let x = centered_row_x(ctx.vw, WIDGET_HEIGHT + wide as f32 * (w + FOOTER_GAP));
    let reload = Button::new(x, y, WIDGET_HEIGHT, WIDGET_HEIGHT, RELOAD).draw(p, ctx);
    Footer {
        reload,
        x: x + WIDGET_HEIGHT + FOOTER_GAP,
        y,
        w,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(in crate::gui) fn open_folder(
    p: &mut Painter,
    ctx: &ScreenCtx,
    at: &Footer,
    x: f32,
    label: &str,
    dir: &Path,
) -> bool {
    if Button::new(x, at.y, at.w, WIDGET_HEIGHT, label).draw(p, ctx) {
        crate::platform::url::open_folder(dir);
    }
    ctx.hovering(x, at.y, at.w, WIDGET_HEIGHT)
}

#[cfg(target_arch = "wasm32")]
pub(in crate::gui) fn open_folder(
    p: &mut Painter,
    ctx: &ScreenCtx,
    at: &Footer,
    x: f32,
    _: &str,
    _: &Path,
) -> bool {
    if Button::new(x, at.y, at.w, WIDGET_HEIGHT, CHOOSE_FILES).draw(p, ctx) {
        crate::platform::upload::choose();
    }
    ctx.hovering(x, at.y, at.w, WIDGET_HEIGHT)
}

#[cfg(target_arch = "wasm32")]
const CHOOSE_FILES: &str = "Choose Files...";
#[cfg(target_arch = "wasm32")]
const IN_MEMORY: &str = "Kept until this tab closes";

pub(in crate::gui) fn folder_tooltip(p: &mut Painter, ctx: &ScreenCtx) {
    #[cfg(not(target_arch = "wasm32"))]
    tooltip(p, ctx, &translate("pack.folderInfo", &[]));
    #[cfg(target_arch = "wasm32")]
    tooltip(p, ctx, IN_MEMORY);
}

pub(in crate::gui) fn tooltip(p: &mut Painter, ctx: &ScreenCtx, text: &str) {
    if let Some(m) = ctx.mouse() {
        crate::gui::tooltip::draw_lines(
            p,
            &[vec![Span::colored(text, 0xFF_FFFF)]],
            m.x,
            m.y,
            ctx.vw,
            ctx.vh,
        );
    }
}

pub(in crate::gui) const MESSAGE_ERROR: u32 = 0xFF_5555;
pub(in crate::gui) const MESSAGE_WARNING: u32 = 0xFF_FF55;
const MESSAGE_MARGIN: f32 = 8.0;

pub(in crate::gui) fn failed_message(names: &[String]) -> Option<String> {
    (!names.is_empty()).then(|| format!("{} failed to load. Check the log.", names.join(", ")))
}

pub(in crate::gui) fn message(
    p: &mut Painter,
    ctx: &ScreenCtx,
    rect: (f32, f32, f32, f32),
    min_h: f32,
    message: Option<(&str, u32)>,
) -> f32 {
    let (_, list_y, list_w, full_h) = rect;
    let Some((text, color)) = message else {
        return full_h;
    };
    let mut lines: Vec<std::ops::Range<usize>> = Vec::new();
    p.atlas
        .font
        .wrap_ranges(text, (list_w - 2.0 * MESSAGE_MARGIN).max(1.0), &mut lines);
    if lines.is_empty() {
        return full_h;
    }
    let list_h = (full_h - lines.len() as f32 * LINE_HEIGHT - MESSAGE_MARGIN).max(min_h);
    let mut y = list_y + list_h + MESSAGE_MARGIN / 2.0;
    for range in &lines {
        let line: String = text
            .chars()
            .skip(range.start)
            .take(range.end - range.start)
            .collect();
        p.text_centered(line.trim_end(), ctx.vw / 2.0, y, color, true);
        y += LINE_HEIGHT;
    }
    list_h
}

pub(in crate::gui) struct Search {
    needle: String,
    blank: bool,
}

impl Search {
    pub(in crate::gui) fn of(search: &TextBox) -> Search {
        let needle = search.text.to_lowercase();
        Search {
            blank: needle.trim().is_empty(),
            needle,
        }
    }

    pub(in crate::gui) fn matches(&self, haystack: &str) -> bool {
        self.blank || haystack.contains(&self.needle)
    }
}

enum Prompt {
    Confirm(Vec<PathBuf>),
    Rejected(String),
}

#[derive(Default)]
pub(in crate::gui) struct Drops {
    waiting: Vec<PathBuf>,
    prompt: Option<Prompt>,
}

impl Drops {
    pub(in crate::gui) fn clear(&mut self) {
        discard(&self.waiting);
        if let Some(Prompt::Confirm(paths)) = &self.prompt {
            discard(paths);
        }
        *self = Drops::default();
    }

    pub(in crate::gui) fn cancel(&mut self) -> bool {
        let prompt = self.prompt.take();
        if let Some(Prompt::Confirm(paths)) = &prompt {
            discard(paths);
        }
        prompt.is_some()
    }

    pub(in crate::gui) fn draw(
        &mut self,
        p: &mut Painter,
        ctx: &ScreenCtx,
        shared: &Arc<SharedMutex>,
        dir: &Path,
        is_pack_dir: fn(&Path) -> bool,
    ) -> Option<bool> {
        match &mut self.prompt {
            Some(Prompt::Confirm(paths)) => paths.append(&mut self.waiting),
            Some(Prompt::Rejected(_)) => {}
            None if !self.waiting.is_empty() => {
                crate::log_info!(
                    "packs",
                    "asking whether to add {} dropped file(s)",
                    self.waiting.len()
                );
                self.prompt = Some(Prompt::Confirm(std::mem::take(&mut self.waiting)));
            }
            None => return None,
        }
        let yes_no = [translate("gui.yes", &[]), translate("gui.no", &[])];
        let back = [translate("gui.back", &[])];
        let (title, message, buttons) = match self.prompt.as_ref()? {
            Prompt::Confirm(paths) => {
                let names: Vec<String> = paths.iter().map(|path| file_name(path)).collect();
                (
                    translate("pack.dropConfirm", &[]),
                    names.join(", "),
                    &yes_no[..],
                )
            }
            Prompt::Rejected(names) => (
                translate("pack.dropRejected.title", &[]),
                translate("pack.dropRejected.message", std::slice::from_ref(names)),
                &back[..],
            ),
        };
        let Some(chosen) = confirmlink::draw_box(
            p,
            ctx,
            &title,
            &message,
            None,
            buttons,
            CONFIRM_BUTTON_WIDTH,
        ) else {
            return Some(false);
        };
        let Some(Prompt::Confirm(paths)) = self.prompt.take() else {
            return Some(false);
        };
        if chosen != 0 {
            discard(&paths);
            return Some(false);
        }
        let dropped = crate::util::pack::copy_in(&paths, dir, is_pack_dir);
        crate::log_info!(
            "packs",
            "added the drop to {}: copied {}, failed {}, not a pack: {:?}",
            dir.display(),
            dropped.copied,
            dropped.failed,
            dropped.rejected
        );
        discard(&paths);
        if dropped.failed {
            copy_failure_toast(shared, &dir.display().to_string());
        }
        if !dropped.rejected.is_empty() {
            self.prompt = Some(Prompt::Rejected(dropped.rejected.join(", ")));
        }
        Some(dropped.copied)
    }
}

pub(crate) fn collect_drops(
    mut drops: bevy::prelude::MessageReader<bevy::window::FileDragAndDrop>,
    mut state: bevy::prelude::ResMut<GuiState>,
) {
    let mut paths: Vec<PathBuf> = drops
        .read()
        .filter_map(|drop| match drop {
            bevy::window::FileDragAndDrop::DroppedFile { path_buf, .. } => Some(path_buf.clone()),
            _ => None,
        })
        .collect();
    let screen = state.screen;
    let pending = match screen {
        #[cfg(resource_packs)]
        Screen::ResourcePacks => Some(&mut state.resourcepacks.drops),
        #[cfg(feature = "shader_support")]
        Screen::ShaderPacks => Some(&mut state.shaderpacks.drops),
        _ => None,
    };
    #[cfg(target_arch = "wasm32")]
    {
        use crate::platform::upload::{self, Taker};
        upload::accept(Taker::Packs, pending.is_some());
        paths.extend(upload::take(Taker::Packs));
    }
    if let Some(pending) = pending {
        pending.waiting.append(&mut paths);
    }
}

fn discard(paths: &[PathBuf]) {
    #[cfg(target_arch = "wasm32")]
    crate::platform::upload::discard(paths);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = paths;
}

pub(in crate::gui) fn copy_failure_toast(shared: &Arc<SharedMutex>, detail: &str) {
    use crate::gui::toast::{SystemId, ToastEvent};
    crate::session::queue_toasts(
        shared,
        [ToastEvent::System {
            id: SystemId::PackCopyFailure,
            title: crate::text::styled(&translate("pack.copyFailure", &[]), Style::default()),
            message: crate::text::styled(detail, Style::default()),
            multiline: false,
        }],
    );
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
