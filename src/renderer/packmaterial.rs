use bevy::asset::{Asset, Handle};
use bevy::image::Image;
use bevy::math::{Mat4, Vec4};
use bevy::mesh::{Mesh, MeshVertexAttribute, MeshVertexBufferLayoutRef, VertexFormat};
use bevy::pbr::{Material, MaterialPipeline, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::{Shader, ShaderRef};

use crate::shaderpack::pipeline::{Attribute, Format, Layout};

const PACK_ATTRIBUTE_UV0: MeshVertexAttribute =
    MeshVertexAttribute::new("Terrain_Uv0", 8, VertexFormat::Unorm16x2);
const PACK_ATTRIBUTE_LIGHT: MeshVertexAttribute =
    MeshVertexAttribute::new("Terrain_Light", 9, VertexFormat::Unorm8x4);
const PACK_ATTRIBUTE_COLOR: MeshVertexAttribute =
    MeshVertexAttribute::new("Terrain_Color", 10, VertexFormat::Unorm8x4);

pub(crate) const MATERIAL_SET: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
pub(crate) struct PackFrame {
    pub(crate) model_view: Mat4,
    pub(crate) projection: Mat4,
    pub(crate) camera: Vec4,
    pub(crate) screen: Vec4,
    pub(crate) time: Vec4,
}

impl Default for PackFrame {
    fn default() -> Self {
        PackFrame {
            model_view: Mat4::IDENTITY,
            projection: Mat4::IDENTITY,
            camera: Vec4::ZERO,
            screen: Vec4::ONE,
            time: Vec4::ZERO,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct PackKey {
    vertex: Handle<Shader>,
    fragment: Handle<Shader>,
    attributes: Vec<u32>,
}

impl From<&PackMaterial> for PackKey {
    fn from(material: &PackMaterial) -> PackKey {
        PackKey {
            vertex: material.vertex.clone(),
            fragment: material.fragment.clone(),
            attributes: material
                .layout
                .attributes
                .iter()
                .map(|a| a.location)
                .collect(),
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
#[bind_group_data(PackKey)]
pub(crate) struct PackMaterial {
    #[uniform(0)]
    pub(crate) frame: PackFrame,
    #[texture(1)]
    #[sampler(2)]
    pub(crate) atlas: Handle<Image>,
    pub(crate) vertex: Handle<Shader>,
    pub(crate) fragment: Handle<Shader>,
    pub(crate) layout: Layout,
}

impl Material for PackMaterial {
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn fragment_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.shader = key.bind_group_data.vertex.clone();
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader = key.bind_group_data.fragment.clone();
        }

        let wanted: Vec<_> = key
            .bind_group_data
            .attributes
            .iter()
            .filter_map(|location| {
                mesh_attribute(*location).map(|a| a.at_shader_location(*location))
            })
            .collect();
        descriptor.vertex.buffers = vec![layout.0.get_layout(&wanted)?];
        Ok(())
    }
}

#[derive(Resource)]
pub(crate) struct CompiledPack {
    pub(crate) name: String,
    pub(crate) revision: u64,
    pub(crate) vertex: Handle<Shader>,
    pub(crate) fragment: Handle<Shader>,
    pub(crate) layout: Layout,
}

pub(crate) fn compile(
    pack: &str,
    program: &str,
    revision: u64,
    shaders: &mut Assets<Shader>,
) -> Result<CompiledPack, String> {
    let (vertex_wgsl, fragment_wgsl, layout) = compile_wgsl(pack, program)?;
    Ok(CompiledPack {
        name: pack.to_owned(),
        revision,
        vertex: shaders.add(Shader::from_wgsl(
            vertex_wgsl,
            format!("shaderpack/{pack}/{program}.vsh"),
        )),
        fragment: shaders.add(Shader::from_wgsl(
            fragment_wgsl,
            format!("shaderpack/{pack}/{program}.fsh"),
        )),
        layout,
    })
}

pub(crate) fn compile_wgsl(pack: &str, program: &str) -> Result<(String, String, Layout), String> {
    use crate::shaderpack::{
        backend, discover, features, include, options, pipeline, source, transform,
    };

    let source = source::Source::open(&discover::dir().join(pack))?;

    let files: Vec<String> = source
        .files()
        .into_iter()
        .filter(|f| {
            [".glsl", ".vsh", ".fsh", ".gsh", ".csh"]
                .iter()
                .any(|e| f.ends_with(e))
        })
        .filter_map(|f| source.read_text(&f))
        .collect();
    let declared = options::discover(files.iter().map(String::as_str));
    let values = std::fs::read_to_string(discover::dir().join(format!("{pack}.txt")))
        .map(|text| options::Values::load(&text, &declared))
        .unwrap_or_default();

    let mut cache = include::Cache::new();
    let mut stage_source = |suffix: &str| -> Result<String, String> {
        let path = format!("/{program}{suffix}");
        let expanded = include::expand(&source, &path, &mut cache).map_err(|e| e.to_string())?;
        let applied = options::apply(&expanded.text, &declared, &values);
        let mut defines = features::base_defines();
        crate::shaderpack::preprocess::preprocess(&applied, &mut defines)
            .map_err(|e| format!("{path}: {e}"))
    };

    let vertex_glsl = stage_source(".vsh")?;
    let fragment_glsl = stage_source(".fsh")?;

    let varyings = transform::varyings(&vertex_glsl);
    let vertex = transform::transform(
        &vertex_glsl,
        transform::Stage::Vertex,
        &varyings,
        MATERIAL_SET,
    );
    let fragment = transform::transform(
        &fragment_glsl,
        transform::Stage::Fragment,
        &varyings,
        MATERIAL_SET,
    );

    let vertex_wgsl = backend::to_wgsl(&vertex.source, backend::ShaderStage::Vertex)
        .map_err(|e| format!("{program}.vsh: {e}"))?;
    let fragment_wgsl = backend::to_wgsl(&fragment.source, backend::ShaderStage::Fragment)
        .map_err(|e| format!("{program}.fsh: {e}"))?;

    let mut layout = pipeline::describe(&vertex.interface);
    let fragment_layout = pipeline::describe(&fragment.interface);
    layout.bindings = fragment_layout.bindings;
    layout.targets = fragment_layout.targets;

    Ok((vertex_wgsl, fragment_wgsl, layout))
}

pub(crate) fn format_size(format: Format) -> u64 {
    match format {
        Format::Float32x2 => 8,
        Format::Float32x3 => 12,
        Format::Float32x4 => 16,
    }
}

pub(crate) fn mesh_attribute(location: u32) -> Option<MeshVertexAttribute> {
    Some(match location {
        0 => Mesh::ATTRIBUTE_POSITION,
        1 => PACK_ATTRIBUTE_UV0,
        2 => PACK_ATTRIBUTE_LIGHT,
        3 => PACK_ATTRIBUTE_COLOR,
        4 => Mesh::ATTRIBUTE_NORMAL,
        _ => return None,
    })
}

fn sync_compiled_pack(
    mut commands: Commands,
    gui: Res<crate::gui::GuiState>,
    compiled: Option<Res<CompiledPack>>,
    mut shaders: ResMut<Assets<Shader>>,
    mut failed: Local<Option<(String, u64)>>,
) {
    let wanted = gui.shaderpacks.loaded.as_ref().map(|l| l.name.clone());
    let revision = gui.shaderpacks.revision();
    let editing = gui.screen == crate::gui::Screen::ShaderOptions;

    let stale = match compiled.as_deref() {
        None => wanted.is_some(),
        Some(current) => {
            Some(&current.name) != wanted.as_ref() || (!editing && current.revision != revision)
        }
    };
    if !stale {
        return;
    }
    let Some(name) = wanted else {
        commands.remove_resource::<CompiledPack>();
        *failed = None;
        return;
    };
    if *failed == Some((name.clone(), revision)) {
        return;
    }

    match compile(&name, "gbuffers_terrain", revision, &mut shaders) {
        Ok(pack) => {
            crate::log_info!("shaders", "compiled {name}: {:?}", pack.layout.attributes);
            *failed = None;
            commands.insert_resource(pack);
        }
        Err(e) => {
            crate::log_warn!("shaders", "{name} did not compile: {e}");
            *failed = Some((name, revision));
            commands.remove_resource::<CompiledPack>();
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct PackFrameSource(PackFrame);

fn read_camera(
    mut frame: ResMut<PackFrameSource>,
    camera: Query<(&GlobalTransform, &Projection), With<super::systems::WorldCamera>>,
    windows: Query<&Window>,
    time: Res<Time>,
) {
    let Ok((transform, projection)) = camera.single() else {
        return;
    };
    let size = windows
        .iter()
        .next()
        .map_or(Vec2::ONE, |w| Vec2::new(w.width(), w.height()));
    let elapsed = time.elapsed_secs();

    frame.0 = PackFrame {
        model_view: transform.to_matrix().inverse(),
        projection: projection.get_clip_from_view(),
        camera: transform.translation().extend(0.0),
        screen: Vec4::new(size.x, size.y, 1.0 / size.x, 1.0 / size.y),
        time: Vec4::new(elapsed, time.delta_secs(), elapsed * 20.0, 0.0),
    };
}

pub(crate) struct PackMaterialPlugin;

impl Plugin for PackMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<PackMaterial>::default())
            .init_resource::<PackFrameSource>()
            .add_systems(Update, (read_camera, sync_compiled_pack).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaderpack::transform::UNIFORMS;

    #[test]
    fn the_uniform_block_matches_the_glsl_it_mirrors() {
        let layout = crate::shaderpack::pipeline::describe(&Default::default()).uniforms;
        assert_eq!(layout.members.len(), UNIFORMS.len());

        let names: Vec<&str> = layout.members.iter().map(|m| m.name).collect();
        assert_eq!(
            names,
            [
                "iris_ModelViewMatrix",
                "iris_ProjectionMatrix",
                "iris_CameraPosition",
                "iris_ScreenSize",
                "iris_Time",
            ]
        );

        assert_eq!(PackFrame::min_size().get(), u64::from(layout.size));
    }

    #[test]
    fn the_locations_a_program_can_ask_for_are_ones_the_mesher_has() {
        for location in 0..=4 {
            assert!(mesh_attribute(location).is_some(), "location {location}");
        }
        assert_eq!(mesh_attribute(1).unwrap(), PACK_ATTRIBUTE_UV0);
        assert_eq!(mesh_attribute(5), None, "mc_Entity is not meshed yet");
    }

    #[test]
    #[ignore = "needs a shader pack installed"]
    fn the_minimal_pack_compiles_with_the_chosen_option_value() {
        use crate::shaderpack::discover;

        let (vertex, fragment, layout) =
            compile_wgsl("Minimal", "gbuffers_terrain").expect("should compile");

        assert!(vertex.contains("@vertex"), "{vertex}");
        assert!(fragment.contains("@fragment"), "{fragment}");
        assert!(fragment.contains("textureSample"), "{fragment}");
        assert_eq!(layout.targets, 1);

        let chosen = std::fs::read_to_string(discover::dir().join("Minimal.txt"))
            .ok()
            .and_then(|text| {
                text.lines()
                    .find_map(|l| l.strip_prefix("TINT_STRENGTH=").map(str::to_owned))
            })
            .unwrap_or_else(|| "1.0".to_owned());

        let number: f32 = chosen.parse().expect("a numeric option value");
        let written = format!("{number}f");
        println!("TINT_STRENGTH is {chosen}; looking for {written} in the compiled shader");
        assert!(
            fragment.contains(&written),
            "the chosen option value never reached the shader\n{fragment}"
        );
    }

    #[test]
    fn attribute_widths_match_their_formats() {
        let attribute = Attribute {
            location: 0,
            format: Format::Float32x3,
        };
        assert_eq!(format_size(attribute.format), 12);
        assert_eq!(format_size(Format::Float32x2), 8);
        assert_eq!(format_size(Format::Float32x4), 16);
    }
}
