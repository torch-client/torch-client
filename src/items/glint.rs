use azalea_inventory::ItemStackData;

pub const STRENGTH: f32 = 0.75;

const SPEED: f64 = 0.5;

const MAX_SPEED_MILLIS: f64 = 8.0;

const PERIOD_U: u64 = 110_000;
const PERIOD_V: u64 = 30_000;

pub const ROTATION: f32 = 0.174_532_92;

pub const ITEM_SPAN: f32 = 0.25;

pub const ARMOR_SCALE: f32 = 0.16;

const MILLIS_PER_SECOND: f64 = 1000.0 * SPEED * MAX_SPEED_MILLIS;

pub fn offset(secs: f64) -> [f32; 2] {
    let millis = (secs.max(0.0) * MILLIS_PER_SECOND) as u64;
    [
        -((millis % PERIOD_U) as f32 / PERIOD_U as f32),
        (millis % PERIOD_V) as f32 / PERIOD_V as f32,
    ]
}

pub fn periods_secs() -> [f32; 2] {
    [
        (PERIOD_U as f64 / MILLIS_PER_SECOND) as f32,
        (PERIOD_V as f64 / MILLIS_PER_SECOND) as f32,
    ]
}

pub fn has_foil(data: &ItemStackData) -> bool {
    use azalea_inventory::components as comp;

    if let Some(o) = data.get_component::<comp::EnchantmentGlintOverride>() {
        return o.show_glint;
    }
    if data
        .get_component::<comp::Enchantments>()
        .is_some_and(|e| !e.levels.is_empty())
    {
        return true;
    }
    data.kind == azalea_registry::builtin::ItemKind::Compass
        && data.get_component::<comp::LodestoneTracker>().is_some()
}

pub fn codec_foil(id: &str, components: Option<&simdnbt::owned::NbtCompound>) -> bool {
    let component = |key: &str| {
        let components = components?;
        components
            .get(&format!("minecraft:{key}"))
            .or_else(|| components.get(key))
    };
    match component("enchantment_glint_override").and_then(|t| t.byte()) {
        Some(b) => b != 0,
        None => {
            component("enchantments")
                .and_then(|t| t.compound())
                .is_some_and(|m| m.iter().next().is_some())
                || default_foil(id)
        }
    }
}

pub fn default_foil(id: &str) -> bool {
    use std::str::FromStr;

    use azalea_inventory::components::EnchantmentGlintOverride;
    use azalea_inventory::default_components::get_default_component;

    azalea_registry::builtin::ItemKind::from_str(id)
        .ok()
        .and_then(get_default_component::<EnchantmentGlintOverride>)
        .is_some_and(|o| o.show_glint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_starts_at_zero_and_wraps_on_vanillas_periods() {
        assert_eq!(offset(0.0), [0.0, 0.0]);
        let [pu, pv] = periods_secs();
        assert!((pu - 27.5).abs() < 1e-4);
        assert!((pv - 7.5).abs() < 1e-4);
        assert_eq!(offset(pu as f64)[0], 0.0);
        assert_eq!(offset(pv as f64)[1], 0.0);
    }

    #[test]
    fn registered_overrides_glint_and_plain_items_do_not() {
        assert!(default_foil("nether_star"));
        assert!(default_foil("enchanted_book"));
        assert!(!default_foil("stick"));
        assert!(!default_foil("not_an_item"));
    }
}
