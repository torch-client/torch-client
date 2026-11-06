use azalea_registry::builtin::EntityKind;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::color::{LinearRgba, Srgba};
use bevy::mesh::{PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use crate::entities::feed::EntityAnim;
use crate::entities::state::{EntityState, Pose};
use crate::lighting::level::LightMap;
use crate::renderer::anim::AnimSample;
use crate::renderer::entity_material::{EntityMaterial, LightMode, Lit};
use crate::renderer::lightmap::{cell_of, levels_in, light_color};
use crate::session::PlayerBody;

const STEPS: usize = 24;
const WIDTH: f32 = 0.05;
const STRIP_VERTS: usize = 2 * 2 * (STEPS + 1);

#[derive(Component)]
pub(crate) struct LeashNode;

#[derive(Resource, Default)]
pub(crate) struct Leashes {
    material: Option<Handle<EntityMaterial>>,
    node: Option<(Entity, Handle<Mesh>)>,
    players: Vec<(i32, Holder)>,
    pos: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    written_pos: Vec<[f32; 3]>,
    written_col: Vec<[f32; 4]>,
}

#[derive(Clone, Copy)]
struct Holder {
    pos: Vec3,
    yaw: f32,
    rope: Vec3,
    size: [f32; 2],
    quad: Option<[f32; 4]>,
    eye: [f64; 3],
    levels: Option<(u8, u8)>,
}

fn y_rot(v: Vec3, deg: f32) -> Vec3 {
    Quat::from_rotation_y(-deg.to_radians()) * v
}

impl Holder {
    fn mob(origin: crate::renderer::RenderOrigin, st: &EntityState, light: &LightMap) -> Holder {
        let pos = origin.to_render_array(st.pos);
        let rope = match st.kind {
            EntityKind::LeashKnot => pos + Vec3::new(0.0, 0.2, 0.0),
            EntityKind::Villager | EntityKind::WanderingTrader => {
                pos + y_rot(
                    Vec3::new(0.0, st.bounding_box_height - 1.0, 0.2),
                    st.body_rot,
                )
            }
            _ => pos + Vec3::Y * (st.eye_height * 0.7),
        };
        let quad = match st.kind {
            EntityKind::Ghast => Some([0.0, 0.5, 0.5, 0.0]),
            EntityKind::HappyGhast => Some([-0.03125, 0.4375, 0.46875, 0.03125]),
            _ => None,
        };
        Holder {
            pos,
            yaw: st.body_rot,
            rope,
            size: [st.bounding_box_width, st.bounding_box_height],
            quad,
            eye: [st.pos[0], st.pos[1] + f64::from(st.eye_height), st.pos[2]],
            levels: Some(super::entity_levels(light, st)),
        }
    }

    fn player(
        origin: crate::renderer::RenderOrigin,
        feet: [f64; 3],
        sample: &AnimSample,
        body: &PlayerBody,
        first: Option<(f32, f32, f32)>,
    ) -> Holder {
        let pos = origin.to_render_array(feet);
        let eye = crate::renderer::player_model::player_eye(feet, sample, body.scale);
        let pitch = |deg: f32| Quat::from_rotation_x(deg.to_radians());
        let side = if sample.main_arm_left { 1.0 } else { -1.0 };
        let rope = match first {
            Some((yaw, xrot, eye_height)) => {
                let hand = Vec3::new(0.39 * side, -0.6, 0.3);
                pos + Vec3::Y * eye_height + y_rot(pitch(xrot) * hand, yaw)
            }
            None if sample.fall_flying || sample.spin_attack => {
                pos + y_rot(
                    pitch(sample.pitch) * Vec3::new(0.22 * side, -0.11, 0.85),
                    sample.body_yaw,
                )
            }
            None if sample.visually_swimming => {
                pos + y_rot(
                    pitch(sample.pitch) * Vec3::new(0.22 * side, 0.2, -0.15),
                    sample.body_yaw,
                )
            }
            None => {
                let z = if sample.crouching { -0.2 } else { 0.07 };
                pos + y_rot(
                    Vec3::new(0.22 * side, body.bounding_box[1] - 1.0, z),
                    sample.body_yaw,
                )
            }
        };
        Holder {
            pos,
            yaw: sample.body_yaw,
            rope,
            size: body.bounding_box,
            quad: None,
            eye,
            levels: None,
        }
    }
}

fn leash_offset(st: &EntityState) -> Vec3 {
    use EntityKind as K;
    let (eye, w, h) = (st.eye_height, st.bounding_box_width, st.bounding_box_height);
    let (y, z) = match st.kind {
        K::Pig | K::Strider | K::Wolf | K::Rabbit => (0.6 * eye, 0.4 * w),
        K::Ocelot | K::Parrot => (0.5 * eye, 0.4 * w),
        K::Fox => (0.55 * eye, 0.4 * w),
        K::SnowGolem => (0.75 * eye, 0.4 * w),
        K::IronGolem => (0.875 * eye, 0.4 * w),
        K::CopperGolem => (0.75 * eye, 0.0),
        K::Llama | K::TraderLlama => (0.75 * eye, 0.5 * w),
        K::Allay => (0.6 * eye, 0.1 * w),
        K::Bee => (0.5 * eye, 0.2 * w),
        K::HappyGhast => (0.0, 0.0),
        K::Camel | K::CamelHusk => {
            let scale = if st.age_scale < 1.0 { 0.6 } else { 1.0 };
            let sit = if st.pose == Pose::Sitting {
                1.43 * scale
            } else {
                0.0
            };
            (h - 0.375 * scale + sit - 0.2 * scale, 0.56 * w)
        }
        k if is_boat(k) => (0.88 * h, 0.64 * w),
        _ => (eye, 0.4 * w),
    };
    Vec3::new(0.0, y, z)
}

fn is_boat(kind: EntityKind) -> bool {
    crate::play::riding::Hull::of(kind).is_some()
}

fn quad_params(kind: EntityKind) -> Option<[f32; 4]> {
    use EntityKind as K;
    Some(match kind {
        k if is_boat(k) => [0.0, 0.64, 0.382, 0.88],
        K::Horse | K::SkeletonHorse => [0.04, 0.52, 0.23, 0.87],
        K::Donkey | K::Mule | K::ZombieHorse => [0.04, 0.41, 0.18, 0.73],
        K::Camel | K::CamelHusk => [0.02, 0.48, 0.25, 0.82],
        K::Sniffer => [-0.01, 0.63, 0.38, 1.15],
        _ => return None,
    })
}

fn quad_offsets(size: [f32; 2], [front, back, side, height]: [f32; 4]) -> [Vec3; 4] {
    let [w, h] = size;
    let (y, f, b, s) = (height * h, front * w, back * w, side * w);
    [
        Vec3::new(-s, y, b + f),
        Vec3::new(-s, y, -b + f),
        Vec3::new(s, y, -b + f),
        Vec3::new(s, y, b + f),
    ]
}

fn stripes() -> [[f32; 3]; 2] {
    [0.7, 1.0].map(|s| {
        let c = LinearRgba::from(Srgba::rgb(0.5 * s, 0.4 * s, 0.3 * s));
        [c.red, c.green, c.blue]
    })
}

fn push_rope(
    pos: &mut Vec<[f32; 3]>,
    col: &mut Vec<[f32; 4]>,
    start: Vec3,
    end: Vec3,
    slack: bool,
    light: &[[f32; 3]; STEPS + 1],
) {
    let d = end - start;
    let horizontal = d.x * d.x + d.z * d.z;
    let factor = if horizontal > 0.0 {
        horizontal.sqrt().recip() * WIDTH / 2.0
    } else {
        0.0
    };
    let (dx_off, dz_off) = (d.z * factor, d.x * factor);
    let stripes = stripes();

    let mut verts = [[0.0f32; 3]; STRIP_VERTS];
    let mut colors = [[0.0f32; 4]; STRIP_VERTS];
    for i in 0..STRIP_VERTS {
        let forward = i < STRIP_VERTS / 2;
        let k = if forward {
            i / 2
        } else {
            STEPS - (i - STRIP_VERTS / 2) / 2
        };
        let progress = k as f32 / STEPS as f32;
        let fudge = if forward { WIDTH } else { 0.0 };
        let y = if !slack {
            d.y * progress
        } else if d.y > 0.0 {
            d.y * progress * progress
        } else {
            d.y - d.y * (1.0 - progress) * (1.0 - progress)
        };
        let (x, z) = (d.x * progress, d.z * progress);
        let local = if i % 2 == 0 {
            Vec3::new(x - dx_off, y + fudge, z + dz_off)
        } else {
            Vec3::new(x + dx_off, y + WIDTH - fudge, z - dz_off)
        };
        verts[i] = (start + local).to_array();
        let stripe = stripes[usize::from(k % 2 != usize::from(!forward))];
        let lit = light[k];
        colors[i] = [
            stripe[0] * lit[0],
            stripe[1] * lit[1],
            stripe[2] * lit[2],
            1.0,
        ];
    }
    for t in 0..STRIP_VERTS - 2 {
        pos.extend_from_slice(&verts[t..t + 3]);
        col.extend_from_slice(&[colors[t + 2]; 3]);
    }
}

fn write_mesh(mesh: &mut Mesh, pos: &[[f32; 3]], col: &[[f32; 4]]) {
    use VertexAttributeValues as V;
    if let Some(V::Float32x3(v)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
        v.clear();
        v.extend_from_slice(pos);
    }
    if let Some(V::Float32x3(v)) = mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL) {
        v.clear();
        v.resize(pos.len(), [0.0, 1.0, 0.0]);
    }
    if let Some(V::Float32x2(v)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
        v.clear();
        v.resize(pos.len(), [0.0; 2]);
    }
    if let Some(V::Float32x4(v)) = mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR) {
        v.clear();
        v.extend_from_slice(col);
    }
}

fn new_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per input and store it writes"
)]
pub(crate) fn sync_leashes(
    mut commands: Commands,
    view: Res<crate::renderer::FrameView>,
    shared: Res<crate::renderer::systems::Shared>,
    camera_view: Res<crate::renderer::input::CameraView>,
    lightmap: Res<crate::renderer::systems::LightmapState>,
    origin: Res<crate::renderer::RenderOrigin>,
    mut leashes: ResMut<Leashes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<EntityMaterial>>,
    mut nodes: Query<&mut Visibility, With<LeashNode>>,
) {
    crate::prof_span!("render:sync_leashes");
    let origin = *origin;
    let partial = view.partial;
    let Leashes {
        material,
        node,
        players,
        pos,
        col,
        written_pos,
        written_col,
    } = &mut *leashes;
    pos.clear();
    col.clear();

    if view
        .entities
        .iter()
        .any(|anim| anim.leash_holder().is_some())
    {
        let local_id = crate::client::tracking::local_entity_id();

        players.clear();
        let mut session = None;
        for id in view.entities.iter().filter_map(EntityAnim::leash_holder) {
            let known = id == local_id
                || view.entities.iter().any(|a| a.id == id)
                || players.iter().any(|(p, _)| *p == id);
            if known {
                continue;
            }
            let s = session.get_or_insert_with(|| shared.0.lock().unwrap());
            if let Some(p) = s.session.other_players.iter().find(|p| p.id == id) {
                let sample = p.anim.sample(partial);
                players.push((
                    id,
                    Holder::player(origin, p.anim.position(partial), &sample, &p.body, None),
                ));
            }
        }
        drop(session);

        let light = crate::client::worldsync::light_map().read();
        let state = &lightmap.current;
        let first_person = camera_view.first_person();
        for anim in view.entities.iter() {
            let Some(id) = anim.leash_holder() else {
                continue;
            };
            let holder = view
                .entities
                .iter()
                .find(|a| a.id == id)
                .map(|a| Holder::mob(origin, &a.sample(partial), &light))
                .or_else(|| {
                    (id == local_id).then(|| {
                        let look = first_person.then_some((
                            -view.camera_yaw - 180.0,
                            -view.camera_pitch,
                            view.eye_height,
                        ));
                        Holder::player(
                            origin,
                            view.player_lerp,
                            &view.local,
                            &view.local_body,
                            look,
                        )
                    })
                })
                .or_else(|| players.iter().find(|(p, _)| *p == id).map(|(_, h)| *h));
            let Some(holder) = holder else { continue };

            let st = anim.sample(partial);
            let (start_block, start_sky) = super::entity_levels(&light, &st);
            let (end_block, end_sky) = holder
                .levels
                .unwrap_or_else(|| levels_in(&light, cell_of(holder.eye)));
            let mut lit = [[0.0f32; 3]; STEPS + 1];
            for (k, slot) in lit.iter_mut().enumerate() {
                let t = k as f32 / STEPS as f32;
                let lerp = |a: u8, b: u8| (f32::from(a) + t * (f32::from(b) - f32::from(a))) as u8;
                *slot = light_color(
                    state,
                    lerp(start_block, end_block),
                    lerp(start_sky, end_sky),
                );
            }

            let feet = origin.to_render_array(st.pos);
            let size = [st.bounding_box_width, st.bounding_box_height];
            match (holder.quad, quad_params(st.kind)) {
                (Some(theirs), Some(mine)) => {
                    let (mine, theirs) =
                        (quad_offsets(size, mine), quad_offsets(holder.size, theirs));
                    for (m, t) in mine.into_iter().zip(theirs) {
                        let start = feet + y_rot(m, st.body_rot);
                        let end = holder.pos + y_rot(t, holder.yaw);
                        push_rope(pos, col, start, end, false, &lit);
                    }
                }
                _ => {
                    let start = feet + y_rot(leash_offset(&st), st.body_rot);
                    push_rope(pos, col, start, holder.rope, true, &lit);
                }
            }
        }
    }

    if pos.is_empty() {
        if let Some((entity, _)) = node {
            if let Ok(mut vis) = nodes.get_mut(*entity) {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
        return;
    }
    if node.is_none() {
        written_pos.clear();
        written_col.clear();
    }
    let (entity, handle) = node.get_or_insert_with(|| {
        let handle = meshes.add(new_mesh());
        let material = material
            .get_or_insert_with(|| {
                let lit = Lit::vertex_lit();
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
        let mut spawned = commands.spawn((
            Mesh3d(handle.clone()),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            Visibility::Inherited,
            NoFrustumCulling,
            LeashNode,
        ));
        #[cfg(feature = "builtin_shaders")]
        spawned.insert(bevy::light::NotShadowCaster);
        (spawned.id(), handle)
    });
    use crate::util::same_bits;
    let changed = !same_bits(&pos[..], &written_pos[..]) || !same_bits(&col[..], &written_col[..]);
    if changed && let Some(mut mesh) = meshes.get_mut(&*handle) {
        write_mesh(&mut mesh, pos, col);
        std::mem::swap(pos, written_pos);
        std::mem::swap(col, written_col);
    }
    if let Ok(mut vis) = nodes.get_mut(*entity) {
        vis.set_if_neq(Visibility::Inherited);
    }
}

pub(crate) fn clear_leashes(mut commands: Commands, mut leashes: ResMut<Leashes>) {
    if let Some((entity, _)) = leashes.node.take() {
        commands.entity(entity).despawn();
    }
}
