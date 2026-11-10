use bevy::asset::AssetEventSystems;
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;

const PAD_VERTICES: usize = 3;

pub struct EmptyMeshPlugin;

impl Plugin for EmptyMeshPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, pad_empty_meshes.after(AssetEventSystems));
    }
}

fn pad_empty_meshes(
    mut events: MessageReader<AssetEvent<Mesh>>,
    meshes: Option<ResMut<Assets<Mesh>>>,
) {
    let Some(mut meshes) = meshes else {
        events.clear();
        return;
    };
    for event in events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id }) = *event else {
            continue;
        };
        let Some(mesh) = meshes.get(id) else { continue };
        let needs_pad = match mesh.try_attributes() {
            Ok(mut attributes) => attributes.next().is_some() && mesh.count_vertices() == 0,
            Err(_) => false,
        };
        if !needs_pad {
            continue;
        }
        if let Some(mesh) = meshes.get_mut_untracked(id) {
            pad(mesh);
        }
    }
}

fn pad(mesh: &mut Mesh) {
    macro_rules! resize_all {
        ($values:expr, $($variant:ident),* $(,)?) => {
            match $values {
                $(VertexAttributeValues::$variant(v) => v.resize(PAD_VERTICES, Default::default()),)*
            }
        };
    }
    let Ok(attributes) = mesh.try_attributes_mut() else {
        return;
    };
    for (_, values) in attributes {
        resize_all!(
            values,
            Uint8,
            Uint8x2,
            Uint8x4,
            Sint8,
            Sint8x2,
            Sint8x4,
            Unorm8,
            Unorm8x2,
            Unorm8x4,
            Snorm8,
            Snorm8x2,
            Snorm8x4,
            Uint16,
            Uint16x2,
            Uint16x4,
            Sint16,
            Sint16x2,
            Sint16x4,
            Unorm16,
            Unorm16x2,
            Unorm16x4,
            Snorm16,
            Snorm16x2,
            Snorm16x4,
            Float16,
            Float16x2,
            Float16x4,
            Float32,
            Float32x2,
            Float32x3,
            Float32x4,
            Uint32,
            Uint32x2,
            Uint32x3,
            Uint32x4,
            Sint32,
            Sint32x2,
            Sint32x3,
            Sint32x4,
            Float64,
            Float64x2,
            Float64x3,
            Float64x4,
            Unorm10_10_10_2,
            Unorm8x4Bgra,
        );
    }
    if let Ok(Some(indices)) = mesh.try_indices_mut_option() {
        *indices = match indices {
            Indices::U16(_) => Indices::U16(vec![0; PAD_VERTICES]),
            Indices::U32(_) => Indices::U32(vec![0; PAD_VERTICES]),
        };
    }
}
