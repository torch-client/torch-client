use azalea_registry::builtin::EntityKind;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;

use super::state::{EntityState, TargetBody};
use super::{Blend, EntityImages};
use crate::renderer::FrameView;
use crate::renderer::entity_material::{EntityMaterial, LightMode, Lit, diffuse_lights};
use crate::renderer::lightmap::cell_light;
use crate::util::mth::rgb_to_linear;

const GUARDIAN_BEAM: &str = "entity/guardian/guardian_beam";
const CRYSTAL_BEAM: &str = "entity/end_crystal/end_crystal_beam";
const POOL_SLACK: usize = 4;

#[derive(Component)]
pub(crate) struct BeamNode;

struct Node {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<EntityMaterial>,
    texture: AssetId<Image>,
}

#[derive(Resource, Default)]
pub(crate) struct Beams {
    nodes: Vec<Node>,
}

#[derive(Default)]
struct Strip {
    pos: Vec<[f32; 3]>,
    normal: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    color: Vec<[f32; 4]>,
}

impl Strip {
    fn quad(&mut self, pose: &Mat4, normal: Vec3, corners: [([f32; 3], [f32; 2], [f32; 4]); 4]) {
        let normal = pose
            .transform_vector3(normal)
            .normalize_or_zero()
            .to_array();
        for i in [0, 1, 2, 0, 2, 3] {
            let (p, uv, color) = corners[i];
            self.pos
                .push(pose.transform_point3(Vec3::from(p)).to_array());
            self.normal.push(normal);
            self.uv.push(uv);
            self.color.push(color);
        }
    }

    fn into_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normal);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uv);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.color);
        mesh
    }
}

fn is_guardian(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::Guardian | EntityKind::ElderGuardian)
}

fn local_body(view: &FrameView) -> TargetBody {
    TargetBody {
        feet: view.player_lerp,
        height: view.local_body.bounding_box[1],
        eye_height: view.eye_height,
    }
}

fn attack_target(
    st: &EntityState,
    view: &FrameView,
    find: impl FnOnce(i32) -> Option<TargetBody>,
) -> Option<TargetBody> {
    let id = st.extras.attack_target;
    if id == 0 {
        return None;
    }
    if id == crate::client::tracking::local_entity_id() {
        return Some(local_body(view));
    }
    find(id).or(st.extras.attack_target_body)
}

pub(crate) fn aim_guardians(frame: &mut [EntityState], view: &FrameView) {
    for i in 0..frame.len() {
        if !is_guardian(frame[i].kind) {
            continue;
        }
        let look = {
            let st = &frame[i];
            if st.extras.attack_target == 0 {
                Some(local_body(view).eye())
            } else {
                let frame = &*frame;
                attack_target(st, view, |id| {
                    frame
                        .iter()
                        .find(|other| other.id == id)
                        .map(|other| TargetBody {
                            feet: other.pos,
                            height: other.bounding_box_height,
                            eye_height: other.eye_height,
                        })
                })
                .map(|target| target.eye())
            }
        };
        frame[i].extras.look_at = look;
    }
}

fn guardian_beam(out: &mut Strip, eye: Vec3, beam: Vec3, attack_time: f32, scale: f32) {
    use std::f32::consts::{FRAC_PI_2, PI};

    let length = beam.length() + 1.0;
    let dir = beam.normalize_or_zero();
    let x_rot = dir.y.clamp(-1.0, 1.0).acos();
    let y_rot = FRAC_PI_2 - dir.z.atan2(dir.x);
    let pose =
        Mat4::from_translation(eye) * Mat4::from_rotation_y(y_rot) * Mat4::from_rotation_x(x_rot);

    let rot = attack_time * 0.05 * -1.5;
    let color_scale = scale * scale;
    let red = 64 + (color_scale * 191.0) as i32;
    let green = 32 + (color_scale * 191.0) as i32;
    let blue = 128 - (color_scale * 64.0) as i32;
    let rgb = |c: i32| (c & 0xFF) as u32;
    let color = rgb_to_linear((rgb(red) << 16) | (rgb(green) << 8) | rgb(blue));

    let at =
        |offset: f32, radius: f32| ((rot + offset).cos() * radius, (rot + offset).sin() * radius);
    let (wnx, wnz) = at(2.356_194_5, 0.282);
    let (enx, enz) = at(0.785_398_2, 0.282);
    let (wsx, wsz) = at(3.926_991, 0.282);
    let (esx, esz) = at(5.497_787_5, 0.282);
    let (wx, wz) = at(PI, 0.2);
    let (ex, ez) = at(0.0, 0.2);
    let (nx, nz) = at(FRAC_PI_2, 0.2);
    let (sx, sz) = at(4.712_389, 0.2);

    let top = length;
    let tex_v_off = attack_time * 0.5 % 1.0;
    let min_v = -1.0 + tex_v_off;
    let max_v = min_v + length * 2.5;
    let up = Vec3::Y;
    out.quad(
        &pose,
        up,
        [
            ([wx, top, wz], [0.4999, max_v], color),
            ([wx, 0.0, wz], [0.4999, min_v], color),
            ([ex, 0.0, ez], [0.0, min_v], color),
            ([ex, top, ez], [0.0, max_v], color),
        ],
    );
    out.quad(
        &pose,
        up,
        [
            ([nx, top, nz], [0.4999, max_v], color),
            ([nx, 0.0, nz], [0.4999, min_v], color),
            ([sx, 0.0, sz], [0.0, min_v], color),
            ([sx, top, sz], [0.0, max_v], color),
        ],
    );
    let v_base = if (attack_time.floor() as i32) % 2 == 0 {
        0.5
    } else {
        0.0
    };
    out.quad(
        &pose,
        up,
        [
            ([wnx, top, wnz], [0.5, v_base + 0.5], color),
            ([enx, top, enz], [1.0, v_base + 0.5], color),
            ([esx, top, esz], [1.0, v_base], color),
            ([wsx, top, wsz], [0.5, v_base], color),
        ],
    );
}

fn crystal_beam(out: &mut Strip, at: Vec3, delta: Vec3, time_in_ticks: f32) {
    use std::f32::consts::{FRAC_PI_2, TAU};

    let horizontal = (delta.x * delta.x + delta.z * delta.z).sqrt();
    let length = delta.length();
    let pose = Mat4::from_translation(at + Vec3::new(0.0, 2.0, 0.0))
        * Mat4::from_rotation_y(-delta.z.atan2(delta.x) - FRAC_PI_2)
        * Mat4::from_rotation_x(-horizontal.atan2(delta.y) - FRAC_PI_2);
    let v0 = 0.0 - time_in_ticks * 0.01;
    let v1 = length / 32.0 - time_in_ticks * 0.01;
    let black = [0.0, 0.0, 0.0, 1.0];
    let white = [1.0; 4];
    let down = Vec3::NEG_Y;

    let (mut last_sin, mut last_cos, mut last_u) = (0.0f32, 0.75f32, 0.0f32);
    for i in 1..=8 {
        let angle = i as f32 * TAU / 8.0;
        let sin = angle.sin() * 0.75;
        let cos = angle.cos() * 0.75;
        let u = i as f32 / 8.0;
        out.quad(
            &pose,
            down,
            [
                ([last_sin * 0.2, last_cos * 0.2, 0.0], [last_u, v0], black),
                ([last_sin, last_cos, length], [last_u, v1], white),
                ([sin, cos, length], [u, v1], white),
                ([sin * 0.2, cos * 0.2, 0.0], [u, v0], black),
            ],
        );
        last_sin = sin;
        last_cos = cos;
        last_u = u;
    }
}

fn block_center(pos: [i32; 3]) -> [f64; 3] {
    pos.map(|c| f64::from(c) + 0.5)
}

struct Drawn {
    texture: &'static str,
    levels: (u8, u8),
    strip: Strip,
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per input and store it writes"
)]
pub(crate) fn sync_beams(
    mut commands: Commands,
    view: Res<FrameView>,
    lightmap: Res<crate::renderer::systems::LightmapState>,
    origin: Res<crate::renderer::RenderOrigin>,
    mut beams: ResMut<Beams>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut entity_images: ResMut<EntityImages>,
    mut materials: ResMut<Assets<EntityMaterial>>,
    mut nodes: Query<&mut Visibility, With<BeamNode>>,
) {
    crate::prof_span!("render:sync_beams");
    let origin = *origin;
    let partial = view.partial;
    let mut drawn: Vec<Drawn> = Vec::new();

    let wanted = |a: &super::feed::EntityAnim| match a.kind {
        EntityKind::Guardian | EntityKind::ElderGuardian => a.shared().attack_target != 0,
        EntityKind::EndCrystal => a.shared().beam_target.is_some(),
        EntityKind::EnderDragon => true,
        _ => false,
    };
    if view.entities.iter().any(wanted) {
        let light = crate::client::worldsync::light_map().read();
        let crystals: Vec<EntityState> = view
            .entities
            .iter()
            .filter(|a| a.kind == EntityKind::EndCrystal)
            .map(|a| a.sample(partial))
            .collect();

        for anim in view.entities.iter().filter(|&a| wanted(a)) {
            match anim.kind {
                EntityKind::Guardian | EntityKind::ElderGuardian => {
                    let st = anim.sample(partial);
                    let Some(target) = attack_target(&st, &view, |id| {
                        view.entities
                            .iter()
                            .find(|a| a.id == id)
                            .map(|a| TargetBody {
                                feet: a.position(partial),
                                height: a.bounding_box_height(),
                                eye_height: a.eye_height(),
                            })
                    }) else {
                        continue;
                    };
                    let eye_world = [st.pos[0], st.pos[1] + f64::from(st.eye_height), st.pos[2]];
                    let beam =
                        Vec3::from_array(crate::util::offset_f32(eye_world, target.middle()));
                    let eye = origin.to_render_array(eye_world);
                    let duration = super::feed::guardian_attack_duration(st.kind) as f32;
                    let attack_time = st.extras.guardian_attack_time;
                    let mut strip = Strip::default();
                    guardian_beam(&mut strip, eye, beam, attack_time, attack_time / duration);
                    drawn.push(Drawn {
                        texture: GUARDIAN_BEAM,
                        levels: (15, 15),
                        strip,
                    });
                }
                EntityKind::EndCrystal => {
                    let Some(st) = crystals.iter().find(|c| c.id == anim.id) else {
                        continue;
                    };
                    let Some(block) = st.extras.beam_target else {
                        continue;
                    };
                    let center = block_center(block);
                    let back = Vec3::from_array(crate::util::offset_f32(center, st.pos));
                    let crystal_y = super::models::objects::misc::crystal_y(st.age_ticks);
                    let mut strip = Strip::default();
                    crystal_beam(
                        &mut strip,
                        origin.to_render_array(center),
                        back + Vec3::new(0.0, crystal_y, 0.0),
                        st.age_ticks,
                    );
                    drawn.push(Drawn {
                        texture: CRYSTAL_BEAM,
                        levels: super::entity_levels(&light, st),
                        strip,
                    });
                }
                EntityKind::EnderDragon => {
                    let st = anim.sample(partial);
                    let Some(crystal) = st
                        .extras
                        .dragon
                        .as_ref()
                        .and_then(|d| d.nearest_crystal)
                        .and_then(|id| crystals.iter().find(|c| c.id == id))
                    else {
                        continue;
                    };
                    let crystal_y = super::models::objects::misc::crystal_y(crystal.age_ticks);
                    let offset = Vec3::from_array(crate::util::offset_f32(st.pos, crystal.pos))
                        + Vec3::new(0.0, crystal_y, 0.0);
                    let mut strip = Strip::default();
                    crystal_beam(
                        &mut strip,
                        origin.to_render_array(st.pos),
                        offset,
                        st.age_ticks,
                    );
                    drawn.push(Drawn {
                        texture: CRYSTAL_BEAM,
                        levels: super::entity_levels(&light, &st),
                        strip,
                    });
                }
                _ => {}
            }
        }
    }

    let dirs = diffuse_lights(crate::renderer::dimension::current());
    let (alpha_mode, cutoff) = Blend::Cutout.alpha();
    let mut shown = 0;
    for beam in drawn {
        let Some(texture) = entity_images.tiled(beam.texture, &mut images) else {
            continue;
        };
        let lit = Lit {
            tint: [1.0; 4],
            light: cell_light(&lightmap.current, beam.levels.0, beam.levels.1),
            dirs,
            overlay_alpha: 1.0,
            overlay_white: false,
        };
        let mesh = beam.strip.into_mesh();
        if let Some(node) = beams.nodes.get_mut(shown) {
            if let Some(mut slot) = meshes.get_mut(&node.mesh) {
                *slot = mesh;
            }
            if node.texture != texture.id()
                && let Some(mut material) = materials.get_mut(&node.material)
            {
                material.texture = Some(texture.clone());
                node.texture = texture.id();
            }
            EntityMaterial::relight(&mut materials, &node.material, lit);
            if let Ok(mut vis) = nodes.get_mut(node.entity) {
                vis.set_if_neq(Visibility::Inherited);
            }
        } else {
            let mesh = meshes.add(mesh);
            let material = materials.add(EntityMaterial::new(
                Some(texture.clone()),
                LightMode::Cardinal,
                alpha_mode,
                cutoff,
                lit,
                0,
            ));
            #[cfg_attr(not(feature = "builtin_shaders"), allow(unused_mut))]
            let mut spawned = commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::IDENTITY,
                Visibility::Inherited,
                NoFrustumCulling,
                BeamNode,
            ));
            #[cfg(feature = "builtin_shaders")]
            spawned.insert(bevy::light::NotShadowCaster);
            let entity = spawned.id();
            beams.nodes.push(Node {
                entity,
                mesh,
                material,
                texture: texture.id(),
            });
        }
        shown += 1;
    }

    for node in beams.nodes.iter().skip(shown) {
        if let Ok(mut vis) = nodes.get_mut(node.entity) {
            vis.set_if_neq(Visibility::Hidden);
        }
    }
    if beams.nodes.len() > shown + POOL_SLACK {
        for node in beams.nodes.drain(shown + POOL_SLACK..) {
            commands.entity(node.entity).despawn();
        }
    }
}

pub(crate) fn clear_beams(mut commands: Commands, mut beams: ResMut<Beams>) {
    for node in beams.nodes.drain(..) {
        commands.entity(node.entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vertical_guardian_beam_is_its_length_plus_one_tall() {
        let mut strip = Strip::default();
        guardian_beam(&mut strip, Vec3::ZERO, Vec3::new(0.0, 4.0, 0.0), 0.0, 0.0);
        assert_eq!(strip.pos.len(), 18);
        let top = strip.pos.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        assert!((top - 5.0).abs() < 1e-4, "{top}");
    }

    #[test]
    fn a_crystal_beam_reaches_its_far_end() {
        let mut strip = Strip::default();
        let delta = Vec3::new(3.0, -2.0, 4.0);
        crystal_beam(&mut strip, Vec3::ZERO, delta, 0.0);
        assert_eq!(strip.pos.len(), 8 * 6);
        let far = strip
            .pos
            .iter()
            .map(|p| Vec3::from(*p))
            .fold(Vec3::ZERO, |acc, p| {
                if (p - Vec3::Y * 2.0).length() > (acc - Vec3::Y * 2.0).length() {
                    p
                } else {
                    acc
                }
            });
        let reach = (far - Vec3::Y * 2.0).length();
        assert!(
            reach <= delta.length() + 0.8 && reach >= delta.length() - 0.01,
            "{reach}"
        );
    }
}
