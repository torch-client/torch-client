use std::collections::HashMap;

use bevy::asset::{Asset, RenderAssetUsages, embedded_asset};
use bevy::camera::{PerspectiveProjection, Projection};
use bevy::mesh::{
    Indices, Mesh, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology,
    VertexAttributeValues, VertexFormat,
};
use bevy::pbr::{Material, MaterialPipeline, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use crate::client::lod::{LodOp, mesh::MeshBuffers, pos::SectionPos};
use crate::renderer::{FrameView, FrameViewSystems, RenderOrigin};

pub const ATTRIBUTE_COLOR: MeshVertexAttribute =
    MeshVertexAttribute::new("Lod_Color", 920, VertexFormat::Unorm8x4);

pub const ATTRIBUTE_PACKED: MeshVertexAttribute =
    MeshVertexAttribute::new("Lod_Packed", 921, VertexFormat::Uint32);

const OPS_PER_FRAME: usize = 128;

#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct LodParams {
    pub clip: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct LodMaterial {
    #[uniform(0)]
    pub params: LodParams,
    #[texture(1)]
    #[sampler(2)]
    pub lightmap: Handle<Image>,
    pub blend: bool,
}

impl Material for LodMaterial {
    crate::renderer::material_shader_fns!();

    fn alpha_mode(&self) -> AlphaMode {
        if self.blend {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        }
    }

    crate::renderer::material_no_prepass_no_shadows!();

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            ATTRIBUTE_COLOR.at_shader_location(1),
            ATTRIBUTE_PACKED.at_shader_location(2),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())
    }
}

fn shader() -> ShaderRef {
    super::embedded_shader("renderer/lod/lod.wgsl")
}

#[derive(Resource)]
struct LodMaterials {
    opaque: Handle<LodMaterial>,
    blend: Handle<LodMaterial>,
}

struct Drawn {
    entities: Vec<Entity>,
}

#[derive(Resource, Default)]
struct LodEntities {
    by_pos: HashMap<SectionPos, Drawn>,
    retiring: Vec<Entity>,
}

#[derive(Component)]
struct LodSection {
    corner: IVec3,
}

pub struct LodPlugin;

impl Plugin for LodPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "lod.wgsl");
        app.add_plugins(MaterialPlugin::<LodMaterial>::default())
            .init_resource::<LodEntities>()
            .add_systems(
                Update,
                (
                    publish_settings,
                    make_materials,
                    apply_ops,
                    follow_origin,
                    update_params,
                    widen_far,
                )
                    .chain()
                    .after(FrameViewSystems),
            );
    }
}

fn publish_settings(state: Res<crate::gui::GuiState>, view: Res<FrameView>) {
    let o = &state.options;
    crate::client::lod::publish_settings(crate::client::lod::Settings {
        render_distance: o.render_distance,
        lod_distance: o.lod_distance,
        quality: o.lod_quality,
        eye: view.camera_world.to_array(),
        in_world: view.in_world,
    });
    #[cfg(target_arch = "wasm32")]
    crate::client::lod::drain_on_caller(WEB_BUDGET);
}

#[cfg(target_arch = "wasm32")]
const WEB_BUDGET: std::time::Duration = std::time::Duration::from_millis(3);

fn make_materials(
    mut commands: Commands,
    existing: Option<Res<LodMaterials>>,
    lightmap: Option<Res<crate::renderer::terrain::LightmapPixels>>,
    mut materials: ResMut<Assets<LodMaterial>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(lightmap) = lightmap else { return };
    let make = |blend| LodMaterial {
        params: LodParams::default(),
        lightmap: lightmap.image.clone(),
        blend,
    };
    commands.insert_resource(LodMaterials {
        opaque: materials.add(make(false)),
        blend: materials.add(make(true)),
    });
}

fn to_mesh(b: MeshBuffers) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.positions);
    mesh.insert_attribute(ATTRIBUTE_COLOR, VertexAttributeValues::Unorm8x4(b.colors));
    mesh.insert_attribute(ATTRIBUTE_PACKED, VertexAttributeValues::Uint32(b.packed));
    mesh.insert_indices(Indices::U32(b.indices));
    mesh
}

fn apply_ops(
    mut commands: Commands,
    materials: Option<Res<LodMaterials>>,
    mut drawn: ResMut<LodEntities>,
    mut meshes: ResMut<Assets<Mesh>>,
    origin: Res<RenderOrigin>,
) {
    for e in std::mem::take(&mut drawn.retiring) {
        commands.entity(e).despawn();
    }
    let Some(materials) = materials else { return };
    let LodEntities { by_pos, retiring } = &mut *drawn;
    for op in crate::client::lod::take_ops(OPS_PER_FRAME) {
        match op {
            LodOp::Clear => {
                for (_, d) in by_pos.drain() {
                    retiring.extend(d.entities);
                }
            }
            LodOp::Remove(pos) => {
                if let Some(d) = by_pos.remove(&pos) {
                    retiring.extend(d.entities);
                }
            }
            LodOp::Upload(pos, built) => {
                if let Some(d) = by_pos.remove(&pos) {
                    retiring.extend(d.entities);
                }
                let corner = IVec3::new(pos.min_block_x(), 0, pos.min_block_z());
                let at = Transform::from_translation(origin.block_to_render(corner));
                let mut entities = Vec::with_capacity(2);
                for (buffers, material) in [
                    (built.opaque, &materials.opaque),
                    (built.transparent, &materials.blend),
                ] {
                    if buffers.indices.is_empty() {
                        continue;
                    }
                    let e = commands
                        .spawn((
                            Mesh3d(meshes.add(to_mesh(buffers))),
                            MeshMaterial3d(material.clone()),
                            at,
                            LodSection { corner },
                        ))
                        .id();
                    entities.push(e);
                }
                by_pos.insert(pos, Drawn { entities });
            }
        }
    }
}

fn follow_origin(origin: Res<RenderOrigin>, mut sections: Query<(&LodSection, &mut Transform)>) {
    if !origin.is_changed() {
        return;
    }
    for (section, mut t) in &mut sections {
        t.translation = origin.block_to_render(section.corner);
    }
}

fn update_params(
    handles: Option<Res<LodMaterials>>,
    mut materials: ResMut<Assets<LodMaterial>>,
    mut last: Local<Option<Vec4>>,
) {
    let Some(handles) = handles else { return };
    let clip = Vec4::new(crate::client::lod::real_radius(), 0.0, 0.0, 0.0);
    if *last == Some(clip) {
        return;
    }
    *last = Some(clip);
    for h in [&handles.opaque, &handles.blend] {
        if let Some(mut m) = materials.get_mut(h) {
            m.params.clip = clip;
        }
    }
}

fn widen_far(
    state: Res<crate::gui::GuiState>,
    mut camera: Query<&mut Projection, With<crate::renderer::systems::WorldCamera>>,
) {
    let Ok(mut projection) = camera.single_mut() else {
        return;
    };
    let lod = (state.options.lod_distance).max(state.options.render_distance + 1);
    let far = (lod as f32 * 16.0 * 1.5).max(PerspectiveProjection::default().far);
    if let Projection::Perspective(p) = &*projection
        && p.far != far
        && let Projection::Perspective(p) = &mut *projection
    {
        p.far = far;
    }
}
