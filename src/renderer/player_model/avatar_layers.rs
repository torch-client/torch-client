use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;

use super::{ItemCtx, LayerQuery, MODEL_UNITS_PER_BLOCK, PART_HEAD, RigPart, player_parts};
use crate::entities::geom::{
    self, BakedModel, Grow, LayerDef, MeshDef, PartPose, PartState, PosedPart,
};
use crate::renderer::anim::AnimSample;
use crate::renderer::entity_material::{EntityMaterial, LightMode, Lit};
use crate::util::mth::{DEG_TO_RAD, wrap_degrees};

const SPIN_TEXTURE: &str = "entity/trident/trident_riptide";
const ARROW_TEXTURE: &str = "entity/projectiles/arrow";
const STINGER_TEXTURE: &str = "entity/bee/bee_stinger";

const SPIN_BOXES: [&str; 2] = ["box0", "box1"];

const BODY_PARTS: [usize; 6] = [0, 1, 3, 2, 5, 4];

const ON_SHOULDER: u8 = crate::entities::models::aquatic::parrot::ParrotPose::OnShoulder as u8;

fn spin_attack_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        for (i, name) in SPIN_BOXES.iter().enumerate() {
            let y_offset = -3.2 + 9.6 * (i + 1) as f32;
            let scale = 0.75 * (i + 1) as f32;
            root.part(
                name,
                (0, 0),
                [-8.0, -16.0 + y_offset, -8.0, 16.0, 32.0, 16.0],
                PartPose::ZERO.with_scale(scale),
            );
        }
    }
    LayerDef::create(mesh, 64, 64)
}

fn ears_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        let box_ = [-3.0, -6.0, -1.0, 6.0, 6.0, 1.0];
        root.part_grow(
            "left_ear",
            (24, 0),
            box_,
            Grow::all(1.0),
            PartPose::offset(-6.0, -6.0, 0.0),
        );
        root.part_grow(
            "right_ear",
            (24, 0),
            box_,
            Grow::all(1.0),
            PartPose::offset(6.0, -6.0, 0.0),
        );
    }
    LayerDef::create(mesh, 64, 64)
}

fn rest_mesh(layer: &LayerDef) -> Mesh {
    let model = geom::bake(layer);
    let states = model.rest_pose();
    let mut posed = Vec::new();
    model.pose_parts(&states, Mat4::IDENTITY, &mut posed);
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    crate::entities::bake::layer(&mut mesh, &model, &posed, &states);
    mesh
}

pub(super) struct LayerAssets {
    spin: BakedModel,
    parrot: BakedModel,
    arrow: Handle<Mesh>,
    stinger: Handle<Mesh>,
    ears: Handle<Mesh>,
}

impl LayerAssets {
    pub(super) fn new(meshes: &mut Assets<Mesh>) -> LayerAssets {
        LayerAssets {
            spin: geom::bake(&spin_attack_layer()),
            parrot: geom::bake(&crate::entities::models::aquatic::parrot::layer()),
            arrow: meshes.add(rest_mesh(
                &crate::entities::models::objects::projectile::arrow_layer(),
            )),
            stinger: meshes.add(rest_mesh(
                &crate::entities::models::aquatic::bee::stinger_layer(),
            )),
            ears: meshes.add(rest_mesh(&ears_layer())),
        }
    }
}

pub(super) fn bake_into(
    node: Entity,
    mesh: &mut Option<Handle<Mesh>>,
    model: &BakedModel,
    posed: &[PosedPart],
    states: &[PartState],
    meshes: &mut Assets<Mesh>,
    commands: &mut Commands,
) {
    if let Some(handle) = mesh {
        if let Some(mut mesh) = meshes.get_mut(handle) {
            crate::entities::bake::layer(&mut mesh, model, posed, states);
        }
    } else {
        let mut fresh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        crate::entities::bake::layer(&mut fresh, model, posed, states);
        let handle = meshes.add(fresh);
        commands.entity(node).insert(Mesh3d(handle.clone()));
        *mesh = Some(handle);
    }
}

struct BakedNode {
    node: Entity,
    mesh: Option<Handle<Mesh>>,
    material: Handle<EntityMaterial>,
    texture: Option<&'static str>,
    loaded: bool,
    states: Vec<PartState>,
    posed: Vec<PosedPart>,
}

impl BakedNode {
    fn show(
        &mut self,
        texture: Option<&'static str>,
        items: &mut ItemCtx,
        layers: &mut LayerQuery<'_, '_>,
    ) -> bool {
        if self.texture != texture {
            self.texture = texture;
            let image = texture.and_then(|path| items.entity_images.get(path, items.images));
            self.loaded = image.is_some();
            if let Some(mut material) = items.entity_materials.get_mut(&self.material) {
                material.texture = image;
            }
            if let Ok((mut vis, _)) = layers.get_mut(self.node) {
                *vis = if self.loaded {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
        self.loaded
    }

    fn reset(&mut self, model: &BakedModel) {
        self.states.clear();
        self.states
            .extend(model.parts.iter().map(|p| PartState::from(p.initial)));
    }

    fn bake(
        &mut self,
        model: &BakedModel,
        base: Mat4,
        items: &mut ItemCtx,
        commands: &mut Commands,
    ) {
        model.pose_parts(&self.states, base, &mut self.posed);
        bake_into(
            self.node,
            &mut self.mesh,
            model,
            &self.posed,
            &self.states,
            items.meshes,
            commands,
        );
    }
}

struct StuckRig {
    placed: Option<(i32, u32, u32, bool)>,
    nodes: Vec<Entity>,
    arrow: Handle<EntityMaterial>,
    stinger: Handle<EntityMaterial>,
}

pub(super) struct LayersRig {
    spin: BakedNode,
    parrots: [BakedNode; 2],
    ears: Entity,
    ears_shown: bool,
    stuck: StuckRig,
    lit: Lit,
}

pub(super) fn spawn(
    commands: &mut Commands,
    assets: &LayerAssets,
    materials: &mut Assets<EntityMaterial>,
    flip: Entity,
    parts: &[Entity; 6],
    skin: &Handle<EntityMaterial>,
    lit: Lit,
) -> LayersRig {
    let mut baked = |name: &'static str| {
        let material = materials.add(EntityMaterial::cutout(None, LightMode::Cardinal, lit));
        let node = commands
            .spawn((
                MeshMaterial3d(material.clone()),
                Transform::default(),
                Visibility::Hidden,
                bevy::camera::primitives::Aabb::from_min_max(Vec3::splat(-64.0), Vec3::splat(64.0)),
                RigPart,
                Name::new(name),
            ))
            .id();
        commands.entity(flip).add_child(node);
        BakedNode {
            node,
            mesh: None,
            material,
            texture: None,
            loaded: false,
            states: Vec::new(),
            posed: Vec::new(),
        }
    };
    let spin = baked("spin_attack");
    let parrots = [
        baked("left_shoulder_parrot"),
        baked("right_shoulder_parrot"),
    ];

    let ears = commands
        .spawn((
            Mesh3d(assets.ears.clone()),
            MeshMaterial3d(skin.clone()),
            Transform::from_scale(Vec3::splat(MODEL_UNITS_PER_BLOCK)),
            Visibility::Hidden,
            RigPart,
            Name::new("ears"),
        ))
        .id();
    commands.entity(parts[PART_HEAD]).add_child(ears);

    let stuck_material = |materials: &mut Assets<EntityMaterial>| {
        materials.add(EntityMaterial::cutout(None, LightMode::Cardinal, lit))
    };
    LayersRig {
        spin,
        parrots,
        ears,
        ears_shown: false,
        stuck: StuckRig {
            placed: None,
            nodes: Vec::new(),
            arrow: stuck_material(materials),
            stinger: stuck_material(materials),
        },
        lit,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    rig: &mut LayersRig,
    parts: &[Entity; 6],
    sample: &AnimSample,
    slim: bool,
    lit: Lit,
    items: &mut ItemCtx,
    layers: &mut LayerQuery<'_, '_>,
    commands: &mut Commands,
) {
    let assets = items.layers;

    if rig
        .spin
        .show(sample.spin_attack.then_some(SPIN_TEXTURE), items, layers)
    {
        let model = &assets.spin;
        rig.spin.reset(model);
        for (i, name) in SPIN_BOXES.iter().enumerate() {
            let angle = sample.age_ticks * -(45 + (i as i32 + 1) * 5) as f32;
            rig.spin.states[model.id(name)].y_rot = wrap_degrees(angle) * DEG_TO_RAD;
        }
        rig.spin.bake(
            model,
            Mat4::from_scale(Vec3::splat(MODEL_UNITS_PER_BLOCK)),
            items,
            commands,
        );
    }

    for (index, node) in rig.parrots.iter_mut().enumerate() {
        let left = index == 0;
        let variant = sample.layers.parrots[index];
        let texture = variant
            .map(|id| crate::entities::render::aquatic::parrot::variant_texture(i64::from(id)));
        if !node.show(texture, items, layers) {
            continue;
        }
        let mut st = crate::entities::state::EntityState::new(
            sample.layers.id,
            azalea_registry::builtin::EntityKind::Parrot,
        );
        st.extras.parrot_pose = ON_SHOULDER;
        st.age_ticks = sample.age_ticks;
        st.walk_pos = sample.walk_pos;
        st.walk_speed = sample.walk_speed;
        (st.x_rot, st.y_rot) = sample.render_rot();
        let model = &assets.parrot;
        node.reset(model);
        crate::entities::models::aquatic::parrot::setup_anim(model, &mut node.states, &st);
        let shoulder = Vec3::new(
            if left { 0.4 } else { -0.4 },
            if sample.crouching { -1.3 } else { -1.5 },
            0.0,
        );
        let base =
            Mat4::from_scale(Vec3::splat(MODEL_UNITS_PER_BLOCK)) * Mat4::from_translation(shoulder);
        node.bake(model, base, items, commands);
    }

    if rig.ears_shown != sample.layers.extra_ears
        && let Ok((mut vis, _)) = layers.get_mut(rig.ears)
    {
        rig.ears_shown = sample.layers.extra_ears;
        *vis = if rig.ears_shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    update_stuck(&mut rig.stuck, parts, sample, slim, items, commands);

    if lit != rig.lit {
        for handle in [&rig.spin.material, &rig.stuck.arrow, &rig.stuck.stinger]
            .into_iter()
            .chain(rig.parrots.iter().map(|node| &node.material))
        {
            if let Some(mut material) = items.entity_materials.get_mut(handle) {
                material.params.set(lit);
            }
        }
        rig.lit = lit;
    }
}

fn update_stuck(
    stuck: &mut StuckRig,
    parts: &[Entity; 6],
    sample: &AnimSample,
    slim: bool,
    items: &mut ItemCtx,
    commands: &mut Commands,
) {
    let layers = sample.layers;
    let key = (layers.id, layers.arrows, layers.stingers, slim);
    if stuck.placed == Some(key) {
        return;
    }
    stuck.placed = Some(key);
    for node in stuck.nodes.drain(..) {
        commands.entity(node).despawn();
    }
    let assets = items.layers;
    for (count, mesh, material, texture, on_surface) in [
        (
            layers.arrows,
            &assets.arrow,
            stuck.arrow.clone(),
            ARROW_TEXTURE,
            false,
        ),
        (
            layers.stingers,
            &assets.stinger,
            stuck.stinger.clone(),
            STINGER_TEXTURE,
            true,
        ),
    ] {
        if count == 0 {
            continue;
        }
        if let Some(mut m) = items.entity_materials.get_mut(&material)
            && m.texture.is_none()
        {
            m.texture = items.entity_images.get(texture, items.images);
        }
        for (part, local) in placements(layers.id, count, slim, on_surface) {
            let node = commands
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    local,
                    Visibility::Inherited,
                    RigPart,
                    Name::new(texture),
                ))
                .id();
            commands.entity(parts[part]).add_child(node);
            stuck.nodes.push(node);
        }
    }
}

fn placements(id: i32, count: u32, slim: bool, on_surface: bool) -> Vec<(usize, Transform)> {
    let cubes = player_parts(slim);
    let mut random = crate::util::javarandom::JavaRandom::new(i64::from(id));
    (0..count)
        .map(|_| {
            let part = BODY_PARTS[random.next_int(BODY_PARTS.len() as u32) as usize];
            let _ = random.next_int(1);
            let cube = &cubes[part];
            let mut mid = [random.next_f32(), random.next_f32(), random.next_f32()];
            if on_surface {
                let plane = random.next_int(3) as usize;
                mid[plane] = snap_to_face(mid[plane]);
            }
            let at = Vec3::from_array(std::array::from_fn(|i| {
                cube.origin[i] + mid[i] * cube.size[i]
            }));
            let [dx, dy, dz] = mid.map(|m| -(m * 2.0 - 1.0));
            let dxz = (dx * dx + dz * dz).sqrt();
            let y_rot = (f64::from(dx).atan2(f64::from(dz)) * 57.295_776_367_187_5) as f32;
            let x_rot = (f64::from(dy).atan2(f64::from(dxz)) * 57.295_776_367_187_5) as f32;
            let rotation = Quat::from_rotation_y((y_rot - 90.0) * DEG_TO_RAD)
                * Quat::from_rotation_z(x_rot * DEG_TO_RAD);
            (
                part,
                Transform {
                    translation: at,
                    rotation,
                    scale: Vec3::splat(MODEL_UNITS_PER_BLOCK),
                },
            )
        })
        .collect()
}

fn snap_to_face(value: f32) -> f32 {
    if value > 0.5 { 1.0 } else { 0.5 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placements_are_seeded_by_id() {
        let a = placements(42, 5, false, false);
        let b = placements(42, 5, false, false);
        assert_eq!(a.len(), 5);
        for ((pa, ta), (pb, tb)) in a.iter().zip(&b) {
            assert_eq!(pa, pb);
            assert_eq!(ta, tb);
            assert!(*pa < 6);
        }
    }

    #[test]
    fn stingers_snap_to_a_face() {
        for (part, t) in placements(7, 8, false, true) {
            let cube = &player_parts(false)[part];
            let on_face = (0..3).any(|i| {
                let f = (t.translation[i] - cube.origin[i]) / cube.size[i];
                (f - 1.0).abs() < 1e-5 || (f - 0.5).abs() < 1e-5
            });
            assert!(on_face);
        }
    }
}
