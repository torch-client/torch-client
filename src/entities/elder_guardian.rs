use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;

use super::models::monsters::guardian;
use super::{Blend, EntityImages, bake, geom};
use crate::renderer::entity_material::{EntityMaterial, LightMode, Lit, diffuse_lights};
use crate::renderer::lightmap::cell_light;

const LIFETIME: u64 = 30;
const SCALE: f32 = 0.425_531_92;
const TEXTURE: &str = "entity/guardian/guardian_elder";
const MAX_LIVE: usize = 16384;
const POOL_SLACK: usize = 4;

#[derive(Component)]
pub(crate) struct ElderGuardianNode;

#[derive(Resource, Default)]
pub(crate) struct ElderGuardianCurse {
    pending: u32,
    held_at: u64,
    live: Vec<u64>,
    mesh: Option<Handle<Mesh>>,
    nodes: Vec<(Entity, Handle<EntityMaterial>)>,
    level: u32,
}

impl ElderGuardianCurse {
    pub(crate) fn add(&mut self, count: u32, level: u32) {
        self.enter(level);
        self.pending = self.pending.saturating_add(count).min(MAX_LIVE as u32);
    }

    fn enter(&mut self, level: u32) {
        if self.level != level {
            self.level = level;
            self.pending = 0;
            self.live.clear();
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per input and store it writes"
)]
pub(crate) fn sync_elder_guardian(
    mut commands: Commands,
    view: Res<crate::renderer::FrameView>,
    shared: Res<crate::renderer::systems::Shared>,
    lightmap: Res<crate::renderer::systems::LightmapState>,
    mut curse: ResMut<ElderGuardianCurse>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut entity_images: ResMut<EntityImages>,
    mut materials: ResMut<Assets<EntityMaterial>>,
    mut nodes: Query<(&mut Transform, &mut Visibility), With<ElderGuardianNode>>,
) {
    crate::prof_span!("render:sync_elder_guardian");
    let curse = &mut *curse;
    let now = view.world_ticks;
    let (effects, level) = {
        let mut s = shared.0.lock().unwrap();
        (
            std::mem::take(&mut s.session.elder_guardian_effects),
            crate::client::tick_rate::level(),
        )
    };
    curse.add(effects, level);

    let dimension = crate::renderer::dimension::current();
    curse
        .live
        .retain(|&start| now.wrapping_sub(start) <= LIFETIME);
    if view.world_run.runs || now > curse.held_at {
        let added = std::mem::take(&mut curse.pending) as usize;
        let over = (curse.live.len() + added).saturating_sub(MAX_LIVE);
        curse.live.drain(..over.min(curse.live.len()));
        curse.live.resize(curse.live.len() + added, now);
    }
    curse.held_at = now;

    let mut drawn = 0;
    let texture = if curse.live.is_empty() {
        None
    } else {
        entity_images.get(TEXTURE, &mut images)
    };
    if let Some(texture) = texture {
        let mesh = curse
            .mesh
            .get_or_insert_with(|| meshes.add(rest_mesh()))
            .clone();
        let light = cell_light(&lightmap.current, 15, 15);
        let dirs = diffuse_lights(dimension);
        let camera =
            Transform::from_translation(view.camera_render).with_rotation(view.camera_rotation);
        let (alpha_mode, cutoff) = Blend::Translucent.alpha();

        for &start in &curse.live {
            let age_scale = (now.wrapping_sub(start) as f32 + view.world_partial) / LIFETIME as f32;
            let lit = Lit {
                tint: [
                    1.0,
                    1.0,
                    1.0,
                    0.05 + 0.5 * (age_scale * std::f32::consts::PI).sin(),
                ],
                light,
                dirs,
                overlay_alpha: 1.0,
                overlay_white: false,
            };
            let pose = camera
                .mul_transform(Transform::from_rotation(Quat::from_rotation_x(
                    (60.0 - 150.0 * age_scale).to_radians(),
                )))
                .mul_transform(Transform::from_scale(Vec3::new(SCALE, -SCALE, -SCALE)))
                .mul_transform(Transform::from_xyz(0.0, -0.56, 3.5));

            match curse.nodes.get(drawn) {
                Some((entity, material)) => {
                    super::relight_unlit(&mut materials, material, lit, LightMode::Cardinal);
                    if let Ok((mut transform, mut vis)) = nodes.get_mut(*entity) {
                        *transform = pose;
                        vis.set_if_neq(Visibility::Inherited);
                    }
                }
                None => {
                    let material = materials.add(EntityMaterial::new(
                        Some(texture.clone()),
                        super::unlit_light_mode(LightMode::Cardinal),
                        alpha_mode,
                        cutoff,
                        lit,
                        0,
                    ));
                    #[cfg_attr(not(feature = "builtin_shaders"), allow(unused_mut))]
                    let mut node = commands.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        pose,
                        Visibility::Inherited,
                        NoFrustumCulling,
                        ElderGuardianNode,
                    ));
                    #[cfg(feature = "builtin_shaders")]
                    node.insert(bevy::light::NotShadowCaster);
                    curse.nodes.push((node.id(), material));
                }
            }
            drawn += 1;
        }
    }

    for (entity, _) in curse.nodes.iter().skip(drawn) {
        if let Ok((_, mut vis)) = nodes.get_mut(*entity) {
            vis.set_if_neq(Visibility::Hidden);
        }
    }
    if curse.nodes.len() > drawn + POOL_SLACK {
        for (entity, _) in curse.nodes.drain(drawn + POOL_SLACK..) {
            commands.entity(entity).despawn();
        }
    }
}

pub(crate) fn clear_elder_guardian(
    mut curse: ResMut<ElderGuardianCurse>,
    mut nodes: Query<&mut Visibility, With<ElderGuardianNode>>,
) {
    curse.live.clear();
    curse.pending = 0;
    for mut vis in &mut nodes {
        vis.set_if_neq(Visibility::Hidden);
    }
}

fn rest_mesh() -> Mesh {
    let model = geom::bake(&guardian::elder_layer());
    let states = model.rest_pose();
    let mut posed = Vec::new();
    model.pose_parts(&states, Mat4::IDENTITY, &mut posed);
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    bake::layer(&mut mesh, &model, &posed, &states);
    mesh
}
