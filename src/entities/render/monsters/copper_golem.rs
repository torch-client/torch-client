use azalea_registry::builtin::EntityKind;

use crate::entities::TexturePath;
use crate::entities::models::monsters::copper_golem;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::CopperGolem,
        RenderSpec::new(
            "copper_golem",
            copper_golem::layer,
            texture,
            copper_golem::setup_anim,
        ),
    );
    registry.add(
        EntityKind::CopperGolem,
        RenderSpec::new(
            "copper_golem_eyes",
            copper_golem::layer,
            eye_texture,
            copper_golem::setup_anim,
        )
        .with_blend(Blend::Additive),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    match st.extras.weather_state {
        1 => "entity/copper_golem/copper_golem_exposed".into(),
        2 => "entity/copper_golem/copper_golem_weathered".into(),
        3 => "entity/copper_golem/copper_golem_oxidized".into(),
        _ => "entity/copper_golem/copper_golem".into(),
    }
}

fn eye_texture(st: &EntityState) -> TexturePath {
    match st.extras.weather_state {
        1 => "entity/copper_golem/copper_golem_eyes_exposed".into(),
        2 => "entity/copper_golem/copper_golem_eyes_weathered".into(),
        3 => "entity/copper_golem/copper_golem_eyes_oxidized".into(),
        _ => "entity/copper_golem/copper_golem_eyes".into(),
    }
}
