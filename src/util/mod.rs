pub(crate) mod biome_color;
pub(crate) mod block_model;
pub(crate) mod bytes;
pub(crate) mod calendar;
pub(crate) mod datapack;
pub(crate) mod dye;
#[cfg(any(
    feature = "asset_download",
    feature = "skins",
    feature = "update_checker",
    resource_packs,
    lua_mods
))]
pub(crate) mod hash;
pub(crate) mod interp;
pub(crate) mod javarandom;
pub(crate) mod level_events;
pub(crate) mod map_color;
pub(crate) mod mth;
pub(crate) mod named_cycle;
#[cfg(any(feature = "shader_support", resource_packs, lua_mods))]
#[cfg_attr(not(any(feature = "shader_support", resource_packs)), allow(dead_code))]
pub(crate) mod pack;
pub(crate) mod particle_assets;
pub(crate) mod particles;
pub(crate) mod reload;
pub(crate) mod variants;
#[cfg_attr(not(feature = "multiversion"), allow(dead_code))]
pub(crate) mod wire;
#[cfg(any(
    feature = "asset_download",
    feature = "shader_support",
    resource_packs,
    lua_mods
))]
pub(crate) mod zip;

use bevy::math::DVec3;

macro_rules! global {
    ($(#[$meta:meta])* $vis:vis fn $name:ident() -> $ty:ty) => {
        $(#[$meta])*
        $vis fn $name() -> &'static $ty {
            static CELL: ::std::sync::OnceLock<$ty> = ::std::sync::OnceLock::new();
            CELL.get_or_init(Default::default)
        }
    };
}
pub(crate) use global;

pub fn block_centre(cell: [i32; 3]) -> DVec3 {
    bevy::math::IVec3::from_array(cell).as_dvec3() + 0.5
}

pub fn offset_f32(from: [f64; 3], to: [f64; 3]) -> [f32; 3] {
    std::array::from_fn(|i| (to[i] - from[i]) as f32)
}
