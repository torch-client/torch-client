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
const PANELS: &[Panel] = &[Panel {
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
}];
#[cfg(not(feature = "click_gui"))]
const PANELS: &[Panel] = &[];

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
