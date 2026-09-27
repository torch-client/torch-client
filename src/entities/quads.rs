use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::Mesh;

pub struct QuadMesh {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

#[allow(
    dead_code,
    reason = "`untextured` and `is_empty` wait on the lightning bolt; the framed map uses the rest"
)]
impl QuadMesh {
    pub fn new() -> QuadMesh {
        QuadMesh {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn quad(&mut self, corners: [[f32; 3]; 4], uvs: [[f32; 2]; 4], normal: [f32; 3]) {
        let base = self.positions.len() as u32;
        for i in 0..4 {
            self.positions.push(corners[i]);
            self.uvs.push(uvs[i]);
            self.normals.push(normal);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub fn untextured(&mut self, corners: [[f32; 3]; 4], normal: [f32; 3]) {
        self.quad(corners, [[0.0, 0.0]; 4], normal);
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    pub fn finish(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        let white = vec![[1.0f32; 4]; self.positions.len()];
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, white);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

impl Default for QuadMesh {
    fn default() -> Self {
        QuadMesh::new()
    }
}

#[allow(
    dead_code,
    reason = "the billboard the orb, bobber and dragon fireball renderers will share"
)]
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
