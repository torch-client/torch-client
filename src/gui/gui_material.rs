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
    crate::renderer::material_shader_fns!();

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
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
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

fn shader() -> ShaderRef {
    crate::renderer::embedded_shader("gui/gui.wgsl")
}

pub struct GuiMaterialPlugin;

impl Plugin for GuiMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "gui.wgsl");
        app.add_plugins(MaterialPlugin::<GuiMaterial>::default());
        crate::renderer::reload::touch_on_reload::<GuiMaterial>(app);
    }
}
