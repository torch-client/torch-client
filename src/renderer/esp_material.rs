use bevy::asset::{Asset, embedded_asset};
use bevy::mesh::{Mesh, MeshVertexAttribute, MeshVertexBufferLayoutRef, VertexFormat};
use bevy::pbr::{Material, MaterialPipeline, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

pub const ATTRIBUTE_COLOR: MeshVertexAttribute =
    MeshVertexAttribute::new("Esp_Color", 900, VertexFormat::Unorm8x4);

pub const ATTRIBUTE_PACKED: MeshVertexAttribute =
    MeshVertexAttribute::new("Esp_Packed", 901, VertexFormat::Uint32);

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct EspParams {
    pub line: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct EspMaterial {
    #[uniform(0)]
    pub params: EspParams,
}

impl Default for EspMaterial {
    fn default() -> Self {
        EspMaterial {
            params: EspParams {
                line: Vec4::new(1.2, 0.0, 0.0, 0.0),
            },
        }
    }
}

impl Material for EspMaterial {
    fn vertex_shader() -> ShaderRef {
        shader()
    }

    fn fragment_shader() -> ShaderRef {
        shader()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
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
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            ATTRIBUTE_COLOR.at_shader_location(1),
            ATTRIBUTE_PACKED.at_shader_location(2),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;
        if let Some(depth_stencil) = descriptor.depth_stencil.as_mut() {
            depth_stencil.depth_compare = bevy::render::render_resource::CompareFunction::Always;
        }
        Ok(())
    }
}

fn shader() -> ShaderRef {
    let crate_name = module_path!().split(':').next().unwrap_or("torch_client");
    ShaderRef::Path(format!("embedded://{crate_name}/renderer/esp.wgsl").into())
}

pub struct EspMaterialPlugin;

impl Plugin for EspMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "esp.wgsl");
        app.add_plugins(MaterialPlugin::<EspMaterial>::default());
    }
}
