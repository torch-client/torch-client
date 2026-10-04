mod atomics;
mod builtins;
pub(crate) mod declarations;
mod indices;
mod interface;
pub(crate) mod lexer;
mod loops;
mod samplers;
mod scalars;
mod storage;
mod wrap;

pub(crate) use indices::parity_only;
pub(crate) use interface::{PACK_ATTRIBUTES, Varying};
pub(crate) use samplers::Sampler;
pub(crate) use storage::{Image, ImageAccess, StorageBuffer, untyped};

use declarations::PackUniform;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Vertex,
    Fragment,
    Compute,
}

pub(crate) const TARGET_VERSION: u32 = 450;

pub(crate) const UNIFORM_BINDING: u32 = 0;

fn sampler_binding(index: u32) -> u32 {
    UNIFORM_BINDING + 1 + 2 * index
}

fn first_image_binding(shared: &Shared) -> u32 {
    sampler_binding(shared.samplers.len() as u32)
}

pub(crate) const UNIFORMS: &[(&str, &str)] = &[
    ("iris_ModelViewMatrix", "mat4"),
    ("iris_ProjectionMatrix", "mat4"),
    ("iris_ModelViewProjectionMatrix", "mat4"),
    ("iris_CameraPosition", "vec4"),
    ("iris_ScreenSize", "vec4"),
    ("iris_Time", "vec4"),
];

pub(crate) const ALPHA_TEST_DEFINE: &str = "IRIS_ALPHA_TEST";

pub(crate) const ALPHA_OP_DEFINE: &str = "IRIS_ALPHA_OP";

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Target<'a> {
    pub(crate) set: u32,
    pub(crate) vertex_prelude: &'a str,
    pub(crate) fragment_prelude: &'a str,
    pub(crate) vertex_end: &'a str,
    pub(crate) vertex_begin: &'a str,
    pub(crate) frame_parity: Option<u32>,
    pub(crate) image_formats: &'a [(&'a str, &'a str)],
    pub(crate) linear_output: bool,
    pub(crate) outputs: Option<u32>,
    pub(crate) gl_clip: bool,
    pub(crate) upright: bool,
    pub(crate) attribute_defaults: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Shared {
    pub(crate) varyings: Vec<Varying>,
    pub(crate) varying_end: u32,
    pub(crate) samplers: Vec<String>,
    pub(crate) uniforms: Vec<PackUniform>,
    pub(crate) images: Vec<String>,
    pub(crate) problems: Vec<String>,
}

impl Shared {
    pub(crate) fn of(vertex_source: &str, fragment_source: &str) -> Shared {
        let vertex = normalize(vertex_source);
        let fragment = normalize(fragment_source);
        let (varyings, varying_end) = interface::interface(&vertex);
        let mut shared = Shared {
            varyings,
            varying_end,
            ..Shared::default()
        };
        for tokens in [&vertex, &fragment] {
            for name in samplers::sampler_names(tokens) {
                if !shared.samplers.contains(&name) {
                    shared.samplers.push(name);
                }
            }
            for name in storage::image_names(tokens) {
                if !shared.images.contains(&name) {
                    shared.images.push(name);
                }
            }
            for (_, _, uniform) in declarations::loose_uniforms(tokens) {
                let uniform = match uniform.and_then(|u| u.size().map(|_| u)) {
                    Ok(uniform) => uniform,
                    Err(problem) => {
                        if !shared.problems.contains(&problem) {
                            shared.problems.push(problem);
                        }
                        continue;
                    }
                };
                match shared.uniforms.iter().position(|u| u.name == uniform.name) {
                    None => shared.uniforms.push(uniform),
                    Some(known) if shared.uniforms[known].same_member(&uniform) => {
                        let known = &mut shared.uniforms[known];
                        known.initial = known.initial.take().or(uniform.initial);
                    }
                    Some(known) => {
                        let problem = format!(
                            "uniform {} is declared as {} and as {}",
                            uniform.name, shared.uniforms[known].ty, uniform.ty
                        );
                        shared.problems.push(problem);
                    }
                }
            }
        }
        shared
    }

    pub(crate) fn of_compute(source: &str) -> Shared {
        Shared::of("", source)
    }
}

fn normalize(source: &str) -> Vec<lexer::Token> {
    let mut tokens = declarations::split_declarator_tokens(lexer::lex(source));
    declarations::rename_texture_sampler_tokens(&mut tokens);
    tokens
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Interface {
    pub(crate) samplers: Vec<Sampler>,
    pub(crate) attributes: Vec<u32>,
    pub(crate) outputs: u32,
    pub(crate) uniforms: Vec<PackUniform>,
    pub(crate) images: Vec<Image>,
    pub(crate) buffers: Vec<StorageBuffer>,
}

#[derive(Clone, Debug)]
pub(crate) struct Transformed {
    pub(crate) source: String,
    pub(crate) interface: Interface,
}

pub(crate) fn transform(
    source: &str,
    stage: Stage,
    shared: &Shared,
    target: &Target,
) -> Transformed {
    let mut tokens = normalize(source);
    let mut interface = Interface {
        uniforms: shared.uniforms.clone(),
        ..Interface::default()
    };

    wrap::version(&mut tokens);
    if let Some(parity) = target.frame_parity {
        indices::fold_frame_parity(&mut tokens, parity);
    }
    indices::drop_unreachable(&mut tokens);
    scalars::scalar_swizzles(&mut tokens);
    atomics::lower_image_atomics(&mut tokens);
    loops::sequence_increments(&mut tokens);
    interface::keywords(&mut tokens, stage, &shared.varyings);
    builtins::unsigned_literals(&mut tokens);
    declarations::gather_uniforms(&mut tokens);
    declarations::size_unsized_arrays(&mut tokens);
    indices::clamp_array_indices(&mut tokens, &shared.uniforms);
    let used = builtins::builtins(&mut tokens, stage);
    if stage != Stage::Fragment {
        builtins::explicit_lod(&mut tokens);
    }
    samplers::split_samplers(&mut tokens, &mut interface, &shared.samplers, target.set);
    storage::bind_images(&mut tokens, &mut interface, shared, target);
    storage::bind_buffers(&mut tokens, &mut interface, shared, target);
    samplers::split_parameters(&mut tokens);
    builtins::outputs(&mut tokens, &mut interface, stage);
    interface::prune_attributes(&mut tokens, stage);
    interface::pack_attributes(&mut tokens, stage, &mut interface, target);
    declarations::hoist_initializers(&mut tokens);
    match stage {
        Stage::Fragment => wrap::epilogue(&mut tokens, &interface, target),
        Stage::Vertex if target.gl_clip => wrap::vertex_epilogue(
            &mut tokens,
            target.upright,
            target.vertex_begin,
            target.vertex_end,
        ),
        Stage::Vertex | Stage::Compute => {}
    }
    let guarded = loops::guard_loops(&mut tokens);
    wrap::preamble(&mut tokens, stage, &interface, &used, target, guarded);

    Transformed {
        source: lexer::render(&tokens),
        interface,
    }
}

#[cfg(test)]
const TEST_SET: u32 = 3;

#[cfg(test)]
const TEST_PRELUDE: &str = "layout(location = 0) in vec3 test_Position;\n\
    vec3 iris_position() { return test_Position; }\n\
    vec2 iris_uv0() { return vec2(0.0); }\n\
    vec2 iris_light() { return vec2(240.0); }\n\
    vec4 iris_color() { return vec4(1.0); }\n\
    vec3 iris_normal() { return vec3(0.0, 1.0, 0.0); }\n\
    vec4 iris_entity() { return vec4(-1.0, 0.0, 0.0, 1.0); }\n\
    vec4 iris_tangent() { return vec4(1.0, 0.0, 0.0, 1.0); }\n\
    vec4 iris_mid_uv() { return vec4(0.0, 0.0, 0.0, 1.0); }\n\
    vec4 iris_mid_block() { return vec4(0.0); }";

#[cfg(test)]
pub(crate) const TEST_TARGET: Target<'static> = Target {
    set: TEST_SET,
    vertex_prelude: TEST_PRELUDE,
    fragment_prelude: "",
    vertex_end: "",
    vertex_begin: "",
    frame_parity: None,
    image_formats: &[],
    linear_output: false,
    outputs: None,
    gl_clip: false,
    upright: false,
    attribute_defaults: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    const TARGET: Target<'static> = TEST_TARGET;

    fn vertex(source: &str) -> String {
        transform(source, Stage::Vertex, &Shared::of(source, ""), &TARGET).source
    }

    fn fragment(source: &str) -> String {
        transform(
            source,
            Stage::Fragment,
            &Shared::of(source, source),
            &TARGET,
        )
        .source
    }

    #[test]
    fn the_version_a_pack_ships_is_promoted() {
        for source in [
            "#version 130\nvoid main() {}\n",
            "#version 430 compatibility\nvoid main() {}\n",
            "void main() {}\n",
        ] {
            let out = vertex(source);
            assert!(out.starts_with("#version 450\n"), "{out}");
            assert!(!out.contains("compatibility"), "{out}");
        }
    }

    #[test]
    fn varying_becomes_an_output_or_an_input_by_stage() {
        let source = "#version 130\nvarying vec2 uv;\n";
        assert!(vertex(source).contains("out vec2 uv;"));
        assert!(fragment(source).contains("in vec2 uv;"));
    }

    #[test]
    fn a_name_that_merely_contains_a_keyword_is_left_alone() {
        let out = vertex("#version 130\nfloat attributes; // attribute\n");
        assert!(out.contains("float attributes;"), "{out}");
        assert!(out.contains("// attribute"), "{out}");
    }

    #[test]
    fn outputs_are_capped_and_the_clip_position_is_converted() {
        let fsh = "#version 130\nvoid main() { gl_FragData[0] = vec4(1.0); gl_FragData[1] = vec4(0.0); }\n";
        let target = Target {
            outputs: Some(1),
            ..TARGET
        };
        let out = transform(fsh, Stage::Fragment, &Shared::default(), &target).source;
        assert!(out.contains("out vec4 iris_Out0;"), "{out}");
        assert!(!out.contains("out vec4 iris_Out1;"), "{out}");
        assert!(out.contains("vec4 iris_FragData1;"), "{out}");

        let vsh = "#version 130\nvoid main() { gl_Position = ftransform(); }\n";
        let target = Target {
            gl_clip: true,
            ..TARGET
        };
        let out = transform(vsh, Stage::Vertex, &Shared::of(vsh, ""), &target).source;
        assert!(out.contains("void iris_main()"), "{out}");
        assert!(
            out.contains("gl_Position.z = (gl_Position.z + gl_Position.w) * 0.5;"),
            "{out}"
        );
        assert_eq!(out.matches("void main()").count(), 1, "{out}");
    }

    #[test]
    fn a_compute_program_reaches_wgsl() {
        let csh = "#version 430 compatibility\nlayout (local_size_x = 8, local_size_y = 8, local_size_z = 8) in;\n\
                   uniform int frameCounter;\nuniform usampler3D voxelSampler;\nwriteonly uniform image3D light_img;\n\
                   void main() { ivec3 pos = ivec3(gl_GlobalInvocationID); uint v = texelFetch(voxelSampler, pos, 0).r;\n\
                   imageStore(light_img, pos, vec4(float(v) + float(frameCounter))); }\n";
        let shared = Shared::of("", csh);
        let target = Target {
            image_formats: &[("light_img", "rgba16f")],
            ..TARGET
        };
        let out = transform(csh, Stage::Compute, &shared, &target);
        let wgsl = crate::shaderpack::backend::to_wgsl(
            &out.source,
            crate::shaderpack::backend::ShaderStage::Compute,
            &[],
        )
        .unwrap_or_else(|e| panic!("{e}\n\n{}", out.source));
        assert!(wgsl.contains("@compute @workgroup_size(8, 8, 8)"), "{wgsl}");
        assert!(
            wgsl.contains("texture_storage_3d<rgba16float,write>"),
            "{wgsl}"
        );
    }

    #[test]
    fn a_storage_buffer_is_rebound() {
        let csh = "#version 430\nlayout (local_size_x = 1) in;\nlayout(std430, binding = 2) readonly buffer Data { vec4 values[]; } data;\n\
                   uniform sampler2D tex;\nwriteonly uniform image2D out_img;\n\
                   void main() { imageStore(out_img, ivec2(0), data.values[0] + texelFetch(tex, ivec2(0), 0)); }\n";
        let shared = Shared::of("", csh);
        let target = Target {
            image_formats: &[("out_img", "rgba16f")],
            ..TARGET
        };
        let out = transform(csh, Stage::Compute, &shared, &target);
        assert_eq!(out.interface.buffers.len(), 1, "{}", out.source);
        let buffer = &out.interface.buffers[0];
        assert_eq!((buffer.index, buffer.read_only), (2, true));
        assert_eq!(buffer.binding, UNIFORM_BINDING + 1 + 2 + 1 + 2);
        let wgsl = crate::shaderpack::backend::to_wgsl(
            &out.source,
            crate::shaderpack::backend::ShaderStage::Compute,
            &[],
        )
        .unwrap_or_else(|e| panic!("{e}\n\n{}", out.source));
        assert!(wgsl.contains("var<storage> data"), "{wgsl}");
    }

    #[test]
    fn vertex_sampling_gets_an_explicit_level() {
        let vsh = "#version 130\nuniform sampler2D noisetex;\nvoid main() { gl_Position = vec4(texture2D(noisetex, vec2(0.5)).r); }\n";
        let out = transform(vsh, Stage::Vertex, &Shared::of(vsh, ""), &TARGET);
        assert!(out.source.contains("textureLod("), "{}", out.source);
        crate::shaderpack::backend::to_wgsl(
            &out.source,
            crate::shaderpack::backend::ShaderStage::Vertex,
            &[],
        )
        .unwrap_or_else(|e| panic!("{e}\n\n{}", out.source));
    }

    #[test]
    fn a_literal_beside_a_uint_is_made_unsigned() {
        let out = fragment(
            "#version 430\nuint f(uint id) { return max(id - 1, 1); }\nint g(int n) { return max(n, 1); }\n\
             void main() { gl_FragData[0] = vec4(float(f(3u)) + float(g(2))); }\n",
        );
        assert!(out.contains("max(id - 1, 1u)"), "{out}");
        assert!(out.contains("max(n, 1)"), "{out}");
    }

    #[test]
    fn an_image_gets_its_format_and_binding() {
        let vsh = "#version 430 compatibility\nwriteonly uniform uimage3D voxel_img;\nuniform sampler2D tex;\n\
                   void main() { imageStore(voxel_img, ivec3(1, 2, 3), uvec4(7u)); gl_Position = vec4(texture2DLod(tex, vec2(0.0), 0.0).x); }\n";
        let shared = Shared::of(vsh, "");
        assert_eq!(shared.images, ["voxel_img"]);
        let target = Target {
            image_formats: &[("voxel_img", "r8ui")],
            ..TARGET
        };
        let out = transform(vsh, Stage::Vertex, &shared, &target);
        assert_eq!(out.interface.images.len(), 1);
        assert_eq!(out.interface.images[0].binding, UNIFORM_BINDING + 3);
        assert_eq!(out.interface.images[0].access, ImageAccess::Write);
        let wgsl = crate::shaderpack::backend::to_wgsl(
            &out.source,
            crate::shaderpack::backend::ShaderStage::Vertex,
            &[],
        )
        .unwrap_or_else(|e| panic!("{e}\n\n{}", out.source));
        assert!(wgsl.contains("texture_storage_3d<r8uint"), "{wgsl}");
    }

    #[test]
    fn a_format_attribute_takes_a_default_when_asked() {
        let vsh = "#version 130\nattribute vec4 mc_Entity;\nattribute vec4 at_tangent;\nvarying float id;\n\
                   void main() { id = mc_Entity.x + at_tangent.w; gl_Position = vec4(0.0); }\n";
        let target = Target {
            attribute_defaults: true,
            ..TARGET
        };
        let result = transform(vsh, Stage::Vertex, &Shared::of(vsh, ""), &target);
        assert!(
            result.interface.attributes.is_empty(),
            "{:?}",
            result.interface.attributes
        );
        assert!(
            result.source.contains("mc_Entity = vec4(iris_entity());"),
            "{}",
            result.source
        );
        assert!(
            result.source.contains("at_tangent = vec4(iris_tangent());"),
            "{}",
            result.source
        );
        assert!(
            !result.source.contains("in vec4 mc_Entity"),
            "{}",
            result.source
        );
    }

    #[test]
    fn a_comma_increment_keeps_every_part() {
        let out = fragment(
            "#version 130\nvoid main() { float a = 0.0; vec3 p = vec3(0.0);\n\
             for (int i = 0; i < 4; i++, p += vec3(1.0), a += 1.0) {\n\
                 if (a > 2.0) continue;\n\
                 for (int j = 0; j < 2; j++) { continue; }\n\
             }\n\
             for (int k = 0; k < 3; k++, a += 2.0) a -= 1.0;\n\
             gl_FragData[0] = vec4(p, a); }\n",
        );
        assert!(
            out.contains("i < 4) && (iris_LoopBudget-- > 0); a += 1.0)"),
            "{out}"
        );
        assert!(
            out.contains("if (a > 2.0) { i++; p += vec3(1.0); continue; }"),
            "{out}"
        );
        assert!(out.contains("j++) { continue; }"), "{out}");
        assert!(out.contains("i++; p += vec3(1.0); }"), "{out}");
        assert!(out.contains("a += 2.0) { a -= 1.0;k++;  }"), "{out}");
        crate::shaderpack::backend::to_wgsl(
            &out,
            crate::shaderpack::backend::ShaderStage::Fragment,
            &[],
        )
        .unwrap_or_else(|e| panic!("{e}\n{out}"));
    }

    #[test]
    fn loops_are_bounded_by_one_budget() {
        let out = fragment(
            "#version 130\nvoid main() { float a = 0.0;\n\
             for (int i = 0; i < n; i++) a += 1.0;\n\
             if (a > 0.0) while (a > 1.0) a -= 1.0;\n\
             for (;;) break;\n\
             do { a += 1.0; } while (a < 3.0);\n\
             gl_FragData[0] = vec4(a); }\n",
        );
        assert!(out.contains("int iris_LoopBudget = 4096;"), "{out}");
        assert!(
            out.contains("for (int i = 0; (i < n) && (iris_LoopBudget-- > 0); i++)"),
            "{out}"
        );
        assert!(
            out.contains("if (a > 0.0) while ((a > 1.0) && (iris_LoopBudget-- > 0))"),
            "{out}"
        );
        assert!(out.contains("for (; iris_LoopBudget-- > 0;)"), "{out}");
        assert!(
            out.contains("while ((a < 3.0) && (iris_LoopBudget-- > 0));"),
            "{out}"
        );
        crate::shaderpack::backend::to_wgsl(
            &out,
            crate::shaderpack::backend::ShaderStage::Fragment,
            &[("n", "4")],
        )
        .unwrap_or_else(|e| panic!("{e}\n{out}"));
    }

    #[test]
    fn a_shader_without_loops_declares_no_budget() {
        assert!(
            !fragment("#version 130\nvoid main() { gl_FragData[0] = vec4(1.0); }\n")
                .contains("iris_LoopBudget")
        );
    }

    #[test]
    fn names_inside_a_define_body_are_rewritten() {
        let out = fragment(
            "#version 130\nuniform sampler2D gtexture;\n#define ALBEDO(uv) texture2D(gtexture, uv)\n#define texture2D_x 1\n\
             void main() { gl_FragData[0] = ALBEDO(vec2(0.0)); }\n",
        );
        assert!(
            out.contains("#define ALBEDO(uv) texture(sampler2D(gtexture, gtexture_sampler), uv)"),
            "{out}"
        );
        assert!(out.contains("#define texture2D_x 1"), "{out}");
    }

    #[test]
    fn a_local_named_like_a_sampler_is_left_alone() {
        let out = fragment(
            "#version 130\nuniform sampler2D specular;\n\
             vec3 ggx(vec3 n) { vec3 specular = n * 2.0; return specular; }\n\
             float shine(float specular) { return specular; }\n\
             void main() { gl_FragData[0] = texture2D(specular, vec2(0.0)) + vec4(ggx(vec3(1.0)), shine(1.0)); }\n",
        );
        assert!(
            out.contains("vec3 specular = n * 2.0; return specular;"),
            "{out}"
        );
        assert!(
            out.contains("float shine(float specular) { return specular; }"),
            "{out}"
        );
        assert!(
            out.contains("texture(sampler2D(specular, specular_sampler), vec2(0.0))"),
            "{out}"
        );
    }

    #[test]
    fn varying_locations_count_matrices_and_arrays() {
        let found = interface::interface(&lexer::lex(
            "out mat4 m;\nout vec3 a[2];\nflat out int id;\n",
        ))
        .0;
        let locations: Vec<(&str, u32)> = found
            .iter()
            .map(|v| (v.name.as_str(), v.location))
            .collect();
        assert_eq!(locations, [("m", 0), ("a", 4), ("id", 6)]);
        let out = vertex(
            "#version 130\nout vec3 a[2];\nout float b;\nvoid main() { a[0] = vec3(0.0); a[1] = vec3(1.0); b = 1.0; }\n",
        );
        assert!(out.contains("layout(location = 0) out vec3 a[2];"), "{out}");
        assert!(out.contains("layout(location = 2) out float b;"), "{out}");
    }

    #[test]
    fn an_attribute_used_in_a_macro_is_kept() {
        let out = transform(
            "#version 130\nattribute vec4 mc_Entity;\n#define ID (mc_Entity.x)\nvoid main() { gl_Position = vec4(ID); }\n",
            Stage::Vertex,
            &Shared::default(),
            &TARGET,
        );
        assert!(out.source.contains("in vec4 mc_Entity;"), "{}", out.source);
    }

    #[test]
    fn an_input_nothing_writes_becomes_a_global() {
        let vsh = "#version 130\nvarying vec2 uv;\nvoid main() { uv = vec2(0.0); }\n";
        let fsh = "#version 130\nvarying vec2 uv;\nflat in int orphan;\nvoid main() { gl_FragData[0] = vec4(uv, float(orphan), 1.0); }\n";
        let out = transform(fsh, Stage::Fragment, &Shared::of(vsh, fsh), &TARGET).source;
        assert!(out.contains("layout(location = 0) in vec2 uv;"), "{out}");
        assert!(out.contains("int orphan;"), "{out}");
        assert!(!out.contains("flat"), "{out}");
        assert!(!out.contains("in int orphan"), "{out}");
    }

    #[test]
    fn a_combined_sampler_splits_into_two_bindings() {
        let out = fragment(
            "#version 130\nuniform sampler2D gtexture;\nvoid main() { gl_FragData[0] = texture2D(gtexture, vec2(0.0)); }\n",
        );
        assert!(out.contains("uniform texture2D gtexture;"), "{out}");
        assert!(out.contains("uniform sampler gtexture_sampler;"), "{out}");
        assert!(
            out.contains("texture(sampler2D(gtexture, gtexture_sampler)"),
            "{out}"
        );
        assert!(!out.contains("texture2D sampler2D("), "{out}");
        assert!(!out.contains("uniform texture gtexture"), "{out}");
    }

    #[test]
    fn a_sampler_has_the_same_binding_in_both_stages() {
        let vsh = "#version 130\nuniform sampler2D lightmap;\nvoid main() { gl_Position = texture2DLod(lightmap, vec2(0.0), 0.0); }\n";
        let fsh = "#version 130\nuniform sampler2D gtexture;\nuniform sampler2D lightmap;\n\
                   void main() { gl_FragData[0] = texture2D(gtexture, vec2(0.0)) * texture2D(lightmap, vec2(0.0)); }\n";
        let shared = Shared::of(vsh, fsh);
        assert_eq!(shared.samplers, ["lightmap", "gtexture"]);

        let binding = |source: &str, stage: Stage, name: &str| {
            transform(source, stage, &shared, &TARGET)
                .interface
                .samplers
                .into_iter()
                .find(|s| s.name == name)
                .map(|s| s.texture_binding)
        };
        assert_eq!(binding(vsh, Stage::Vertex, "lightmap"), Some(1));
        assert_eq!(binding(fsh, Stage::Fragment, "lightmap"), Some(1));
        assert_eq!(binding(fsh, Stage::Fragment, "gtexture"), Some(3));
    }

    #[test]
    fn fragment_outputs_are_wrapped_and_counted() {
        let source = "#version 130\nvoid main() { gl_FragData[0] = vec4(1.0); gl_FragData[2] = vec4(0.0); }\n";
        let result = transform(source, Stage::Fragment, &Shared::default(), &TARGET);
        assert_eq!(result.interface.outputs, 3);
        let out = &result.source;
        assert!(
            out.contains("layout(location = 2) out vec4 iris_Out2;"),
            "{out}"
        );
        assert!(out.contains("vec4 iris_FragData2;"), "{out}");
        assert!(out.contains("void iris_main()"), "{out}");
        assert!(out.contains("iris_FragData0 = vec4(1.0)"), "{out}");
        assert!(out.contains("iris_Out2 = iris_FragData2;"), "{out}");
        assert!(
            out.contains(&format!("#ifdef {ALPHA_TEST_DEFINE}")),
            "{out}"
        );
        assert_eq!(out.matches("void main()").count(), 1, "{out}");
    }

    #[test]
    fn the_first_output_is_made_linear_when_asked() {
        let source = "#version 130\nvoid main() { gl_FragData[0] = vec4(0.5); }\n";
        let target = Target {
            linear_output: true,
            ..TARGET
        };
        let out = transform(source, Stage::Fragment, &Shared::default(), &target).source;
        assert!(out.contains("iris_Out0 = vec4(iris_Linear"), "{out}");
    }

    #[test]
    fn the_builtins_are_served_by_the_prelude() {
        let source =
            "#version 130\nvarying vec4 t;\nvoid main() { t = gl_Color * gl_MultiTexCoord1; }\n";
        let result = transform(source, Stage::Vertex, &Shared::of(source, ""), &TARGET);
        assert!(result.interface.attributes.is_empty());
        assert!(
            result
                .source
                .contains("t = iris_color() * vec4(iris_light(), 0.0, 1.0);"),
            "{}",
            result.source
        );
        assert!(result.source.contains(TEST_PRELUDE), "{}", result.source);
    }

    #[test]
    fn an_attribute_that_is_never_read_is_dropped() {
        let unused =
            "#version 130\nattribute vec3 mc_Entity;\nvoid main() { gl_Position = vec4(0.0); }\n";
        let out = transform(unused, Stage::Vertex, &Shared::default(), &TARGET);
        assert!(!out.source.contains("mc_Entity"), "{}", out.source);
        assert!(out.interface.attributes.is_empty());

        let used = "#version 130\nattribute vec3 mc_Entity;\nvoid main() { gl_Position = vec4(mc_Entity, 1.0); }\n";
        let out = transform(used, Stage::Vertex, &Shared::default(), &TARGET);
        assert!(
            out.source
                .contains("layout(location = 5) in vec3 mc_Entity;"),
            "{}",
            out.source
        );
        assert_eq!(out.interface.attributes, [5]);
    }

    #[test]
    fn the_fixed_function_transform_becomes_a_helper() {
        let out = vertex("#version 130\nvoid main() { gl_Position = ftransform(); }\n");
        assert!(out.contains("vec4 iris_ftransform()"), "{out}");
        assert!(out.contains("gl_Position = iris_ftransform();"), "{out}");
        assert!(out.contains("vec4(iris_position(), 1.0)"), "{out}");
    }

    #[test]
    fn the_texture_matrix_is_folded_to_a_constant() {
        let out =
            vertex("#version 130\nvoid main() { vec4 c = gl_TextureMatrix[0] * vec4(1.0); }\n");
        assert!(out.contains("mat4(1.0) * vec4(1.0)"), "{out}");
        assert!(!out.contains("gl_TextureMatrix"), "{out}");

        let out = vertex(
            "#version 130\nvoid main() { vec4 c = gl_TextureMatrix[1] * gl_MultiTexCoord1; }\n",
        );
        assert!(
            out.contains("vec4(0.03125, 0.03125, 0.03125, 1.0)"),
            "{out}"
        );
    }

    #[test]
    fn a_program_in_a_packs_dialect_reaches_wgsl() {
        use crate::shaderpack::backend::{self, ShaderStage};

        let vsh = "#version 130\nattribute vec3 mc_Entity;\nvarying vec2 texcoord;\nvarying vec2 lmcoord;\nvarying vec4 tint;\n\
                   void main() { gl_Position = ftransform(); texcoord = (gl_TextureMatrix[0] * gl_MultiTexCoord0).xy;\n\
                   lmcoord = (gl_TextureMatrix[1] * gl_MultiTexCoord1).xy; tint = gl_Color; }\n";
        let fsh = "#version 130\nuniform sampler2D gtexture;\nuniform sampler2D lightmap;\nvarying vec2 texcoord;\nvarying vec2 lmcoord;\nvarying vec4 tint;\n\
                   void main() { gl_FragData[0] = texture2D(gtexture, texcoord) * texture2D(lightmap, lmcoord) * tint; }\n";
        let shared = Shared::of(vsh, fsh);
        let target = Target {
            linear_output: true,
            ..TARGET
        };

        let vertex = transform(vsh, Stage::Vertex, &shared, &target);
        backend::to_wgsl(&vertex.source, ShaderStage::Vertex, &[])
            .unwrap_or_else(|e| panic!("{e}\n\n{}", vertex.source));

        let fragment = transform(fsh, Stage::Fragment, &shared, &target);
        for defines in [
            &[][..],
            &[(ALPHA_TEST_DEFINE, "0.1")][..],
            &[(ALPHA_TEST_DEFINE, "0.5"), (ALPHA_OP_DEFINE, ">=")][..],
        ] {
            let wgsl = backend::to_wgsl(&fragment.source, ShaderStage::Fragment, defines)
                .unwrap_or_else(|e| panic!("{e}\n\n{}", fragment.source));
            assert_eq!(wgsl.contains("discard"), !defines.is_empty(), "{wgsl}");
        }
    }

    fn wgsl(source: &str, stage: crate::shaderpack::backend::ShaderStage) -> String {
        crate::shaderpack::backend::to_wgsl(source, stage, &[])
            .unwrap_or_else(|e| panic!("{e}\n\n{source}"))
    }

    #[test]
    fn a_sized_constructor_array_is_clamped() {
        let out = fragment(
            "#version 430\nuniform int k;\nconst vec3[] tints = vec3[](vec3(1.0), vec3(0.5), vec3(0.0));\n\
             void main() { gl_FragData[0] = vec4(tints[k], 1.0); }\n",
        );
        assert!(out.contains("vec3[3] tints = vec3[3]("), "{out}");
        assert!(out.contains("tints[clamp(int(k), 0, 2)]"), "{out}");
        wgsl(&out, crate::shaderpack::backend::ShaderStage::Fragment);
    }

    #[test]
    fn a_uniform_array_index_is_clamped() {
        let out = fragment(
            "#version 430\nuniform vec4 lights[4];\nuniform int k;\nvoid main() { gl_FragData[0] = lights[k]; }\n",
        );
        assert!(out.contains("lights[clamp(int(k), 0, 3)]"), "{out}");
        wgsl(&out, crate::shaderpack::backend::ShaderStage::Fragment);
    }

    #[test]
    fn define_bodies_get_every_expression_rewrite() {
        let out = fragment(
            "#version 130\n#define WRITE(c) gl_FragData[1] = c\nvoid main() { WRITE(vec4(1.0)); }\n",
        );
        assert!(out.contains("#define WRITE(c) iris_FragData1 = c"), "{out}");
        assert!(
            out.contains("out vec4 iris_Out1;"),
            "the output counts: {out}"
        );
        wgsl(&out, crate::shaderpack::backend::ShaderStage::Fragment);

        let vsh = "#version 130\nuniform sampler2D noisetex;\n#define LIGHT (gl_TextureMatrix[1] * gl_MultiTexCoord1)\n\
                   #define NOISE(p) texture2D(noisetex, p)\nvoid main() { gl_Position = LIGHT * NOISE(vec2(0.5)).r; }\n";
        let out = vertex(vsh);
        assert!(out.contains("#define LIGHT (mat4(vec4(0.00390625"), "{out}");
        assert!(
            out.contains("textureLod(sampler2D(noisetex, noisetex_sampler), p, 0.0)"),
            "{out}"
        );
        wgsl(&out, crate::shaderpack::backend::ShaderStage::Vertex);

        let out = fragment(
            "#version 430\nuint id = 3u;\n#define PREV max(id - 1, 1)\nvoid main() { gl_FragData[0] = vec4(float(PREV)); }\n",
        );
        assert!(out.contains("#define PREV max(id - 1, 1u)"), "{out}");
    }

    #[test]
    fn declared_fragment_outputs_are_wrapped() {
        let out = fragment(
            "#version 430\nout vec4 color;\nlayout(location = 2) out vec4 data;\n\
             void main() { color = vec4(1.0); data = vec4(0.5); }\n",
        );
        assert!(
            out.contains("iris_FragData0 = vec4(1.0)")
                && out.contains("iris_FragData2 = vec4(0.5)"),
            "{out}"
        );
        assert!(
            out.contains("layout(location = 2) out vec4 iris_Out2;"),
            "{out}"
        );
        assert!(!out.contains("out vec4 color"), "{out}");
        wgsl(&out, crate::shaderpack::backend::ShaderStage::Fragment);
    }

    #[test]
    fn the_preamble_follows_the_extensions() {
        let out = fragment(
            "#version 430\n#extension GL_ARB_shader_texture_lod : enable\nvoid main() { gl_FragData[0] = vec4(1.0); }\n",
        );
        let extension = out.find("#extension").expect("kept");
        assert!(
            extension < out.find("uniform IrisFrame").expect("declared"),
            "{out}"
        );
        wgsl(&out, crate::shaderpack::backend::ShaderStage::Fragment);
    }

    #[test]
    fn an_image_with_its_own_layout_reaches_wgsl() {
        let csh = "#version 430\nlayout (local_size_x = 1) in;\nlayout (r32ui) uniform uimage3D vox;\n\
                   void main() { imageAtomicMax(vox, ivec3(0), 7u); }\n";
        let target = Target {
            image_formats: &[("vox", "r32ui")],
            ..TARGET
        };
        let out = transform(csh, Stage::Compute, &Shared::of("", csh), &target);
        assert_eq!(out.interface.images[0].access, ImageAccess::ReadWrite);
        wgsl(
            &out.source,
            crate::shaderpack::backend::ShaderStage::Compute,
        );
    }
}
