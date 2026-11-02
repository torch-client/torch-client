use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::aquatic::dolphin;
use crate::entities::registry::Registry;
use crate::entities::render::add_adult_and_baby;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &[EntityKind::Dolphin],
        RenderSpec::new(
            "dolphin",
            dolphin::dolphin_layer,
            adult_texture,
            dolphin::setup_anim,
        ),
        RenderSpec::new(
            "dolphin_baby",
            dolphin::baby_dolphin_layer,
            baby_texture,
            dolphin::setup_anim,
        ),
    );
}

fn adult_texture(_st: &EntityState) -> TexturePath {
    "entity/dolphin/dolphin".into()
}

fn baby_texture(_st: &EntityState) -> TexturePath {
    "entity/dolphin/dolphin_baby".into()
}
