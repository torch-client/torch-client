use azalea_registry::builtin::EntityKind;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::entities::quads::QuadMesh;
use crate::renderer::anim::AnimSample;
use crate::renderer::entity_material::{EntityMaterial, LightMode, Lit};

const SEGMENTS: usize = 16;
const HOOK_LIFT: f32 = 0.25;
const LINE_WIDTH_PX: f32 = 2.5;
const LINE_WIDTH_REFERENCE: f32 = 1920.0;
const Z_NEAR: f32 = 0.05;
const VIEW_BOBBING_SCALE: f32 = 960.0;
const POOL_SLACK: usize = 4;

#[derive(Component)]
pub(crate) struct FishingLineNode;

#[derive(Resource, Default)]
pub(crate) struct FishingLines {
    material: Option<Handle<EntityMaterial>>,
    nodes: Vec<(Entity, Handle<Mesh>)>,
}

struct Owner {
    feet: Vec3,
    sample: AnimSample,
    scale: f32,
    local: bool,
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per input and store it writes"
)]
pub(crate) fn sync_fishing_lines(
    mut commands: Commands,
    view: Res<crate::renderer::FrameView>,
    shared: Res<crate::renderer::systems::Shared>,
    gui: Res<crate::gui::GuiState>,
    camera_view: Res<crate::renderer::input::CameraView>,
    origin: Res<crate::renderer::RenderOrigin>,
    projection_q: Query<&Projection, With<crate::renderer::systems::WorldCamera>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut lines: ResMut<FishingLines>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<EntityMaterial>>,
    mut nodes: Query<(&mut Transform, &mut Visibility), With<FishingLineNode>>,
) {
    crate::prof_span!("render:sync_fishing_lines");
    let partial = view.partial;
    let origin = *origin;
    let hooks: Vec<_> = view
        .entities
        .iter()
        .filter(|anim| anim.kind == EntityKind::FishingBobber)
        .map(|anim| anim.sample(partial))
        .filter_map(|st| Some((origin.to_render_array(st.pos), st.extras.hook_owner?)))
        .collect();

    let mut drawn = 0;
    if let (false, Ok(Projection::Perspective(projection)), Ok(window)) =
        (hooks.is_empty(), projection_q.single(), window_q.single())
    {
        let (camera, rotation) = (view.camera_render, view.camera_rotation);
        let local_id = crate::client::tracking::local_entity_id();
        let owners: Vec<Option<Owner>> = {
            let s = shared.0.lock().unwrap();
            hooks
                .iter()
                .map(|&(_, owner)| {
                    if owner == local_id {
                        return Some(Owner {
                            feet: origin.to_render_array(view.player_lerp),
                            sample: view.local.clone(),
                            scale: view.local_body.scale,
                            local: true,
                        });
                    }
                    let p = s.session.other_players.iter().find(|p| p.id == owner)?;
                    Some(Owner {
                        feet: origin.to_render_array(p.anim.position(partial)),
                        sample: p.anim.sample(partial),
                        scale: p.body.scale,
                        local: false,
                    })
                })
                .collect()
        };

        let (width, height) = (
            window.physical_width() as f32,
            window.physical_height() as f32,
        );
        let strip = Strip {
            camera,
            forward: rotation * Vec3::NEG_Z,
            half_width_per_depth: (LINE_WIDTH_PX.max(width / LINE_WIDTH_REFERENCE * LINE_WIDTH_PX)
                / 2.0)
                * 2.0
                * (projection.fov / 2.0).tan()
                / height.max(1.0),
        };
        let first_person = camera_view.first_person();
        let material = lines
            .material
            .get_or_insert_with(|| {
                let lit = Lit {
                    tint: [0.0, 0.0, 0.0, 1.0],
                    ..Lit::full_bright()
                };
                materials.add(EntityMaterial::new(
                    None,
                    LightMode::Flat,
                    AlphaMode::Opaque,
                    0.0,
                    lit,
                    0,
                ))
            })
            .clone();

        for (&(hook, _), owner) in hooks.iter().zip(&owners) {
            let Some(owner) = owner else { continue };
            let hand = if owner.local && first_person {
                first_person_hand(
                    camera,
                    rotation,
                    gui.options.fov as f32,
                    width / height.max(1.0),
                    &owner.sample,
                )
            } else {
                third_person_hand(owner)
            };
            let mesh = strip.mesh(hook, hand - (hook + Vec3::Y * HOOK_LIFT));
            match lines.nodes.get(drawn) {
                Some((entity, handle)) => {
                    let _ = meshes.insert(handle, mesh);
                    if let Ok((mut transform, mut vis)) = nodes.get_mut(*entity) {
                        transform.translation = hook;
                        vis.set_if_neq(Visibility::Inherited);
                    }
                }
                None => {
                    let handle = meshes.add(mesh);
                    #[cfg_attr(not(feature = "builtin_shaders"), allow(unused_mut))]
                    let mut node = commands.spawn((
                        Mesh3d(handle.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::from_translation(hook),
                        Visibility::Inherited,
                        NoFrustumCulling,
                        FishingLineNode,
                    ));
                    #[cfg(feature = "builtin_shaders")]
                    node.insert(bevy::light::NotShadowCaster);
                    lines.nodes.push((node.id(), handle));
                }
            }
            drawn += 1;
        }
    }

    for (entity, _) in lines.nodes.iter().skip(drawn) {
        if let Ok((_, mut vis)) = nodes.get_mut(*entity) {
            vis.set_if_neq(Visibility::Hidden);
        }
    }
    if lines.nodes.len() > drawn + POOL_SLACK {
        for (entity, _) in lines.nodes.drain(drawn + POOL_SLACK..) {
            commands.entity(entity).despawn();
        }
    }
}

fn holding_side(sample: &AnimSample) -> f32 {
    let rod_in_main = sample.main_hand.id == "fishing_rod";
    if rod_in_main != sample.main_arm_left {
        1.0
    } else {
        -1.0
    }
}

fn swing(sample: &AnimSample) -> f32 {
    (sample.attack_time.sqrt() * std::f32::consts::PI).sin()
}

fn third_person_hand(owner: &Owner) -> Vec3 {
    let invert = holding_side(&owner.sample);
    let (sin, cos) = owner.sample.body_yaw.to_radians().sin_cos();
    let right = invert * 0.35 * owner.scale;
    let forward = 0.8 * owner.scale;
    let crouch = if owner.sample.crouching { -0.1875 } else { 0.0 };
    let eye = owner.feet
        + Vec3::Y * crate::renderer::player_model::player_eye_height(&owner.sample, owner.scale);
    eye + Vec3::new(
        -cos * right - sin * forward,
        crouch - 0.45 * owner.scale,
        -sin * right + cos * forward,
    )
}

fn first_person_hand(
    camera: Vec3,
    rotation: Quat,
    fov: f32,
    aspect: f32,
    sample: &AnimSample,
) -> Vec3 {
    let plane_height = (fov.to_radians() / 2.0).tan() * Z_NEAR;
    let plane_width = plane_height * aspect;
    let forward = rotation * Vec3::NEG_Z * Z_NEAR;
    let left = rotation * Vec3::NEG_X * plane_width;
    let up = rotation * Vec3::Y * plane_height;
    let point = forward + up * -0.1 - left * (holding_side(sample) * 0.525);
    let swing = swing(sample);
    let turn = Quat::from_rotation_x(swing * 0.7) * Quat::from_rotation_y(swing * 0.5);
    camera + turn * (point * (VIEW_BOBBING_SCALE / fov))
}

struct Strip {
    camera: Vec3,
    forward: Vec3,
    half_width_per_depth: f32,
}

impl Strip {
    fn mesh(&self, hook: Vec3, offset: Vec3) -> Mesh {
        let point = |a: f32| {
            Vec3::new(
                offset.x * a,
                offset.y * (a * a + a) * 0.5 + HOOK_LIFT,
                offset.z * a,
            )
        };
        let mut out = QuadMesh::new();
        for i in 0..SEGMENTS {
            let (p0, p1) = (
                point(i as f32 / SEGMENTS as f32),
                point((i + 1) as f32 / SEGMENTS as f32),
            );
            let along = (p1 - p0).normalize_or_zero();
            let side = |p: Vec3| {
                let to_camera = hook + p - self.camera;
                let depth = to_camera.dot(self.forward).max(Z_NEAR);
                along.cross(to_camera).normalize_or_zero() * depth * self.half_width_per_depth
            };
            let (s0, s1) = (side(p0), side(p1));
            out.untextured(
                [
                    (p0 - s0).to_array(),
                    (p1 - s1).to_array(),
                    (p1 + s1).to_array(),
                    (p0 + s0).to_array(),
                ],
                [0.0, 1.0, 0.0],
            );
        }
        out.finish()
    }
}
