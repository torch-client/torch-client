use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::models::animals::quadruped;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Pig,
        RenderSpec::new(
            "pig",
            quadruped::pig_layer,
            adult_texture,
            quadruped::setup_anim,
        )
        .with_visible(is_adult_normal),
    );
    registry.add(
        EntityKind::Pig,
        RenderSpec::new(
            "pig_cold",
            quadruped::cold_pig_layer,
            adult_texture,
            quadruped::setup_anim,
        )
        .with_visible(is_adult_cold),
    );
    registry.add(
        EntityKind::Pig,
        RenderSpec::new(
            "pig_baby",
            quadruped::baby_pig_layer,
            baby_texture,
            quadruped::setup_anim,
        )
        .with_visible(is_baby),
    );
    registry.add(
        EntityKind::Pig,
        RenderSpec::new(
            "pig_saddle",
            quadruped::pig_saddle_layer,
            saddle_texture,
            quadruped::setup_anim,
        )
        .with_visible(|st| is_adult(st) && st.extras.saddled),
    );
}

fn is_adult(st: &EntityState) -> bool {
    !st.extras.is_baby
}

fn is_adult_normal(st: &EntityState) -> bool {
    is_adult(st) && !is_cold(st)
}

fn is_adult_cold(st: &EntityState) -> bool {
    is_adult(st) && is_cold(st)
}

fn is_baby(st: &EntityState) -> bool {
    st.extras.is_baby
}

fn is_cold(st: &EntityState) -> bool {
    let name = variant(st);
    match crate::util::variants::pig(name) {
        Some(entry) => entry.model() == Some("cold"),
        None => name == "cold",
    }
}

fn adult_texture(st: &EntityState) -> String {
    let name = variant(st);
    if let Some(entry) = crate::util::variants::pig(name) {
        return entry.texture(false).to_string();
    }
    match name {
        "cold" => "entity/pig/pig_cold".to_string(),
        "warm" => "entity/pig/pig_warm".to_string(),
        _ => "entity/pig/pig_temperate".to_string(),
    }
}

fn saddle_texture(_st: &EntityState) -> String {
    "entity/equipment/pig_saddle/saddle".to_string()
}

fn baby_texture(st: &EntityState) -> String {
    let name = variant(st);
    if let Some(entry) = crate::util::variants::pig(name) {
        return entry.texture(true).to_string();
    }
    match name {
        "cold" => "entity/pig/pig_cold_baby".to_string(),
        "warm" => "entity/pig/pig_warm_baby".to_string(),
        _ => "entity/pig/pig_temperate_baby".to_string(),
    }
}

fn variant(st: &EntityState) -> &str {
    match &st.extras.variant {
        Some(id) => id.rsplit(':').next().unwrap_or("temperate"),
        None => "temperate",
    }
}
