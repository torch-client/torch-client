use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::{cow, quadruped};
use crate::entities::registry::Registry;
use crate::entities::render::add_adult_and_baby;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &[EntityKind::Mooshroom],
        RenderSpec::new(
            "mooshroom",
            cow::layer,
            adult_texture,
            quadruped::setup_anim,
        ),
        RenderSpec::new(
            "mooshroom_baby",
            cow::baby_layer,
            baby_texture,
            quadruped::setup_anim,
        ),
    );
}

fn variant(st: &EntityState) -> &'static str {
    if st.extras.variant_id == 1 {
        "brown"
    } else {
        "red"
    }
}

fn adult_texture(st: &EntityState) -> TexturePath {
    format!("entity/cow/mooshroom_{}", variant(st)).into()
}

fn baby_texture(st: &EntityState) -> TexturePath {
    format!("entity/cow/mooshroom_{}_baby", variant(st)).into()
}
