use std::collections::HashMap;

use super::lexer::{self, Kind, Token};

fn vector(ty: &str, n: usize) -> Option<String> {
    let prefix = match ty {
        "float" => "",
        "int" => "i",
        "uint" => "u",
        "bool" => "b",
        _ => return None,
    };
    Some(format!("{prefix}vec{n}"))
}

fn first_component(swizzle: &str) -> bool {
    (1..=4).contains(&swizzle.len())
        && ["x", "r", "s"]
            .iter()
            .any(|c| swizzle.chars().all(|s| s.to_string() == *c))
}

fn scalar_functions(tokens: &[Token]) -> HashMap<String, Option<String>> {
    let mut out: HashMap<String, Option<String>> = HashMap::new();
    let depths = lexer::brace_depths(tokens);
    for at in 0..tokens.len() {
        if depths[at] != 0 || tokens[at].kind != Kind::Word {
            continue;
        }
        let Some([name, open]) = lexer::solid_run::<2>(tokens, at + 1) else {
            continue;
        };
        if tokens[name].kind != Kind::Word || !tokens[open].is_punct('(') {
            continue;
        }
        let ty = tokens[at].text.as_str();
        let scalar = matches!(ty, "float" | "int" | "uint" | "bool").then(|| ty.to_owned());
        let entry = out
            .entry(tokens[name].text.clone())
            .or_insert_with(|| scalar.clone());
        if *entry != scalar {
            *entry = None;
        }
    }
    out
}

pub(super) fn scalar_swizzles(tokens: &mut Vec<Token>) {
    let functions = scalar_functions(tokens);
    let mut at = tokens.len();
    while at > 0 {
        at -= 1;
        let Some(Some(ty)) = functions
            .get(&tokens[at].text)
            .filter(|_| tokens[at].kind == Kind::Word)
        else {
            continue;
        };
        let Some(open) = lexer::next_solid(tokens, at + 1).filter(|o| tokens[*o].is_punct('('))
        else {
            continue;
        };
        let Some(close) = lexer::matching(tokens, open) else {
            continue;
        };
        let Some([dot, swizzle]) = lexer::solid_run::<2>(tokens, close + 1) else {
            continue;
        };
        if !tokens[dot].is_punct('.') || !first_component(&tokens[swizzle].text) {
            continue;
        }
        let n = tokens[swizzle].text.len();
        if n == 1 {
            tokens.drain(close + 1..=swizzle);
        } else if let Some(vector) = vector(ty, n) {
            tokens.drain(close + 1..=swizzle);
            tokens.insert(close + 1, Token::raw(")"));
            tokens.insert(at, Token::raw(&format!("{vector}(")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewritten(source: &str) -> String {
        let mut tokens = lexer::lex(source);
        scalar_swizzles(&mut tokens);
        lexer::render(&tokens)
    }

    #[test]
    fn a_scalar_function_s_swizzle_is_its_value() {
        let out = rewritten(
            "float d(vec2 uv, float l) { return l; }\nvoid main() { float a = d(uv + f(x), 1.0).x; vec2 b = d(uv, 2.0).rr; vec4 c = g(uv).x; }\n",
        );
        assert!(out.contains("float a = d(uv + f(x), 1.0);"), "{out}");
        assert!(out.contains("vec2 b = vec2(d(uv, 2.0));"), "{out}");
        assert!(
            out.contains("g(uv).x"),
            "an unknown function is left alone: {out}"
        );
        let overloaded = rewritten(
            "float h(float a) { return a; }\nvec3 h(vec3 a) { return a; }\nvoid main() { x = h(v).x; }\n",
        );
        assert!(overloaded.contains("h(v).x"), "{overloaded}");
    }
}
