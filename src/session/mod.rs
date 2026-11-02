mod connection;
mod intent;
mod published;
mod shared;

pub use connection::*;
pub use intent::*;
pub use published::*;
pub use shared::*;

pub const MOVE_FORWARD: u8 = 1 << 0;
pub const MOVE_BACK: u8 = 1 << 1;
pub const MOVE_LEFT: u8 = 1 << 2;
pub const MOVE_RIGHT: u8 = 1 << 3;
pub const MOVE_JUMP: u8 = 1 << 4;
pub const MOVE_SPRINT: u8 = 1 << 5;
pub const MOVE_DESCEND: u8 = 1 << 6;
pub const MOVE_SNEAK: u8 = 1 << 7;

pub const MAX_AIR_SUPPLY: i32 = 300;

pub const VANILLA_MAX_HEALTH: f32 =
    crate::client::tracking::ranged(azalea_registry::builtin::Attribute::MaxHealth).default as f32;
