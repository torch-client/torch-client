use azalea_registry::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::geom::{BakedModel, PartState};
use crate::entities::models::humanoid::elytra as model;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

use super::armor::{self, LAYER_WINGS};

pub fn register(registry: &mut Registry, kinds: &[EntityKind]) {
    registry.add_many(
        kinds,
        RenderSpec::new("elytra", model::layer, texture, setup).with_visible(visible),
    );
    registry.add_many(
        kinds,
        RenderSpec::new("elytra_baby", model::baby_layer, texture, setup)
            .with_visible(visible_baby),
    );
}

fn texture(st: &EntityState) -> String {
    armor::slot_texture(st.extras.chestplate, LAYER_WINGS)
}

fn wearing(st: &EntityState) -> bool {
    armor::has_layer(st.extras.chestplate, LAYER_WINGS)
}

fn visible(st: &EntityState) -> bool {
    wearing(st) && !st.extras.is_baby
}

fn visible_baby(st: &EntityState) -> bool {
    wearing(st) && st.extras.is_baby
}

fn setup(model_def: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let (left, right) = model::wing_angles(st.extras.elytra, st.extras.crouching);
    for (name, [y, x_rot, y_rot, z_rot]) in [("left_wing", left), ("right_wing", right)] {
        let Some(id) = model_def.find(name) else {
            continue;
        };
        let part = &mut parts[id];
        part.y = y;
        part.x_rot = x_rot;
        part.y_rot = y_rot;
        part.z_rot = z_rot;
    }
}
