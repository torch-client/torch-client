use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::models::aquatic::frog;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Frog,
        RenderSpec::new("frog", frog::frog_layer, frog_texture, frog::frog_setup),
    );
    registry.add(
        EntityKind::Tadpole,
        RenderSpec::new(
            "tadpole",
            frog::tadpole_layer,
            tadpole_texture,
            frog::tadpole_setup,
        ),
    );
}

fn frog_texture(st: &EntityState) -> String {
    let variant = match &st.extras.variant {
        Some(id) => id.rsplit(':').next().unwrap_or("temperate"),
        None => "temperate",
    };
    if let Some(entry) = crate::util::variants::frog(variant) {
        return entry.texture(false).to_string();
    }
    match variant {
        "cold" => "entity/frog/frog_cold".to_string(),
        "warm" => "entity/frog/frog_warm".to_string(),
        _ => "entity/frog/frog_temperate".to_string(),
    }
}

fn tadpole_texture(_st: &EntityState) -> String {
    "entity/tadpole/tadpole".to_string()
}
