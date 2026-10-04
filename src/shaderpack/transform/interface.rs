use super::super::literal;
use super::super::pipeline::Format;
use super::lexer::{self, Kind, Token};
use super::{Interface, Stage, Target};

pub(crate) const PACK_ATTRIBUTES: &[(&str, u32, Format)] = &[
    ("mc_Entity", 5, Format::Float32x4),
    ("mc_midTexCoord", 6, Format::Float32x2),
    ("at_tangent", 7, Format::Float32x4),
    ("at_midBlock", 8, Format::Float32x4),
];

const LOC_UNKNOWN_BASE: u32 = 9;

pub(super) fn interface(tokens: &[Token]) -> (Vec<Varying>, u32) {
    let depths = lexer::brace_depths(tokens);
    let mut out: Vec<Varying> = Vec::new();
    let mut next = 0;

    for at in 0..tokens.len() {
        if depths[at] != 0 || !(tokens[at].is_word("varying") || tokens[at].is_word("out")) {
            continue;
        }
        if let Some((name, slots)) = interface_declaration(tokens, at + 1)
            && !out.iter().any(|v| v.name == tokens[name].text)
        {
            out.push(Varying {
                name: tokens[name].text.clone(),
                location: next,
            });
            next += slots;
        }
    }
    (out, next)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Varying {
    pub(crate) name: String,
    pub(crate) location: u32,
}

fn interface_declaration(tokens: &[Token], from: usize) -> Option<(usize, u32)> {
    let [ty, name, next] = lexer::solid_run::<3>(tokens, from)?;
    if tokens[name].kind != Kind::Word {
        return None;
    }
    let per_element = match tokens[ty].text.as_str() {
        "mat2" | "mat2x2" | "mat2x3" | "mat2x4" => 2,
        "mat3" | "mat3x2" | "mat3x3" | "mat3x4" => 3,
        "mat4" | "mat4x2" | "mat4x3" | "mat4x4" => 4,
        _ => 1,
    };
    if tokens[next].is_punct(';') {
        return Some((name, per_element));
    }
    let [count, close, end] = lexer::solid_run::<3>(tokens, next + 1)?;
    let count = u32::try_from(literal::int(&tokens[count].text)?).ok()?;
    (tokens[next].is_punct('[') && tokens[close].is_punct(']') && tokens[end].is_punct(';'))
        .then_some((name, per_element * count))
}

const INTERPOLATION: &[&str] = &["flat", "smooth", "noperspective", "centroid"];

pub(super) fn keywords(tokens: &mut [Token], stage: Stage, varyings: &[Varying]) {
    let keyword = match stage {
        Stage::Vertex => "out",
        Stage::Fragment => "in",
        Stage::Compute => return,
    };
    let depths = lexer::brace_depths(tokens);

    for at in 0..tokens.len() {
        if tokens[at].is_word("attribute") {
            tokens[at].text = "in".to_owned();
            continue;
        }
        let is_varying = tokens[at].is_word("varying");
        if depths[at] != 0 || !(is_varying || tokens[at].is_word(keyword)) {
            continue;
        }
        let Some((name, _)) = interface_declaration(tokens, at + 1) else {
            continue;
        };
        let Some(location) = varyings
            .iter()
            .find(|v| v.name == tokens[name].text)
            .map(|v| v.location)
        else {
            if stage == Stage::Fragment {
                tokens[at].text.clear();
                let mut back = at;
                while let Some(prev) = lexer::prev_solid(tokens, back) {
                    if !INTERPOLATION.contains(&tokens[prev].text.as_str()) {
                        break;
                    }
                    tokens[prev].text.clear();
                    back = prev;
                }
            }
            continue;
        };
        tokens[at].text = format!("layout(location = {location}) {keyword}");
    }
}

fn top_level_inputs(tokens: &[Token]) -> Vec<[usize; 4]> {
    let depths = lexer::brace_depths(tokens);
    (0..tokens.len())
        .filter(|at| depths[*at] == 0 && tokens[*at].is_word("in"))
        .filter_map(|at| {
            let [ty, name, end] = lexer::solid_run::<3>(tokens, at + 1)?;
            (tokens[name].kind == Kind::Word && tokens[end].is_punct(';'))
                .then_some([at, ty, name, end])
        })
        .collect()
}

pub(super) fn prune_attributes(tokens: &mut Vec<Token>, stage: Stage) {
    if stage != Stage::Vertex {
        return;
    }
    let mut doomed: Vec<(usize, usize)> = Vec::new();

    let inputs = top_level_inputs(tokens);
    if inputs.is_empty() {
        return;
    }
    let in_macros: std::collections::HashSet<String> = tokens
        .iter()
        .filter_map(lexer::define_body)
        .flatten()
        .filter(|w| w.kind == Kind::Word)
        .map(|w| w.text)
        .collect();

    for [at, _, name, end] in inputs {
        let target = &tokens[name].text;
        let uses = tokens
            .iter()
            .filter(|t| t.kind == Kind::Word && t.text == *target)
            .count();
        if uses == 1 && !in_macros.contains(target) {
            doomed.push((at, end));
        }
    }

    for (from, to) in doomed.into_iter().rev() {
        tokens.drain(from..=to);
    }
}

pub(super) fn pack_attributes(
    tokens: &mut Vec<Token>,
    stage: Stage,
    interface: &mut Interface,
    target: &Target,
) {
    if stage != Stage::Vertex {
        return;
    }
    let mut next_unknown = LOC_UNKNOWN_BASE;
    let mut defaulted: Vec<(usize, usize, String)> = Vec::new();

    for [at, ty, name, end] in top_level_inputs(tokens) {
        let known = PACK_ATTRIBUTES
            .iter()
            .find(|(known, _, _)| *known == tokens[name].text);
        if target.attribute_defaults
            && known.is_some()
            && let Some(default) = attribute_default(&tokens[name].text, &tokens[ty].text)
        {
            defaulted.push((at, end, default));
            continue;
        }
        let location = match known {
            Some((_, location, _)) => *location,
            None => {
                next_unknown += 1;
                next_unknown - 1
            }
        };
        tokens[at].text = format!("layout(location = {location}) in");
        interface.attributes.push(location);
    }

    for (qualifier, semicolon, default) in defaulted.into_iter().rev() {
        tokens.splice(semicolon..semicolon, lexer::lex(&format!(" = {default}")));
        tokens[qualifier].text.clear();
    }

    interface.attributes.sort_unstable();
    interface.attributes.dedup();
}

fn attribute_default(name: &str, ty: &str) -> Option<String> {
    let call = match name {
        "mc_Entity" => "iris_entity()",
        "at_tangent" => "iris_tangent()",
        "mc_midTexCoord" => "iris_mid_uv()",
        "at_midBlock" => "iris_mid_block()",
        _ => return None,
    };
    let swizzle = match ty {
        "float" | "int" | "uint" => ".x",
        "vec2" | "ivec2" | "uvec2" => ".xy",
        "vec3" | "ivec3" | "uvec3" => ".xyz",
        "vec4" | "ivec4" | "uvec4" => "",
        _ => return None,
    };
    Some(format!("{ty}({call}{swizzle})"))
}
