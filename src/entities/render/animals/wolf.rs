use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::models::animals::wolf;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

use super::dye;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Wolf,
        RenderSpec::new("wolf", wolf::layer, texture, wolf::setup_anim)
            .with_visible(|st| !st.extras.is_baby)
            .with_tint(wet_shade),
    );
    registry.add(
        EntityKind::Wolf,
        RenderSpec::new(
            "wolf_baby",
            wolf::baby_layer,
            texture,
            wolf::baby_setup_anim,
        )
        .with_visible(|st| st.extras.is_baby)
        .with_tint(wet_shade),
    );
    registry.add(
        EntityKind::Wolf,
        RenderSpec::new(
            "wolf_armor",
            wolf::armor_layer,
            armor_texture,
            wolf::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && st.extras.body_armor.is_some()),
    );
    registry.add(
        EntityKind::Wolf,
        RenderSpec::new("wolf_collar", wolf::layer, collar_texture, wolf::setup_anim)
            .with_visible(|st| st.extras.tame && !st.extras.is_baby)
            .with_tint(|st| dye::dye_tint(st.extras.collar_color)),
    );
    registry.add(
        EntityKind::Wolf,
        RenderSpec::new(
            "wolf_collar_baby",
            wolf::baby_layer,
            collar_texture,
            wolf::baby_setup_anim,
        )
        .with_visible(|st| st.extras.tame && st.extras.is_baby)
        .with_tint(|st| dye::dye_tint(st.extras.collar_color)),
    );
}

fn wet_shade(st: &EntityState) -> [f32; 4] {
    let shade = st.extras.wet_shade;
    [shade, shade, shade, 1.0]
}

fn texture(st: &EntityState) -> String {
    let variant = match &st.extras.variant {
        Some(id) => id.rsplit(':').next().unwrap_or("pale"),
        None => "pale",
    };
    let angry = st.extras.anger_ticks > 0;
    if let Some(entry) = crate::util::variants::wolf(variant) {
        return entry
            .texture(st.extras.tame, angry, st.extras.is_baby)
            .to_string();
    }
    let base = if variant == "pale" {
        "wolf".to_string()
    } else {
        format!("wolf_{variant}")
    };
    let state = if st.extras.tame {
        "_tame"
    } else if angry {
        "_angry"
    } else {
        ""
    };
    let baby = if st.extras.is_baby { "_baby" } else { "" };
    format!("entity/wolf/{base}{state}{baby}")
}

fn armor_texture(_st: &EntityState) -> String {
    "entity/equipment/wolf_body/armadillo_scute".to_string()
}

fn collar_texture(st: &EntityState) -> String {
    if st.extras.is_baby {
        "entity/wolf/wolf_collar_baby".to_string()
    } else {
        "entity/wolf/wolf_collar".to_string()
    }
}
