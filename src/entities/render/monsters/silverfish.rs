use azalea_registry::builtin::EntityKind;

use crate::entities::TexturePath;
use crate::entities::models::monsters::silverfish;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, RootPose, setup_rotations};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Silverfish,
        RenderSpec::new(
            "silverfish",
            silverfish::silverfish_layer,
            silverfish_texture,
            silverfish::silverfish_setup_anim,
        )
        .with_root(root),
    );
    registry.add(
        EntityKind::Endermite,
        RenderSpec::new(
            "endermite",
            silverfish::endermite_layer,
            endermite_texture,
            silverfish::endermite_setup_anim,
        )
        .with_root(root),
    );
}

fn silverfish_texture(_st: &EntityState) -> TexturePath {
    "entity/silverfish/silverfish".into()
}

fn endermite_texture(_st: &EntityState) -> TexturePath {
    "entity/endermite/endermite".into()
}

fn root(st: &EntityState) -> RootPose {
    setup_rotations(st, 180.0)
}
