pub mod consume;
pub mod dispatch;
pub mod glint;
pub mod icons;
pub(crate) mod json;
pub mod kinetic;
pub mod local_time;
pub mod mesh;
pub mod model;
pub mod placed;
pub mod potions;
pub mod raster;

use std::borrow::Cow;

use azalea_inventory::ItemStackData;
use azalea_inventory::components::DataComponentTrait;

pub(crate) fn component<T: DataComponentTrait>(data: &ItemStackData) -> Option<Cow<'_, T>> {
    data.get_unless_removed::<T>()
}
