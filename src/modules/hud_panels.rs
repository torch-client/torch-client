#![cfg_attr(not(feature = "click_gui"), allow(unused_imports))]

use bevy::math::Vec2;

use super::registry::Id;
use super::store;
use crate::gui::Snapshot;
use crate::gui::hud_layout::{Anchor, ElementId, Hud, ModPanel};
use crate::gui::painter::Painter;

pub struct Panel {
    pub module: Id,
    pub def: ModPanel,
    pub draw: fn(&mut Painter, &Snapshot),
}

#[cfg(feature = "click_gui")]
const PANELS: &[Panel] = &[
    Panel {
        module: Id::ExtraHud,
        def: ModPanel {
            key: "mod:builtin.extra_hud/armor",
            label: "Armor",
            w: 82.0,
            h: 34.0,
            anchor: Anchor::Bottom,
            offset: Vec2::new(-168.0, 0.0),
        },
        draw: super::extra_hud::draw_armor,
    },
    Panel {
        module: Id::SeedCracker,
        def: ModPanel {
            key: "mod:builtin.seed_cracker/progress",
            label: "Seed Cracker",
            w: super::seed_cracker::HUD_W,
            h: super::seed_cracker::HUD_H,
            anchor: Anchor::TopLeft,
            offset: Vec2::new(2.0, 40.0),
        },
        draw: super::seed_cracker::draw_hud,
    },
    Panel {
        module: Id::AutoMine,
        def: ModPanel {
            key: "mod:builtin.auto_mine/progress",
            label: "AutoMine",
            w: super::auto_mine::hud::HUD_W,
            h: super::auto_mine::hud::HUD_H,
            anchor: Anchor::TopLeft,
            offset: Vec2::new(2.0, 100.0),
        },
        draw: super::auto_mine::hud::draw,
    },
];
#[cfg(not(feature = "click_gui"))]
const PANELS: &[Panel] = &[];

pub struct TextPanel<'p, 'a> {
    p: &'p mut Painter<'a>,
    w: f32,
    y: f32,
}

impl<'p, 'a> TextPanel<'p, 'a> {
    pub const PAD: f32 = 4.0;
    const BACKGROUND: u32 = 0x9000_0000;
    const BAR_H: f32 = 3.0;
    const BAR_GAP: f32 = 2.0;
    const BAR_BACK: u32 = 0xFF40_4040;

    pub const fn height(lines: usize, bars: usize) -> f32 {
        Self::PAD * 2.0
            + crate::text::LINE_HEIGHT * lines as f32
            + (Self::BAR_H + Self::BAR_GAP * 2.0) * bars as f32
    }

    pub fn new(p: &'p mut Painter<'a>, w: f32) -> Self {
        TextPanel { p, w, y: Self::PAD }
    }

    fn row(&mut self, h: f32) {
        let top = if self.y == Self::PAD { 0.0 } else { self.y };
        self.p
            .fill(0.0, top, self.w, self.y + h - top, Self::BACKGROUND);
    }

    pub fn text(&mut self, text: &str, rgb: u32) {
        self.row(crate::text::LINE_HEIGHT);
        let inner = self.w - Self::PAD * 2.0;
        let font = &self.p.atlas.font;
        let shown = if font.width_str(text) <= inner {
            std::borrow::Cow::Borrowed(text)
        } else {
            let mut cut = String::new();
            for c in text.chars() {
                cut.push(c);
                if font.width_str(&cut) + font.width_str("...") > inner {
                    cut.pop();
                    break;
                }
            }
            std::borrow::Cow::Owned(cut + "...")
        };
        self.p.text_plain(&shown, Self::PAD, self.y, rgb, true);
        self.y += crate::text::LINE_HEIGHT;
    }

    pub fn bar(&mut self, share: f32, rgb: u32) {
        self.row(Self::BAR_H + Self::BAR_GAP * 2.0);
        let inner = self.w - Self::PAD * 2.0;
        let y = self.y + Self::BAR_GAP;
        self.p
            .fill(Self::PAD, y, inner, Self::BAR_H, Self::BAR_BACK);
        let share = share.clamp(0.0, 1.0);
        if share > 0.0 {
            self.p.fill(
                Self::PAD,
                y,
                (inner * share).max(1.0),
                Self::BAR_H,
                0xFF00_0000 | rgb,
            );
        }
        self.y = y + Self::BAR_H + Self::BAR_GAP;
    }
}

impl Drop for TextPanel<'_, '_> {
    fn drop(&mut self) {
        if self.y > Self::PAD {
            self.p
                .fill(0.0, self.y, self.w, Self::PAD, Self::BACKGROUND);
        }
    }
}

pub const COUNT: usize = PANELS.len();
const _: () = assert!(COUNT <= crate::gui::hud_layout::MOD_PANELS);

pub fn def(i: usize) -> Option<ModPanel> {
    PANELS.get(i).map(|p| p.def)
}

pub fn draw(p: &mut Painter, vw: f32, vh: f32, snap: &Snapshot, hud: &mut Hud) {
    let s = store();
    for (i, panel) in PANELS.iter().enumerate() {
        if s.enabled(panel.module) {
            hud.draw(p, vw, vh, ElementId::mod_panel_slot(i), |p| {
                (panel.draw)(p, snap)
            });
        }
    }
}
