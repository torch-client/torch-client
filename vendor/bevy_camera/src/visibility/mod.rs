mod range;
mod render_layers;

use core::any::TypeId;

use bevy_ecs::entity::EntityHashMap;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::world::DeferredWorld;
use derive_more::derive::{Deref, DerefMut};
pub use range::*;
pub use render_layers::*;

use bevy_app::{Plugin, PostUpdate};
use bevy_asset::prelude::AssetChanged;
use bevy_asset::{AssetEventSystems, Assets};
use bevy_ecs::{hierarchy::validate_parent_has_component, prelude::*};
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_transform::{components::GlobalTransform, TransformSystems};
use bevy_utils::{Parallel, TypeIdMap};
use smallvec::SmallVec;

use crate::{
    camera::Camera,
    primitives::{Aabb, Frustum, MeshAabb, Sphere},
    Projection,
};
use bevy_mesh::{mark_3d_meshes_as_changed_if_their_assets_changed, Mesh, Mesh2d, Mesh3d};

#[derive(Component, Default)]
pub struct NoCpuCulling;

#[derive(Component, Clone, Copy, Reflect, Debug, PartialEq, Eq, Default)]
#[reflect(Component, Default, Debug, PartialEq, Clone)]
#[require(InheritedVisibility, ViewVisibility)]
pub enum Visibility {
    #[default]
    Inherited,
    Hidden,
    Visible,
}

impl Visibility {
    #[inline]
    pub fn toggle_inherited_visible(&mut self) {
        *self = match *self {
            Visibility::Inherited => Visibility::Visible,
            Visibility::Visible => Visibility::Inherited,
            _ => *self,
        };
    }
    #[inline]
    pub fn toggle_inherited_hidden(&mut self) {
        *self = match *self {
            Visibility::Inherited => Visibility::Hidden,
            Visibility::Hidden => Visibility::Inherited,
            _ => *self,
        };
    }
    #[inline]
    pub fn toggle_visible_hidden(&mut self) {
        *self = match *self {
            Visibility::Visible => Visibility::Hidden,
            Visibility::Hidden => Visibility::Visible,
            _ => *self,
        };
    }
}

impl PartialEq<Visibility> for &Visibility {
    #[inline]
    fn eq(&self, other: &Visibility) -> bool {
        <Visibility as PartialEq<Visibility>>::eq(*self, other)
    }
}

impl PartialEq<&Visibility> for Visibility {
    #[inline]
    fn eq(&self, other: &&Visibility) -> bool {
        <Visibility as PartialEq<Visibility>>::eq(self, *other)
    }
}

#[derive(Component, Deref, Debug, Default, Clone, Copy, Reflect, PartialEq, Eq)]
#[reflect(Component, Default, Debug, PartialEq, Clone)]
#[component(on_insert = validate_parent_has_component::<Self>)]
pub struct InheritedVisibility(bool);

impl InheritedVisibility {
    pub const HIDDEN: Self = Self(false);
    pub const VISIBLE: Self = Self(true);

    #[inline]
    pub fn get(self) -> bool {
        self.0
    }
}

#[derive(Clone, Component, Default, Reflect, Deref, DerefMut)]
#[reflect(Component, Default, Clone)]
#[component(clone_behavior=Ignore)]
pub struct VisibilityClass(pub SmallVec<[TypeId; 1]>);

#[derive(Component, Debug, Default, Clone, Copy, Reflect, PartialEq, Eq)]
#[reflect(Component, Default, Debug, PartialEq, Clone)]
pub struct ViewVisibility(
    u8,
);

impl ViewVisibility {
    pub const HIDDEN: Self = Self(0);

    #[inline]
    pub fn get(self) -> bool {
        self.0 & 1 != 0
    }

    #[inline]
    fn was_visible_now_hidden(self) -> bool {
        (self.0 & 0b11) == 0b10
    }

    #[inline]
    fn update(&mut self) {
        self.0 = (self.0 & 1) << 1;
    }
}

pub trait SetViewVisibility {
    fn set_visible(&mut self);
}

impl<'a> SetViewVisibility for Mut<'a, ViewVisibility> {
    #[inline]
    fn set_visible(&mut self) {
        if self.0 & 1 == 0 {
            if self.0 & 2 != 0 {
                self.bypass_change_detection().0 |= 1;
            } else {
                self.0 |= 1;
            }
        }
    }
}

#[derive(Debug, Component, Default, Reflect)]
#[reflect(Component, Default, Debug)]
pub struct NoFrustumCulling;

#[derive(Clone, Component, Default, Debug, Reflect)]
#[reflect(Component, Default, Debug, Clone)]
pub struct VisibleEntities {
    #[reflect(ignore, clone)]
    pub entities: TypeIdMap<Vec<Entity>>,
}

impl VisibleEntities {
    pub fn get(&self, type_id: TypeId) -> &[Entity] {
        match self.entities.get(&type_id) {
            Some(entities) => &entities[..],
            None => &[],
        }
    }

    pub fn get_mut(&mut self, type_id: TypeId) -> &mut Vec<Entity> {
        self.entities.entry(type_id).or_default()
    }

    pub fn iter(&self, type_id: TypeId) -> impl DoubleEndedIterator<Item = &Entity> {
        self.get(type_id).iter()
    }

    pub fn len(&self, type_id: TypeId) -> usize {
        self.get(type_id).len()
    }

    pub fn is_empty(&self, type_id: TypeId) -> bool {
        self.get(type_id).is_empty()
    }

    pub fn clear(&mut self, type_id: TypeId) {
        self.get_mut(type_id).clear();
    }

    pub fn clear_all(&mut self) {
        for entities in self.entities.values_mut() {
            entities.clear();
        }
    }

    pub fn push(&mut self, entity: Entity, type_id: TypeId) {
        self.get_mut(type_id).push(entity);
    }
}

#[derive(Component, Clone, Debug, Default, Reflect, Deref, DerefMut)]
#[reflect(Component, Debug, Default, Clone)]
pub struct VisibleMeshEntities {
    #[reflect(ignore, clone)]
    pub entities: Vec<Entity>,
}

#[derive(Component, Clone, Debug, Default, Reflect)]
#[reflect(Component, Debug, Default, Clone)]
pub struct CubemapVisibleEntities {
    #[reflect(ignore, clone)]
    data: [VisibleMeshEntities; 6],
}

impl CubemapVisibleEntities {
    pub fn get(&self, i: usize) -> &VisibleMeshEntities {
        &self.data[i]
    }

    pub fn get_mut(&mut self, i: usize) -> &mut VisibleMeshEntities {
        &mut self.data[i]
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &VisibleMeshEntities> {
        self.data.iter()
    }

    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut VisibleMeshEntities> {
        self.data.iter_mut()
    }
}

#[derive(Component, Clone, Debug, Default, Reflect)]
#[reflect(Component, Default, Clone)]
pub struct CascadesVisibleEntities {
    #[reflect(ignore, clone)]
    pub entities: EntityHashMap<Vec<VisibleMeshEntities>>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum VisibilitySystems {
    CalculateBounds,
    UpdateFrusta,
    VisibilityPropagate,
    CheckVisibility,
    MarkNewlyHiddenEntitiesInvisible,
}

pub struct VisibilityPlugin;

impl Plugin for VisibilityPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        use VisibilitySystems::*;

        app.register_required_components::<Mesh3d, Visibility>()
            .register_required_components::<Mesh3d, VisibilityClass>()
            .register_required_components::<Mesh2d, Visibility>()
            .register_required_components::<Mesh2d, VisibilityClass>()
            .configure_sets(
                PostUpdate,
                (UpdateFrusta, VisibilityPropagate)
                    .before(CheckVisibility)
                    .after(TransformSystems::Propagate),
            )
            .configure_sets(
                PostUpdate,
                MarkNewlyHiddenEntitiesInvisible.after(CheckVisibility),
            )
            .configure_sets(
                PostUpdate,
                (CalculateBounds)
                    .before(CheckVisibility)
                    .after(TransformSystems::Propagate)
                    .after(AssetEventSystems)
                    .ambiguous_with(CalculateBounds)
                    .ambiguous_with(mark_3d_meshes_as_changed_if_their_assets_changed),
            )
            .add_systems(
                PostUpdate,
                (
                    calculate_bounds.in_set(CalculateBounds),
                    (visibility_propagate_system, reset_view_visibility)
                        .in_set(VisibilityPropagate),
                    check_visibility.in_set(CheckVisibility),
                    mark_newly_hidden_entities_invisible.in_set(MarkNewlyHiddenEntitiesInvisible),
                ),
            );
        app.world_mut()
            .register_component_hooks::<Mesh3d>()
            .on_add(add_visibility_class::<Mesh3d>);
        app.world_mut()
            .register_component_hooks::<Mesh2d>()
            .on_add(add_visibility_class::<Mesh2d>);
    }
}

#[derive(Component, Clone, Debug, Default, Reflect)]
pub struct NoAutoAabb;

pub fn calculate_bounds(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    new_aabb: Query<
        (Entity, &Mesh3d),
        (
            Without<Aabb>,
            Without<NoFrustumCulling>,
            Without<NoAutoAabb>,
        ),
    >,
    mut update_aabb: Query<
        (&Mesh3d, &mut Aabb),
        (
            Or<(AssetChanged<Mesh3d>, Changed<Mesh3d>)>,
            Without<NoFrustumCulling>,
            Without<NoAutoAabb>,
        ),
    >,
) {
    for (entity, mesh_handle) in &new_aabb {
        if let Some(mesh) = meshes.get(mesh_handle)
            && let Some(aabb) = mesh.compute_aabb()
        {
            commands.entity(entity).try_insert(aabb);
        }
    }

    update_aabb
        .par_iter_mut()
        .for_each(|(mesh_handle, mut old_aabb)| {
            if let Some(aabb) = meshes.get(mesh_handle).and_then(MeshAabb::compute_aabb) {
                *old_aabb = aabb;
            }
        });
}

pub fn update_frusta(
    mut views: Query<
        (&GlobalTransform, &Projection, &mut Frustum),
        Or<(Changed<GlobalTransform>, Changed<Projection>)>,
    >,
) {
    for (transform, projection, mut frustum) in &mut views {
        *frustum = projection.compute_frustum(transform);
    }
}

fn visibility_propagate_system(
    changed: Query<
        (Entity, &Visibility, Option<&ChildOf>, Option<&Children>),
        (
            With<InheritedVisibility>,
            Or<(Changed<Visibility>, Changed<ChildOf>)>,
        ),
    >,
    mut visibility_query: Query<(&Visibility, &mut InheritedVisibility)>,
    children_query: Query<&Children, (With<Visibility>, With<InheritedVisibility>)>,
) {
    for (entity, visibility, child_of, children) in &changed {
        let is_visible = match visibility {
            Visibility::Visible => true,
            Visibility::Hidden => false,
            Visibility::Inherited => child_of
                .and_then(|c| visibility_query.get(c.parent()).ok())
                .is_none_or(|(_, x)| x.get()),
        };
        let (_, mut inherited_visibility) = visibility_query
            .get_mut(entity)
            .expect("With<InheritedVisibility> ensures this query will return a value");

        if inherited_visibility.get() != is_visible {
            inherited_visibility.0 = is_visible;

            for &child in children.into_iter().flatten() {
                let _ =
                    propagate_recursive(is_visible, child, &mut visibility_query, &children_query);
            }
        }
    }
}

fn propagate_recursive(
    parent_is_visible: bool,
    entity: Entity,
    visibility_query: &mut Query<(&Visibility, &mut InheritedVisibility)>,
    children_query: &Query<&Children, (With<Visibility>, With<InheritedVisibility>)>,
) -> Result<(), ()> {
    let (visibility, mut inherited_visibility) = visibility_query.get_mut(entity).map_err(drop)?;

    let is_visible = match visibility {
        Visibility::Visible => true,
        Visibility::Hidden => false,
        Visibility::Inherited => parent_is_visible,
    };

    if inherited_visibility.get() != is_visible {
        inherited_visibility.0 = is_visible;

        for &child in children_query.get(entity).ok().into_iter().flatten() {
            let _ = propagate_recursive(is_visible, child, visibility_query, children_query);
        }
    }

    Ok(())
}

fn reset_view_visibility(mut query: Query<&mut ViewVisibility>) {
    query.par_iter_mut().for_each(|mut view_visibility| {
        view_visibility.bypass_change_detection().update();
    });
}

pub fn check_visibility(
    mut thread_queues: Local<Parallel<TypeIdMap<Vec<Entity>>>>,
    mut view_query: Query<(
        Entity,
        &mut VisibleEntities,
        &Frustum,
        Option<&RenderLayers>,
        &Camera,
        Has<NoCpuCulling>,
    )>,
    mut unlayered_query: Query<(
        Entity,
        &InheritedVisibility,
        &mut ViewVisibility,
        Option<&VisibilityClass>,
        Option<&RenderLayers>,
        Option<&Aabb>,
        &GlobalTransform,
        Has<NoFrustumCulling>,
        Has<VisibilityRange>,
    ), Without<RenderLayers>>,
    mut layered_query: Query<(
        Entity,
        &InheritedVisibility,
        &mut ViewVisibility,
        Option<&VisibilityClass>,
        Option<&RenderLayers>,
        Option<&Aabb>,
        &GlobalTransform,
        Has<NoFrustumCulling>,
        Has<VisibilityRange>,
    ), With<RenderLayers>>,
    visible_entity_ranges: Option<Res<VisibleEntityRanges>>,
) {
    let visible_entity_ranges = visible_entity_ranges.as_deref();

    for (view, mut visible_entities, frustum, maybe_view_mask, camera, no_cpu_culling) in
        &mut view_query
    {
        if !camera.is_active {
            continue;
        }

        let view_mask = maybe_view_mask.unwrap_or_default();

        macro_rules! sweep {
            ($query:expr) => {
        $query.par_iter_mut().for_each_init(
            || thread_queues.borrow_local_mut(),
            |queue, query_item| {
                let (
                    entity,
                    inherited_visibility,
                    mut view_visibility,
                    visibility_class,
                    maybe_entity_mask,
                    maybe_model_aabb,
                    transform,
                    no_frustum_culling,
                    has_visibility_range,
                ) = query_item;

                if !inherited_visibility.get() {
                    return;
                }

                let entity_mask = maybe_entity_mask.unwrap_or_default();
                if !view_mask.intersects(entity_mask) {
                    return;
                }

                if has_visibility_range
                    && visible_entity_ranges.is_some_and(|visible_entity_ranges| {
                        !visible_entity_ranges.entity_is_in_range_of_view(entity, view)
                    })
                {
                    return;
                }

                if !no_frustum_culling
                    && !no_cpu_culling
                    && let Some(model_aabb) = maybe_model_aabb
                {
                    let world_from_local = transform.affine();
                    let model_sphere = Sphere {
                        center: world_from_local.transform_point3a(model_aabb.center),
                        radius: transform.radius_vec3a(model_aabb.half_extents),
                    };
                    if !frustum.intersects_sphere(&model_sphere, false) {
                        return;
                    }
                    if !frustum.intersects_obb(model_aabb, &world_from_local, true, false) {
                        return;
                    }
                }

                view_visibility.set_visible();

                if let Some(visibility_class) = visibility_class {
                    for visibility_class_id in visibility_class.iter() {
                        queue.entry(*visibility_class_id).or_default().push(entity);
                    }
                }
            },
        );
            };
        }

        if view_mask.intersects(&RenderLayers::default()) {
            sweep!(unlayered_query);
        }
        sweep!(layered_query);

        visible_entities.clear_all();

        for class_queues in thread_queues.iter_mut() {
            for (class, entities) in class_queues {
                visible_entities.get_mut(*class).append(entities);
            }
        }
    }
}

fn mark_newly_hidden_entities_invisible(mut view_visibilities: Query<&mut ViewVisibility>) {
    view_visibilities
        .par_iter_mut()
        .for_each(|mut view_visibility| {
            if view_visibility.as_ref().was_visible_now_hidden() {
                *view_visibility = ViewVisibility::HIDDEN;
            }
        });
}

pub fn add_visibility_class<C>(
    mut world: DeferredWorld<'_>,
    HookContext { entity, .. }: HookContext,
) where
    C: 'static,
{
    if let Some(mut visibility_class) = world.get_mut::<VisibilityClass>(entity) {
        visibility_class.push(TypeId::of::<C>());
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bevy_app::prelude::*;

    #[test]
    fn visibility_propagation() {
        let mut app = App::new();
        app.add_systems(Update, visibility_propagate_system);

        let root1 = app.world_mut().spawn(Visibility::Hidden).id();
        let root1_child1 = app.world_mut().spawn(Visibility::default()).id();
        let root1_child2 = app.world_mut().spawn(Visibility::Hidden).id();
        let root1_child1_grandchild1 = app.world_mut().spawn(Visibility::default()).id();
        let root1_child2_grandchild1 = app.world_mut().spawn(Visibility::default()).id();

        app.world_mut()
            .entity_mut(root1)
            .add_children(&[root1_child1, root1_child2]);
        app.world_mut()
            .entity_mut(root1_child1)
            .add_children(&[root1_child1_grandchild1]);
        app.world_mut()
            .entity_mut(root1_child2)
            .add_children(&[root1_child2_grandchild1]);

        let root2 = app.world_mut().spawn(Visibility::default()).id();
        let root2_child1 = app.world_mut().spawn(Visibility::default()).id();
        let root2_child2 = app.world_mut().spawn(Visibility::Hidden).id();
        let root2_child1_grandchild1 = app.world_mut().spawn(Visibility::default()).id();
        let root2_child2_grandchild1 = app.world_mut().spawn(Visibility::default()).id();

        app.world_mut()
            .entity_mut(root2)
            .add_children(&[root2_child1, root2_child2]);
        app.world_mut()
            .entity_mut(root2_child1)
            .add_children(&[root2_child1_grandchild1]);
        app.world_mut()
            .entity_mut(root2_child2)
            .add_children(&[root2_child2_grandchild1]);

        app.update();

        let is_visible = |e: Entity| {
            app.world()
                .entity(e)
                .get::<InheritedVisibility>()
                .unwrap()
                .get()
        };
        assert!(
            !is_visible(root1),
            "invisibility propagates down tree from root"
        );
        assert!(
            !is_visible(root1_child1),
            "invisibility propagates down tree from root"
        );
        assert!(
            !is_visible(root1_child2),
            "invisibility propagates down tree from root"
        );
        assert!(
            !is_visible(root1_child1_grandchild1),
            "invisibility propagates down tree from root"
        );
        assert!(
            !is_visible(root1_child2_grandchild1),
            "invisibility propagates down tree from root"
        );

        assert!(
            is_visible(root2),
            "visibility propagates down tree from root"
        );
        assert!(
            is_visible(root2_child1),
            "visibility propagates down tree from root"
        );
        assert!(
            !is_visible(root2_child2),
            "visibility propagates down tree from root, but local invisibility is preserved"
        );
        assert!(
            is_visible(root2_child1_grandchild1),
            "visibility propagates down tree from root"
        );
        assert!(
            !is_visible(root2_child2_grandchild1),
            "child's invisibility propagates down to grandchild"
        );
    }

    #[test]
    fn test_visibility_propagation_on_parent_change() {
        let mut app = App::new();

        app.add_systems(Update, visibility_propagate_system);

        let parent1 = app.world_mut().spawn((Visibility::Hidden,)).id();
        let parent2 = app.world_mut().spawn((Visibility::Visible,)).id();
        let child1 = app.world_mut().spawn((Visibility::Inherited,)).id();
        let child2 = app.world_mut().spawn((Visibility::Inherited,)).id();

        app.world_mut()
            .entity_mut(parent1)
            .add_children(&[child1, child2]);

        app.update();

        app.world_mut()
            .entity_mut(parent2)
            .insert(Visibility::Visible);
        app.world_mut().entity_mut(child2).insert(ChildOf(parent2));

        app.update();

        let is_visible = |e: Entity| {
            app.world()
                .entity(e)
                .get::<InheritedVisibility>()
                .unwrap()
                .get()
        };

        assert!(
            !is_visible(child1),
            "Child1 should inherit visibility from parent"
        );

        assert!(
            is_visible(child2),
            "Child2 should inherit visibility from parent"
        );
    }

    #[test]
    fn visibility_propagation_unconditional_visible() {
        use Visibility::{Hidden, Inherited, Visible};

        let mut app = App::new();
        app.add_systems(Update, visibility_propagate_system);

        let root1 = app.world_mut().spawn(Visible).id();
        let root1_child1 = app.world_mut().spawn(Inherited).id();
        let root1_child2 = app.world_mut().spawn(Hidden).id();
        let root1_child1_grandchild1 = app.world_mut().spawn(Visible).id();
        let root1_child2_grandchild1 = app.world_mut().spawn(Visible).id();

        let root2 = app.world_mut().spawn(Inherited).id();
        let root3 = app.world_mut().spawn(Hidden).id();

        app.world_mut()
            .entity_mut(root1)
            .add_children(&[root1_child1, root1_child2]);
        app.world_mut()
            .entity_mut(root1_child1)
            .add_children(&[root1_child1_grandchild1]);
        app.world_mut()
            .entity_mut(root1_child2)
            .add_children(&[root1_child2_grandchild1]);

        app.update();

        let is_visible = |e: Entity| {
            app.world()
                .entity(e)
                .get::<InheritedVisibility>()
                .unwrap()
                .get()
        };
        assert!(
            is_visible(root1),
            "an unconditionally visible root is visible"
        );
        assert!(
            is_visible(root1_child1),
            "an inheriting child of an unconditionally visible parent is visible"
        );
        assert!(
            !is_visible(root1_child2),
            "a hidden child on an unconditionally visible parent is hidden"
        );
        assert!(
            is_visible(root1_child1_grandchild1),
            "an unconditionally visible child of an inheriting parent is visible"
        );
        assert!(
            is_visible(root1_child2_grandchild1),
            "an unconditionally visible child of a hidden parent is visible"
        );
        assert!(is_visible(root2), "an inheriting root is visible");
        assert!(!is_visible(root3), "a hidden root is hidden");
    }

    #[test]
    fn visibility_propagation_change_detection() {
        let mut world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(visibility_propagate_system);

        let id1 = world.spawn(Visibility::default()).id();

        let id2 = world.spawn(Visibility::default()).id();
        world.entity_mut(id1).add_children(&[id2]);

        let id3 = world.spawn(Visibility::Hidden).id();
        world.entity_mut(id2).add_children(&[id3]);

        let id4 = world.spawn(Visibility::default()).id();
        world.entity_mut(id3).add_children(&[id4]);

        schedule.run(&mut world);
        world.clear_trackers();

        let mut q = world.query::<Ref<InheritedVisibility>>();

        assert!(!q.get(&world, id1).unwrap().is_changed());
        assert!(!q.get(&world, id2).unwrap().is_changed());
        assert!(!q.get(&world, id3).unwrap().is_changed());
        assert!(!q.get(&world, id4).unwrap().is_changed());

        world.clear_trackers();
        world.entity_mut(id1).insert(Visibility::Hidden);
        schedule.run(&mut world);

        assert!(q.get(&world, id1).unwrap().is_changed());
        assert!(q.get(&world, id2).unwrap().is_changed());
        assert!(!q.get(&world, id3).unwrap().is_changed());
        assert!(!q.get(&world, id4).unwrap().is_changed());

        world.clear_trackers();
        schedule.run(&mut world);

        assert!(!q.get(&world, id1).unwrap().is_changed());
        assert!(!q.get(&world, id2).unwrap().is_changed());
        assert!(!q.get(&world, id3).unwrap().is_changed());
        assert!(!q.get(&world, id4).unwrap().is_changed());

        world.clear_trackers();
        world.entity_mut(id3).insert(Visibility::Inherited);
        schedule.run(&mut world);

        assert!(!q.get(&world, id1).unwrap().is_changed());
        assert!(!q.get(&world, id2).unwrap().is_changed());
        assert!(!q.get(&world, id3).unwrap().is_changed());
        assert!(!q.get(&world, id4).unwrap().is_changed());

        world.clear_trackers();
        world.entity_mut(id2).insert(Visibility::Visible);
        schedule.run(&mut world);

        assert!(!q.get(&world, id1).unwrap().is_changed());
        assert!(q.get(&world, id2).unwrap().is_changed());
        assert!(q.get(&world, id3).unwrap().is_changed());
        assert!(q.get(&world, id4).unwrap().is_changed());

        world.clear_trackers();
        schedule.run(&mut world);

        assert!(!q.get(&world, id1).unwrap().is_changed());
        assert!(!q.get(&world, id2).unwrap().is_changed());
        assert!(!q.get(&world, id3).unwrap().is_changed());
        assert!(!q.get(&world, id4).unwrap().is_changed());
    }

    #[test]
    fn visibility_propagation_with_invalid_parent() {
        let mut world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(visibility_propagate_system);

        let parent = world.spawn(()).id();
        let child = world.spawn(Visibility::default()).id();
        world.entity_mut(parent).add_children(&[child]);

        schedule.run(&mut world);
        world.clear_trackers();

        let child_visible = world.entity(child).get::<InheritedVisibility>().unwrap().0;
        assert!(child_visible);
    }

    #[test]
    fn ensure_visibility_enum_size() {
        assert_eq!(1, size_of::<Visibility>());
        assert_eq!(1, size_of::<Option<Visibility>>());
    }

    #[derive(Component, Default, Clone, Reflect)]
    #[require(VisibilityClass)]
    #[reflect(Component, Default, Clone)]
    #[component(on_add = add_visibility_class::<Self>)]
    struct TestVisibilityClassHook;

    #[test]
    fn test_add_visibility_class_hook() {
        let mut world = World::new();
        let entity = world.spawn(TestVisibilityClassHook).id();
        let entity_clone = world.spawn_empty().id();
        world
            .entity_mut(entity)
            .clone_with_opt_out(entity_clone, |_| {});

        let entity_visibility_class = world.entity(entity).get::<VisibilityClass>().unwrap();
        assert_eq!(entity_visibility_class.len(), 1);

        let entity_clone_visibility_class =
            world.entity(entity_clone).get::<VisibilityClass>().unwrap();
        assert_eq!(entity_clone_visibility_class.len(), 1);
    }

    #[test]
    fn view_visibility_lifecycle() {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            bevy_asset::AssetPlugin::default(),
            bevy_mesh::MeshPlugin,
            bevy_transform::TransformPlugin,
            VisibilityPlugin,
        ));

        #[derive(Resource, Default)]
        struct ManualMark(bool);
        #[derive(Resource, Default)]
        struct ObservedChanged(bool);
        app.init_resource::<ManualMark>();
        app.init_resource::<ObservedChanged>();

        app.add_systems(
            PostUpdate,
            (
                (|mut q: Query<&mut ViewVisibility>, mark: Res<ManualMark>| {
                    if mark.0 {
                        for mut v in &mut q {
                            v.set_visible();
                        }
                    }
                })
                .in_set(VisibilitySystems::CheckVisibility),
                (|q: Query<(), Changed<ViewVisibility>>, mut observed: ResMut<ObservedChanged>| {
                    if !q.is_empty() {
                        observed.0 = true;
                    }
                })
                .after(VisibilitySystems::MarkNewlyHiddenEntitiesInvisible),
            ),
        );

        let entity = app.world_mut().spawn(ViewVisibility::HIDDEN).id();

        app.update();
        app.world_mut().resource_mut::<ObservedChanged>().0 = false;

        app.update();
        {
            assert!(
                !app.world()
                    .entity(entity)
                    .get::<ViewVisibility>()
                    .unwrap()
                    .get(),
                "Frame 1: should be hidden"
            );
            assert!(
                !app.world().resource::<ObservedChanged>().0,
                "Frame 1: should not be changed"
            );
        }

        app.world_mut().resource_mut::<ManualMark>().0 = true;
        app.update();
        {
            assert!(
                app.world()
                    .entity(entity)
                    .get::<ViewVisibility>()
                    .unwrap()
                    .get(),
                "Frame 2: should be visible"
            );
            assert!(
                app.world().resource::<ObservedChanged>().0,
                "Frame 2: should be changed"
            );
        }

        app.world_mut().resource_mut::<ManualMark>().0 = true;
        app.world_mut().resource_mut::<ObservedChanged>().0 = false;
        app.update();
        {
            assert!(
                app.world()
                    .entity(entity)
                    .get::<ViewVisibility>()
                    .unwrap()
                    .get(),
                "Frame 3: should be visible"
            );
            assert!(
                !app.world().resource::<ObservedChanged>().0,
                "Frame 3: should NOT be changed"
            );
        }

        app.world_mut().resource_mut::<ManualMark>().0 = false;
        app.world_mut().resource_mut::<ObservedChanged>().0 = false;
        app.update();
        {
            assert!(
                !app.world()
                    .entity(entity)
                    .get::<ViewVisibility>()
                    .unwrap()
                    .get(),
                "Frame 4: should be hidden"
            );
            assert!(
                app.world().resource::<ObservedChanged>().0,
                "Frame 4: should be changed"
            );
        }

        app.world_mut().resource_mut::<ManualMark>().0 = false;
        app.world_mut().resource_mut::<ObservedChanged>().0 = false;
        app.update();
        {
            assert!(
                !app.world()
                    .entity(entity)
                    .get::<ViewVisibility>()
                    .unwrap()
                    .get(),
                "Frame 5: should be hidden"
            );
            assert!(
                !app.world().resource::<ObservedChanged>().0,
                "Frame 5: should NOT be changed"
            );
        }
    }
}
