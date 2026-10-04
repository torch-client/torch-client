use super::super::sources::TextureSource;
use super::pack::Pack;
use super::preludes::{ENTITY_PRELUDE, ENTITY_UNIFORMS, TERRAIN_BEGIN, entity_varyings};
use super::{AlphaTest, Dispatch, PACK_SET, Program, ProgramKind, SCREEN_PRELUDE, terrain_prelude};
use crate::shaderpack::pipeline::{Binding, Layout};
use crate::shaderpack::{backend, directives, pipeline, transform};

pub(super) fn compile_program(
    pack: &mut Pack,
    name: &str,
    kind: ProgramKind,
    tests: &[AlphaTest],
    direct: bool,
) -> Result<(Program, [String; 2]), String> {
    let vertex_glsl = pack.stage(name, ".vsh")?;
    let fragment_glsl = pack.stage(name, ".fsh")?;

    let is_final = name == "final";
    let draw_buffers = if is_final {
        vec![0]
    } else {
        directives::draw_buffers(&fragment_glsl).map_err(|e| format!("{name}: {e}"))?
    };
    if kind.is_shadow()
        && let Some(past) = draw_buffers
            .iter()
            .find(|b| usize::from(**b) >= directives::MAX_SHADOW_COLORS)
    {
        return Err(format!(
            "{name} draws to shadowcolor{past}, past the last shadow colour target"
        ));
    }

    let mut shared = transform::Shared::of(&vertex_glsl, &fragment_glsl);
    if kind.draws_entities() {
        shared
            .uniforms
            .retain(|u| !ENTITY_UNIFORMS.iter().any(|(name, _)| u.name == *name));
    }
    let image_formats = image_formats(pack, name, &shared)?;
    if !shared.problems.is_empty() {
        return Err(format!("{name}: {}", shared.problems.join("; ")));
    }
    let (fragment_prelude, entity_outputs, vertex_end) = if kind.draws_entities() {
        entity_varyings(&fragment_glsl, shared.varying_end)
    } else {
        Default::default()
    };
    let prelude = match kind {
        ProgramKind::Geometry | ProgramKind::Shadow => terrain_prelude(direct),
        ProgramKind::Entity | ProgramKind::ShadowEntity => {
            format!("{ENTITY_PRELUDE}{entity_outputs}")
        }
        ProgramKind::Screen => SCREEN_PRELUDE.to_owned(),
        ProgramKind::Compute => unreachable!("`compile_compute` compiles compute programs"),
    };
    let target = transform::Target {
        set: PACK_SET,
        vertex_prelude: &prelude,
        fragment_prelude: &fragment_prelude,
        vertex_end: &vertex_end,
        vertex_begin: if matches!(kind, ProgramKind::Geometry | ProgramKind::Shadow) {
            TERRAIN_BEGIN
        } else {
            ""
        },
        image_formats: &image_formats,
        linear_output: false,
        outputs: Some(draw_buffers.len() as u32),
        gl_clip: true,
        upright: is_final,
        attribute_defaults: kind != ProgramKind::Screen,
        ..Default::default()
    };
    let vertex = transform::transform(&vertex_glsl, transform::Stage::Vertex, &shared, &target);
    let fragment =
        transform::transform(&fragment_glsl, transform::Stage::Fragment, &shared, &target);

    let layout = pipeline::describe_program(pipeline::Program::Draw {
        vertex: &vertex.interface,
        fragment: &fragment.interface,
    });
    undeclared_buffer(pack, name, &layout)?;
    if !layout.attributes.is_empty() {
        let locations: Vec<u32> = layout.attributes.iter().map(|a| a.location).collect();
        return Err(format!(
            "{name} reads vertex attributes this client does not have (locations {locations:?})"
        ));
    }
    let textures = texture_sources(pack, name, kind, &layout)?;

    let defines: Vec<(&str, &str)> = pack.environment.iter().collect();
    let vertex_wgsl = backend::to_wgsl(&vertex.source, backend::ShaderStage::Vertex, &defines)
        .map_err(|e| format!("{name}.vsh: {e}"))?;
    let fragment_wgsl =
        backend::to_wgsl(&fragment.source, backend::ShaderStage::Fragment, &defines)
            .map_err(|e| format!("{name}.fsh: {e}"))?;
    let fragment_linear = if is_final {
        let linear = transform::transform(
            &fragment_glsl,
            transform::Stage::Fragment,
            &shared,
            &transform::Target {
                linear_output: true,
                ..target
            },
        );
        Some(
            backend::to_wgsl(&linear.source, backend::ShaderStage::Fragment, &defines)
                .map_err(|e| format!("{name}.fsh (linear): {e}"))?,
        )
    } else {
        None
    };
    let mut fragment_tests: Vec<(AlphaTest, String)> = Vec::with_capacity(tests.len());
    for test in tests {
        if fragment_tests.iter().any(|(t, _)| t == test) {
            continue;
        }
        let threshold = format!("{:.6}", test.wrapper_reference());
        let mut defines = defines.clone();
        defines.push((transform::ALPHA_TEST_DEFINE, &threshold));
        defines.push((transform::ALPHA_OP_DEFINE, test.function.operator()));
        let wgsl = backend::to_wgsl(&fragment.source, backend::ShaderStage::Fragment, &defines)
            .map_err(|e| format!("{name}.fsh (alpha test): {e}"))?;
        fragment_tests.push((*test, wgsl));
    }

    Ok((
        Program {
            name: name.to_owned(),
            kind,
            vertex: vertex_wgsl,
            fragment: fragment_wgsl,
            fragment_tests,
            alpha_override: pack.alpha_tests.get(name).copied(),
            fragment_linear,
            compute: None,
            compute_used: None,
            compute_parity: None,
            dispatch: Dispatch::Fixed([0; 3]),
            layout,
            draw_buffers,
            textures,
            mipmapped: directives::mipmapped([vertex_glsl.as_str(), fragment_glsl.as_str()]),
        },
        [vertex_glsl, fragment_glsl],
    ))
}

pub(super) fn compile_compute(pack: &mut Pack, name: &str) -> Result<(Program, String), String> {
    let path = pack
        .compute_path(name)
        .ok_or_else(|| format!("{name}.csh is missing"))?;
    let glsl = pack.stage_at(&path)?;
    let dispatch = match directives::work_groups(&glsl) {
        Some(groups) => Dispatch::Fixed(groups),
        None => {
            let [x, y, _] = directives::local_size(&glsl);
            Dispatch::Screen {
                scale: directives::work_groups_render(&glsl).unwrap_or([1.0, 1.0]),
                local: [x, y],
            }
        }
    };
    let shared = transform::Shared::of_compute(&glsl);
    if !shared.problems.is_empty() {
        return Err(format!("{name}: {}", shared.problems.join("; ")));
    }
    let image_formats = image_formats(pack, name, &shared)?;
    let target = transform::Target {
        set: PACK_SET,
        image_formats: &image_formats,
        ..Default::default()
    };
    let out = transform::transform(&glsl, transform::Stage::Compute, &shared, &target);
    let layout = pipeline::describe_program(pipeline::Program::Compute(&out.interface));
    undeclared_buffer(pack, name, &layout)?;
    let textures = texture_sources(pack, name, ProgramKind::Compute, &layout)?;
    let defines: Vec<(&str, &str)> = pack.environment.iter().collect();
    let wgsl = backend::to_wgsl(&out.source, backend::ShaderStage::Compute, &defines)
        .map_err(|e| format!("{name}.csh: {e}"))?;
    let used = used_bindings(&wgsl);
    let compute_parity = (!shared.images.is_empty() && transform::parity_only(&glsl))
        .then(|| {
            let variant = |parity: u32| -> Option<(String, Vec<u32>)> {
                let target = transform::Target {
                    frame_parity: Some(parity),
                    ..target
                };
                let out = transform::transform(&glsl, transform::Stage::Compute, &shared, &target);
                let wgsl =
                    backend::to_wgsl(&out.source, backend::ShaderStage::Compute, &defines).ok()?;
                let used = used_bindings(&wgsl)?;
                Some((wgsl, used))
            };
            Some([variant(0)?, variant(1)?])
        })
        .flatten();
    let mipmapped = directives::mipmapped([glsl.as_str()]);
    Ok((
        Program {
            name: name.to_owned(),
            kind: ProgramKind::Compute,
            vertex: String::new(),
            fragment: String::new(),
            fragment_tests: Vec::new(),
            alpha_override: None,
            fragment_linear: None,
            compute: Some(wgsl),
            compute_used: used,
            compute_parity,
            dispatch,
            layout,
            draw_buffers: Vec::new(),
            textures,
            mipmapped,
        },
        glsl,
    ))
}

fn image_formats<'p>(
    pack: &'p Pack,
    name: &str,
    shared: &transform::Shared,
) -> Result<Vec<(&'p str, &'p str)>, String> {
    if let Some(missing) = shared
        .images
        .iter()
        .find(|n| !pack.custom_images.iter().any(|i| i.name == **n))
    {
        return Err(format!(
            "{name}: {missing} is not declared by an image.{missing} key"
        ));
    }
    Ok(pack
        .custom_images
        .iter()
        .map(|i| (i.name.as_str(), i.format.glsl()))
        .collect())
}

fn undeclared_buffer(pack: &Pack, name: &str, layout: &Layout) -> Result<(), String> {
    for binding in &layout.bindings {
        if let Binding::Buffer { index, .. } = binding
            && !pack.buffers.iter().any(|b| b.index == *index)
        {
            return Err(format!(
                "{name}: buffer binding {index} has no bufferObject.{index} key"
            ));
        }
    }
    Ok(())
}

fn used_bindings(wgsl: &str) -> Option<Vec<u32>> {
    let module = naga::front::wgsl::parse_str(wgsl).ok()?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .ok()?;
    let entry = info.get_entry_point(0);
    let mut used: Vec<u32> = module
        .global_variables
        .iter()
        .filter(|(handle, _)| !entry[*handle].is_empty())
        .filter_map(|(_, var)| {
            var.binding
                .as_ref()
                .filter(|b| b.group == PACK_SET)
                .map(|b| b.binding)
        })
        .collect();
    used.sort_unstable();
    Some(used)
}

fn texture_sources(
    pack: &Pack,
    name: &str,
    kind: ProgramKind,
    layout: &Layout,
) -> Result<Vec<(u32, TextureSource)>, String> {
    layout
        .bindings
        .iter()
        .filter_map(|binding| match binding {
            Binding::Texture {
                binding,
                ty,
                name: sampler,
            } => Some(
                if let Some(index) = pack.images.lookup(name, sampler) {
                    TextureSource::custom(index, sampler, ty)
                } else if kind.draws_entities()
                    && matches!(sampler.as_str(), "gtexture" | "tex" | "texture")
                {
                    TextureSource::entity(sampler, ty)
                } else if let Some(index) = pack
                    .custom_images
                    .iter()
                    .position(|i| i.sampler.as_deref() == Some(sampler.as_str()))
                {
                    TextureSource::image(
                        index as u8,
                        pack.custom_images[index].dimension(),
                        sampler,
                        ty,
                    )
                } else {
                    TextureSource::resolve(sampler, ty)
                }
                .map(|s| (*binding, s)),
            ),
            _ => None,
        })
        .collect::<Result<Vec<_>, String>>()
        .map_err(|e| format!("{name}: {e}"))
}
