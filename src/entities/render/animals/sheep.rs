use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::models::animals::sheep;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

use super::dye;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Sheep,
        RenderSpec::new("sheep", sheep::layer, adult_texture, sheep::setup_anim)
            .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Sheep,
        RenderSpec::new(
            "sheep_baby",
            sheep::baby_layer,
            baby_texture,
            sheep::setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
    registry.add(
        EntityKind::Sheep,
        RenderSpec::new(
            "sheep_wool_undercoat",
            sheep::layer,
            undercoat_texture,
            sheep::setup_anim,
        )
        .with_visible(undercoat_visible)
        .with_tint(dye::sheep_wool_tint),
    );
    registry.add(
        EntityKind::Sheep,
        RenderSpec::new(
            "sheep_wool",
            sheep::fur_layer,
            wool_texture,
            sheep::setup_anim,
        )
        .with_visible(|st| !st.extras.sheared && !st.extras.is_baby)
        .with_tint(dye::sheep_wool_tint),
    );
    registry.add(
        EntityKind::Sheep,
        RenderSpec::new(
            "sheep_wool_baby",
            sheep::baby_layer,
            wool_texture,
            sheep::setup_anim,
        )
        .with_visible(|st| !st.extras.sheared && st.extras.is_baby)
        .with_tint(dye::sheep_wool_tint),
    );
}

fn adult_texture(_st: &EntityState) -> String {
    "entity/sheep/sheep".to_string()
}

fn baby_texture(_st: &EntityState) -> String {
    "entity/sheep/sheep_baby".to_string()
}

fn undercoat_texture(_st: &EntityState) -> String {
    "entity/sheep/sheep_wool_undercoat".to_string()
}

fn wool_texture(st: &EntityState) -> String {
    if st.extras.is_baby {
        "entity/sheep/sheep_wool_baby".to_string()
    } else {
        "entity/sheep/sheep_wool".to_string()
    }
}

fn undercoat_visible(st: &EntityState) -> bool {
    !st.extras.is_baby && (st.extras.magic_name_jeb || st.extras.wool_color != 0)
}
