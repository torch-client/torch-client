use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::{cow, quadruped};
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Cow,
        RenderSpec::new("cow", cow::layer, adult_texture, quadruped::setup_anim)
            .with_visible(|st| !st.extras.is_baby && model(st).is_none()),
    );
    registry.add(
        EntityKind::Cow,
        RenderSpec::new(
            "cow_warm",
            cow::warm_layer,
            adult_texture,
            quadruped::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && model(st) == Some("warm")),
    );
    registry.add(
        EntityKind::Cow,
        RenderSpec::new(
            "cow_cold",
            cow::cold_layer,
            adult_texture,
            quadruped::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && model(st) == Some("cold")),
    );
    registry.add(
        EntityKind::Cow,
        RenderSpec::new(
            "cow_baby",
            cow::baby_layer,
            baby_texture,
            quadruped::setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
}

pub fn variant(st: &EntityState) -> &str {
    match &st.extras.variant {
        Some(id) => id.rsplit(':').next().unwrap_or("temperate"),
        None => "temperate",
    }
}

fn model(st: &EntityState) -> Option<&'static str> {
    let name = variant(st);
    match crate::util::variants::cow(name) {
        Some(entry) => match entry.model() {
            Some("warm") => Some("warm"),
            Some("cold") => Some("cold"),
            _ => None,
        },
        None => match name {
            "warm" => Some("warm"),
            "cold" => Some("cold"),
            _ => None,
        },
    }
}

fn adult_texture(st: &EntityState) -> TexturePath {
    let name = variant(st);
    match crate::util::variants::cow(name) {
        Some(entry) => entry.texture(false).into(),
        None => format!("entity/cow/cow_{name}").into(),
    }
}

fn baby_texture(st: &EntityState) -> TexturePath {
    let name = variant(st);
    match crate::util::variants::cow(name) {
        Some(entry) => entry.texture(true).into(),
        None => format!("entity/cow/cow_{name}_baby").into(),
    }
}
