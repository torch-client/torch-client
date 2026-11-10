#![cfg_attr(not(feature = "click_gui"), allow(dead_code))]

pub mod aim_assist;
pub mod auto_mace;
pub mod auto_mine;
pub mod auto_sell;
pub mod auto_totem;
pub mod auto_walk;
pub mod better_tooltips;
pub mod chat_math;
pub mod commands;
pub mod crystal_aura;
mod edge;
pub mod entities;
pub mod esp;
pub mod extra_hud;
pub mod flight;
pub mod freecam;
pub mod fullbright;
pub mod hooks;
pub mod hud_panels;
pub mod items;
pub mod list;
pub mod nametags;
mod nearby;
pub mod no_fall;
pub mod no_render;
pub mod pathfinder;
#[cfg(feature = "click_gui")]
pub mod persist;
pub mod registry;
pub mod scaffold;
#[cfg(lua_mods)]
pub mod script;
pub mod seed_cracker;
pub mod seed_xray;
pub(crate) mod sent;
pub mod sneak;
pub mod spam;
pub mod special;
pub mod speed;
#[cfg(feature = "click_gui")]
mod store;
#[cfg(not(feature = "click_gui"))]
#[path = "store_stub.rs"]
mod store;
pub mod triggerbot;
mod turn;
mod value;
pub mod vclip;
pub mod velocity;
pub mod zoom;

pub use edge::{Edge, MS_PER_TICK, Phase, roll};
pub use store::{Store, flush_changed, save, store};
pub use value::Value;

pub(crate) fn nearest_first(
    centre: (i32, i32),
    chunks: impl IntoIterator<Item = (i32, i32)>,
) -> Vec<(i32, i32)> {
    let mut wanted: Vec<(i32, (i32, i32))> = chunks
        .into_iter()
        .map(|at| {
            let (dx, dz) = (at.0 - centre.0, at.1 - centre.1);
            (dx * dx + dz * dz, at)
        })
        .collect();
    wanted.sort_unstable();
    wanted.into_iter().map(|(_, at)| at).collect()
}

#[cfg_attr(not(feature = "click_gui"), allow(dead_code))]
pub fn builtin_button(ix: registry::ModuleIx, key: &'static str, now: f32) -> ButtonAction {
    if ix == registry::ModuleIx::from(registry::Id::AutoMine) {
        return auto_mine::button(key);
    }
    #[cfg(feature = "seed_cracker")]
    if ix == registry::ModuleIx::from(registry::Id::SeedCracker) {
        seed_cracker::button(key, now);
    }
    #[cfg(not(feature = "seed_cracker"))]
    let _ = (ix, key, now);
    ButtonAction::None
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonAction {
    None,
    Close,
}

#[cfg_attr(not(feature = "click_gui"), allow(dead_code))]
pub fn builtin_button_text(
    ix: registry::ModuleIx,
    key: &'static str,
    text: &'static str,
    now: f32,
) -> &'static str {
    #[cfg(feature = "seed_cracker")]
    if ix == registry::ModuleIx::from(registry::Id::SeedCracker) {
        return seed_cracker::button_text(key, text, now);
    }
    let _ = (ix, key, now);
    text
}
