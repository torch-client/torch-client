use core::{
    hash::{Hash, Hasher},
    ops::Range,
};

use bevy_app::{App, Plugin, PostUpdate};
use bevy_ecs::{
    component::Component,
    entity::{Entity, EntityHashMap},
    query::With,
    reflect::ReflectComponent,
    resource::Resource,
    schedule::IntoScheduleConfigs as _,
    system::{Local, Query, ResMut},
};
use bevy_math::FloatOrd;
use bevy_reflect::Reflect;
use bevy_transform::components::GlobalTransform;
use bevy_utils::Parallel;

use super::{check_visibility, VisibilitySystems};
use crate::{camera::Camera, primitives::Aabb};

pub struct VisibilityRangePlugin;

impl Plugin for VisibilityRangePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VisibleEntityRanges>().add_systems(
            PostUpdate,
            check_visibility_ranges
                .in_set(VisibilitySystems::CheckVisibility)
                .before(check_visibility),
        );
    }
}

#[derive(Component, Clone, PartialEq, Default, Reflect)]
#[reflect(Component, PartialEq, Hash, Clone)]
pub struct VisibilityRange {
    pub start_margin: Range<f32>,

    pub end_margin: Range<f32>,

    pub use_aabb: bool,
}

impl Eq for VisibilityRange {}

impl Hash for VisibilityRange {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        FloatOrd(self.start_margin.start).hash(state);
        FloatOrd(self.start_margin.end).hash(state);
        FloatOrd(self.end_margin.start).hash(state);
        FloatOrd(self.end_margin.end).hash(state);
    }
}

impl VisibilityRange {
    #[inline]
    pub fn abrupt(start: f32, end: f32) -> Self {
        Self {
            start_margin: start..start,
            end_margin: end..end,
            use_aabb: false,
        }
    }

    #[inline]
    pub fn is_abrupt(&self) -> bool {
        self.start_margin.start == self.start_margin.end
            && self.end_margin.start == self.end_margin.end
    }

    #[inline]
    pub fn is_visible_at_all(&self, camera_distance: f32) -> bool {
        camera_distance >= self.start_margin.start && camera_distance < self.end_margin.end
    }

    #[inline]
    pub fn is_culled(&self, camera_distance: f32) -> bool {
        !self.is_visible_at_all(camera_distance)
    }
}

#[derive(Resource, Default)]
pub struct VisibleEntityRanges {
    views: EntityHashMap<u8>,

    entities: EntityHashMap<u32>,
}

impl VisibleEntityRanges {
    fn clear(&mut self) {
        self.views.clear();
        self.entities.clear();
    }

    #[inline]
    pub fn entity_is_in_range_of_view(&self, entity: Entity, view: Entity) -> bool {
        let Some(visibility_bitmask) = self.entities.get(&entity) else {
            return false;
        };
        let Some(view_index) = self.views.get(&view) else {
            return false;
        };
        (visibility_bitmask & (1 << view_index)) != 0
    }

    #[inline]
    pub fn entity_is_in_range_of_any_view(&self, entity: Entity) -> bool {
        self.entities.contains_key(&entity)
    }
}

pub fn check_visibility_ranges(
    mut visible_entity_ranges: ResMut<VisibleEntityRanges>,
    view_query: Query<(Entity, &GlobalTransform), With<Camera>>,
    mut par_local: Local<Parallel<Vec<(Entity, u32)>>>,
    entity_query: Query<(Entity, &GlobalTransform, Option<&Aabb>, &VisibilityRange)>,
) {
    visible_entity_ranges.clear();

    if entity_query.is_empty() {
        return;
    }

    let mut views = vec![];
    for (view, view_transform) in view_query.iter().take(32) {
        let view_index = views.len() as u8;
        visible_entity_ranges.views.insert(view, view_index);
        views.push((view, view_transform.translation_vec3a()));
    }

    entity_query.par_iter().for_each(
        |(entity, entity_transform, maybe_model_aabb, visibility_range)| {
            let mut visibility = 0;
            for (view_index, &(_, view_position)) in views.iter().enumerate() {
                let model_position = match (visibility_range.use_aabb, maybe_model_aabb) {
                    (true, Some(model_aabb)) => entity_transform
                        .affine()
                        .transform_point3a(model_aabb.center),
                    _ => entity_transform.translation_vec3a(),
                };

                if visibility_range.is_visible_at_all((view_position - model_position).length()) {
                    visibility |= 1 << view_index;
                }
            }

            if visibility != 0 {
                par_local.borrow_local_mut().push((entity, visibility));
            }
        },
    );

    visible_entity_ranges.entities.extend(par_local.drain());
}
