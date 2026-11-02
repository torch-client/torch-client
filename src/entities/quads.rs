use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::Mesh;

pub const QUAD_INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];

pub struct QuadMesh {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
}

impl QuadMesh {
    pub fn new() -> QuadMesh {
        QuadMesh {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
        }
    }

    pub fn quad(&mut self, corners: [[f32; 3]; 4], uvs: [[f32; 2]; 4], normal: [f32; 3]) {
        self.colored(corners, uvs, normal, [1.0; 4]);
    }

    pub fn colored(
        &mut self,
        corners: [[f32; 3]; 4],
        uvs: [[f32; 2]; 4],
        normal: [f32; 3],
        color: [f32; 4],
    ) {
        self.positions.extend_from_slice(&corners);
        self.uvs.extend_from_slice(&uvs);
        self.normals.extend_from_slice(&[normal; 4]);
        self.colors.extend_from_slice(&[color; 4]);
    }

    pub fn untextured(&mut self, corners: [[f32; 3]; 4], normal: [f32; 3]) {
        self.quad(corners, [[0.0, 0.0]; 4], normal);
    }

    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    pub fn uvs(&self) -> &[[f32; 2]] {
        &self.uvs
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    pub fn finish(self) -> Mesh {
        self.into_mesh(RenderAssetUsages::RENDER_WORLD, true)
    }

    pub fn into_mesh(self, usage: RenderAssetUsages, with_color: bool) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, usage);
        let indices = quad_indices(self.positions.len() / 4);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        if with_color {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        }
        mesh.insert_indices(indices);
        mesh
    }
}

pub fn quad_indices(quads: usize) -> Indices {
    Indices::U32(
        (0..quads as u32)
            .flat_map(|quad| QUAD_INDICES.map(|i| quad * 4 + i))
            .collect(),
    )
}

impl Default for QuadMesh {
    fn default() -> Self {
        QuadMesh::new()
    }
}

pub fn billboard_quad(y_offset: f32, uv: [f32; 4]) -> Mesh {
    let [u0, v0, u1, v1] = uv;
    let mut out = QuadMesh::new();
    out.quad(
        [
            [-0.5, -y_offset, 0.0],
            [0.5, -y_offset, 0.0],
            [0.5, 1.0 - y_offset, 0.0],
            [-0.5, 1.0 - y_offset, 0.0],
        ],
        [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
        [0.0, 1.0, 0.0],
    );
    out.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn billboard_quad_matches_the_dragon_fireball_vertices() {
        let mesh = billboard_quad(0.25, [0.0, 0.0, 1.0, 1.0]);
        let pos = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        assert_eq!(pos[0], [-0.5, -0.25, 0.0]);
        assert_eq!(pos[2], [0.5, 0.75, 0.0]);
        assert_eq!(mesh.count_vertices(), 4);
    }
}
