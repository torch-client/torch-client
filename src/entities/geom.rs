use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

pub use crate::direction::{Direction, Faces};

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Grow(pub f32, pub f32, pub f32);

impl Grow {
    pub const NONE: Grow = Grow(0.0, 0.0, 0.0);

    pub const fn all(grow: f32) -> Grow {
        Grow(grow, grow, grow)
    }

    pub const fn extend(self, factor: f32) -> Grow {
        Grow(self.0 + factor, self.1 + factor, self.2 + factor)
    }

    #[allow(
        dead_code,
        reason = "CubeDeformation.extend(x, y, z), kept so the builder matches the Java one to one"
    )]
    pub const fn extend_xyz(self, x: f32, y: f32, z: f32) -> Grow {
        Grow(self.0 + x, self.1 + y, self.2 + z)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PartPose {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub x_rot: f32,
    pub y_rot: f32,
    pub z_rot: f32,
    pub x_scale: f32,
    pub y_scale: f32,
    pub z_scale: f32,
}

impl Default for PartPose {
    fn default() -> Self {
        PartPose::ZERO
    }
}

impl PartPose {
    pub const ZERO: PartPose = PartPose {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        x_rot: 0.0,
        y_rot: 0.0,
        z_rot: 0.0,
        x_scale: 1.0,
        y_scale: 1.0,
        z_scale: 1.0,
    };

    pub const fn offset(x: f32, y: f32, z: f32) -> PartPose {
        PartPose {
            x,
            y,
            z,
            ..PartPose::ZERO
        }
    }

    pub const fn rotation(x_rot: f32, y_rot: f32, z_rot: f32) -> PartPose {
        PartPose {
            x_rot,
            y_rot,
            z_rot,
            ..PartPose::ZERO
        }
    }

    pub const fn offset_rotation(
        x: f32,
        y: f32,
        z: f32,
        x_rot: f32,
        y_rot: f32,
        z_rot: f32,
    ) -> PartPose {
        PartPose {
            x,
            y,
            z,
            x_rot,
            y_rot,
            z_rot,
            ..PartPose::ZERO
        }
    }

    pub const fn translated(self, x: f32, y: f32, z: f32) -> PartPose {
        PartPose {
            x: self.x + x,
            y: self.y + y,
            z: self.z + z,
            ..self
        }
    }

    pub const fn with_scale(self, scale: f32) -> PartPose {
        PartPose {
            x_scale: scale,
            y_scale: scale,
            z_scale: scale,
            ..self
        }
    }

    pub fn scaled(self, factor: f32) -> PartPose {
        if factor == 1.0 {
            self
        } else {
            self.scaled_xyz(factor, factor, factor)
        }
    }

    pub const fn scaled_xyz(self, sx: f32, sy: f32, sz: f32) -> PartPose {
        PartPose {
            x: self.x * sx,
            y: self.y * sy,
            z: self.z * sz,
            x_scale: self.x_scale * sx,
            y_scale: self.y_scale * sy,
            z_scale: self.z_scale * sz,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CubeDef {
    pub tex: [f32; 2],
    pub origin: [f32; 3],
    pub size: [f32; 3],
    pub grow: Grow,
    pub mirror: bool,
    pub tex_scale: [f32; 2],
    pub faces: Faces,
}

#[derive(Clone, Debug, Default)]
pub struct CubeList {
    cubes: Vec<CubeDef>,
    tex: [f32; 2],
    mirror: bool,
}

impl CubeList {
    pub fn new() -> CubeList {
        CubeList::default()
    }

    pub fn tex_offs(mut self, u: i32, v: i32) -> CubeList {
        self.tex = [u as f32, v as f32];
        self
    }

    pub fn mirror(self) -> CubeList {
        self.mirror_if(true)
    }

    pub fn mirror_if(mut self, mirror: bool) -> CubeList {
        self.mirror = mirror;
        self
    }

    pub fn add_box(self, x: f32, y: f32, z: f32, w: f32, h: f32, d: f32) -> CubeList {
        let mirror = self.mirror;
        self.push(x, y, z, w, h, d, Grow::NONE, mirror, [1.0, 1.0], Faces::ALL)
    }

    pub fn add_box_grow(
        self,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
        h: f32,
        d: f32,
        grow: Grow,
    ) -> CubeList {
        let mirror = self.mirror;
        self.push(x, y, z, w, h, d, grow, mirror, [1.0, 1.0], Faces::ALL)
    }

    pub fn add_box_mirror(
        self,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
        h: f32,
        d: f32,
        mirror: bool,
    ) -> CubeList {
        self.push(x, y, z, w, h, d, Grow::NONE, mirror, [1.0, 1.0], Faces::ALL)
    }

    pub fn add_box_faces(
        self,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
        h: f32,
        d: f32,
        faces: Faces,
    ) -> CubeList {
        let mirror = self.mirror;
        self.push(x, y, z, w, h, d, Grow::NONE, mirror, [1.0, 1.0], faces)
    }

    pub fn add_box_tex_scale(
        self,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
        h: f32,
        d: f32,
        grow: Grow,
        u_scale: f32,
        v_scale: f32,
    ) -> CubeList {
        let mirror = self.mirror;
        self.push(
            x,
            y,
            z,
            w,
            h,
            d,
            grow,
            mirror,
            [u_scale, v_scale],
            Faces::ALL,
        )
    }

    pub fn add_box_at(
        mut self,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
        h: f32,
        d: f32,
        grow: Grow,
        u: i32,
        v: i32,
    ) -> CubeList {
        self.tex = [u as f32, v as f32];
        let mirror = self.mirror;
        self.push(x, y, z, w, h, d, grow, mirror, [1.0, 1.0], Faces::ALL)
    }

    #[allow(clippy::too_many_arguments)]
    fn push(
        mut self,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
        h: f32,
        d: f32,
        grow: Grow,
        mirror: bool,
        tex_scale: [f32; 2],
        faces: Faces,
    ) -> CubeList {
        self.cubes.push(CubeDef {
            tex: self.tex,
            origin: [x, y, z],
            size: [w, h, d],
            grow,
            mirror,
            tex_scale,
            faces,
        });
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct PartDef {
    cubes: Vec<CubeDef>,
    pose: PartPose,
    children: Vec<(String, PartDef)>,
}

impl PartDef {
    fn new(cubes: Vec<CubeDef>, pose: PartPose) -> PartDef {
        PartDef {
            cubes,
            pose,
            children: Vec::new(),
        }
    }

    pub fn child(&mut self, name: &str, cubes: CubeList, pose: PartPose) -> &mut PartDef {
        let mut child = PartDef::new(cubes.cubes, pose);
        if let Some(idx) = self.children.iter().position(|(n, _)| n == name) {
            let (_, previous) = self.children.remove(idx);
            child.children = previous.children;
        }
        self.children.push((name.to_string(), child));
        let last = self.children.len() - 1;
        &mut self.children[last].1
    }

    #[allow(dead_code, reason = "PartDefinition methods no ported model needs yet")]
    pub fn child_def(&mut self, name: &str, mut child: PartDef) -> &mut PartDef {
        if let Some(idx) = self.children.iter().position(|(n, _)| n == name) {
            let (_, previous) = self.children.remove(idx);
            for (n, node) in previous.children {
                if !child.children.iter().any(|(existing, _)| *existing == n) {
                    child.children.push((n, node));
                }
            }
        }
        self.children.push((name.to_string(), child));
        let last = self.children.len() - 1;
        &mut self.children[last].1
    }

    #[allow(dead_code, reason = "PartDefinition methods no ported model needs yet")]
    pub fn detached(pose: PartPose) -> PartDef {
        PartDef::new(Vec::new(), pose)
    }

    pub fn get(&mut self, path: &str) -> &mut PartDef {
        let mut node = self;
        for name in path.split('/') {
            let idx = node
                .children
                .iter()
                .position(|(n, _)| n == name)
                .unwrap_or_else(|| panic!("no child with name: {name}"));
            node = &mut node.children[idx].1;
        }
        node
    }

    pub fn clear_child(&mut self, name: &str) -> &mut PartDef {
        let node = self.get(name);
        node.cubes.clear();
        node
    }

    pub fn transformed(&mut self, f: impl FnOnce(PartPose) -> PartPose) -> &mut PartDef {
        self.pose = f(self.pose);
        self
    }

    #[allow(dead_code, reason = "PartDefinition methods no ported model needs yet")]
    pub fn inflate(&mut self, grow: f32) -> &mut PartDef {
        for cube in &mut self.cubes {
            cube.grow = cube.grow.extend(grow);
        }
        for (_, child) in &mut self.children {
            child.inflate(grow);
        }
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct MeshDef {
    root: PartDef,
}

impl MeshDef {
    pub fn new() -> MeshDef {
        MeshDef {
            root: PartDef::new(Vec::new(), PartPose::ZERO),
        }
    }

    pub fn root(&mut self) -> &mut PartDef {
        &mut self.root
    }

    pub fn transformed(mut self, f: impl FnOnce(PartPose) -> PartPose) -> MeshDef {
        self.root.pose = f(self.root.pose);
        self
    }
}

#[derive(Clone, Debug)]
pub struct LayerDef {
    pub mesh: MeshDef,
    pub tex_w: u32,
    pub tex_h: u32,
}

impl LayerDef {
    pub fn create(mesh: MeshDef, tex_w: u32, tex_h: u32) -> LayerDef {
        LayerDef { mesh, tex_w, tex_h }
    }
}

pub type PartId = usize;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PartState {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub x_rot: f32,
    pub y_rot: f32,
    pub z_rot: f32,
    pub x_scale: f32,
    pub y_scale: f32,
    pub z_scale: f32,
    pub visible: bool,
    pub skip_draw: bool,
}

impl Default for PartState {
    fn default() -> Self {
        PartState::from(PartPose::ZERO)
    }
}

impl From<PartPose> for PartState {
    fn from(p: PartPose) -> PartState {
        PartState {
            x: p.x,
            y: p.y,
            z: p.z,
            x_rot: p.x_rot,
            y_rot: p.y_rot,
            z_rot: p.z_rot,
            x_scale: p.x_scale,
            y_scale: p.y_scale,
            z_scale: p.z_scale,
            visible: true,
            skip_draw: false,
        }
    }
}

impl PartState {
    pub fn load_pose(&mut self, p: PartPose) {
        let visible = self.visible;
        let skip_draw = self.skip_draw;
        *self = PartState::from(p);
        self.visible = visible;
        self.skip_draw = skip_draw;
    }

    pub fn set_pos(&mut self, x: f32, y: f32, z: f32) {
        self.x = x;
        self.y = y;
        self.z = z;
    }

    pub fn set_rotation(&mut self, x_rot: f32, y_rot: f32, z_rot: f32) {
        self.x_rot = x_rot;
        self.y_rot = y_rot;
        self.z_rot = z_rot;
    }

    pub fn offset_pos(&mut self, x: f32, y: f32, z: f32) {
        self.x += x;
        self.y += y;
        self.z += z;
    }

    pub fn offset_rotation(&mut self, x: f32, y: f32, z: f32) {
        self.x_rot += x;
        self.y_rot += y;
        self.z_rot += z;
    }

    pub fn offset_scale(&mut self, x: f32, y: f32, z: f32) {
        self.x_scale += x;
        self.y_scale += y;
        self.z_scale += z;
    }

    pub fn transform(&self) -> Transform {
        Transform {
            translation: Vec3::new(self.x / 16.0, self.y / 16.0, self.z / 16.0),
            rotation: Quat::from_rotation_z(self.z_rot)
                * Quat::from_rotation_y(self.y_rot)
                * Quat::from_rotation_x(self.x_rot),
            scale: Vec3::new(self.x_scale, self.y_scale, self.z_scale),
        }
    }
}

pub struct BakedPart {
    pub name: String,
    pub initial: PartPose,
    pub parent: Option<PartId>,
    pub children: Vec<PartId>,
    pub mesh: Option<Mesh>,
}

pub struct PartArrays<'a> {
    pub positions: &'a [[f32; 3]],
    pub normals: &'a [[f32; 3]],
    pub uvs: &'a [[f32; 2]],
    pub indices: &'a [u32],
}

impl BakedPart {
    pub fn arrays(&self) -> Option<PartArrays<'_>> {
        let mesh = self.mesh.as_ref()?;
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return None;
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            return None;
        };
        let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            return None;
        };
        let Some(Indices::U32(indices)) = mesh.indices() else {
            return None;
        };
        Some(PartArrays {
            positions,
            normals,
            uvs,
            indices,
        })
    }
}

#[derive(Clone, Copy)]
pub struct PosedPart {
    pub mat: Mat4,
    pub visible: bool,
}

pub struct BakedModel {
    pub parts: Vec<BakedPart>,
    lookup: HashMap<String, PartId>,
    #[allow(
        dead_code,
        reason = "LayerDefinition's declared texture size, carried for the models that will scale UVs by it"
    )]
    pub tex_size: [u32; 2],
}

impl BakedModel {
    pub fn id(&self, name: &str) -> PartId {
        match self.lookup.get(name) {
            Some(id) => *id,
            None => panic!("no part named {name} in this model"),
        }
    }

    pub fn find(&self, name: &str) -> Option<PartId> {
        self.lookup.get(name).copied()
    }

    pub fn rest_pose(&self) -> Vec<PartState> {
        self.parts
            .iter()
            .map(|p| PartState::from(p.initial))
            .collect()
    }

    pub fn pose_parts(&self, states: &[PartState], root: Mat4, out: &mut Vec<PosedPart>) {
        out.clear();
        out.reserve(self.parts.len());
        for (part, state) in self.parts.iter().zip(states) {
            let local = state.transform().to_matrix();
            let posed = match part.parent {
                Some(p) => {
                    let parent = out[p];
                    PosedPart {
                        mat: parent.mat * local,
                        visible: parent.visible && state.visible,
                    }
                }
                None => PosedPart {
                    mat: root * local,
                    visible: state.visible,
                },
            };
            out.push(posed);
        }
    }

    pub fn reset_pose(&self, states: &mut [PartState]) {
        for (state, part) in states.iter_mut().zip(&self.parts) {
            state.load_pose(part.initial);
            state.visible = true;
            state.skip_draw = false;
        }
    }
}

pub fn bake(layer: &LayerDef) -> BakedModel {
    let mut model = BakedModel {
        parts: Vec::new(),
        lookup: HashMap::new(),
        tex_size: [layer.tex_w, layer.tex_h],
    };
    let root = bake_part(
        &mut model,
        "root",
        &layer.mesh.root,
        None,
        layer.tex_w,
        layer.tex_h,
    );
    debug_assert_eq!(root, 0);
    let mut order: Vec<PartId> = vec![0];
    let mut i = 0;
    while i < order.len() {
        let id = order[i];
        i += 1;
        for child in model.parts[id].children.clone() {
            order.push(child);
        }
    }
    model.lookup.insert("root".to_string(), 0);
    for id in order {
        let name = model.parts[id].name.clone();
        model.lookup.entry(name).or_insert(id);
    }
    model
}

fn bake_part(
    model: &mut BakedModel,
    name: &str,
    def: &PartDef,
    parent: Option<PartId>,
    tex_w: u32,
    tex_h: u32,
) -> PartId {
    let id = model.parts.len();
    model.parts.push(BakedPart {
        name: name.to_string(),
        initial: def.pose,
        parent,
        children: Vec::new(),
        mesh: cube_mesh(&def.cubes, tex_w, tex_h),
    });
    for (child_name, child) in &def.children {
        let child_id = bake_part(model, child_name, child, Some(id), tex_w, tex_h);
        model.parts[id].children.push(child_id);
    }
    id
}

fn cube_mesh(cubes: &[CubeDef], tex_w: u32, tex_h: u32) -> Option<Mesh> {
    if cubes.is_empty() {
        return None;
    }
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for cube in cubes {
        let [w, h, d] = cube.size;
        let sx = tex_w as f32 * cube.tex_scale[0];
        let sy = tex_h as f32 * cube.tex_scale[1];

        let mut min_x = cube.origin[0] - cube.grow.0;
        let min_y = cube.origin[1] - cube.grow.1;
        let min_z = cube.origin[2] - cube.grow.2;
        let mut max_x = cube.origin[0] + w + cube.grow.0;
        let max_y = cube.origin[1] + h + cube.grow.1;
        let max_z = cube.origin[2] + d + cube.grow.2;
        if cube.mirror {
            std::mem::swap(&mut min_x, &mut max_x);
        }
        let t0 = [min_x, min_y, min_z];
        let t1 = [max_x, min_y, min_z];
        let t2 = [max_x, max_y, min_z];
        let t3 = [min_x, max_y, min_z];
        let l0 = [min_x, min_y, max_z];
        let l1 = [max_x, min_y, max_z];
        let l2 = [max_x, max_y, max_z];
        let l3 = [min_x, max_y, max_z];
        let u0 = cube.tex[0];
        let u1 = cube.tex[0] + d;
        let u2 = cube.tex[0] + d + w;
        let u22 = cube.tex[0] + d + w + w;
        let u3 = cube.tex[0] + d + w + d;
        let u4 = cube.tex[0] + d + w + d + w;
        let v0 = cube.tex[1];
        let v1 = cube.tex[1] + d;
        let v2 = cube.tex[1] + d + h;

        let faces: [(Direction, [[f32; 3]; 4], [f32; 4]); 6] = [
            (Direction::Down, [l1, l0, t0, t1], [u1, v0, u2, v1]),
            (Direction::Up, [t2, t3, l3, l2], [u2, v1, u22, v0]),
            (Direction::West, [t0, l0, l3, t3], [u0, v1, u1, v2]),
            (Direction::North, [t1, t0, t3, t2], [u1, v1, u2, v2]),
            (Direction::East, [l1, t1, t2, l2], [u2, v1, u3, v2]),
            (Direction::South, [l0, l1, l2, l3], [u3, v1, u4, v2]),
        ];

        for (face, corners, [fu0, fv0, fu1, fv1]) in faces {
            if !cube.faces.contains(face) {
                continue;
            }
            let mut verts = [
                (corners[0], [fu1 / sx, fv0 / sy]),
                (corners[1], [fu0 / sx, fv0 / sy]),
                (corners[2], [fu0 / sx, fv1 / sy]),
                (corners[3], [fu1 / sx, fv1 / sy]),
            ];
            if cube.mirror {
                verts.reverse();
            }
            let normal = if cube.mirror { face.mirrored() } else { face }.normal();
            let base = positions.len() as u32;
            for (pos, uv) in verts {
                positions.push([pos[0] / 16.0, pos[1] / 16.0, pos[2] / 16.0]);
                normals.push(normal);
                uvs.push(uv);
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    let white = vec![[1.0f32; 4]; positions.len()];
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, white);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quadruped_bakes_six_parts_plus_root() {
        let mut mesh = MeshDef::new();
        {
            let root = mesh.root();
            root.child(
                "head",
                CubeList::new()
                    .tex_offs(0, 0)
                    .add_box(-4.0, -4.0, -8.0, 8.0, 8.0, 8.0),
                PartPose::offset(0.0, 6.0, -6.0),
            );
            root.child(
                "body",
                CubeList::new()
                    .tex_offs(28, 8)
                    .add_box(-5.0, -10.0, -7.0, 10.0, 16.0, 8.0),
                PartPose::offset_rotation(0.0, 5.0, 2.0, 1.5707964, 0.0, 0.0),
            );
        }
        let baked = bake(&LayerDef::create(mesh, 64, 32));
        assert_eq!(baked.parts.len(), 3);
        assert_eq!(baked.parts[0].children, vec![1, 2]);
        assert_eq!(baked.id("head"), 1);
        assert_eq!(baked.id("root"), 0);
        assert!(baked.parts[0].mesh.is_none());
        let head = baked.parts[1].mesh.as_ref().unwrap();
        assert_eq!(head.count_vertices(), 24);
    }

    #[test]
    fn grandchildren_hang_off_the_returned_child() {
        let mut mesh = MeshDef::new();
        {
            let root = mesh.root();
            let body = root.child("body", CubeList::new(), PartPose::ZERO);
            body.child(
                "arm",
                CubeList::new().add_box(0.0, 0.0, 0.0, 1.0, 1.0, 1.0),
                PartPose::ZERO,
            );
        }
        let baked = bake(&LayerDef::create(mesh, 16, 16));
        assert_eq!(baked.parts.len(), 3);
        assert_eq!(baked.parts[baked.id("arm")].parent, Some(baked.id("body")));
    }

    #[test]
    fn mirrored_box_keeps_outward_normals() {
        let mut mesh = MeshDef::new();
        mesh.root().child(
            "leg",
            CubeList::new()
                .mirror()
                .add_box(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0),
            PartPose::ZERO,
        );
        let baked = bake(&LayerDef::create(mesh, 64, 32));
        let leg = baked.parts[baked.id("leg")].mesh.as_ref().unwrap();
        let normals = leg
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();
        assert!(normals.contains(&[1.0, 0.0, 0.0]));
        assert!(normals.contains(&[-1.0, 0.0, 0.0]));
    }

    #[test]
    fn part_state_transform_matches_translate_and_rotate() {
        let mut state = PartState::default();
        state.set_pos(0.0, 16.0, 0.0);
        state.set_rotation(std::f32::consts::FRAC_PI_2, 0.0, 0.0);
        let t = state.transform();
        assert!((t.translation.y - 1.0).abs() < 1e-6);
        let rotated = t.rotation * Vec3::Y;
        assert!((rotated.z - 1.0).abs() < 1e-5, "{rotated:?}");
    }
}
