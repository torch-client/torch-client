use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use super::CameraView;
use super::quads::QuadMesh;
use crate::renderer::entity_material::{EntityMaterial, LightMode, Lit, diffuse_lights};
use crate::renderer::lightmap::{FIRE_BLOCK_LIGHT, cell_light, entity_light};

const WIDTH_SCALE: f32 = 1.4;
const FIRST_HALF_WIDTH: f32 = 0.5;
const QUAD_HEIGHT: f32 = 1.4;
const QUAD_STEP: f32 = 0.45;
const HALF_WIDTH_FALLOFF: f32 = 0.9;
const QUAD_DEPTH_STEP: f32 = 0.03;
const FORWARD_SHIFT: f32 = 0.3;
const FORWARD_SHIFT_PER_UNIT: f32 = 0.02;
const FIRE_TEXTURES: [&str; 2] = ["block/fire_0#0", "block/fire_1#0"];
const POOL_SLACK: usize = 8;

#[derive(Component)]
pub(crate) struct FlameRoot;

#[derive(Component)]
pub(crate) struct FlameQuads;

struct FlameNode {
    root: Entity,
    quads: [Entity; 2],
    materials: [Handle<EntityMaterial>; 2],
    key: u32,
}

#[derive(Resource, Default)]
pub(crate) struct FlameAssets {
    meshes: HashMap<u32, [Option<Handle<Mesh>>; 2]>,
    nodes: Vec<FlameNode>,
}

fn flame_meshes(h: f32) -> [Option<Mesh>; 2] {
    let mut out = [QuadMesh::new(), QuadMesh::new()];
    let shift = FORWARD_SHIFT - (h as i32) as f32 * FORWARD_SHIFT_PER_UNIT;
    let (mut h, mut r, mut yo, mut zo) = (h, FIRST_HALF_WIDTH, 0.0f32, 0.0f32);
    let mut ss = 0usize;
    while h > 0.0 {
        let (u0, u1) = if ss / 2 % 2 == 0 {
            (1.0, 0.0)
        } else {
            (0.0, 1.0)
        };
        let z = shift + zo;
        out[ss % 2].quad(
            [
                [-r, -yo, z],
                [r, -yo, z],
                [r, QUAD_HEIGHT - yo, z],
                [-r, QUAD_HEIGHT - yo, z],
            ],
            [[u1, 1.0], [u0, 1.0], [u0, 0.0], [u1, 0.0]],
            [0.0, 1.0, 0.0],
        );
        h -= QUAD_STEP;
        yo -= QUAD_STEP;
        r *= HALF_WIDTH_FALLOFF;
        zo -= QUAD_DEPTH_STEP;
        ss += 1;
    }
    out.map(|quads| (!quads.is_empty()).then(|| quads.finish()))
}

pub(crate) struct Burning {
    pos: Vec3,
    bounding_box: [f32; 2],
    light: crate::renderer::lightmap::CellLight,
}

fn flame_size([width, height]: [f32; 2]) -> Option<(f32, f32)> {
    let s = width * WIDTH_SCALE;
    let h = height / s;
    (s > 0.0 && h.is_finite()).then_some((s, h))
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per asset store and query it writes"
)]
pub(crate) fn sync_flames(
    mut commands: Commands,
    view: Res<crate::renderer::FrameView>,
    shared: Res<crate::renderer::systems::Shared>,
    camera_view: Res<crate::renderer::input::CameraView>,
    lightmap: Res<crate::renderer::systems::LightmapState>,
    origin: Res<crate::renderer::RenderOrigin>,
    mut flames: ResMut<FlameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut entity_images: ResMut<super::EntityImages>,
    mut materials: ResMut<Assets<EntityMaterial>>,
    mut roots: Query<(&mut Transform, &mut Visibility), (With<FlameRoot>, Without<FlameQuads>)>,
    mut quads: Query<(&mut Mesh3d, &mut Visibility), (With<FlameQuads>, Without<FlameRoot>)>,
    mut burning: Local<Vec<Burning>>,
) {
    crate::prof_span!("render:sync_flames");
    let FlameAssets {
        meshes: flame_meshes_by_key,
        nodes,
    } = &mut *flames;
    let origin = *origin;
    let camera = CameraView::of(&view);

    let partial = view.partial;
    burning.clear();
    {
        let light = crate::client::worldsync::light_map().read();
        let lm = &lightmap.current;
        burning.extend(
            view.entities
                .iter()
                .filter(|anim| anim.display_fire())
                .map(|anim| anim.sample(partial))
                .filter(|st| super::within_view_range(st, &camera))
                .map(|st| {
                    let (_, sky) = super::entity_levels(&light, &st);
                    Burning {
                        pos: origin.to_render_array(st.pos) + Vec3::from(st.extras.render_offset),
                        bounding_box: [st.bounding_box_width, st.bounding_box_height],
                        light: cell_light(lm, FIRE_BLOCK_LIGHT, sky),
                    }
                }),
        );
    }
    let player = |feet: [f64; 3],
                  sample: &crate::renderer::anim::AnimSample,
                  body: &crate::session::PlayerBody| {
        let eye = crate::renderer::player_model::player_eye(feet, sample, body.scale);
        Burning {
            pos: origin.to_render_array(feet),
            bounding_box: body.bounding_box,
            light: entity_light(&lightmap.current, eye, true),
        }
    };
    {
        let s = shared.0.lock().unwrap();
        burning.extend(
            s.session
                .other_players
                .iter()
                .filter(|p| p.body.on_fire)
                .map(|p| player(p.anim.position(partial), &p.anim.sample(partial), &p.body)),
        );
    }
    if view.local_body.on_fire && camera_view.detached() {
        burning.push(player(view.player_lerp, &view.local, &view.local_body));
    }
    burning.retain(|b| flame_size(b.bounding_box).is_some());

    let textures = if burning.is_empty() {
        None
    } else {
        let [a, b] = FIRE_TEXTURES.map(|path| entity_images.get(path, &mut images));
        a.zip(b).map(|(a, b)| [a, b])
    };
    let used = if textures.is_some() { burning.len() } else { 0 };

    if let Some(textures) = &textures {
        let rotation = camera.yaw_rotation();
        let dirs = diffuse_lights(crate::renderer::dimension::current());
        for (i, flame) in burning.iter().enumerate() {
            let Some((s, h)) = flame_size(flame.bounding_box) else {
                continue;
            };
            let key = h.to_bits();
            let pair = flame_meshes_by_key
                .entry(key)
                .or_insert_with(|| flame_meshes(h).map(|mesh| mesh.map(|mesh| meshes.add(mesh))))
                .clone();
            let lit = Lit {
                tint: [1.0; 4],
                light: flame.light,
                dirs,
                overlay_alpha: 1.0,
                overlay_white: false,
            };
            let place = Transform {
                translation: flame.pos,
                rotation,
                scale: Vec3::splat(s),
            };

            let Some(node) = nodes.get_mut(i) else {
                nodes.push(spawn_node(
                    &mut commands,
                    &mut materials,
                    textures,
                    &pair,
                    place,
                    lit,
                    key,
                ));
                continue;
            };
            if let Ok((mut transform, mut vis)) = roots.get_mut(node.root) {
                if *transform != place {
                    *transform = place;
                }
                vis.set_if_neq(Visibility::Inherited);
            }
            if node.key != key {
                node.key = key;
                for (entity, mesh) in node.quads.iter().zip(&pair) {
                    let Ok((mut slot, mut vis)) = quads.get_mut(*entity) else {
                        continue;
                    };
                    match mesh {
                        Some(mesh) => {
                            if slot.0 != *mesh {
                                slot.0 = mesh.clone();
                            }
                            vis.set_if_neq(Visibility::Inherited);
                        }
                        None => {
                            vis.set_if_neq(Visibility::Hidden);
                        }
                    }
                }
            }
            for material in &node.materials {
                EntityMaterial::relight(&mut materials, material, lit);
            }
        }
    }

    for node in nodes.iter().skip(used) {
        if let Ok((_, mut vis)) = roots.get_mut(node.root) {
            vis.set_if_neq(Visibility::Hidden);
        }
    }
    if nodes.len() > used + POOL_SLACK {
        for node in nodes.drain(used + POOL_SLACK..) {
            commands.entity(node.root).despawn();
        }
    }
}

fn spawn_node(
    commands: &mut Commands,
    materials: &mut Assets<EntityMaterial>,
    textures: &[Handle<Image>; 2],
    pair: &[Option<Handle<Mesh>>; 2],
    place: Transform,
    lit: Lit,
    key: u32,
) -> FlameNode {
    let materials = textures.clone().map(|texture| {
        materials.add(EntityMaterial::cutout(
            Some(texture),
            LightMode::Cardinal,
            lit,
        ))
    });
    let root = commands
        .spawn((place, Visibility::Inherited, FlameRoot, Name::new("flame")))
        .id();
    let quads = [0, 1].map(|k| {
        let vis = if pair[k].is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let mut quad = commands.spawn((
            Mesh3d(pair[k].clone().unwrap_or_default()),
            MeshMaterial3d(materials[k].clone()),
            Transform::default(),
            vis,
            FlameQuads,
        ));
        #[cfg(feature = "builtin_shaders")]
        quad.insert(bevy::light::NotShadowCaster);
        let quad = quad.id();
        commands.entity(root).add_child(quad);
        quad
    });
    FlameNode {
        root,
        quads,
        materials,
        key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn positions(mesh: &Mesh) -> Vec<[f32; 3]> {
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap()
            .to_vec()
    }

    #[test]
    fn a_zombie_flame_is_six_quads_split_between_the_sprites() {
        let (s, h) = (0.6f32 * WIDTH_SCALE, 1.95f32 / (0.6 * WIDTH_SCALE));
        assert!((s - 0.84).abs() < 1e-6);
        let [even, odd] = flame_meshes(h);
        let (even, odd) = (positions(&even.unwrap()), positions(&odd.unwrap()));
        assert_eq!(even.len(), 12);
        assert_eq!(odd.len(), 12);
        assert_eq!([even[0][0], even[0][1]], [-0.5, 0.0]);
        assert_eq!([even[2][0], even[2][1]], [0.5, 1.4]);
        assert!((even[0][2] - 0.26).abs() < 1e-6);
        assert!((odd[0][1] - 0.45).abs() < 1e-6);
        assert!((odd[1][0] - 0.45).abs() < 1e-6);
        assert!((odd[0][2] - 0.23).abs() < 1e-6);
    }

    #[test]
    fn every_other_pair_of_quads_is_mirrored() {
        let [even, _] = flame_meshes(1.0);
        let even = even.unwrap();
        let uv = |i: usize| match even.attribute(Mesh::ATTRIBUTE_UV_0).unwrap() {
            bevy::mesh::VertexAttributeValues::Float32x2(v) => v[i],
            _ => unreachable!(),
        };
        assert_eq!(uv(0), [0.0, 1.0]);
        assert_eq!(uv(4), [1.0, 1.0]);
    }

    #[test]
    fn a_short_flame_has_no_second_sprite() {
        let [even, odd] = flame_meshes(0.3);
        assert!(even.is_some());
        assert!(odd.is_none());
    }

    #[test]
    fn the_flame_turns_with_the_camera_yaw_only() {
        let camera = CameraView::of(&crate::renderer::FrameView {
            camera_rotation: Quat::from_euler(EulerRot::YXZ, 0.7, -0.4, 0.0),
            ..Default::default()
        });
        let turned = camera.yaw_rotation();
        let (yaw, pitch, roll) = turned.to_euler(EulerRot::YXZ);
        assert!((yaw - 0.7).abs() < 1e-5, "{yaw}");
        assert!(pitch.abs() < 1e-5 && roll.abs() < 1e-5);
    }
}
