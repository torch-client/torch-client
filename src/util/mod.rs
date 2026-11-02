pub(crate) mod biome_color;
#[cfg(lua_mods)]
pub(crate) mod block_map_color;
pub(crate) mod block_model;
pub(crate) mod bytes;
pub(crate) mod datapack;
pub(crate) mod dye;
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
#[cfg(any(
    feature = "asset_download",
    feature = "shader_support",
    resource_packs,
    lua_mods
))]
pub(crate) mod zip;

use bevy::math::DVec3;

pub fn block_centre(cell: [i32; 3]) -> DVec3 {
    bevy::math::IVec3::from_array(cell).as_dvec3() + 0.5
}

pub fn offset_f32(from: [f64; 3], to: [f64; 3]) -> [f32; 3] {
    std::array::from_fn(|i| (to[i] - from[i]) as f32)
}
