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
#[bind_group_data(EspKey)]
pub struct EspMaterial {
    #[uniform(0)]
    pub params: EspParams,
    pub through: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct EspKey {
    through: bool,
}

impl From<&EspMaterial> for EspKey {
    fn from(material: &EspMaterial) -> Self {
        EspKey {
            through: material.through,
        }
    }
}

impl Default for EspMaterial {
    fn default() -> Self {
        EspMaterial {
            through: true,
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
        key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            ATTRIBUTE_COLOR.at_shader_location(1),
            ATTRIBUTE_PACKED.at_shader_location(2),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;
        if !key.bind_group_data.through {
            descriptor.vertex.shader_defs.push("DEPTH_TESTED".into());
        } else if let Some(depth_stencil) = descriptor.depth_stencil.as_mut() {
            depth_stencil.depth_compare =
                bevy::render::render_resource::CompareFunction::GreaterEqual;
        }
        descriptor
            .vertex
            .shader_defs
            .push(super::hand::band_floor_def());
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.push(super::hand::band_floor_def());
        }
        Ok(())
    }
}

fn shader() -> ShaderRef {
    super::embedded_shader("renderer/esp.wgsl")
}

pub struct EspMaterialPlugin;

impl Plugin for EspMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "esp.wgsl");
        app.add_plugins(MaterialPlugin::<EspMaterial>::default());
    }
}
