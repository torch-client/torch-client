use azalea_registry::builtin::EntityKind;

use crate::entities::TexturePath;
use crate::entities::models::monsters::breeze;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Breeze,
        RenderSpec::new("breeze", breeze::layer, texture, breeze::setup_anim)
            .with_blend(Blend::Translucent),
    );
    registry.add(
        EntityKind::Breeze,
        RenderSpec::new(
            "breeze_wind",
            breeze::wind_layer,
            wind_texture,
            breeze::setup_anim,
        )
        .with_blend(Blend::Translucent),
    );
    registry.add(
        EntityKind::Breeze,
        RenderSpec::new(
            "breeze_eyes",
            breeze::eyes_layer,
            eyes_texture,
            breeze::setup_anim,
        )
        .with_blend(Blend::Additive),
    );
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/breeze/breeze".into()
}

fn wind_texture(_st: &EntityState) -> TexturePath {
    "entity/breeze/breeze_wind".into()
}

fn eyes_texture(_st: &EntityState) -> TexturePath {
    "entity/breeze/breeze_eyes".into()
}
