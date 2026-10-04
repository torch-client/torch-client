use std::collections::HashMap;

use super::super::literal;
use super::lexer::{self, Kind, Token};
use super::{Interface, Stage};

const LIGHTMAP_TEXTURE_MATRIX: &str = "mat4(vec4(0.00390625, 0.0, 0.0, 0.0), vec4(0.0, 0.00390625, 0.0, 0.0), \
     vec4(0.0, 0.0, 0.00390625, 0.0), vec4(0.03125, 0.03125, 0.03125, 1.0))";

#[derive(Debug, Default)]
pub(super) struct Used {
    pub(super) ftransform: bool,
    pub(super) dot: bool,
}

pub(super) fn builtins(tokens: &mut Vec<Token>, stage: Stage) -> Used {
    let mut used = Used::default();

    texture_matrix(tokens);

    for token in tokens.iter_mut() {
        match token.kind {
            Kind::Word => {
                if let Some(replacement) = builtin(&token.text, stage, &mut used) {
                    token.text = replacement.to_owned();
                }
            }
            Kind::Directive => lexer::in_define(token, |word| {
                if let Some(replacement) = builtin(&word.text, stage, &mut used) {
                    word.text = replacement.to_owned();
                }
            }),
            _ => {}
        }
    }
    used
}

fn builtin(word: &str, stage: Stage, used: &mut Used) -> Option<&'static str> {
    Some(match word {
        "ftransform" => {
            used.ftransform = true;
            "iris_ftransform"
        }
        "dot" => {
            used.dot = true;
            "iris_dot"
        }
        "gl_Vertex" if stage == Stage::Vertex => "vec4(iris_position(), 1.0)",
        "gl_Normal" if stage == Stage::Vertex => "iris_normal()",
        "gl_VertexID" if stage == Stage::Vertex => "gl_VertexIndex",
        "gl_InstanceID" if stage == Stage::Vertex => "gl_InstanceIndex",
        "gl_Color" if stage == Stage::Vertex => "iris_color()",
        "gl_MultiTexCoord0" if stage == Stage::Vertex => "vec4(iris_uv0(), 0.0, 1.0)",
        "gl_MultiTexCoord1" | "gl_MultiTexCoord2" if stage == Stage::Vertex => {
            "vec4(iris_light(), 0.0, 1.0)"
        }
        "gl_NormalMatrix" => "mat3(iris_ModelViewMatrix)",
        "gl_ModelViewMatrix" => "iris_ModelViewMatrix",
        "gl_ProjectionMatrix" => "iris_ProjectionMatrix",
        "gl_ModelViewProjectionMatrix" => "iris_ModelViewProjectionMatrix",
        "texture2D" | "texture3D" | "textureCube" => "texture",
        "texture2DLod" | "texture3DLod" | "texture2DLodARB" => "textureLod",
        "texture2DGrad" | "texture2DGradARB" | "texture3DGradARB" => "textureGrad",
        "texture2DProj" => "textureProj",
        _ => return None,
    })
}

fn texture_matrix(tokens: &mut Vec<Token>) {
    lexer::in_defines(tokens, texture_matrix_in);
    texture_matrix_in(tokens);
}

fn texture_matrix_in(tokens: &mut Vec<Token>) {
    let mut at = 0;
    while at < tokens.len() {
        if !tokens[at].is_word("gl_TextureMatrix") {
            at += 1;
            continue;
        }
        match lexer::solid_run::<3>(tokens, at + 1) {
            Some([open, index, close])
                if tokens[open].is_punct('[') && tokens[close].is_punct(']') =>
            {
                let matrix = match tokens[index].text.as_str() {
                    "1" | "2" => LIGHTMAP_TEXTURE_MATRIX,
                    _ => "mat4(1.0)",
                };
                tokens.drain(at..=close);
                tokens.insert(at, Token::raw(matrix));
            }
            _ => tokens[at].text = "mat4(1.0)".to_owned(),
        }
        at += 1;
    }
}

pub(super) fn outputs(tokens: &mut Vec<Token>, interface: &mut Interface, stage: Stage) {
    let mut highest: Option<u32> = None;
    if stage == Stage::Fragment {
        declared_outputs(tokens, &mut highest);
    }
    lexer::in_defines(tokens, |body| frag_data(body, &mut highest));
    frag_data(tokens, &mut highest);
    interface.outputs = highest.map_or(0, |n| n + 1);
}

fn frag_data(tokens: &mut Vec<Token>, highest: &mut Option<u32>) {
    let mut at = 0;
    while at < tokens.len() {
        if tokens[at].is_word("gl_FragColor") {
            tokens[at].text = "iris_FragData0".to_owned();
            *highest = Some(highest.unwrap_or(0));
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
        let Some(n) = literal::int(&tokens[index].text).and_then(|n| u32::try_from(n).ok()) else {
            at += 1;
            continue;
        };
        tokens.drain(at..=close);
        tokens.insert(at, Token::raw(&format!("iris_FragData{n}")));
        *highest = Some(highest.unwrap_or(0).max(n));
        at += 1;
    }
}

fn declared_outputs(tokens: &mut Vec<Token>, highest: &mut Option<u32>) {
    let depths = lexer::brace_depths(tokens);
    let mut found: Vec<(usize, usize, String, u32)> = Vec::new();
    let mut next = 0u32;
    for at in 0..tokens.len() {
        if depths[at] != 0 || !tokens[at].is_word("out") {
            continue;
        }
        let Some([ty, name, end]) = lexer::solid_run::<3>(tokens, at + 1) else {
            continue;
        };
        if !tokens[ty].is_word("vec4")
            || tokens[name].kind != Kind::Word
            || !tokens[end].is_punct(';')
        {
            continue;
        }
        let mut first = at;
        let mut location = None;
        if let Some(close) = lexer::prev_solid(tokens, at).filter(|c| tokens[*c].is_punct(')')) {
            let open = (0..close).rev().find(|i| tokens[*i].is_punct('('));
            let layout = open
                .and_then(|o| lexer::prev_solid(tokens, o))
                .filter(|l| tokens[*l].is_word("layout"));
            if let (Some(open), Some(layout)) = (open, layout) {
                let inner: Vec<usize> = (open + 1..close)
                    .filter(|i| !tokens[*i].is_trivia())
                    .collect();
                match inner[..] {
                    [key, eq, n] if tokens[key].is_word("location") && tokens[eq].is_punct('=') => {
                        let Some(n) =
                            literal::int(&tokens[n].text).and_then(|n| u32::try_from(n).ok())
                        else {
                            continue;
                        };
                        location = Some(n);
                        first = layout;
                    }
                    _ => continue,
                }
            }
        }
        let location = location.unwrap_or_else(|| {
            next += 1;
            next - 1
        });
        found.push((first, end, tokens[name].text.clone(), location));
    }
    if found.is_empty() {
        return;
    }
    for &(_, _, _, location) in &found {
        *highest = Some(highest.unwrap_or(0).max(location));
    }
    let renames: Vec<(String, String)> = found
        .iter()
        .map(|(_, _, name, location)| (name.clone(), format!("iris_FragData{location}")))
        .collect();
    for (first, end, _, _) in found.into_iter().rev() {
        tokens.drain(first..=end);
    }
    let rename = |word: &mut Token| {
        if let Some((_, to)) = renames.iter().find(|(from, _)| *from == word.text) {
            word.text = to.clone();
        }
    };
    for token in tokens.iter_mut() {
        match token.kind {
            Kind::Word => rename(token),
            Kind::Directive => lexer::in_define(token, &rename),
            _ => {}
        }
    }
}

pub(super) fn explicit_lod(tokens: &mut Vec<Token>) {
    lexer::in_defines(tokens, explicit_lod_in);
    explicit_lod_in(tokens);
}

fn explicit_lod_in(tokens: &mut Vec<Token>) {
    let mut at = 0;
    while at < tokens.len() {
        if tokens[at].is_word("texture")
            && let Some(open) =
                lexer::next_solid(tokens, at + 1).filter(|o| tokens[*o].is_punct('('))
            && let Some((args, close)) = lexer::arguments(tokens, open)
            && args.len() == 2
        {
            tokens[at].text = "textureLod".to_owned();
            tokens.splice(close..close, lexer::lex(", 0.0"));
        }
        at += 1;
    }
}

pub(super) fn unsigned_literals(tokens: &mut [Token]) {
    let mut unsigned: HashMap<String, bool> = HashMap::new();
    for at in 0..tokens.len() {
        let ty = tokens[at].text.as_str();
        if tokens[at].kind != Kind::Word
            || !matches!(ty, "uint" | "int" | "float" | "bool" | "double")
        {
            continue;
        }
        let Some(name) = lexer::next_solid(tokens, at + 1) else {
            continue;
        };
        if tokens[name].kind != Kind::Word {
            continue;
        }
        let is_uint = ty == "uint";
        unsigned
            .entry(tokens[name].text.clone())
            .and_modify(|known| *known &= is_uint)
            .or_insert(is_uint);
    }
    if !unsigned.values().any(|u| *u) {
        return;
    }
    lexer::in_defines(tokens, |body| suffix_literals(body, &unsigned));
    suffix_literals(tokens, &unsigned);
}

fn suffix_literals(tokens: &mut [Token], unsigned: &HashMap<String, bool>) {
    for at in 0..tokens.len() {
        if !matches!(tokens[at].text.as_str(), "min" | "max" | "clamp")
            || tokens[at].kind != Kind::Word
        {
            continue;
        }
        let Some(open) = lexer::next_solid(tokens, at + 1).filter(|o| tokens[*o].is_punct('('))
        else {
            continue;
        };
        let Some((ranges, _)) = lexer::arguments(tokens, open) else {
            continue;
        };
        let arguments: Vec<(usize, bool)> = ranges
            .iter()
            .filter_map(|&(from, to)| {
                let solid: Vec<usize> = (from..to).filter(|j| !tokens[*j].is_trivia()).collect();
                let &first = solid.first()?;
                let literal = solid.len() == 1
                    && tokens[first].kind == Kind::Number
                    && literal::int(&tokens[first].text).is_some()
                    && !tokens[first].text.ends_with(['u', 'U']);
                Some((first, literal))
            })
            .collect();
        let beside_uint = arguments
            .iter()
            .any(|(first, literal)| !literal && unsigned.get(&tokens[*first].text) == Some(&true));
        if beside_uint {
            for (first, literal) in arguments {
                if literal {
                    tokens[first].text.push('u');
                }
            }
        }
    }
}
