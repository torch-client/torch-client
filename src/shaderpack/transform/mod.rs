pub(crate) mod lexer;

use std::collections::BTreeMap;

use lexer::{Kind, Token};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Vertex,
    Fragment,
}

const TARGET_VERSION: u32 = 450;

pub(crate) const UNIFORM_BINDING: u32 = 0;

const LOC_POSITION: u32 = 0;
const LOC_UV0: u32 = 1;
const LOC_UV2: u32 = 2;
const LOC_COLOR: u32 = 3;
const LOC_NORMAL: u32 = 4;

const PACK_ATTRIBUTES: &[(&str, u32)] = &[
    ("mc_Entity", 5),
    ("mc_midTexCoord", 6),
    ("at_tangent", 7),
    ("at_midBlock", 8),
];

const LOC_UNKNOWN_BASE: u32 = 9;

pub(crate) const UNIFORMS: &[(&str, &str)] = &[
    ("iris_ModelViewMatrix", "mat4"),
    ("iris_ProjectionMatrix", "mat4"),
    ("iris_CameraPosition", "vec4"),
    ("iris_ScreenSize", "vec4"),
    ("iris_Time", "vec4"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Sampler {
    pub(crate) name: String,
    pub(crate) dimension: String,
    pub(crate) texture_binding: u32,
    pub(crate) sampler_binding: u32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Interface {
    pub(crate) samplers: Vec<Sampler>,
    pub(crate) attributes: Vec<u32>,
    pub(crate) outputs: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Transformed {
    pub(crate) source: String,
    pub(crate) interface: Interface,
}

pub(crate) fn transform(source: &str, stage: Stage, varyings: &[String], set: u32) -> Transformed {
    let mut tokens = lexer::lex(source);
    let mut interface = Interface::default();

    version(&mut tokens);
    keywords(&mut tokens, stage, varyings);
    let used = builtins(&mut tokens, stage, &mut interface);
    split_samplers(&mut tokens, &mut interface, set);
    outputs(&mut tokens, &mut interface);
    prune_attributes(&mut tokens, stage);
    pack_attributes(&mut tokens, stage, &mut interface);
    preamble(&mut tokens, stage, &interface, &used, set);

    Transformed {
        source: lexer::render(&tokens),
        interface,
    }
}

fn version(tokens: &mut Vec<Token>) {
    let line = format!("#version {TARGET_VERSION}");
    match tokens
        .iter_mut()
        .find(|t| t.kind == Kind::Directive && t.text.trim_start().starts_with("#version"))
    {
        Some(directive) => directive.text = line,
        None => {
            tokens.insert(0, Token::directive(&line));
            tokens.insert(1, Token::newline());
        }
    }
}

pub(crate) fn varyings(vertex_source: &str) -> Vec<String> {
    let tokens = lexer::lex(vertex_source);
    let mut names = Vec::new();
    let mut depth = 0i32;

    for at in 0..tokens.len() {
        if tokens[at].is_punct('{') {
            depth += 1;
        } else if tokens[at].is_punct('}') {
            depth -= 1;
        }
        if depth != 0 || !(tokens[at].is_word("varying") || tokens[at].is_word("out")) {
            continue;
        }
        if let Some([_ty, name, end]) = lexer::solid_run::<3>(&tokens, at + 1)
            && tokens[name].kind == Kind::Word
            && tokens[end].is_punct(';')
        {
            names.push(tokens[name].text.clone());
        }
    }
    names.dedup();
    names
}

fn keywords(tokens: &mut [Token], stage: Stage, varyings: &[String]) {
    let keyword = match stage {
        Stage::Vertex => "out",
        Stage::Fragment => "in",
    };
    let mut depth = 0i32;

    for at in 0..tokens.len() {
        if tokens[at].is_punct('{') {
            depth += 1;
        } else if tokens[at].is_punct('}') {
            depth -= 1;
        }
        if tokens[at].is_word("attribute") {
            tokens[at].text = "in".to_owned();
            continue;
        }
        let is_varying = tokens[at].is_word("varying");
        if depth != 0 || !(is_varying || tokens[at].is_word(keyword)) {
            continue;
        }
        let Some([_ty, name, end]) = lexer::solid_run::<3>(tokens, at + 1) else {
            continue;
        };
        if tokens[name].kind != Kind::Word || !tokens[end].is_punct(';') {
            continue;
        }
        let Some(location) = varyings.iter().position(|v| *v == tokens[name].text) else {
            if is_varying {
                tokens[at].text = keyword.to_owned();
            }
            continue;
        };
        tokens[at].text = format!("layout(location = {location}) {keyword}");
    }
}

fn split_samplers(tokens: &mut Vec<Token>, interface: &mut Interface, set: u32) {
    let mut declarations: Vec<(usize, String, String)> = Vec::new();
    let mut binding = UNIFORM_BINDING + 1;

    for at in 0..tokens.len() {
        if !tokens[at].is_word("uniform") {
            continue;
        }
        let Some([ty, name]) = lexer::solid_run::<2>(tokens, at + 1) else {
            continue;
        };
        let Some(dimension) = tokens[ty]
            .text
            .strip_prefix("sampler")
            .filter(|d| matches!(*d, "1D" | "2D" | "3D" | "Cube"))
        else {
            continue;
        };
        if tokens[name].kind != Kind::Word {
            continue;
        }
        let dimension = dimension.to_owned();
        interface.samplers.push(Sampler {
            name: tokens[name].text.clone(),
            dimension: dimension.clone(),
            texture_binding: binding,
            sampler_binding: binding + 1,
        });
        declarations.push((ty, tokens[name].text.clone(), dimension));
        binding += 2;
    }

    if interface.samplers.is_empty() {
        return;
    }

    let declared: Vec<usize> = declarations
        .iter()
        .filter_map(|(ty, _, _)| lexer::next_solid(tokens, ty + 1))
        .collect();
    for at in 0..tokens.len() {
        if declared.contains(&at) || tokens[at].kind != Kind::Word {
            continue;
        }
        if let Some(sampler) = interface
            .samplers
            .iter()
            .find(|s| s.name == tokens[at].text)
        {
            tokens[at].text = format!(
                "sampler{}({}, {}_sampler)",
                sampler.dimension, sampler.name, sampler.name
            );
        }
    }

    for (ty, name, dimension) in declarations.into_iter().rev() {
        let sampler = interface
            .samplers
            .iter()
            .find(|s| s.name == name)
            .expect("just recorded");
        tokens[ty].text = format!("texture{dimension}");
        let uniform = (0..ty).rev().find(|at| tokens[*at].is_word("uniform"));
        if let Some(uniform) = uniform {
            tokens[uniform].text = format!(
                "layout(set = {set}, binding = {}) uniform",
                sampler.texture_binding
            );
        }
        if let Some(semi) = (ty..tokens.len()).find(|at| tokens[*at].is_punct(';')) {
            tokens.insert(
                semi + 1,
                Token::raw(&format!(
                    "\nlayout(set = {set}, binding = {}) uniform sampler {name}_sampler;",
                    sampler.sampler_binding
                )),
            );
        }
    }
}

fn prune_attributes(tokens: &mut Vec<Token>, stage: Stage) {
    if stage != Stage::Vertex {
        return;
    }
    let mut depth = 0i32;
    let mut doomed: Vec<(usize, usize)> = Vec::new();

    for at in 0..tokens.len() {
        if tokens[at].is_punct('{') {
            depth += 1;
        } else if tokens[at].is_punct('}') {
            depth -= 1;
        }
        if depth != 0 || !tokens[at].is_word("in") {
            continue;
        }
        let Some([_ty, name, end]) = lexer::solid_run::<3>(tokens, at + 1) else {
            continue;
        };
        if tokens[name].kind != Kind::Word || !tokens[end].is_punct(';') {
            continue;
        }
        let uses = tokens
            .iter()
            .filter(|t| t.kind == Kind::Word && t.text == tokens[name].text)
            .count();
        if uses == 1 {
            doomed.push((at, end));
        }
    }

    for (from, to) in doomed.into_iter().rev() {
        tokens.drain(from..=to);
    }
}

fn pack_attributes(tokens: &mut [Token], stage: Stage, interface: &mut Interface) {
    if stage != Stage::Vertex {
        return;
    }
    let mut depth = 0i32;
    let mut next_unknown = LOC_UNKNOWN_BASE;

    for at in 0..tokens.len() {
        if tokens[at].is_punct('{') {
            depth += 1;
        } else if tokens[at].is_punct('}') {
            depth -= 1;
        }
        if depth != 0 || !tokens[at].is_word("in") {
            continue;
        }
        let Some([_ty, name, end]) = lexer::solid_run::<3>(tokens, at + 1) else {
            continue;
        };
        if tokens[name].kind != Kind::Word || !tokens[end].is_punct(';') {
            continue;
        }

        let location = match PACK_ATTRIBUTES
            .iter()
            .find(|(known, _)| *known == tokens[name].text)
        {
            Some((_, location)) => *location,
            None => {
                next_unknown += 1;
                next_unknown - 1
            }
        };
        tokens[at].text = format!("layout(location = {location}) in");
        interface.attributes.push(location);
    }

    interface.attributes.sort_unstable();
    interface.attributes.dedup();
}

#[derive(Debug, Default)]
struct Used {
    ftransform: bool,
}

fn builtins(tokens: &mut Vec<Token>, stage: Stage, interface: &mut Interface) -> Used {
    let mut used = Used::default();
    let mut attributes = Vec::new();

    texture_matrix(tokens);

    for token in tokens.iter_mut() {
        if token.kind != Kind::Word {
            continue;
        }
        let replacement = match token.text.as_str() {
            "ftransform" => {
                used.ftransform = true;
                attributes.push(LOC_POSITION);
                "iris_ftransform"
            }
            "gl_Vertex" if stage == Stage::Vertex => {
                attributes.push(LOC_POSITION);
                "vec4(iris_Position, 1.0)"
            }
            "gl_Normal" if stage == Stage::Vertex => {
                attributes.push(LOC_NORMAL);
                "iris_Normal"
            }
            "gl_Color" if stage == Stage::Vertex => {
                attributes.push(LOC_COLOR);
                "iris_Color"
            }
            "gl_MultiTexCoord0" if stage == Stage::Vertex => {
                attributes.push(LOC_UV0);
                "vec4(iris_UV0, 0.0, 1.0)"
            }
            "gl_MultiTexCoord1" | "gl_MultiTexCoord2" if stage == Stage::Vertex => {
                attributes.push(LOC_UV2);
                "vec4(iris_UV2, 0.0, 1.0)"
            }
            "gl_ModelViewMatrix" => "iris_ModelViewMatrix",
            "gl_ProjectionMatrix" => "iris_ProjectionMatrix",
            "gl_ModelViewProjectionMatrix" => "(iris_ProjectionMatrix * iris_ModelViewMatrix)",
            "texture2D" | "texture3D" | "textureCube" => "texture",
            "texture2DLod" | "texture3DLod" => "textureLod",
            "texture2DProj" => "textureProj",
            _ => continue,
        };
        token.text = replacement.to_owned();
    }

    attributes.sort_unstable();
    attributes.dedup();
    interface.attributes = attributes;
    used
}

fn texture_matrix(tokens: &mut Vec<Token>) {
    let mut at = 0;
    while at < tokens.len() {
        if !tokens[at].is_word("gl_TextureMatrix") {
            at += 1;
            continue;
        }
        match lexer::solid_run::<3>(tokens, at + 1) {
            Some([open, _, close]) if tokens[open].is_punct('[') && tokens[close].is_punct(']') => {
                tokens.drain(at..=close);
                tokens.insert(at, Token::raw("mat4(1.0)"));
            }
            _ => tokens[at].text = "mat4(1.0)".to_owned(),
        }
        at += 1;
    }
}

fn outputs(tokens: &mut Vec<Token>, interface: &mut Interface) {
    let mut highest: Option<u32> = None;
    let mut at = 0;

    while at < tokens.len() {
        if tokens[at].is_word("gl_FragColor") {
            tokens[at].text = "iris_FragData0".to_owned();
            highest = Some(highest.unwrap_or(0).max(0));
            at += 1;
            continue;
        }
        if !tokens[at].is_word("gl_FragData") {
            at += 1;
            continue;
        }
        let Some([open, index, close]) = lexer::solid_run::<3>(tokens, at + 1) else {
            at += 1;
            continue;
        };
        if !tokens[open].is_punct('[') || !tokens[close].is_punct(']') {
            at += 1;
            continue;
        }
        let Ok(n) = tokens[index].text.parse::<u32>() else {
            at += 1;
            continue;
        };
        tokens.drain(at..=close);
        tokens.insert(at, Token::raw(&format!("iris_FragData{n}")));
        highest = Some(highest.unwrap_or(0).max(n));
        at += 1;
    }

    interface.outputs = highest.map_or(0, |n| n + 1);
}

fn preamble(tokens: &mut Vec<Token>, stage: Stage, interface: &Interface, used: &Used, set: u32) {
    let mut out = String::new();
    out.push_str("\n// Inserted by shaderpack::transform.\n");

    out.push_str(&format!(
        "layout(set = {set}, binding = {UNIFORM_BINDING}) uniform IrisFrame {{\n"
    ));
    for (name, ty) in UNIFORMS {
        out.push_str(&format!("    {ty} {name};\n"));
    }
    out.push_str("};\n");

    if stage == Stage::Vertex {
        let declarations: BTreeMap<u32, &str> = [
            (LOC_POSITION, "in vec3 iris_Position;"),
            (LOC_UV0, "in vec2 iris_UV0;"),
            (LOC_UV2, "in vec2 iris_UV2;"),
            (LOC_COLOR, "in vec4 iris_Color;"),
            (LOC_NORMAL, "in vec3 iris_Normal;"),
        ]
        .into_iter()
        .collect();
        for location in &interface.attributes {
            if let Some(declaration) = declarations.get(location) {
                out.push_str(&format!("layout(location = {location}) {declaration}\n"));
            }
        }
        if used.ftransform {
            out.push_str(
                "vec4 iris_ftransform() {\n    \
                 return iris_ProjectionMatrix * iris_ModelViewMatrix * vec4(iris_Position, 1.0);\n\
                 }\n",
            );
        }
    }

    for n in 0..interface.outputs {
        out.push_str(&format!(
            "layout(location = {n}) out vec4 iris_FragData{n};\n"
        ));
    }

    let after = tokens
        .iter()
        .position(|t| t.kind == Kind::Directive && t.text.starts_with("#version"))
        .map_or(0, |at| at + 1);
    tokens.insert(after, Token::raw(&out));
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SET: u32 = 3;

    fn vertex(source: &str) -> String {
        transform(source, Stage::Vertex, &varyings(source), TEST_SET).source
    }

    fn fragment(source: &str) -> String {
        transform(source, Stage::Fragment, &varyings(source), TEST_SET).source
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
    fn fragment_outputs_are_declared_and_counted() {
        let result = transform(
            "#version 130\nvoid main() { gl_FragData[0] = vec4(1.0); gl_FragData[2] = vec4(0.0); }\n",
            Stage::Fragment,
            &[],
            TEST_SET,
        );
        assert_eq!(result.interface.outputs, 3);
        assert!(
            result
                .source
                .contains("layout(location = 2) out vec4 iris_FragData2;")
        );
        assert!(result.source.contains("iris_FragData0 = vec4(1.0)"));
    }

    #[test]
    fn only_the_attributes_a_program_reads_are_declared() {
        let bare = transform(
            "#version 130\nvoid main() { gl_Position = vec4(0.0); }\n",
            Stage::Vertex,
            &[],
            TEST_SET,
        );
        assert!(bare.interface.attributes.is_empty());
        assert!(!bare.source.contains("iris_Color"), "{}", bare.source);

        let source = "#version 130\nvarying vec4 t;\nvoid main() { t = gl_Color; }\n";
        let colour = transform(source, Stage::Vertex, &varyings(source), TEST_SET);
        assert_eq!(colour.interface.attributes, [LOC_COLOR]);
        assert!(
            colour
                .source
                .contains("layout(location = 3) in vec4 iris_Color;")
        );
    }

    #[test]
    fn an_attribute_that_is_never_read_is_dropped() {
        let unused =
            "#version 130\nattribute vec3 mc_Entity;\nvoid main() { gl_Position = vec4(0.0); }\n";
        let out = transform(unused, Stage::Vertex, &[], TEST_SET);
        assert!(!out.source.contains("mc_Entity"), "{}", out.source);
        assert!(out.interface.attributes.is_empty());

        let used = "#version 130\nattribute vec3 mc_Entity;\nvoid main() { gl_Position = vec4(mc_Entity, 1.0); }\n";
        let out = transform(used, Stage::Vertex, &[], TEST_SET);
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
        assert!(
            out.contains("layout(location = 0) in vec3 iris_Position;"),
            "{out}"
        );
    }

    #[test]
    #[ignore = "needs a shader pack installed"]
    fn the_minimal_pack_reaches_wgsl() {
        use crate::shaderpack::backend::{self, ShaderStage};
        use crate::shaderpack::{discover, source::Source};

        let at = discover::dir().join("Minimal");
        let source = Source::open(&at).expect("the Minimal pack should be installed");

        let vertex_source = source.read_text("/gbuffers_terrain.vsh").expect("vsh");
        let shared = varyings(&vertex_source);
        println!("varyings: {shared:?}");

        for (file, stage, naga_stage) in [
            ("/gbuffers_terrain.vsh", Stage::Vertex, ShaderStage::Vertex),
            (
                "/gbuffers_terrain.fsh",
                Stage::Fragment,
                ShaderStage::Fragment,
            ),
        ] {
            let glsl = source.read_text(file).expect(file);
            let result = transform(&glsl, stage, &shared, TEST_SET);
            println!("\n=== {file} ===\n{}", result.source);

            match backend::to_wgsl(&result.source, naga_stage) {
                Ok(wgsl) => println!("--- wgsl ---\n{wgsl}"),
                Err(e) => panic!("{file}: {e}\n\n{}", result.source),
            }
            println!("interface: {:?}", result.interface);
        }
    }

    #[test]
    fn the_texture_matrix_is_folded_to_identity() {
        let out =
            vertex("#version 130\nvoid main() { vec4 c = gl_TextureMatrix[0] * vec4(1.0); }\n");
        assert!(out.contains("mat4(1.0) * vec4(1.0)"), "{out}");
        assert!(!out.contains("gl_TextureMatrix"), "{out}");
    }
}
