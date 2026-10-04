use super::super::literal;
use super::lexer::{self, Kind, Token, at_depth_zero};

const PRECISION: &[&str] = &["lowp", "mediump", "highp"];

fn statements(tokens: &[Token]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let mut braced = false;
    for at in 0..tokens.len() {
        let token = &tokens[at];
        if token.is_punct('{') {
            depth += 1;
            braced = true;
        } else if token.is_punct('}') {
            depth -= 1;
            if depth == 0 {
                start = at + 1;
                braced = false;
            }
        } else if depth == 0 && token.kind == Kind::Directive {
            start = at + 1;
        } else if depth == 0 && token.is_punct(';') {
            if !braced {
                out.push((start, at));
            }
            start = at + 1;
            braced = false;
        }
    }
    out
}

fn declarator_name(tokens: &[Token], from: usize, to: usize) -> Option<usize> {
    let end = at_depth_zero(tokens, from, to, |t| t.is_punct('=') || t.is_punct('['))
        .first()
        .copied()
        .unwrap_or(to);
    (from..end).rev().find(|at| tokens[*at].kind == Kind::Word)
}

pub(crate) fn split_declarators(source: &str) -> String {
    lexer::render(&split_declarator_tokens(lexer::lex(source)))
}

pub(crate) fn split_declarator_tokens(tokens: Vec<Token>) -> Vec<Token> {
    match split_text(&tokens) {
        Some(text) => lexer::lex(&text),
        None => tokens,
    }
}

fn split_text(tokens: &[Token]) -> Option<String> {
    let len: usize = tokens.iter().map(|t| t.text.len()).sum();
    let mut out = String::with_capacity(len + len / 16);
    let mut copied = 0;

    for (start, end) in statements(tokens) {
        let commas = at_depth_zero(tokens, start, end, |t| t.is_punct(','));
        if commas.is_empty() {
            continue;
        }
        let Some(first) = lexer::next_solid(tokens, start) else {
            continue;
        };
        let Some(name) = declarator_name(tokens, first, commas[0]) else {
            continue;
        };
        let head = lexer::render(&tokens[first..name]);

        out.push_str(&lexer::render(&tokens[copied..name]));
        let mut from = name;
        for comma in commas.iter().copied().chain(std::iter::once(end)) {
            let declarator = lexer::render(&tokens[from..comma]);
            if from != name {
                out.push(' ');
                out.push_str(&head);
            }
            out.push_str(if from == name {
                &declarator
            } else {
                declarator.trim_start()
            });
            out.push(';');
            from = comma + 1;
        }
        copied = end + 1;
    }
    if copied == 0 {
        return None;
    }
    out.push_str(&lexer::render(&tokens[copied..]));
    Some(out)
}

pub(crate) fn rename_texture_sampler(source: &str) -> String {
    let mut tokens = lexer::lex(source);
    rename_texture_sampler_tokens(&mut tokens);
    lexer::render(&tokens)
}

pub(crate) fn rename_texture_sampler_tokens(tokens: &mut [Token]) {
    let declares = lexer::uniform_declarations(tokens).any(|(_, ty, name)| {
        tokens[ty].text.starts_with("sampler") && tokens[name].is_word("texture")
    });
    if declares {
        rename_sampler_uses(tokens);
    }
}

fn rename_sampler_uses(tokens: &mut [Token]) {
    for at in 0..tokens.len() {
        if tokens[at].kind == Kind::Directive {
            lexer::in_define_body(&mut tokens[at], |body| rename_sampler_uses(body));
            continue;
        }
        if !tokens[at].is_word("texture") {
            continue;
        }
        let call = lexer::next_solid(tokens, at + 1).is_some_and(|next| tokens[next].is_punct('('));
        if !call {
            tokens[at].text = "gtexture".to_owned();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PackUniform {
    pub(crate) name: String,
    pub(crate) ty: String,
    pub(crate) array: Option<u32>,
    pub(crate) initial: Option<Vec<u32>>,
}

impl PackUniform {
    pub(crate) fn same_member(&self, other: &PackUniform) -> bool {
        self.name == other.name && self.ty == other.ty && self.array == other.array
    }

    pub(crate) fn size(&self) -> Result<u32, String> {
        let Some(element) = std140_size(&self.ty) else {
            return Err(format!(
                "uniform {0} {1}: no std140 layout is written for {0}",
                self.ty, self.name
            ));
        };
        match self.array {
            None => Ok(element),
            Some(count) if matches!(self.ty.as_str(), "vec4" | "ivec4" | "uvec4" | "mat4") => {
                Ok(count * element)
            }
            Some(count) => Err(format!(
                "uniform {} {}[{count}]: an array of {} has a std140 stride WGSL cannot declare",
                self.ty, self.name, self.ty
            )),
        }
    }

    pub(crate) fn member(&self) -> String {
        let name = &self.name;
        let mut out = match (self.ty.as_str(), self.array) {
            ("bool", _) => format!("    int iris_bool_{name};"),
            (ty, Some(count)) => format!("    {ty} {name}[{count}];"),
            (ty, None) => format!("    {ty} {name};"),
        };
        let pads = match self.ty.as_str() {
            "float" | "int" | "uint" | "bool" => 3,
            "vec2" | "ivec2" | "uvec2" => 2,
            "vec3" | "ivec3" | "uvec3" => 1,
            _ => 0,
        };
        for pad in 0..pads {
            out.push_str(&format!(" float iris_pad_{name}_{pad};"));
        }
        out.push('\n');
        out
    }

    pub(crate) fn define(&self) -> Option<String> {
        (self.ty == "bool").then(|| format!("#define {0} (iris_bool_{0} != 0)\n", self.name))
    }
}

pub(crate) fn std140_size(ty: &str) -> Option<u32> {
    Some(match ty {
        "float" | "int" | "uint" | "bool" | "vec2" | "ivec2" | "uvec2" | "vec3" | "ivec3"
        | "uvec3" | "vec4" | "ivec4" | "uvec4" => 16,
        "mat3" => 48,
        "mat4" => 64,
        _ => return None,
    })
}

fn initial_words(ty: &str, text: &str) -> Option<Vec<u32>> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let (count, kind) = match ty {
        "float" => (1, 'f'),
        "vec2" => (2, 'f'),
        "vec3" => (3, 'f'),
        "vec4" => (4, 'f'),
        "int" => (1, 'i'),
        "ivec2" => (2, 'i'),
        "ivec3" => (3, 'i'),
        "ivec4" => (4, 'i'),
        "uint" => (1, 'u'),
        "uvec2" => (2, 'u'),
        "uvec3" => (3, 'u'),
        "uvec4" => (4, 'u'),
        "bool" => (1, 'b'),
        _ => return None,
    };
    let args: Vec<&str> = match text
        .strip_prefix(ty)
        .and_then(|r| r.strip_prefix('(')?.strip_suffix(')'))
    {
        Some(inner) => inner.split(',').collect(),
        None => vec![text.as_str()],
    };
    let word = |arg: &str| -> Option<u32> {
        if kind == 'b' {
            return match arg {
                "true" => Some(1),
                "false" => Some(0),
                _ => None,
            };
        }
        let value: f64 = arg.trim_end_matches(['f', 'F', 'u', 'U']).parse().ok()?;
        Some(match kind {
            'f' => (value as f32).to_bits(),
            'i' => (value as i32) as u32,
            _ => value as u32,
        })
    };
    let words = args.into_iter().map(word).collect::<Option<Vec<u32>>>()?;
    match words.len() {
        1 => Some(vec![words[0]; count]),
        n if n == count => Some(words),
        _ => None,
    }
}

fn opaque(ty: &str) -> bool {
    let bare = super::untyped(ty);
    ["sampler", "texture", "image"]
        .iter()
        .any(|p| bare.starts_with(p))
}

pub(crate) fn loose_uniforms(tokens: &[Token]) -> Vec<(usize, usize, Result<PackUniform, String>)> {
    let mut out = Vec::new();
    for (start, end) in statements(tokens) {
        let solid: Vec<usize> = (start..end).filter(|at| !tokens[*at].is_trivia()).collect();
        let Some(uniform) = solid.iter().position(|at| tokens[*at].is_word("uniform")) else {
            continue;
        };
        let rest: Vec<usize> = solid[uniform + 1..]
            .iter()
            .copied()
            .filter(|at| !PRECISION.contains(&tokens[*at].text.as_str()))
            .collect();
        let [ty, name, ..] = rest[..] else { continue };
        if opaque(&tokens[ty].text) {
            continue;
        }
        let from = solid[0];
        let uniform =
            |array: Option<u32>, initial: Option<Vec<u32>>| -> Result<PackUniform, String> {
                Ok(PackUniform {
                    name: tokens[name].text.clone(),
                    ty: tokens[ty].text.clone(),
                    array,
                    initial,
                })
            };
        let parsed = match rest[2..] {
            [] => uniform(None, None),
            [eq, first, ..] if tokens[eq].is_punct('=') => {
                let text = lexer::render(&tokens[first..=*rest.last().unwrap_or(&first)]);
                uniform(None, initial_words(&tokens[ty].text, &text))
            }
            [open, count, close] if tokens[open].is_punct('[') && tokens[close].is_punct(']') => {
                match literal::int(&tokens[count].text).and_then(|n| u32::try_from(n).ok()) {
                    Some(count) => uniform(Some(count), None),
                    None => Err(format!(
                        "uniform {} {}[{}]: the array size is not a number after preprocessing",
                        tokens[ty].text, tokens[name].text, tokens[count].text
                    )),
                }
            }
            _ => Err(format!(
                "uniform {} {}: an initializer or a form this does not read",
                tokens[ty].text, tokens[name].text
            )),
        };
        out.push((from, end, parsed));
    }
    out
}

pub(crate) fn gather_uniforms(tokens: &mut Vec<Token>) {
    for (from, to, _) in loose_uniforms(tokens).into_iter().rev() {
        tokens.drain(from..=to);
    }
}

const NOT_HOISTED: &[&str] = &[
    "uniform",
    "in",
    "out",
    "attribute",
    "varying",
    "buffer",
    "shared",
    "layout",
    "precision",
    "struct",
];

fn constructor(name: &str) -> bool {
    let bare = name.strip_prefix(['i', 'u', 'b', 'd']).unwrap_or(name);
    matches!(name, "float" | "int" | "uint" | "bool" | "double")
        || bare
            .strip_prefix("vec")
            .is_some_and(|n| matches!(n, "2" | "3" | "4"))
        || name.strip_prefix("mat").is_some_and(|n| {
            matches!(
                n,
                "2" | "3"
                    | "4"
                    | "2x2"
                    | "2x3"
                    | "2x4"
                    | "3x2"
                    | "3x3"
                    | "3x4"
                    | "4x2"
                    | "4x3"
                    | "4x4"
            )
        })
}

fn unfoldable(tokens: &[Token], from: usize, to: usize, demoted: &[String]) -> bool {
    (from..to).any(|at| {
        let token = &tokens[at];
        if token.kind != Kind::Word {
            return false;
        }
        let call =
            lexer::next_solid(tokens, at + 1).is_some_and(|n| n < to && tokens[n].is_punct('('));
        (call && !constructor(&token.text)) || demoted.contains(&token.text)
    })
}

pub(crate) fn hoist_initializers(tokens: &mut Vec<Token>) {
    let mut hoisted = String::new();
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    let mut demoted: Vec<String> = Vec::new();

    for (start, end) in statements(tokens) {
        let solid: Vec<usize> = (start..end).filter(|at| !tokens[*at].is_trivia()).collect();
        if solid.is_empty()
            || solid
                .iter()
                .any(|at| NOT_HOISTED.contains(&tokens[*at].text.as_str()))
        {
            continue;
        }
        let Some(&equals) = at_depth_zero(tokens, solid[0], end, |t| t.is_punct('=')).first()
        else {
            continue;
        };
        let Some(name) = declarator_name(tokens, solid[0], equals) else {
            continue;
        };
        if (solid[0]..equals).any(|at| tokens[at].is_punct('(')) {
            continue;
        }
        if let Some(&qualifier) = solid[..].iter().find(|at| tokens[**at].is_word("const")) {
            if !unfoldable(tokens, equals + 1, end, &demoted) {
                continue;
            }
            demoted.push(tokens[name].text.clone());
            tokens[qualifier].text.clear();
        }
        hoisted.push_str(&format!(
            "\n    {} = {};",
            tokens[name].text,
            lexer::render(&tokens[equals + 1..end]).trim()
        ));
        let kept = (solid[0]..equals)
            .rev()
            .find(|at| !tokens[*at].is_trivia())
            .unwrap_or(name);
        cuts.push((kept + 1, end));
    }
    if cuts.is_empty() {
        return;
    }
    for (from, to) in cuts.into_iter().rev() {
        tokens.drain(from..to);
    }

    let depths = lexer::brace_depths(tokens);
    for at in 0..tokens.len() {
        if depths[at] != 0 || !tokens[at].is_word("main") {
            continue;
        }
        if !lexer::next_solid(tokens, at + 1).is_some_and(|next| tokens[next].is_punct('(')) {
            continue;
        }
        if let Some(open) = (at..tokens.len()).find(|i| tokens[*i].is_punct('{')) {
            hoisted.push_str(" // Hoisted by shaderpack::transform.\n");
            tokens.insert(open + 1, Token::raw(&hoisted));
            return;
        }
    }
}

pub(crate) fn size_unsized_arrays(tokens: &mut Vec<Token>) {
    let mut sizes: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for at in 0..tokens.len() {
        if !tokens[at].is_punct('[') {
            continue;
        }
        let Some([close, open]) = lexer::solid_run::<2>(tokens, at + 1) else {
            continue;
        };
        if !tokens[close].is_punct(']') || !tokens[open].is_punct('(') {
            continue;
        }
        let Some((args, _)) = lexer::arguments(tokens, open) else {
            continue;
        };
        let count = args.len();
        sizes.entry(close).or_insert(count);
        let mut back = at;
        while back > 0 {
            back -= 1;
            let token = &tokens[back];
            if token.is_punct(';') || token.is_punct('{') || token.is_punct('}') {
                break;
            }
            if token.is_punct('[')
                && let Some(next) = lexer::next_solid(tokens, back + 1)
                && tokens[next].is_punct(']')
            {
                sizes.entry(next).or_insert(count);
            }
        }
    }
    for (close, count) in sizes.into_iter().rev() {
        tokens.insert(close, Token::number(count));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_initializer_is_kept_as_the_uniform_s_value() {
        let tokens = lexer::lex(
            "uniform float Foggy=0.5;\nuniform vec3 c = vec3(1.0, 2.0, 3.0);\nuniform int k = 2;\nuniform float odd = sin(1.0);\n",
        );
        let found: Vec<PackUniform> = loose_uniforms(&tokens)
            .into_iter()
            .map(|(_, _, u)| u.unwrap())
            .collect();
        assert_eq!(found[0].initial, Some(vec![0.5f32.to_bits()]));
        assert_eq!(
            found[1].initial,
            Some([1.0f32, 2.0, 3.0].map(f32::to_bits).to_vec())
        );
        assert_eq!(found[2].initial, Some(vec![2]));
        assert_eq!(found[3].initial, None, "an expression is not evaluated");
    }

    fn hoist(source: &str) -> String {
        let mut tokens = lexer::lex(source);
        hoist_initializers(&mut tokens);
        lexer::render(&tokens)
    }

    #[test]
    fn an_unsized_array_takes_the_size_of_its_constructor() {
        let mut tokens = lexer::lex(
            "const vec3[] tints = vec3[](vec3(1.0), vec3(0.5, 0.5, 0.5), vec3(0.0));\nfloat w[] = float[](1.0, 2.0);\n",
        );
        size_unsized_arrays(&mut tokens);
        let out = lexer::render(&tokens);
        assert!(
            out.contains(
                "const vec3[3] tints = vec3[3](vec3(1.0), vec3(0.5, 0.5, 0.5), vec3(0.0));"
            ),
            "{out}"
        );
        assert!(out.contains("float w[2] = float[2](1.0, 2.0);"), "{out}");
    }

    #[test]
    fn several_names_become_several_statements_on_one_line() {
        let out = split_declarators("uniform float far, near;\nflat in int a, b;\n");
        assert_eq!(
            out,
            "uniform float far; uniform float near;\nflat in int a; flat in int b;\n"
        );
    }

    #[test]
    fn commas_inside_calls_and_prototypes_do_not_split() {
        let source =
            "vec2 a = vec2(0.0, 1.0), b;\nvec4 f(vec2 p, float q);\nvoid main() { float x, y; }\n";
        let out = split_declarators(source);
        assert!(out.contains("vec2 a = vec2(0.0, 1.0); vec2 b;"), "{out}");
        assert!(out.contains("vec4 f(vec2 p, float q);"), "{out}");
        assert!(out.contains("float x, y;"), "{out}");
    }

    #[test]
    fn a_sampler_named_texture_is_renamed_and_the_builtin_is_not() {
        let out = rename_texture_sampler(
            "uniform sampler2D texture;\nvoid main() { vec4 c = texture(texture, uv); }\n",
        );
        assert!(out.contains("uniform sampler2D gtexture;"), "{out}");
        assert!(out.contains("texture(gtexture, uv)"), "{out}");
        let defined = rename_texture_sampler(
            "uniform sampler2D texture;\n#define ALBEDO textureLod(texture, uv, 0)\n",
        );
        assert!(
            defined.contains("#define ALBEDO textureLod(gtexture, uv, 0)"),
            "{defined}"
        );
    }

    #[test]
    fn loose_uniforms_are_found_and_opaque_ones_are_not() {
        let tokens = lexer::lex(
            "uniform float far;\nuniform highp vec3 cameraPosition;\nuniform sampler2D gtexture;\nuniform vec4 lights[4];\nuniform float bad[COUNT];\n",
        );
        let found: Vec<_> = loose_uniforms(&tokens)
            .into_iter()
            .map(|(_, _, u)| u)
            .collect();
        assert_eq!(found.len(), 4);
        assert_eq!(found[0].as_ref().unwrap().name, "far");
        assert_eq!(found[1].as_ref().unwrap().ty, "vec3");
        assert_eq!(found[2].as_ref().unwrap().array, Some(4));
        assert!(found[3].is_err());
    }

    #[test]
    fn a_member_is_padded_to_sixteen_bytes() {
        let far = PackUniform {
            name: "far".into(),
            ty: "float".into(),
            array: None,
            initial: None,
        };
        assert_eq!(far.size(), Ok(16));
        assert_eq!(far.member().matches("iris_pad_far").count(), 3);
        let floats = PackUniform {
            name: "w".into(),
            ty: "float".into(),
            array: Some(4),
            initial: None,
        };
        assert!(floats.size().is_err());
        let flag = PackUniform {
            name: "on".into(),
            ty: "bool".into(),
            array: None,
            initial: None,
        };
        assert!(flag.member().contains("int iris_bool_on;"));
        assert_eq!(
            flag.define().as_deref(),
            Some("#define on (iris_bool_on != 0)\n")
        );
    }

    #[test]
    fn a_global_initializer_moves_into_main() {
        let out = hoist(
            "uniform float t;\nconst float k = 2.0;\nfloat a = fract(t - 0.25);\nvec3 v;\nvoid main() {\n    gl_Position = vec4(a);\n}\n",
        );
        assert!(out.contains("float a;"), "{out}");
        assert!(
            out.contains("void main() {\n    a = fract(t - 0.25);"),
            "{out}"
        );
        assert!(out.contains("const float k = 2.0;"), "{out}");
    }

    #[test]
    fn a_constant_naga_cannot_fold_is_demoted_with_its_dependents() {
        let out = hoist(
            "const vec3 a = normalize(vec3(1.0, 2.0, 3.0)) * 2.0;
const vec3 b = a * 0.5;
const vec3 c = vec3(1.0);
void main() {}
",
        );
        assert!(out.contains(" vec3 a;"), "{out}");
        assert!(out.contains(" vec3 b;"), "{out}");
        assert!(
            out.contains("a = normalize(vec3(1.0, 2.0, 3.0)) * 2.0;"),
            "{out}"
        );
        assert!(out.contains("b = a * 0.5;"), "{out}");
        assert!(out.contains("const vec3 c = vec3(1.0);"), "{out}");
    }
}
