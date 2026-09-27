use bevy::camera::primitives::Aabb;
use bevy::math::{Mat3, Vec3A};
use bevy::mesh::{Indices, Mesh, VertexAttributeValues};
use bevy::prelude::*;

use super::geom::{BakedModel, PartState, PosedPart};

pub fn layer(
    mesh: &mut Mesh,
    model: &BakedModel,
    posed: &[PosedPart],
    states: &[PartState],
) -> Aabb {
    let mut positions = f32x3(mesh.remove_attribute(Mesh::ATTRIBUTE_POSITION));
    let mut normals = f32x3(mesh.remove_attribute(Mesh::ATTRIBUTE_NORMAL));
    let mut uvs = f32x2(mesh.remove_attribute(Mesh::ATTRIBUTE_UV_0));
    let mut colors = f32x4(mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR));
    let mut indices = match mesh.remove_indices() {
        Some(Indices::U32(v)) => v,
        _ => Vec::new(),
    };
    positions.clear();
    normals.clear();
    uvs.clear();
    indices.clear();

    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);

    for ((part, pose), state) in model.parts.iter().zip(posed).zip(states) {
        if !pose.visible || state.skip_draw {
            continue;
        }
        let Some(arrays) = part.arrays() else {
            continue;
        };

        let linear = Mat3::from_mat4(pose.mat);
        let normal_mat = if linear.determinant().abs() > 1e-9 {
            linear.inverse().transpose()
        } else {
            linear
        };

        let base = positions.len() as u32;
        for p in arrays.positions {
            let v = pose.mat.transform_point3(Vec3::from(*p));
            min = min.min(v);
            max = max.max(v);
            positions.push(v.to_array());
        }
        for n in arrays.normals {
            let v = (normal_mat * Vec3::from(*n)).normalize_or_zero();
            normals.push(v.to_array());
        }
        uvs.extend_from_slice(arrays.uvs);
        indices.extend(arrays.indices.iter().map(|i| i + base));
    }

    colors.clear();
    colors.resize(positions.len(), [1.0; 4]);

    if positions.is_empty() {
        min = Vec3::ZERO;
        max = Vec3::ZERO;
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    Aabb::from_min_max(min, max)
}

pub fn empty_aabb() -> Aabb {
    Aabb {
        center: Vec3A::ZERO,
        half_extents: Vec3A::ZERO,
    }
}

fn f32x3(values: Option<VertexAttributeValues>) -> Vec<[f32; 3]> {
    match values {
        Some(VertexAttributeValues::Float32x3(v)) => v,
        _ => Vec::new(),
    }
}

fn f32x2(values: Option<VertexAttributeValues>) -> Vec<[f32; 2]> {
    match values {
        Some(VertexAttributeValues::Float32x2(v)) => v,
        _ => Vec::new(),
    }
}

fn f32x4(values: Option<VertexAttributeValues>) -> Vec<[f32; 4]> {
    match values {
        Some(VertexAttributeValues::Float32x4(v)) => v,
        _ => Vec::new(),
    }
}
