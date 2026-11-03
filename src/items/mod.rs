pub mod consume;
pub mod dispatch;
pub mod glint;
pub mod icons;
pub(crate) mod json;
pub mod kinetic;
pub mod mesh;
pub mod model;
pub mod placed;
pub mod potions;
pub mod raster;

use std::borrow::Cow;

use azalea_inventory::ItemStackData;
use azalea_inventory::components::DataComponentTrait;

pub(crate) fn component<T: DataComponentTrait>(data: &ItemStackData) -> Option<Cow<'_, T>> {
    match data
        .component_patch
        .iter()
        .find(|(kind, _)| *kind == T::KIND)
    {
        Some((_, None)) => None,
        _ => data.get_component::<T>(),
    }
}
