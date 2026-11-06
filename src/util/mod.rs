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

#[cfg(any(feature = "audio", feature = "shader_support"))]
macro_rules! indexed_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $($(#[$variant_meta])* $variant),*
        }

        impl $name {
            $vis const COUNT: usize = [$($name::$variant),*].len();

            $vis const ALL: [$name; Self::COUNT] = [$($name::$variant),*];

            #[allow(dead_code)]
            $vis const fn index(self) -> usize {
                self as usize
            }
        }
    };
}
#[cfg(any(feature = "audio", feature = "shader_support"))]
pub(crate) use indexed_enum;

pub fn block_centre(cell: [i32; 3]) -> DVec3 {
    bevy::math::IVec3::from_array(cell).as_dvec3() + 0.5
}

pub fn offset_f32(from: [f64; 3], to: [f64; 3]) -> [f32; 3] {
    std::array::from_fn(|i| (to[i] - from[i]) as f32)
}

pub(crate) fn same_bits<T: bytemuck::Pod>(a: &[T], b: &[T]) -> bool {
    bytemuck::cast_slice::<T, u8>(a) == bytemuck::cast_slice::<T, u8>(b)
}

#[cfg(any(feature = "audio", lua_mods))]
pub(crate) fn intern(s: String) -> &'static str {
    use std::collections::HashSet;
    use std::sync::{Mutex, PoisonError};
    static SEEN: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);
    let mut seen = SEEN.lock().unwrap_or_else(PoisonError::into_inner);
    let seen = seen.get_or_insert_with(HashSet::new);
    if let Some(&s) = seen.get(s.as_str()) {
        return s;
    }
    let s: &'static str = Box::leak(s.into_boxed_str());
    seen.insert(s);
    s
}
