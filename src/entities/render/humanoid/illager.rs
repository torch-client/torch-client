use azalea_registry::builtin::EntityKind;

use crate::entities::models::humanoid::{illager, vex};
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Evoker,
        RenderSpec::new(
            "evoker",
            illager::illager_layer,
            evoker_texture,
            illager::evoker_setup_anim,
        ),
    );
    registry.add(
        EntityKind::Illusioner,
        RenderSpec::new(
            "illusioner",
            illager::illager_layer,
            illusioner_texture,
            illager::illusioner_setup_anim,
        ),
    );
    registry.add(
        EntityKind::Pillager,
        RenderSpec::new(
            "pillager",
            illager::illager_layer,
            pillager_texture,
            illager::pillager_setup_anim,
        ),
    );
    registry.add(
        EntityKind::Vindicator,
        RenderSpec::new(
            "vindicator",
            illager::illager_layer,
            vindicator_texture,
            illager::vindicator_setup_anim,
        ),
    );
    registry.add(
        EntityKind::Vex,
        RenderSpec::new("vex", vex::vex_layer, vex_texture, vex::setup_anim)
            .with_blend(Blend::Translucent),
    );
}

fn evoker_texture(_st: &EntityState) -> String {
    "entity/illager/evoker".to_string()
}

fn illusioner_texture(_st: &EntityState) -> String {
    "entity/illager/illusioner".to_string()
}

fn pillager_texture(_st: &EntityState) -> String {
    "entity/illager/pillager".to_string()
}

fn vindicator_texture(_st: &EntityState) -> String {
    "entity/illager/vindicator".to_string()
}

fn vex_texture(st: &EntityState) -> String {
    if st.extras.charging {
        "entity/illager/vex_charging".to_string()
    } else {
        "entity/illager/vex".to_string()
    }
}
