use std::collections::HashMap;

use super::lexer::{self, Kind, Token};
use super::storage::{image_type, untyped};

fn texel(ty: &str) -> &'static str {
    match ty.strip_suffix(untyped(ty)) {
        Some("u") => "uvec4",
        Some("i") => "ivec4",
        _ => "vec4",
    }
}

fn image_texels(tokens: &[Token]) -> HashMap<String, &'static str> {
    let mut out = HashMap::new();
    for at in 0..tokens.len() {
        if tokens[at].kind != Kind::Word || !image_type(&tokens[at].text) {
            continue;
        }
        if let Some(name) =
            lexer::next_solid(tokens, at + 1).filter(|n| tokens[*n].kind == Kind::Word)
        {
            out.insert(tokens[name].text.clone(), texel(&tokens[at].text));
        }
    }
    out
}

fn combine(function: &str, old: &str, value: &str) -> Option<String> {
    Some(match function {
        "imageAtomicMax" => format!("max({old}, {value})"),
        "imageAtomicMin" => format!("min({old}, {value})"),
        "imageAtomicAdd" => format!("{old} + {value}"),
        "imageAtomicAnd" => format!("{old} & {value}"),
        "imageAtomicOr" => format!("{old} | {value}"),
        "imageAtomicXor" => format!("{old} ^ {value}"),
        "imageAtomicExchange" => value.to_owned(),
        _ => return None,
    })
}

pub(super) fn lower_image_atomics(tokens: &mut Vec<Token>) {
    let images = image_texels(tokens);
    let mut at = tokens.len();
    while at > 0 {
        at -= 1;
        if tokens[at].kind != Kind::Word || !tokens[at].text.starts_with("imageAtomic") {
            continue;
        }
        let statement = lexer::starts_statement(tokens, at)
            || lexer::prev_solid(tokens, at)
                .is_some_and(|p| tokens[p].is_punct(')') || tokens[p].is_word("else"));
        let Some(open) = lexer::next_solid(tokens, at + 1).filter(|o| tokens[*o].is_punct('('))
        else {
            continue;
        };
        let Some((ranges, close)) = lexer::arguments(tokens, open) else {
            continue;
        };
        let args: Vec<String> = ranges
            .iter()
            .map(|&(from, to)| lexer::render(&tokens[from..to]).trim().to_owned())
            .collect();
        let Some(semi) = lexer::next_solid(tokens, close + 1).filter(|s| tokens[*s].is_punct(';'))
        else {
            continue;
        };
        let [image, coord, value] = &args[..] else {
            continue;
        };
        let Some(texel) = images.get(image) else {
            continue;
        };
        if !statement {
            continue;
        }
        let Some(stored) = combine(&tokens[at].text, "iris_old", &format!("{texel}({value})"))
        else {
            continue;
        };
        let lowered = format!(
            "{{ {texel} iris_old = imageLoad({image}, {coord}); imageStore({image}, {coord}, {stored}); }}"
        );
        tokens.splice(at..=semi, [Token::raw(&lowered)]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_statement_atomic_becomes_a_load_and_store() {
        let mut tokens = lexer::lex(
            "layout (r32ui) uniform uimage3D vox;\nvoid main() { if (a) { imageAtomicMax( vox, p + ivec3(1, 0, 0), v ); } uint o = imageAtomicAdd(vox, p, 1u); }\n",
        );
        lower_image_atomics(&mut tokens);
        let out = lexer::render(&tokens);
        assert!(
            out.contains("{ uvec4 iris_old = imageLoad(vox, p + ivec3(1, 0, 0)); imageStore(vox, p + ivec3(1, 0, 0), max(iris_old, uvec4(v))); }"),
            "{out}"
        );
        assert!(
            out.contains("uint o = imageAtomicAdd(vox, p, 1u);"),
            "a used result is left alone: {out}"
        );
    }

    #[test]
    fn float_images_and_branch_bodies_are_lowered() {
        let mut tokens = lexer::lex(
            "layout (r32f) uniform image2D depth;\nvoid main() { if (c) imageAtomicExchange(depth, p, 1.0); else imageAtomicExchange(depth, p, 2.0);\n\
             switch (k) { case 0: imageAtomicExchange(depth, p, 3.0); break; } }\n",
        );
        lower_image_atomics(&mut tokens);
        let out = lexer::render(&tokens);
        assert!(!out.contains("imageAtomic"), "{out}");
        assert!(
            out.contains(
                "if (c) { vec4 iris_old = imageLoad(depth, p); imageStore(depth, p, vec4(1.0)); }"
            ),
            "{out}"
        );
        assert!(out.contains("case 0: { vec4 iris_old"), "{out}");
    }
}
