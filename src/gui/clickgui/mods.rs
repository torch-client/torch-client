use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use super::{
    BIND_H, BindTarget, ClickGuiState, Focus, INDENT, KeyOwner, Overlay, PILL_H, PILL_W, TIP_LINE,
    Tab, Ui, Window, pill, text_y, theme,
};
use crate::gui::painter::Painter;
use crate::modules::registry::{self, COUNT, Mode, ModuleIx, SETTING_COUNT};
use crate::modules::script::{self, Verdict};

static DROPPED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

pub(crate) fn collect_drops(
    mut drops: bevy::prelude::MessageReader<bevy::window::FileDragAndDrop>,
    state: bevy::prelude::Res<crate::gui::GuiState>,
) {
    let on_tab = state.screen == crate::gui::Screen::ClickGui && state.clickgui.tab == Tab::Mods;
    for drop in drops.read() {
        if let bevy::window::FileDragAndDrop::DroppedFile { path_buf, .. } = drop
            && on_tab
        {
            DROPPED
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(path_buf.clone());
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        use crate::platform::upload::{self, Taker};
        upload::accept(Taker::Mods, on_tab);
        DROPPED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend(upload::take(Taker::Mods));
    }
}

pub(super) const PENDING: u32 = 0xFF_FF_55;

pub(super) const ERROR: u32 = 0xFF_55_55;
pub(super) const OK: u32 = 0x55_FF_55;

const CARD_PAD: f32 = 5.0;
const CARD_GAP: f32 = 5.0;
const STRIP_W: f32 = 2.0;

enum Act {
    Reload,
    #[cfg(not(target_arch = "wasm32"))]
    Folder,
    Explore,
    Reset(String),
    AskUninstall(Option<String>),
    Uninstall(String),
    Install,
    Cancel,
    DismissNote,
    SetEnabled(String, bool),
    SetGrant(String, &'static str, bool),
    Select(String),
    Author(Option<String>),
}

const MODS_W: f32 = 330.0;
const TILE_MIN_W: f32 = 100.0;
const TILE_H: f32 = 2.0 * CARD_PAD + 3.0 * TIP_LINE;
const TILE_GAP: f32 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Style {
    Primary,
    Plain,
    Idle,
}

impl ClickGuiState {
    pub(super) fn sync_scripts(&mut self) {
        let now = registry::generation();
        if now == self.scripts_seen {
            return;
        }
        self.scripts_seen = now;
        let row = |id: ModuleIx| id.0 as usize >= COUNT;
        let slot = |s: u16| s as usize >= SETTING_COUNT;

        if self.expanded.is_some_and(row) {
            #[cfg(feature = "module_panel")]
            self.close_sheet();
            #[cfg(not(feature = "module_panel"))]
            {
                self.expanded = None;
            }
        }
        if matches!(&self.owner, KeyOwner::Capture(BindTarget::Module(id)) if row(*id)) {
            self.owner = KeyOwner::None;
        }
        self.release_slots(slot);
        if matches!(&self.overlay, Some(Overlay::Picker(p)) if p.list.inline) {
            self.overlay = None;
        }
        self.refresh_matches();
    }

    pub(super) fn mods_window(&mut self, p: &mut Painter, ui: &Ui) {
        if self.mod_editor.is_some() {
            return self.mod_editor_window(p, ui);
        }
        let look = self.look;
        let active = self.focus == Focus::On(Window::Config) && self.drag.is_none();
        let Some((x, top, cw, view_h, scroll)) =
            self.config_shell(p, ui, "Mods", self.mods_h, active, MODS_W)
        else {
            return;
        };
        let guard = p.push_clip(x, top, cw, view_h);
        self.band = (top, top + view_h);
        let ix = x + INDENT;
        let iw = cw - 2.0 * INDENT;
        let bh = BIND_H - 1.0;
        let mut cy = top + 4.0 - scroll;
        let mut act = None;

        let busy = script::busy();
        let profile = crate::modules::store().profile();
        let (reload, style) = if busy {
            ("Reloading...", Style::Idle)
        } else {
            ("Reload mods", Style::Primary)
        };
        #[cfg(not(target_arch = "wasm32"))]
        let buttons = [
            (reload, style, Act::Reload),
            ("Open mods folder", Style::Plain, Act::Folder),
            ("Explore mods", Style::Plain, Act::Explore),
        ];
        #[cfg(target_arch = "wasm32")]
        let buttons = [
            (reload, style, Act::Reload),
            ("Explore mods", Style::Plain, Act::Explore),
        ];
        let n = buttons.len() as f32;
        let cell = ((iw - 4.0 * (n - 1.0)) / n).floor();
        let widest = buttons
            .iter()
            .map(|b| p.atlas.font.width_str(b.0))
            .fold(0.0, f32::max);
        let row = cell >= widest + 10.0;
        let (bw, step_x, step_y) = if row {
            (cell, cell + 4.0, 0.0)
        } else {
            (iw, 0.0, bh + 3.0)
        };
        for (k, (label, style, chosen)) in buttons.into_iter().enumerate() {
            let (bx, by) = (ix + k as f32 * step_x, cy + k as f32 * step_y);
            if self.button(p, ui, bx, by, bw, bh, label, style, active) && style != Style::Idle {
                act = Some(chosen);
            }
        }
        cy += if row { bh } else { n * bh + (n - 1.0) * 3.0 } + 3.0;
        const NEW: &str = "New mod";
        let nw = (p.atlas.font.width_str(NEW) + 16.0).floor();
        let dw = iw - nw - 4.0;
        if self.button(p, ui, ix + dw + 4.0, cy, nw, bh, NEW, Style::Plain, active) {
            act = Some(Act::Author(None));
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.button(
            p,
            ui,
            ix,
            cy,
            dw,
            bh,
            "Drop mods to upload",
            Style::Idle,
            active,
        );
        #[cfg(target_arch = "wasm32")]
        if self.button(
            p,
            ui,
            ix,
            cy,
            dw,
            bh,
            "Choose or drop mods",
            Style::Plain,
            active,
        ) {
            crate::platform::upload::choose_mods();
        }
        cy += bh + 6.0;

        let dropped = std::mem::take(&mut *DROPPED.lock().unwrap_or_else(PoisonError::into_inner));
        if !dropped.is_empty() {
            let checked = script::check_drop(dropped, &self.mod_drop);
            self.mod_drop.extend(checked);
        }
        if !self.mod_drop.is_empty() {
            cy = self.install_card(p, ui, ix, cy, iw, busy, active, &mut act);
            cy += CARD_GAP + 2.0;
        }
        if let Some(note) = script::install_note() {
            cy = self.paragraph(p, ix, cy, iw, &note, ERROR);
            let dw = (p.atlas.font.width_str("Dismiss") + 12.0).floor();
            if self.button(
                p,
                ui,
                ix + iw - dw,
                cy + 2.0,
                dw,
                bh,
                "Dismiss",
                Style::Plain,
                active,
            ) {
                act = Some(Act::DismissNote);
            }
            cy += bh + 2.0 + CARD_GAP;
        }

        script::with_mods(|mods| {
            use std::fmt::Write;
            self.scratch.clear();
            let failed = mods
                .iter()
                .filter(|m| m.error.is_some() && !m.disabled)
                .count();
            let off = mods.iter().filter(|m| m.disabled).count();
            let _ = write!(self.scratch, "Installed: {}", mods.len());
            if off > 0 {
                let _ = write!(self.scratch, ", {off} disabled");
            }
            if failed > 0 {
                let _ = write!(self.scratch, ", {failed} failed");
            }
            self.label(p, &self.scratch, ix, cy, iw, look.text_dim());
            cy += TIP_LINE + 2.0;

            if mods.is_empty() {
                let h = self.mod_detail_h;
                let start = cy;
                p.rounded_rect(ix, cy, iw, h, look.radius, look.nest());
                cy += CARD_PAD;
                cy = self.paragraph(
                    p,
                    ix + CARD_PAD,
                    cy,
                    iw - 2.0 * CARD_PAD,
                    "No mods yet.",
                    look.text(),
                );
                cy = self.paragraph(
                    p,
                    ix + CARD_PAD,
                    cy,
                    iw - 2.0 * CARD_PAD,
                    "Drop a .lua file, a mod folder or a .zip on this window, or put it in the mods folder and \
                     press Reload mods.",
                    look.text_dim(),
                );
                cy += CARD_PAD;
                self.mod_detail_h = cy - start;
            }

            let cols = (((iw + TILE_GAP) / (TILE_MIN_W + TILE_GAP)).floor() as usize).max(1);
            let tw = ((iw - (cols - 1) as f32 * TILE_GAP) / cols as f32).floor();
            if !busy
                && self
                    .mod_selected
                    .as_ref()
                    .is_some_and(|id| !mods.iter().any(|m| &m.id == id))
            {
                self.mod_selected = None;
            }
            for (i, m) in mods.iter().enumerate() {
                let tx = ix + (i % cols) as f32 * (tw + TILE_GAP);
                let ty = cy + (i / cols) as f32 * (TILE_H + TILE_GAP);
                let selected = self.mod_selected.as_deref() == Some(m.id.as_str());
                if let Some(a) = self.tile(p, ui, m, tx, ty, tw, selected, busy, active) {
                    act = Some(a);
                }
            }
            let rows = mods.len().div_ceil(cols);
            if rows > 0 {
                cy += rows as f32 * (TILE_H + TILE_GAP) + CARD_GAP - TILE_GAP;
            }

            let Some(m) = self
                .mod_selected
                .as_ref()
                .and_then(|id| mods.iter().find(|m| &m.id == id))
            else {
                return;
            };
            {
                let h = self.mod_detail_h;
                let ok = m.error.is_none();
                let start = cy;
                let strip = if m.disabled {
                    look.text_dim()
                } else if ok {
                    OK
                } else {
                    ERROR
                };
                if h > 0.0 {
                    p.rounded_rect(ix, cy, iw, h, look.radius, look.nest());
                    p.rounded_rect(ix, cy, STRIP_W, h, 1.0, 0xFF00_0000 | strip);
                }
                let cx = ix + STRIP_W + CARD_PAD;
                let cw = iw - STRIP_W - 2.0 * CARD_PAD;
                cy += CARD_PAD;

                let version = m.version.as_deref().unwrap_or("");
                let vw = p.atlas.font.width_str(version);
                self.label(p, version, cx + cw - vw, cy, vw, look.text_dim());
                let name = m.name.as_deref().unwrap_or(&m.id);
                let ink = if m.disabled {
                    look.text_dim()
                } else {
                    look.text_title()
                };
                self.label(p, name, cx, cy, (cw - vw - 4.0).max(0.0), ink);
                cy += TIP_LINE;

                self.scratch.clear();
                let _ = write!(self.scratch, "{}, {}", m.id, m.shape.name());
                if let Some(l) = &m.license {
                    let _ = write!(self.scratch, ", {l}");
                }
                self.label(p, &self.scratch, cx, cy, cw, look.text_dim());
                cy += TIP_LINE;
                for (prefix, value) in [("By ", &m.author), ("Source: ", &m.source)] {
                    let Some(value) = value else { continue };
                    let mut line = std::mem::take(&mut self.scratch);
                    line.clear();
                    let _ = write!(line, "{prefix}{value}");
                    cy = self.paragraph(p, cx, cy, cw, &line, look.text_dim());
                    self.scratch = line;
                }
                if let Some(d) = &m.description {
                    let ink = if m.disabled {
                        look.text_dim()
                    } else {
                        look.text()
                    };
                    cy = self.paragraph(p, cx, cy + 2.0, cw, d, ink);
                }
                cy += 3.0;

                match &m.error {
                    _ if m.disabled => {
                        self.label(p, "Disabled", cx, cy, cw, look.text_dim());
                        cy += TIP_LINE;
                    }
                    None => {
                        self.scratch.clear();
                        let n = m.modules.len();
                        let _ = write!(
                            self.scratch,
                            "Loaded, {n} module{}",
                            if n == 1 { "" } else { "s" }
                        );
                        self.label(p, &self.scratch, cx, cy, cw, OK);
                        cy += TIP_LINE;
                        for module in &m.modules {
                            let hidden = module.mode > profile;
                            let dot = if hidden {
                                look.text_dim()
                            } else {
                                look.accent()
                            };
                            p.rounded_rect(cx + 1.0, cy + 3.0, 3.0, 3.0, 1.0, 0xFF00_0000 | dot);
                            let tag = if hidden {
                                Mode::NAMES[module.mode as usize]
                            } else {
                                ""
                            };
                            let tw = p.atlas.font.width_str(tag);
                            if hidden {
                                self.label(p, tag, cx + cw - tw, cy, tw, look.text_dim());
                            }
                            let ink = if hidden { look.text_dim() } else { look.text() };
                            self.label(
                                p,
                                module.name,
                                cx + 7.0,
                                cy,
                                (cw - 7.0 - tw - 4.0).max(0.0),
                                ink,
                            );
                            cy += TIP_LINE;
                        }
                        if m.modules.iter().any(|module| module.mode > profile) {
                            cy = self.paragraph(
                                p,
                                cx,
                                cy,
                                cw,
                                "Dimmed modules are hidden at this profile; the tag is the profile they need.",
                                look.text_dim(),
                            );
                        }
                    }
                    Some(e) => {
                        self.label(p, "Failed to load", cx, cy, cw, ERROR);
                        cy += TIP_LINE;
                        let e = e.split("\nstack traceback").next().unwrap_or(e);
                        cy = self.paragraph(p, cx, cy, cw, e, look.text());
                    }
                }
                if let Some(e) = &m.last_error {
                    cy += 3.0;
                    self.label(p, "Switched off by an error", cx, cy, cw, ERROR);
                    cy += TIP_LINE;
                    cy = self.paragraph(p, cx, cy, cw, e, look.text());
                }

                for g in &m.perms {
                    cy += 3.0;
                    let line = if g.granted {
                        format!("{}: allowed", g.perm)
                    } else {
                        format!(
                            "{}: blocked. The mod cannot use {} until you allow it.",
                            g.perm, g.perm
                        )
                    };
                    let ink = if g.granted { OK } else { PENDING };
                    cy = self.paragraph(p, cx, cy, cw, &line, ink);
                    cy = self.paragraph(
                        p,
                        cx,
                        cy,
                        cw,
                        &script::permission_notice(g.perm, &m.id),
                        look.text_dim(),
                    );
                    cy += 2.0;
                    let text = if g.granted { g.block } else { g.allow };
                    let lw = (p.atlas.font.width_str(text) + 12.0).floor();
                    let style = if g.granted {
                        Style::Plain
                    } else {
                        Style::Primary
                    };
                    if self.button(p, ui, cx, cy, lw, bh, text, style, active) {
                        act = Some(Act::SetGrant(m.id.clone(), g.perm, !g.granted));
                    }
                    cy += bh;
                }

                cy += 4.0;
                if self.mod_uninstall.as_deref() == Some(m.id.as_str()) {
                    let q = format!("Delete {name}? This removes the mod and its saved settings.");
                    cy = self.paragraph(p, cx, cy, cw, &q, PENDING);
                    cy += 2.0;
                    let half = ((cw - 4.0) * 0.5).floor();
                    let style = if busy { Style::Idle } else { Style::Primary };
                    if self.button(p, ui, cx, cy, half, bh, "Delete", style, active) && !busy {
                        act = Some(Act::Uninstall(m.id.clone()));
                    }
                    if self.button(
                        p,
                        ui,
                        cx + half + 4.0,
                        cy,
                        cw - half - 4.0,
                        bh,
                        "Cancel",
                        Style::Plain,
                        active,
                    ) {
                        act = Some(Act::AskUninstall(None));
                    }
                } else {
                    const RESET: &str = "Reset data";
                    let rw = (p.atlas.font.width_str(RESET) + 12.0).floor();
                    if self.button(p, ui, cx + cw - rw, cy, rw, bh, RESET, Style::Plain, active)
                        && !busy
                    {
                        act = Some(Act::Reset(m.id.clone()));
                    }
                    if m.shape == script::Shape::File {
                        const EDIT: &str = "Edit";
                        let ew = (p.atlas.font.width_str(EDIT) + 16.0).floor();
                        let ex = cx + cw - rw - 4.0 - ew;
                        if self.button(p, ui, ex, cy, ew, bh, EDIT, Style::Primary, active) {
                            act = Some(Act::Author(Some(m.file.clone())));
                        }
                    }
                    const DELETE: &str = "Delete";
                    let uw = (p.atlas.font.width_str(DELETE) + 12.0).floor();
                    if self.button(p, ui, cx, cy, uw, bh, DELETE, Style::Plain, active) && !busy {
                        act = Some(Act::AskUninstall(Some(m.id.clone())));
                    }
                }
                cy += bh + CARD_PAD;
                self.mod_detail_h = cy - start;
                cy += CARD_GAP;
            }
        });

        self.mods_h = cy + scroll - top;
        p.pop_clip(guard);

        match act {
            Some(Act::Reload) => script::reload(),
            #[cfg(not(target_arch = "wasm32"))]
            Some(Act::Folder) => crate::platform::url::open_folder(&script::folder()),
            Some(Act::Explore) => crate::platform::url::open(script::EXPLORE_URL),
            Some(Act::Reset(id)) => script::reset(id),
            Some(Act::AskUninstall(id)) => self.mod_uninstall = id,
            Some(Act::Uninstall(id)) => {
                self.mod_uninstall = None;
                script::uninstall(id);
            }
            Some(Act::Install) => self.answer_mod_drop(true),
            Some(Act::Cancel) => self.answer_mod_drop(false),
            Some(Act::DismissNote) => script::clear_install_note(),
            Some(Act::SetEnabled(id, on)) => script::set_mod_enabled(id, on),
            Some(Act::SetGrant(id, perm, on)) => script::set_granted(&id, perm, on),
            Some(Act::Select(id)) => {
                self.mod_uninstall = None;
                self.mod_selected = if self.mod_selected.as_deref() == Some(id.as_str()) {
                    None
                } else {
                    Some(id)
                };
            }
            Some(Act::Author(file)) => self.open_mod_editor(file),
            None => {}
        }
    }

    pub(super) fn answer_mod_drop(&mut self, install: bool) {
        let drop = std::mem::take(&mut self.mod_drop);
        #[cfg(target_arch = "wasm32")]
        let staged: Vec<PathBuf> = drop.iter().map(|c| c.path.clone()).collect();
        if install {
            script::install(drop);
        }
        #[cfg(target_arch = "wasm32")]
        crate::platform::upload::discard(&staged);
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn tile(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        m: &script::ModInfo,
        x: f32,
        y: f32,
        w: f32,
        selected: bool,
        busy: bool,
        active: bool,
    ) -> Option<Act> {
        let look = self.look;
        let hovered = self.lit(ui, x, y, w, TILE_H, active);
        let fill = if selected {
            look.header()
        } else if hovered {
            look.raised()
        } else {
            look.nest()
        };
        p.rounded_rect(x, y, w, TILE_H, look.radius, fill);
        self.scratch.clear();
        let (status, colour) = if m.disabled {
            ("Disabled", look.text_dim())
        } else if m.error.is_some() {
            ("Failed to load", ERROR)
        } else if m.last_error.is_some() {
            ("Error, see details", ERROR)
        } else if let Some(g) = m.perms.iter().find(|g| !g.granted) {
            use std::fmt::Write;
            let _ = write!(self.scratch, "Needs {}: click", g.perm);
            ("", PENDING)
        } else {
            use std::fmt::Write;
            let n = m.modules.len();
            let _ = write!(self.scratch, "{n} module{}", if n == 1 { "" } else { "s" });
            ("", OK)
        };
        let strip = if m.disabled {
            look.text_dim()
        } else if m.error.is_some() {
            ERROR
        } else if m.perms.iter().any(|g| !g.granted) {
            PENDING
        } else {
            OK
        };
        p.rounded_rect(x, y, STRIP_W, TILE_H, 1.0, 0xFF00_0000 | strip);

        let cx = x + STRIP_W + CARD_PAD;
        let cw = w - STRIP_W - 2.0 * CARD_PAD;
        let mut cy = y + CARD_PAD;
        let (sx, sy) = (
            cx + cw - PILL_W,
            cy + ((TIP_LINE - PILL_H) * 0.5).floor() - 1.0,
        );
        pill(p, look, sx, sy, !m.disabled);
        let name = m.name.as_deref().unwrap_or(&m.id);
        let ink = if m.disabled {
            look.text_dim()
        } else {
            look.text_title()
        };
        self.label(p, name, cx, cy, (cw - PILL_W - 4.0).max(0.0), ink);
        cy += TIP_LINE;
        let second = m.version.as_deref().unwrap_or(m.shape.name());
        self.label(p, second, cx, cy, cw, look.text_dim());
        cy += TIP_LINE;
        let status = if status.is_empty() {
            self.scratch.as_str()
        } else {
            status
        };
        self.label(p, status, cx, cy, cw, colour);

        if self.press(ui, sx - 2.0, sy - 2.0, PILL_W + 4.0, PILL_H + 4.0, active) {
            return (!busy).then(|| Act::SetEnabled(m.id.clone(), m.disabled));
        }
        self.press(ui, x, y, w, TILE_H, active)
            .then(|| Act::Select(m.id.clone()))
    }

    #[allow(clippy::too_many_arguments, reason = "one card, flat arguments")]
    fn install_card(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        ix: f32,
        top: f32,
        iw: f32,
        busy: bool,
        active: bool,
        act: &mut Option<Act>,
    ) -> f32 {
        use std::fmt::Write;
        let look = self.look;
        let bh = BIND_H - 1.0;
        let h = self.drop_card_h;
        if h > 0.0 {
            p.rounded_rect(ix, top, iw, h, look.radius, look.nest());
            p.rounded_rect(ix, top, STRIP_W, h, 1.0, 0xFF00_0000 | PENDING);
        }
        let cx = ix + STRIP_W + CARD_PAD;
        let cw = iw - STRIP_W - 2.0 * CARD_PAD;
        let mut cy = top + CARD_PAD;

        let installs = self.mod_drop.iter().filter(|c| c.installs()).count();
        self.scratch.clear();
        let _ = write!(
            self.scratch,
            "Install {installs} mod{}?",
            if installs == 1 { "" } else { "s" }
        );
        self.label(p, &self.scratch, cx, cy, cw, PENDING);
        cy += TIP_LINE + 3.0;

        let drop = std::mem::take(&mut self.mod_drop);
        for c in &drop {
            let version = c.version.as_deref().unwrap_or("");
            let vw = p.atlas.font.width_str(version);
            self.label(p, version, cx + cw - vw, cy, vw, look.text_dim());
            self.label(
                p,
                &c.title,
                cx,
                cy,
                (cw - vw - 4.0).max(0.0),
                look.text_title(),
            );
            cy += TIP_LINE;
            let (line, colour) = match &c.verdict {
                Verdict::New => (format!("{}: new", c.file), OK),
                Verdict::Replaces(old) => {
                    (format!("{}: replaces {}", c.file, old.join(", ")), PENDING)
                }
                Verdict::Rejected(why) => (format!("{}: {why}", c.file), ERROR),
            };
            cy = self.paragraph(p, cx, cy, cw, &line, colour);
            if c.installs() {
                for perm in &c.perms {
                    cy = self.paragraph(
                        p,
                        cx,
                        cy,
                        cw,
                        &script::permission_notice(perm, &c.id),
                        PENDING,
                    );
                }
            }
            cy += 3.0;
        }
        self.mod_drop = drop;

        cy += 2.0;
        let half = ((cw - 4.0) * 0.5).floor();
        let style = if installs > 0 && !busy {
            Style::Primary
        } else {
            Style::Idle
        };
        if self.button(p, ui, cx, cy, half, bh, "Install", style, active) && style == Style::Primary
        {
            *act = Some(Act::Install);
        }
        if self.button(
            p,
            ui,
            cx + half + 4.0,
            cy,
            cw - half - 4.0,
            bh,
            "Cancel",
            Style::Plain,
            active,
        ) {
            *act = Some(Act::Cancel);
        }
        cy += bh + CARD_PAD;
        self.drop_card_h = cy - top;
        cy
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn paragraph(&mut self, p: &mut Painter, x: f32, y: f32, w: f32, text: &str, argb: u32) -> f32 {
        self.wrap(p, text, w);
        for (i, &(a, b)) in self.tip_lines.iter().enumerate() {
            self.label(p, &text[a..b], x, y + i as f32 * TIP_LINE, w, argb);
        }
        y + self.tip_lines.len().max(1) as f32 * TIP_LINE
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    pub(super) fn button(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        text: &str,
        style: Style,
        active: bool,
    ) -> bool {
        let look = self.look;
        let hovered = self.lit(ui, x, y, w, h, active) && style != Style::Idle;
        let (fill, ink) = match style {
            Style::Primary => (
                if hovered {
                    look.accent_hi()
                } else {
                    look.accent()
                },
                theme::TEXT_ON,
            ),
            Style::Plain => (
                if hovered {
                    look.nest_hi()
                } else {
                    look.header()
                },
                look.text(),
            ),
            Style::Idle => (look.header(), look.text_dim()),
        };
        p.rounded_rect(x, y, w, h, 3.0, fill);
        let tw = p.atlas.font.width_str(text);
        p.text_plain(
            text,
            (x + (w - tw) * 0.5).floor(),
            text_y(y, h),
            ink,
            look.shadow,
        );
        self.press(ui, x, y, w, h, active)
    }
}
