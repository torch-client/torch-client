use std::sync::{Arc, RwLock};

use azalea_core::registry_holder::RegistryHolder;
use azalea_inventory::ItemStackData;
use azalea_inventory::components::{DataComponentTrait, TooltipDisplay};
use azalea_registry::builtin::DataComponentKind;

use crate::text::Span;

mod contents;
mod equipment;

pub(crate) type Line = Vec<Span>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TipAt {
    ItemHover,
    BeforeBook,
    BeforePotion,
    BeforeEnchantments,
    BeforeLore,
    AfterLore,
    BeforeAdvanced,
}

pub(crate) struct Ctx<'a> {
    pub registries: Option<&'a RegistryHolder>,
    pub advanced: bool,
    pub attack_speed_base: Option<f64>,
    pub peaceful: bool,
}

pub(crate) fn shows(data: &ItemStackData, kind: DataComponentKind) -> bool {
    crate::items::component::<TooltipDisplay>(data)
        .is_none_or(|display| !display.hidden_components.contains(&kind))
}

pub(crate) fn hides_tooltip(data: &ItemStackData) -> bool {
    crate::items::component::<TooltipDisplay>(data).is_some_and(|display| display.hide_tooltip)
}

pub(crate) fn shows_component<T: DataComponentTrait>(data: &ItemStackData) -> bool {
    shows(data, T::KIND)
}

pub(crate) fn lines(at: TipAt, data: &ItemStackData, ctx: &Ctx, out: &mut Vec<Line>) {
    match at {
        TipAt::AfterLore => {
            contents::push(at, data, ctx, out);
            equipment::push(at, data, ctx, out);
        }
        _ => {
            equipment::push(at, data, ctx, out);
            contents::push(at, data, ctx, out);
        }
    }
}

static ATTACK_SPEED_BASE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(4.0f64.to_bits());
static PEACEFUL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn publish_player(attack_speed_base: f64, peaceful: bool) {
    use std::sync::atomic::Ordering;
    let bits = attack_speed_base.to_bits();
    let changed = ATTACK_SPEED_BASE.swap(bits, Ordering::Relaxed) != bits;
    if PEACEFUL.swap(peaceful, Ordering::Relaxed) != peaceful || changed {
        EPOCH.fetch_add(1, Ordering::Relaxed);
    }
}

static EPOCH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(crate) fn epoch() -> u32 {
    EPOCH.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) fn player() -> (f64, bool) {
    use std::sync::atomic::Ordering;
    (
        f64::from_bits(ATTACK_SPEED_BASE.load(Ordering::Relaxed)),
        PEACEFUL.load(Ordering::Relaxed),
    )
}

static REGISTRIES: RwLock<Option<Arc<RegistryHolder>>> = RwLock::new(None);

pub(crate) fn publish(registries: Option<Arc<RegistryHolder>>) {
    if let Ok(mut slot) = REGISTRIES.write() {
        *slot = registries;
    }
    EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn registries() -> Option<Arc<RegistryHolder>> {
    REGISTRIES.read().ok().and_then(|slot| slot.clone())
}
