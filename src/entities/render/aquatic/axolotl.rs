use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::models::aquatic::axolotl;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Axolotl,
        RenderSpec::new(
            "axolotl",
            axolotl::adult_layer,
            adult_texture,
            axolotl::adult_setup,
        )
        .with_visible(is_adult),
    );
    registry.add(
        EntityKind::Axolotl,
        RenderSpec::new(
            "axolotl_baby",
            axolotl::baby_layer,
            baby_texture,
            axolotl::baby_setup,
        )
        .with_visible(is_baby),
    );
}

fn is_adult(st: &EntityState) -> bool {
    !st.extras.is_baby
}

fn is_baby(st: &EntityState) -> bool {
    st.extras.is_baby
}

fn variant_name(st: &EntityState) -> &'static str {
    match st.extras.variant_id {
        1 => "wild",
        2 => "gold",
        3 => "cyan",
        4 => "blue",
        _ => "lucy",
    }
}

fn adult_texture(st: &EntityState) -> String {
    format!("entity/axolotl/axolotl_{}", variant_name(st))
}

fn baby_texture(st: &EntityState) -> String {
    format!("entity/axolotl/axolotl_{}_baby", variant_name(st))
}
