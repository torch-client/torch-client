use azalea_registry::builtin::EntityKind;

use crate::entities::TexturePath;
use crate::entities::models::animals::equine;
use crate::entities::registry::Registry;
use crate::entities::render::humanoid::armor;
use crate::entities::render::{add_adult_and_baby, saddle_spec};
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec};

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &[EntityKind::Horse],
        RenderSpec::new("horse", equine::horse_layer, texture, equine::setup_anim),
        RenderSpec::new(
            "horse_baby",
            equine::baby_horse_layer,
            texture,
            equine::baby_setup_anim,
        ),
    );
    registry.add(
        EntityKind::Horse,
        RenderSpec::new(
            "horse_markings",
            equine::horse_layer,
            markings_texture,
            equine::setup_anim,
        )
        .with_blend(Blend::Translucent)
        .with_visible(|st| !st.extras.is_baby && markings(st) != 0),
    );
    registry.add(
        EntityKind::Horse,
        RenderSpec::new(
            "horse_markings_baby",
            equine::baby_horse_layer,
            markings_texture,
            equine::baby_setup_anim,
        )
        .with_blend(Blend::Translucent)
        .with_visible(|st| st.extras.is_baby && markings(st) != 0),
    );
    registry.add(
        EntityKind::Horse,
        RenderSpec::new(
            "horse_armor",
            equine::horse_armor_layer,
            armor_texture,
            equine::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && wears_armor(st))
        .ignoring_invisibility(),
    );
    registry.add(
        EntityKind::Horse,
        saddle_spec(
            "horse_saddle",
            equine::horse_saddle_layer,
            saddle_texture,
            equine::saddle_setup_anim,
        ),
    );
}

pub fn coat(st: &EntityState) -> usize {
    (st.extras.variant_id & 0xFF) as usize % 7
}

pub fn markings(st: &EntityState) -> u8 {
    ((st.extras.variant_id >> 8) & 0xFF) as u8
}

const COATS: [&str; 7] = [
    "white",
    "creamy",
    "chestnut",
    "brown",
    "black",
    "gray",
    "darkbrown",
];

fn texture(st: &EntityState) -> TexturePath {
    let coat = COATS[coat(st)];
    if st.extras.is_baby {
        format!("entity/horse/horse_{coat}_baby").into()
    } else {
        format!("entity/horse/horse_{coat}").into()
    }
}

fn markings_texture(st: &EntityState) -> TexturePath {
    let name = match markings(st) {
        2 => "whitefield",
        3 => "whitedots",
        4 => "blackdots",
        _ => "white",
    };
    if st.extras.is_baby {
        format!("entity/horse/horse_markings_{name}_baby").into()
    } else {
        format!("entity/horse/horse_markings_{name}").into()
    }
}

pub fn wears_armor(st: &EntityState) -> bool {
    armor::has_layer(st.extras.body_armor.as_deref(), armor::LAYER_HORSE_BODY)
}

pub fn armor_texture(st: &EntityState) -> TexturePath {
    armor::slot_texture(st.extras.body_armor.as_deref(), armor::LAYER_HORSE_BODY).into()
}

fn saddle_texture(_st: &EntityState) -> TexturePath {
    "entity/equipment/horse_saddle/saddle".into()
}
