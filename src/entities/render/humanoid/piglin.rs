use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::humanoid::piglin;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

use super::armor;
use super::zombie::{is_adult, is_baby};

pub fn register(registry: &mut Registry) {
    registry.add_many(
        &[EntityKind::Piglin, EntityKind::PiglinBrute],
        RenderSpec::new(
            "piglin",
            piglin::adult_piglin_layer,
            piglin_texture,
            piglin::piglin_setup_anim,
        )
        .with_visible(is_adult),
    );
    registry.add_many(
        &[EntityKind::Piglin, EntityKind::PiglinBrute],
        RenderSpec::new(
            "piglin_baby",
            piglin::baby_piglin_layer,
            piglin_texture,
            piglin::piglin_setup_anim,
        )
        .with_visible(is_baby),
    );
    armor::register_set(
        registry,
        &[EntityKind::Piglin, EntityKind::PiglinBrute],
        [
            "piglin_helmet",
            "piglin_chestplate",
            "piglin_leggings",
            "piglin_boots",
        ],
        armor::PIGLIN,
        piglin::piglin_setup_anim,
        armor::ADULT,
    );

    registry.add(
        EntityKind::ZombifiedPiglin,
        RenderSpec::new(
            "zombified_piglin",
            piglin::adult_piglin_layer,
            zombified_piglin_texture,
            piglin::zombified_piglin_setup_anim,
        )
        .with_visible(is_adult),
    );
    registry.add(
        EntityKind::ZombifiedPiglin,
        RenderSpec::new(
            "zombified_piglin_baby",
            piglin::baby_piglin_layer,
            zombified_piglin_texture,
            piglin::zombified_piglin_setup_anim,
        )
        .with_visible(is_baby),
    );
    armor::register_set(
        registry,
        &[EntityKind::ZombifiedPiglin],
        [
            "zombified_piglin_helmet",
            "zombified_piglin_chestplate",
            "zombified_piglin_leggings",
            "zombified_piglin_boots",
        ],
        armor::PIGLIN,
        piglin::zombified_piglin_setup_anim,
        armor::ADULT,
    );
}

fn piglin_texture(st: &EntityState) -> TexturePath {
    if st.kind == EntityKind::PiglinBrute {
        "entity/piglin/piglin_brute".into()
    } else if st.extras.is_baby {
        "entity/piglin/piglin_baby".into()
    } else {
        "entity/piglin/piglin".into()
    }
}

fn zombified_piglin_texture(st: &EntityState) -> TexturePath {
    if st.extras.is_baby {
        "entity/piglin/zombified_piglin_baby".into()
    } else {
        "entity/piglin/zombified_piglin".into()
    }
}
