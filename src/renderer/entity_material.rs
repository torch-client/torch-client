use bevy::asset::{Asset, embedded_asset};
use bevy::mesh::{Mesh, MeshVertexBufferLayoutRef};
use bevy::pbr::{Material, MaterialPipeline, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use super::dimension::Dimension;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LightMode {
    Cardinal = 0,
    Flat = 1,
    Emissive = 2,
    Text = 3,
}

const DIFFUSE_LIGHT: [Vec3; 2] = [Vec3::new(0.2, 1.0, -0.7), Vec3::new(-0.2, 1.0, 0.7)];
const NETHER_DIFFUSE_LIGHT: [Vec3; 2] = [Vec3::new(0.2, 1.0, -0.7), Vec3::new(-0.2, -1.0, 0.7)];

pub fn diffuse_lights(dim: Dimension) -> [Vec3; 2] {
    let pair = match dim {
        Dimension::Nether => NETHER_DIFFUSE_LIGHT,
        _ => DIFFUSE_LIGHT,
    };
    [pair[0].normalize(), pair[1].normalize()]
}

#[derive(Clone, Copy, PartialEq, Debug, ShaderType)]
pub struct EntityParams {
    pub tint: Vec4,
    pub light: Vec4,
    pub light0: Vec4,
    pub light1: Vec4,
    pub light_floor: Vec4,
    pub light_block: Vec4,
    pub light_sky: Vec4,
    pub mode: u32,
}

impl Default for EntityParams {
    fn default() -> Self {
        EntityParams {
            tint: Vec4::ONE,
            light: Vec4::new(1.0, 1.0, 1.0, 0.0),
            light0: DIFFUSE_LIGHT[0].normalize().extend(0.0),
            light1: DIFFUSE_LIGHT[1].normalize().extend(1.0),
            light_floor: Vec4::ZERO,
            light_block: Vec4::ZERO,
            light_sky: Vec4::new(1.0, 1.0, 1.0, 0.0),
            mode: LightMode::Cardinal as u32,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Lit {
    pub tint: [f32; 4],
    pub light: super::lightmap::CellLight,
    pub dirs: [Vec3; 2],
    pub overlay_alpha: f32,
    pub overlay_white: bool,
}

impl Lit {
    pub fn full_bright() -> Lit {
        Lit {
            tint: [1.0; 4],
            light: super::lightmap::CellLight::full_bright(),
            dirs: diffuse_lights(Dimension::Overworld),
            overlay_alpha: 1.0,
            overlay_white: false,
        }
    }
}

impl EntityParams {
    pub fn new(mode: LightMode, cutoff: f32, lit: Lit) -> EntityParams {
        let mut params = EntityParams {
            light: Vec4::new(1.0, 1.0, 1.0, cutoff),
            mode: mode as u32,
            ..default()
        };
        params.set(lit);
        params
    }

    pub fn set(&mut self, lit: Lit) {
        self.tint = Vec4::from(lit.tint);
        self.light.x = lit.light.color[0];
        self.light.y = lit.light.color[1];
        self.light.z = lit.light.color[2];
        self.light0 = lit.dirs[0].extend(if lit.overlay_white { 1.0 } else { 0.0 });
        self.light1 = lit.dirs[1].extend(lit.overlay_alpha);
        self.light_floor = Vec3::from(lit.light.floor).extend(0.0);
        self.light_block = Vec3::from(lit.light.block).extend(0.0);
        self.light_sky = Vec3::from(lit.light.sky).extend(0.0);
    }
}

fn shader() -> ShaderRef {
    embedded("entity.wgsl")
}

#[cfg(feature = "builtin_shaders")]
fn prepass_shader() -> ShaderRef {
    embedded("entity_prepass.wgsl")
}

fn embedded(file: &str) -> ShaderRef {
    let crate_name = module_path!().split(':').next().unwrap_or("torch_client");
    ShaderRef::Path(format!("embedded://{crate_name}/renderer/{file}").into())
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
#[bind_group_data(EntityMaterialKey)]
pub struct EntityMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Option<Handle<Image>>,
    #[uniform(2)]
    pub params: EntityParams,
    pub alpha_mode: AlphaMode,
    pub depth_bias: i32,
    pub see_through: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct EntityMaterialKey {
    depth_bias: i32,
    see_through: bool,
    fancy: bool,
}

impl From<&EntityMaterial> for EntityMaterialKey {
    fn from(material: &EntityMaterial) -> Self {
        EntityMaterialKey {
            depth_bias: material.depth_bias,
            see_through: material.see_through,
            fancy: crate::renderer::terrain::builtin_shaders_enabled(),
        }
    }
}

impl EntityMaterial {
    pub fn new(
        texture: Option<Handle<Image>>,
        mode: LightMode,
        alpha_mode: AlphaMode,
        cutoff: f32,
        lit: Lit,
        depth_bias: i32,
    ) -> EntityMaterial {
        EntityMaterial {
            texture,
            params: EntityParams::new(mode, cutoff, lit),
            alpha_mode,
            depth_bias,
            see_through: false,
        }
    }
}

impl Material for EntityMaterial {
    fn vertex_shader() -> ShaderRef {
        shader()
    }

    fn fragment_shader() -> ShaderRef {
        shader()
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        cfg!(feature = "builtin_shaders")
    }

    #[cfg(feature = "builtin_shaders")]
    fn prepass_vertex_shader() -> ShaderRef {
        prepass_shader()
    }

    #[cfg(feature = "builtin_shaders")]
    fn prepass_fragment_shader() -> ShaderRef {
        prepass_shader()
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(3),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;
        if let Some(depth_stencil) = descriptor.depth_stencil.as_mut() {
            depth_stencil.bias.constant = key.bind_group_data.depth_bias;
            if key.bind_group_data.see_through {
                depth_stencil.depth_compare =
                    bevy::render::render_resource::CompareFunction::Always;
            }
        }
        if key.bind_group_data.fancy {
            descriptor.vertex.shader_defs.push("FANCY_SHADERS".into());
            if let Some(fragment) = descriptor.fragment.as_mut() {
                fragment.shader_defs.push("FANCY_SHADERS".into());
            }
        }
        Ok(())
    }
}

pub struct EntityMaterialPlugin;

impl Plugin for EntityMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "entity.wgsl");
        #[cfg(feature = "builtin_shaders")]
        embedded_asset!(app, "entity_prepass.wgsl");
        app.add_plugins(MaterialPlugin::<EntityMaterial>::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mix(light: [f32; 2]) -> f32 {
        let clamped = [light[0].max(0.0), light[1].max(0.0)];
        (1.0f32).min((clamped[0] + clamped[1]) * 0.6 + 0.4)
    }

    fn dots(normal: Vec3, dirs: [Vec3; 2]) -> [f32; 2] {
        [dirs[0].dot(normal), dirs[1].dot(normal)]
    }

    #[test]
    fn the_top_of_a_mob_is_lit_and_its_underside_is_not() {
        let dirs = diffuse_lights(Dimension::Overworld);
        assert_eq!(mix(dots(Vec3::Y, dirs)), 1.0);
        assert_eq!(mix(dots(Vec3::NEG_Y, dirs)), 0.4);
    }

    #[test]
    fn the_sides_of_a_mob_match_across_each_axis() {
        let dirs = diffuse_lights(Dimension::Overworld);
        let north = mix(dots(Vec3::NEG_Z, dirs));
        let south = mix(dots(Vec3::Z, dirs));
        let east = mix(dots(Vec3::X, dirs));
        let west = mix(dots(Vec3::NEG_X, dirs));
        assert!((north - south).abs() < 1e-6, "{north} vs {south}");
        assert!((east - west).abs() < 1e-6, "{east} vs {west}");
        for side in [north, east] {
            assert!((0.4..1.0).contains(&side), "{side}");
        }
    }

    #[test]
    fn the_nether_lights_the_underside_of_a_mob() {
        let dirs = diffuse_lights(Dimension::Nether);
        let below = mix(dots(Vec3::NEG_Y, dirs));
        assert!(below > 0.4, "{below}");
        assert!(below < mix(dots(Vec3::Y, diffuse_lights(Dimension::Overworld))));
    }

    #[test]
    fn the_end_lights_like_the_overworld() {
        assert_eq!(
            diffuse_lights(Dimension::End),
            diffuse_lights(Dimension::Overworld)
        );
    }
}
