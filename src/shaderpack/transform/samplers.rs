use std::collections::HashMap;

use super::lexer::{self, Kind, Token};
use super::{Interface, sampler_binding};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Combined<'a> {
    pub(crate) ty: &'a str,
    pub(crate) texture: &'static str,
    pub(crate) sampler: &'static str,
}

pub(crate) fn combined(ty: &str) -> Option<Combined<'_>> {
    const TABLE: &[(&str, &str, &str)] = &[
        ("sampler1D", "texture1D", "sampler"),
        ("sampler2D", "texture2D", "sampler"),
        ("sampler3D", "texture3D", "sampler"),
        ("samplerCube", "textureCube", "sampler"),
        ("sampler2DArray", "texture2DArray", "sampler"),
        ("sampler2DShadow", "texture2D", "samplerShadow"),
        ("isampler2D", "itexture2D", "sampler"),
        ("isampler3D", "itexture3D", "sampler"),
        ("usampler2D", "utexture2D", "sampler"),
        ("usampler3D", "utexture3D", "sampler"),
    ];
    TABLE
        .iter()
        .find(|(name, _, _)| *name == ty)
        .map(|(_, texture, sampler)| Combined {
            ty,
            texture,
            sampler,
        })
}

pub(crate) fn constructor(ty: &str, name: &str) -> String {
    format!("{ty}({name}, {name}_sampler)")
}

fn constructor_arguments(text: &str) -> Option<&str> {
    let open = text.find('(')?;
    combined(&text[..open])?;
    text[open + 1..].strip_suffix(')')
}

pub(crate) fn split_parameters(tokens: &mut [Token]) {
    let mut functions: HashMap<String, Vec<usize>> = HashMap::new();
    let mut bodies: Vec<(usize, usize, Vec<(String, String)>)> = Vec::new();

    let mut depth = 0i32;
    let mut at = 0;
    while at < tokens.len() {
        if tokens[at].is_punct('{') {
            depth += 1;
        } else if tokens[at].is_punct('}') {
            depth -= 1;
        }
        let header = depth == 0
            && tokens[at].kind == Kind::Word
            && lexer::next_solid(tokens, at + 1).is_some_and(|n| tokens[n].is_punct('('))
            && lexer::prev_solid(tokens, at).is_some_and(|prev| tokens[prev].kind == Kind::Word);
        if !header {
            at += 1;
            continue;
        }
        let open = lexer::next_solid(tokens, at + 1).expect("checked above");
        let Some((params, close)) = lexer::arguments(tokens, open) else {
            break;
        };

        let mut split = Vec::new();
        for (position, (from, to)) in params.iter().copied().enumerate() {
            let solid: Vec<usize> = (from..to).filter(|i| !tokens[*i].is_trivia()).collect();
            let [.., ty, name] = solid[..] else { continue };
            let Some((ty_text, texture, sampler)) =
                combined(&tokens[ty].text).map(|k| (k.ty.to_owned(), k.texture, k.sampler))
            else {
                continue;
            };
            let name_text = tokens[name].text.clone();
            tokens[ty].text = texture.to_owned();
            tokens[name].text = format!("{name_text}, {sampler} {name_text}_sampler");
            split.push((name_text, ty_text));
            functions
                .entry(tokens[at].text.clone())
                .or_default()
                .push(position);
        }

        if !split.is_empty()
            && let Some(brace) =
                lexer::next_solid(tokens, close + 1).filter(|b| tokens[*b].is_punct('{'))
        {
            if let Some(end) = lexer::matching(tokens, brace) {
                bodies.push((brace, end, split));
            }
        }
        at = close + 1;
    }

    for (from, to, params) in bodies {
        for at in from..to {
            if tokens[at].kind != Kind::Word {
                continue;
            }
            let member =
                lexer::prev_solid(tokens, at).is_some_and(|prev| tokens[prev].is_punct('.'));
            if member {
                continue;
            }
            if let Some((name, ty)) = params.iter().find(|(name, _)| *name == tokens[at].text) {
                tokens[at].text = constructor(ty, name);
            }
        }
    }

    for positions in functions.values_mut() {
        positions.sort_unstable();
        positions.dedup();
    }
    for at in 0..tokens.len() {
        let Some(positions) = functions.get(&tokens[at].text) else {
            continue;
        };
        if tokens[at].kind != Kind::Word {
            continue;
        }
        let Some(open) = lexer::next_solid(tokens, at + 1).filter(|n| tokens[*n].is_punct('('))
        else {
            continue;
        };
        let Some((args, _)) = lexer::arguments(tokens, open) else {
            continue;
        };
        for position in positions {
            let Some(&(from, to)) = args.get(*position) else {
                continue;
            };
            let solid: Vec<usize> = (from..to).filter(|i| !tokens[*i].is_trivia()).collect();
            if let [only] = solid[..]
                && let Some(halves) = constructor_arguments(&tokens[only].text)
            {
                tokens[only].text = halves.to_owned();
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Sampler {
    pub(crate) name: String,
    pub(crate) ty: String,
    pub(crate) texture: String,
    pub(crate) texture_binding: u32,
    pub(crate) sampler_binding: u32,
}

pub(super) fn sampler_names(tokens: &[Token]) -> Vec<String> {
    let mut names = Vec::new();
    for (_, ty, name) in lexer::uniform_declarations(tokens) {
        if combined(&tokens[ty].text).is_some() && !names.contains(&tokens[name].text) {
            names.push(tokens[name].text.clone());
        }
    }
    names
}

pub(super) fn split_samplers(
    tokens: &mut Vec<Token>,
    interface: &mut Interface,
    shared: &[String],
    set: u32,
) {
    let mut declarations: Vec<(usize, String, &'static str)> = Vec::new();

    for (_, ty, name) in lexer::uniform_declarations(tokens) {
        let Some(kind) = combined(&tokens[ty].text) else {
            continue;
        };
        let index = shared
            .iter()
            .position(|s| *s == tokens[name].text)
            .unwrap_or(shared.len() + interface.samplers.len()) as u32;
        let binding = sampler_binding(index);
        interface.samplers.push(Sampler {
            name: tokens[name].text.clone(),
            ty: kind.ty.to_owned(),
            texture: kind.texture.to_owned(),
            texture_binding: binding,
            sampler_binding: binding + 1,
        });
        declarations.push((ty, tokens[name].text.clone(), kind.sampler));
    }

    if interface.samplers.is_empty() {
        return;
    }

    let declared: Vec<usize> = declarations
        .iter()
        .filter_map(|(ty, _, _)| lexer::next_solid(tokens, ty + 1))
        .collect();
    let mut shadowed: Vec<(String, i32)> = Vec::new();
    let (mut braces, mut parens) = (0i32, 0i32);
    for at in 0..tokens.len() {
        match tokens[at].text.as_str() {
            "{" if tokens[at].kind == Kind::Punct => braces += 1,
            "}" if tokens[at].kind == Kind::Punct => {
                braces -= 1;
                shadowed.retain(|(_, depth)| *depth <= braces);
            }
            "(" if tokens[at].kind == Kind::Punct => parens += 1,
            ")" if tokens[at].kind == Kind::Punct => parens -= 1,
            _ => {}
        }
        if declared.contains(&at) || tokens[at].kind != Kind::Word {
            continue;
        }
        let Some(sampler) = interface
            .samplers
            .iter()
            .find(|s| s.name == tokens[at].text)
        else {
            continue;
        };
        if declares_local(tokens, at) {
            let depth = if braces == 0 && parens > 0 { 1 } else { braces };
            shadowed.push((sampler.name.clone(), depth));
            continue;
        }
        if shadowed.iter().any(|(name, _)| *name == sampler.name) {
            continue;
        }
        tokens[at].text = constructor(&sampler.ty, &sampler.name);
    }
    for token in tokens.iter_mut().filter(|t| t.kind == Kind::Directive) {
        lexer::in_define(token, |word| {
            if let Some(sampler) = interface.samplers.iter().find(|s| s.name == word.text) {
                word.text = constructor(&sampler.ty, &sampler.name);
            }
        });
    }

    for (ty, name, sampler_type) in declarations.into_iter().rev() {
        let sampler = interface
            .samplers
            .iter()
            .find(|s| s.name == name)
            .expect("just recorded");
        tokens[ty].text = sampler.texture.clone();
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
                    " layout(set = {set}, binding = {}) uniform {sampler_type} {name}_sampler;",
                    sampler.sampler_binding
                )),
            );
        }
    }
}

fn declares_local(tokens: &[Token], at: usize) -> bool {
    let ends = lexer::next_solid(tokens, at + 1).is_some_and(|a| {
        [';', '=', ',', ')', '[']
            .iter()
            .any(|c| tokens[a].is_punct(*c))
    });
    lexer::after_type(tokens, at) && ends
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(source: &str) -> String {
        let mut tokens = lexer::lex(source);
        for token in &mut tokens {
            if token.is_word("colortex0") {
                token.text = constructor("sampler2D", "colortex0");
            }
        }
        split_parameters(&mut tokens);
        lexer::render(&tokens)
    }

    #[test]
    fn a_sampler_parameter_its_uses_and_its_callers_all_split() {
        let out = split(
            "vec4 blur(sampler2D tex, vec2 uv) { return texture(tex, uv); }\n\
             vec4 twice(sampler2D t, vec2 uv) { return blur(t, uv); }\n\
             void main() { vec4 c = twice(colortex0, vec2(0.0)); }\n",
        );
        assert!(
            out.contains("vec4 blur(texture2D tex, sampler tex_sampler, vec2 uv)"),
            "{out}"
        );
        assert!(
            out.contains("texture(sampler2D(tex, tex_sampler), uv)"),
            "{out}"
        );
        assert!(out.contains("blur(t, t_sampler, uv)"), "{out}");
        assert!(
            out.contains("twice(colortex0, colortex0_sampler, vec2(0.0))"),
            "{out}"
        );
    }

    #[test]
    fn a_shadow_sampler_splits_into_a_comparison_sampler() {
        let out = split("float pcf(sampler2DShadow s, vec3 p) { return texture(s, p); }\n");
        assert!(
            out.contains("texture2D s, samplerShadow s_sampler"),
            "{out}"
        );
        assert!(
            out.contains("texture(sampler2DShadow(s, s_sampler), p)"),
            "{out}"
        );
    }

    #[test]
    fn a_function_without_samplers_is_untouched() {
        let source = "float f(float x) { return x; }\nvoid main() { float y = f(1.0); }\n";
        assert_eq!(split(source), source);
    }
}
