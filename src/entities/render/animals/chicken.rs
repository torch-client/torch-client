use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::chicken;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Chicken,
        RenderSpec::new(
            "chicken",
            chicken::layer,
            adult_texture,
            chicken::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && !is_cold(st)),
    );
    registry.add(
        EntityKind::Chicken,
        RenderSpec::new(
            "chicken_cold",
            chicken::cold_layer,
            adult_texture,
            chicken::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && is_cold(st)),
    );
    registry.add(
        EntityKind::Chicken,
        RenderSpec::new(
            "chicken_baby",
            chicken::baby_layer,
            baby_texture,
            chicken::setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
}

fn variant(st: &EntityState) -> &str {
    match &st.extras.variant {
        Some(id) => id.rsplit(':').next().unwrap_or("temperate"),
        None => "temperate",
    }
}

fn is_cold(st: &EntityState) -> bool {
    let name = variant(st);
    match crate::util::variants::chicken(name) {
        Some(entry) => entry.model() == Some("cold"),
        None => name == "cold",
    }
}

fn adult_texture(st: &EntityState) -> TexturePath {
    let name = variant(st);
    match crate::util::variants::chicken(name) {
        Some(entry) => entry.texture(false).into(),
        None => format!("entity/chicken/chicken_{name}").into(),
    }
}

fn baby_texture(st: &EntityState) -> TexturePath {
    let name = variant(st);
    match crate::util::variants::chicken(name) {
        Some(entry) => entry.texture(true).into(),
        None => format!("entity/chicken/chicken_{name}_baby").into(),
    }
}
