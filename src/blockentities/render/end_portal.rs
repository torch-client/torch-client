use std::collections::HashMap;

use azalea_registry::builtin::BlockEntityKind;
use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::mesh::{MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError, TextureFormat,
};
use bevy::shader::ShaderRef;

use crate::blockentities::feed::BlockEntityInfo;
use crate::entities::quads::quad_indices;
use crate::items::model::face_positions;
use crate::renderer::RenderOrigin;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct EndPortalMaterial {
    #[texture(0)]
    #[sampler(1)]
    sky: Handle<Image>,
    #[texture(2)]
    #[sampler(3)]
    portal: Handle<Image>,
    #[uniform(4)]
    params: Vec4,
}

impl Material for EndPortalMaterial {
    fn vertex_shader() -> ShaderRef {
        shader()
    }

    fn fragment_shader() -> ShaderRef {
        shader()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![
            layout
                .0
                .get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)])?,
        ];
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

fn shader() -> ShaderRef {
    crate::renderer::embedded_shader("blockentities/render/end_portal.wgsl")
}

struct Cube {
    kind: BlockEntityKind,
    name: &'static str,
    layers: f32,
    from: [f32; 3],
    to: [f32; 3],
    faces: &'static [usize],
}

const CUBES: [Cube; 2] = [
    Cube {
        kind: BlockEntityKind::EndPortal,
        name: "end_portal",
        layers: 15.0,
        from: [0.0, 0.375, 0.0],
        to: [1.0, 0.75, 1.0],
        faces: &[0, 1],
    },
    Cube {
        kind: BlockEntityKind::EndGateway,
        name: "end_gateway",
        layers: 16.0,
        from: [0.0; 3],
        to: [1.0; 3],
        faces: &[0, 1, 2, 3, 4, 5],
    },
];

impl Cube {
    fn mesh(&self, blocks: &[[i32; 3]]) -> Mesh {
        let origin = blocks[0];
        let quads = blocks.len() * self.faces.len();
        let mut positions = Vec::with_capacity(quads * 4);
        for block in blocks {
            let offset = [0, 1, 2].map(|axis| (block[axis] - origin[axis]) as f32);
            for &face in self.faces {
                for corner in face_positions(self.from, self.to, face) {
                    positions.push([0, 1, 2].map(|axis| corner[axis] + offset[axis]));
                }
            }
        }
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(quad_indices(quads))
    }
}

#[derive(Component)]
struct EndPortalNode;

#[derive(Default)]
struct Batch {
    blocks: Vec<[i32; 3]>,
    scratch: Vec<[i32; 3]>,
    node: Option<Entity>,
    material: Option<Handle<EndPortalMaterial>>,
}

#[derive(Resource, Default)]
struct Portals {
    version: u64,
    textures: Option<Option<(Handle<Image>, Handle<Image>)>>,
    batches: [Batch; 2],
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "end_portal.wgsl");
    crate::renderer::reload::touch_on_reload::<EndPortalMaterial>(app);
    app.add_plugins(MaterialPlugin::<EndPortalMaterial>::default())
        .init_resource::<Portals>()
        .add_systems(
            Update,
            sync_portals
                .after(crate::renderer::FrameViewSystems)
                .run_if(in_state(crate::renderer::AppState::InGame)),
        );
}

fn sync_portals(
    mut commands: Commands,
    view: Res<crate::renderer::FrameView>,
    mut portals: ResMut<Portals>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<EndPortalMaterial>>,
    origin: Res<RenderOrigin>,
    mut nodes: Query<&mut Transform, With<EndPortalNode>>,
) {
    let origin = *origin;
    let snapshot =
        (view.block_entities_version != portals.version).then(|| view.block_entities.clone());
    portals.version = view.block_entities_version;
    let game_time = ((view.game_time % 24000) as f32 + view.partial) / 24000.0;
    let Portals {
        textures, batches, ..
    } = &mut *portals;

    for (cube, batch) in CUBES.iter().zip(batches.iter_mut()) {
        if let Some(snapshot) = &snapshot
            && batch.collect(cube, snapshot)
        {
            batch.rebuild(cube, &mut commands, &mut meshes, origin, || {
                let (sky, portal) = textures
                    .get_or_insert_with(|| {
                        Some((
                            load_tiled("environment/end_sky", &mut images)?,
                            load_tiled("entity/end_portal/end_portal", &mut images)?,
                        ))
                    })
                    .clone()?;
                Some(materials.add(EndPortalMaterial {
                    sky,
                    portal,
                    params: Vec4::new(game_time, cube.layers, 0.0, 0.0),
                }))
            });
        }
        if let Some(handle) = &batch.material
            && let Some(material) = materials.get_mut(handle)
        {
            material.params.x = game_time;
        }
        if let Some(node) = batch.node
            && let Some(&first) = batch.blocks.first()
            && let Ok(mut transform) = nodes.get_mut(node)
        {
            let want = origin.block_to_render(first);
            if transform.translation != want {
                transform.translation = want;
            }
        }
    }
}

impl Batch {
    fn collect(&mut self, cube: &Cube, snapshot: &HashMap<[i32; 3], BlockEntityInfo>) -> bool {
        self.scratch.clear();
        self.scratch.extend(
            snapshot
                .iter()
                .filter(|(_, info)| info.kind == cube.kind)
                .map(|(pos, _)| *pos),
        );
        self.scratch.sort_unstable();
        if self.scratch == self.blocks {
            return false;
        }
        std::mem::swap(&mut self.scratch, &mut self.blocks);
        true
    }

    fn rebuild(
        &mut self,
        cube: &Cube,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        origin: RenderOrigin,
        material: impl FnOnce() -> Option<Handle<EndPortalMaterial>>,
    ) {
        let Some(&first) = self.blocks.first() else {
            if let Some(node) = self.node.take() {
                commands.entity(node).despawn();
            }
            return;
        };
        if self.material.is_none() {
            self.material = material();
        }
        let Some(material) = self.material.clone() else {
            return;
        };
        let mesh = Mesh3d(meshes.add(cube.mesh(&self.blocks)));
        let transform = Transform::from_translation(origin.block_to_render(first));
        match self.node {
            Some(node) => {
                commands.entity(node).try_insert((mesh, transform));
            }
            None => {
                self.node = Some(
                    commands
                        .spawn((
                            mesh,
                            MeshMaterial3d(material),
                            transform,
                            EndPortalNode,
                            Visibility::default(),
                            Name::new(cube.name),
                        ))
                        .id(),
                );
            }
        }
    }
}

fn load_tiled(path: &str, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let handle = crate::entities::load_entity_image(path, images)?;
    let image = images.get_mut(&handle)?;
    image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    image.sampler = crate::entities::repeating_nearest();
    Some(handle)
}

#[cfg(test)]
pub(super) fn test_states(out: &mut Vec<crate::blockentities::BeState>) {
    out.push(super::test_state("end_portal", &[]));
    out.push(super::test_state("end_gateway", &[]));
}

#[cfg(test)]
mod tests {
    use bevy::mesh::VertexAttributeValues;

    use super::*;

    fn positions(mesh: &Mesh) -> Vec<[f32; 3]> {
        match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(values)) => values.clone(),
            _ => panic!("no positions"),
        }
    }

    #[test]
    fn the_portal_draws_its_top_and_bottom_at_three_eighths_and_three_quarters() {
        let mesh = CUBES[0].mesh(&[[10, 64, 10], [11, 64, 10]]);
        let positions = positions(&mesh);
        assert_eq!(positions.len(), 2 * 2 * 4);
        assert!(positions.iter().all(|p| p[1] == 0.375 || p[1] == 0.75));
        assert!(positions.iter().any(|p| p[0] == 2.0));
        assert_eq!(mesh.indices().map(|i| i.len()), Some(2 * 2 * 6));
    }

    #[test]
    fn the_gateway_draws_a_whole_block() {
        let positions = positions(&CUBES[1].mesh(&[[0, 0, 0]]));
        assert_eq!(positions.len(), 6 * 4);
        for axis in 0..3 {
            assert!(positions.iter().all(|p| p[axis] == 0.0 || p[axis] == 1.0));
        }
    }

    #[test]
    fn a_batch_rebuilds_only_when_its_blocks_move() {
        let mut batch = Batch::default();
        let empty = HashMap::new();
        assert!(!batch.collect(&CUBES[0], &empty));
        let mut st = super::super::test_state("end_portal", &[]);
        st.pos = [1, 2, 3];
        let info = BlockEntityInfo {
            kind: st.kind,
            state: st.state.clone(),
            data: st.data.clone(),
            anim: st.anim.clone(),
            partner: None,
            rev: 0,
        };
        let one = HashMap::from([(st.pos, info)]);
        assert!(batch.collect(&CUBES[0], &one));
        assert!(!batch.collect(&CUBES[0], &one));
        assert!(!Batch::default().collect(&CUBES[1], &one));
        assert!(batch.collect(&CUBES[0], &empty));
    }
}
