use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::monsters::snow_golem;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::SnowGolem,
        RenderSpec::new(
            "snow_golem",
            snow_golem::layer,
            texture,
            snow_golem::setup_anim,
        ),
    );
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/snow_golem/snow_golem".into()
}
