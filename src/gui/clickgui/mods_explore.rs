use super::mods::{CARD_GAP, CARD_PAD, ERROR, MODS_W, OK, STRIP_W, Style};
use super::{BIND_H, ClickGuiState, Focus, INDENT, TIP_LINE, Ui, Window};
use crate::gui::painter::Painter;
use crate::modules::script::{self, online};

enum Act {
    Back,
    Website,
    Allow,
    Forbid,
    Retry,
    Install(usize),
}

const ASK: &str = "Nothing here touches the network until you press the button below. It then downloads \
                   torchclient.com/mods-2026/index.json and every mod zip it lists into memory, now and each time \
                   this screen opens. A mod is only saved when you press its Install.";

impl ClickGuiState {
    pub(super) fn open_explore(&mut self) {
        self.mod_explore = true;
        self.config.scroll = 0.0;
        online::refresh();
    }

    pub(super) fn close_explore(&mut self) {
        self.mod_explore = false;
        self.config.scroll = 0.0;
    }

    pub(super) fn explore_window(&mut self, p: &mut Painter, ui: &Ui) {
        let look = self.look;
        let active = self.focus == Focus::On(Window::Config) && self.drag.is_none();
        let Some((x, top, cw, view_h, scroll)) =
            self.config_shell(p, ui, "Explore mods", self.mods_h, active, MODS_W)
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
        let view = online::view();

        let half = ((iw - 4.0) * 0.5).floor();
        if self.button(p, ui, ix, cy, half, bh, "Back", Style::Plain, active) {
            act = Some(Act::Back);
        }
        if self.button(
            p,
            ui,
            ix + half + 4.0,
            cy,
            iw - half - 4.0,
            bh,
            "Open website",
            Style::Plain,
            active,
        ) {
            act = Some(Act::Website);
        }
        cy += bh + 6.0;
        cy = self.install_note(p, ui, ix, cy, iw, active);

        if !view.allowed {
            cy = self.paragraph(p, ix, cy, iw, ASK, look.text());
            cy += 4.0;
            if self.button(
                p,
                ui,
                ix,
                cy,
                iw,
                bh,
                "Load mods from online server",
                Style::Primary,
                active,
            ) {
                act = Some(Act::Allow);
            }
            cy += bh + CARD_GAP;
        } else {
            let count = view.listing.as_ref().map_or(0, |l| l.mods.len());
            let status = match &view.phase {
                online::Phase::Fetching { total: 0, .. } => "Reading the mod list...".to_string(),
                online::Phase::Fetching { done, total } => {
                    format!("Downloading {} of {total}...", done + 1)
                }
                _ => match &view.listing {
                    Some(l) => format!(
                        "{count} mod{}, updated {}",
                        if count == 1 { "" } else { "s" },
                        l.updated
                    ),
                    None => String::new(),
                },
            };
            const STOP: &str = "Stop loading online";
            let sw = (p.atlas.font.width_str(STOP) + 12.0).floor();
            self.label(p, &status, ix, cy + 2.0, iw - sw - 4.0, look.text_dim());
            if self.button(p, ui, ix + iw - sw, cy, sw, bh, STOP, Style::Plain, active) {
                act = Some(Act::Forbid);
            }
            cy += bh + 4.0;

            if let online::Phase::Failed(e) = &view.phase {
                cy = self.paragraph(
                    p,
                    ix,
                    cy,
                    iw,
                    &format!("Could not load the mod list: {e}"),
                    ERROR,
                );
                cy += 2.0;
                const RETRY: &str = "Retry";
                let rw = (p.atlas.font.width_str(RETRY) + 16.0).floor();
                if self.button(p, ui, ix, cy, rw, bh, RETRY, Style::Primary, active) {
                    act = Some(Act::Retry);
                }
                cy += bh + CARD_GAP;
            }

            if let Some(listing) = &view.listing {
                self.explore_h.resize(listing.mods.len(), 0.0);
                for (i, m) in listing.mods.iter().enumerate() {
                    let (under, pressed) = self.remote_card(p, ui, m, i, ix, cy, iw, busy, active);
                    if pressed {
                        act = Some(Act::Install(i));
                    }
                    cy = under + CARD_GAP;
                }
                for line in &listing.refused {
                    cy = self.paragraph(p, ix, cy, iw, line, ERROR);
                }
            }
        }

        self.mods_h = cy + scroll - top;
        p.pop_clip(guard);

        match act {
            Some(Act::Back) => self.close_explore(),
            Some(Act::Website) => crate::platform::url::open(script::EXPLORE_URL),
            Some(Act::Allow) => online::allow(),
            Some(Act::Forbid) => online::forbid(),
            Some(Act::Retry) => online::refresh(),
            Some(Act::Install(i)) => {
                if let Some(m) = view.listing.as_ref().and_then(|l| l.mods.get(i)) {
                    m.install();
                }
            }
            None => {}
        }
    }

    #[allow(clippy::too_many_arguments, reason = "one card, flat arguments")]
    fn remote_card(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        m: &online::Remote,
        i: usize,
        ix: f32,
        top: f32,
        iw: f32,
        busy: bool,
        active: bool,
    ) -> (f32, bool) {
        use std::fmt::Write;
        let look = self.look;
        let bh = BIND_H - 1.0;
        let have = script::with_mods(|mods| {
            mods.iter()
                .find(|i| i.id == m.id)
                .map(|i| i.version.clone())
        });
        let current = have
            .as_ref()
            .is_some_and(|v| v.as_deref() == Some(m.version.as_str()));
        let label = match (&have, current) {
            (None, _) => "Install",
            (_, true) => "Installed",
            (_, false) => "Update",
        };

        let h = self.explore_h[i];
        if h > 0.0 {
            p.rounded_rect(ix, top, iw, h, look.radius, look.nest());
            let strip = if current { OK } else { look.accent() };
            p.rounded_rect(ix, top, STRIP_W, h, 1.0, 0xFF00_0000 | strip);
        }
        let cx = ix + STRIP_W + CARD_PAD;
        let cw = iw - STRIP_W - 2.0 * CARD_PAD;
        let mut cy = top + CARD_PAD;

        let vw = p.atlas.font.width_str(&m.version);
        self.label(p, &m.version, cx + cw - vw, cy, vw, look.text_dim());
        self.label(
            p,
            &m.name,
            cx,
            cy,
            (cw - vw - 4.0).max(0.0),
            look.text_title(),
        );
        cy += TIP_LINE;

        self.scratch.clear();
        let _ = write!(self.scratch, "{}", m.id);
        if let Some(l) = &m.license {
            let _ = write!(self.scratch, ", {l}");
        }
        self.label(p, &self.scratch, cx, cy, cw, look.text_dim());
        cy += TIP_LINE;
        cy = self.credits(p, cx, cy, cw, &m.author, &m.source);
        if let Some(d) = &m.description {
            cy = self.paragraph(p, cx, cy + 2.0, cw, d, look.text());
        }
        cy += 3.0;
        for perm in &m.perms {
            cy = self.paragraph(
                p,
                cx,
                cy,
                cw,
                &script::permission_notice(perm, &m.id),
                super::mods::PENDING,
            );
        }
        cy += 2.0;

        let style = if current || busy {
            Style::Idle
        } else {
            Style::Primary
        };
        let bw = (p.atlas.font.width_str(label) + 16.0).floor();
        let pressed =
            self.button(p, ui, cx, cy, bw, bh, label, style, active) && style == Style::Primary;
        cy += bh + CARD_PAD;
        self.explore_h[i] = cy - top;
        (cy, pressed)
    }
}
