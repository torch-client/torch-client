use bevy::asset::{Asset, embedded_asset};
use bevy::mesh::{Mesh, MeshVertexBufferLayoutRef};
use bevy::pbr::{Material, MaterialPipeline, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

pub const UNIHEX_UV_BIAS: f32 = 2.0;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct GuiMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub atlas: Handle<Image>,
    #[texture(2)]
    #[sampler(3)]
    pub unihex: Handle<Image>,
}

impl Material for GuiMaterial {
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
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

fn shader() -> ShaderRef {
    let crate_name = module_path!().split(':').next().unwrap_or("torch_client");
    ShaderRef::Path(format!("embedded://{crate_name}/gui/gui.wgsl").into())
}

pub struct GuiMaterialPlugin;

impl Plugin for GuiMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "gui.wgsl");
        app.add_plugins(MaterialPlugin::<GuiMaterial>::default());
    }
}
