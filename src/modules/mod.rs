#![cfg_attr(not(feature = "click_gui"), allow(dead_code))]

pub mod aim_assist;
pub mod auto_mace;
pub mod auto_mine;
pub mod auto_sell;
pub mod auto_totem;
pub mod chat_math;
mod edge;
pub mod entities;
pub mod esp;
pub mod flight;
pub mod freecam;
pub mod hooks;
pub mod items;
pub mod list;
pub mod nametags;
mod nearby;
pub mod no_fall;
#[cfg(feature = "click_gui")]
pub mod persist;
pub mod registry;
pub mod sneak;
pub mod special;
#[cfg(feature = "click_gui")]
mod store;
#[cfg(not(feature = "click_gui"))]
#[path = "store_stub.rs"]
mod store;
pub mod triggerbot;
mod turn;
mod value;
pub mod zoom;

pub use edge::{Edge, MS_PER_TICK, Phase, roll};
pub use store::{Store, save, store};
pub use value::Value;
