use azalea_registry::builtin::EntityKind;

use crate::entities::models::animals::equine;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Horse,
        RenderSpec::new("horse", equine::horse_layer, texture, equine::setup_anim)
            .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Horse,
        RenderSpec::new(
            "horse_baby",
            equine::baby_horse_layer,
            texture,
            equine::baby_setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
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
        .with_visible(|st| !st.extras.is_baby && armor_material(st).is_some()),
    );
    registry.add(
        EntityKind::Horse,
        RenderSpec::new(
            "horse_saddle",
            equine::horse_saddle_layer,
            saddle_texture,
            equine::saddle_setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && st.extras.saddled),
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

fn texture(st: &EntityState) -> String {
    let coat = COATS[coat(st)];
    if st.extras.is_baby {
        format!("entity/horse/horse_{coat}_baby")
    } else {
        format!("entity/horse/horse_{coat}")
    }
}

fn markings_texture(st: &EntityState) -> String {
    let name = match markings(st) {
        2 => "whitefield",
        3 => "whitedots",
        4 => "blackdots",
        _ => "white",
    };
    if st.extras.is_baby {
        format!("entity/horse/horse_markings_{name}_baby")
    } else {
        format!("entity/horse/horse_markings_{name}")
    }
}

pub fn armor_material(st: &EntityState) -> Option<&'static str> {
    let item = st.extras.body_armor.as_ref()?;
    let id = item.rsplit(':').next()?;
    let material = id.strip_suffix("_horse_armor")?;
    ["copper", "diamond", "gold", "iron", "leather", "netherite"]
        .into_iter()
        .find(|known| *known == material)
}

fn armor_texture(st: &EntityState) -> String {
    let material = armor_material(st).unwrap_or("iron");
    format!("entity/equipment/horse_body/{material}")
}

fn saddle_texture(_st: &EntityState) -> String {
    "entity/equipment/horse_saddle/saddle".to_string()
}
