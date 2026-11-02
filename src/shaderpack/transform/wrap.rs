use super::builtins::Used;
use super::lexer::{self, Kind, Token};
use super::loops::LOOP_BUDGET;
use super::{
    ALPHA_OP_DEFINE, ALPHA_TEST_DEFINE, Interface, Stage, TARGET_VERSION, Target, UNIFORM_BINDING,
    UNIFORMS,
};

pub(super) fn version(tokens: &mut Vec<Token>) {
    let line = format!("#version {TARGET_VERSION}");
    match tokens.iter().position(|t| !t.is_trivia()) {
        Some(at) if lexer::is_version(&tokens[at]) => tokens[at].text = line,
        _ => {
            tokens.retain(|t| !lexer::is_version(t));
            tokens.insert(0, Token::directive(&line));
            tokens.insert(1, Token::newline());
        }
    }

    let Some(code) = tokens
        .iter()
        .position(|t| !t.is_trivia() && t.kind != Kind::Directive)
    else {
        return;
    };
    let late: Vec<usize> = (code..tokens.len())
        .filter(|&at| lexer::is_extension(&tokens[at]))
        .collect();
    let mut moved = Vec::with_capacity(2 * late.len());
    for &at in late.iter().rev() {
        moved.push(tokens.remove(at));
        moved.push(Token::newline());
    }
    moved.reverse();
    let after = tokens
        .iter()
        .position(lexer::is_version)
        .map_or(0, |at| at + 1);
    tokens.splice(after..after, moved);
}

pub(super) fn epilogue(tokens: &mut Vec<Token>, interface: &Interface, target: &Target) {
    rename_main(tokens);

    let mut out =
        String::from("\n// Inserted by shaderpack::transform.\nvoid main() {\n    iris_main();\n");
    if interface.outputs > 0 {
        out.push_str(&format!(
            "#ifdef {ALPHA_TEST_DEFINE}\n#ifndef {ALPHA_OP_DEFINE}\n#define {ALPHA_OP_DEFINE} >\n#endif\n    if (!(iris_FragData0.a {ALPHA_OP_DEFINE} {ALPHA_TEST_DEFINE})) {{\n        discard;\n    }}\n#endif\n"
        ));
    }
    for n in 0..attached(interface, target) {
        if n == 0 && target.linear_output {
            out.push_str(
                "    vec3 iris_Display = clamp(iris_FragData0.rgb, 0.0, 1.0);\n    \
                 vec3 iris_Low = step(iris_Display, vec3(0.04045));\n    \
                 vec3 iris_Linear = mix(pow((iris_Display + 0.055) / 1.055, vec3(2.4)), iris_Display / 12.92, iris_Low);\n    \
                 iris_Out0 = vec4(iris_Linear, iris_FragData0.a);\n",
            );
        } else {
            out.push_str(&format!("    iris_Out{n} = iris_FragData{n};\n"));
        }
    }
    out.push_str("}\n");
    tokens.push(Token::raw(&out));
}

fn rename_main(tokens: &mut [Token]) {
    for at in 0..tokens.len() {
        if tokens[at].is_word("main")
            && lexer::next_solid(tokens, at + 1).is_some_and(|next| tokens[next].is_punct('('))
        {
            tokens[at].text = "iris_main".to_owned();
        }
    }
}

pub(super) fn vertex_epilogue(tokens: &mut Vec<Token>, upright: bool, begin: &str, end: &str) {
    rename_main(tokens);
    let flip = if upright {
        ""
    } else {
        "gl_Position.y = -gl_Position.y;\n    "
    };
    tokens.push(Token::raw(&format!(
        "\n// Inserted by shaderpack::transform.\nvoid main() {{\n    {begin}iris_main();\n    {end}\
         {flip}gl_Position.z = (gl_Position.z + gl_Position.w) * 0.5;\n}}\n"
    )));
}

fn attached(interface: &Interface, target: &Target) -> u32 {
    target
        .outputs
        .map_or(interface.outputs, |n| n.min(interface.outputs))
}

pub(super) fn preamble(
    tokens: &mut Vec<Token>,
    stage: Stage,
    interface: &Interface,
    used: &Used,
    target: &Target,
    guarded: bool,
) {
    let set = target.set;
    let mut out = String::new();
    out.push_str("\n// Inserted by shaderpack::transform.\n");
    if guarded {
        out.push_str(&format!("int iris_LoopBudget = {LOOP_BUDGET};\n"));
    }

    out.push_str(&format!(
        "layout(set = {set}, binding = {UNIFORM_BINDING}) uniform IrisFrame {{\n"
    ));
    for (name, ty) in UNIFORMS {
        out.push_str(&format!("    {ty} {name};\n"));
    }
    for uniform in &interface.uniforms {
        out.push_str(&uniform.member());
    }
    out.push_str("};\n");
    for uniform in &interface.uniforms {
        if let Some(define) = uniform.define() {
            out.push_str(&define);
        }
    }

    if stage == Stage::Fragment && !target.fragment_prelude.is_empty() {
        out.push_str(target.fragment_prelude);
        out.push('\n');
    }

    if used.dot {
        out.push_str(
            "float iris_dot(float a, float b) { return a * b; }\n\
             float iris_dot(vec2 a, vec2 b) { return dot(a, b); }\n\
             float iris_dot(vec3 a, vec3 b) { return dot(a, b); }\n\
             float iris_dot(vec4 a, vec4 b) { return dot(a, b); }\n",
        );
    }

    if used.modf {
        for ty in ["float", "vec2", "vec3", "vec4"] {
            out.push_str(&format!(
                "{ty} iris_modf({ty} x, out {ty} whole) {{ whole = trunc(x); return x - whole; }}\n"
            ));
        }
    }
    for (wanted, name, test) in [
        (
            used.isnan,
            "iris_isnan",
            "(floatBitsToUint(x) & 0x7fffffffu) > 0x7f800000u",
        ),
        (
            used.isinf,
            "iris_isinf",
            "(floatBitsToUint(x) & 0x7fffffffu) == 0x7f800000u",
        ),
    ] {
        if wanted {
            out.push_str(&format!("bool {name}(float x) {{ return {test}; }}\n"));
            for (ty, bvec, n) in [
                ("vec2", "bvec2", 2),
                ("vec3", "bvec3", 3),
                ("vec4", "bvec4", 4),
            ] {
                let parts: Vec<String> = ["x", "y", "z", "w"][..n]
                    .iter()
                    .map(|c| format!("{name}(v.{c})"))
                    .collect();
                out.push_str(&format!(
                    "{bvec} {name}({ty} v) {{ return {bvec}({}); }}\n",
                    parts.join(", ")
                ));
            }
        }
    }

    if used.fog {
        out.push_str(
            "struct iris_FogParameters { vec4 color; float density; float start; float end; float scale; };\n\
             iris_FogParameters iris_gl_Fog() {\n    \
             return iris_FogParameters(iris_FogColor, iris_FogParams.z, iris_FogParams.x, iris_FogParams.y, \
             1.0 / (iris_FogParams.y - iris_FogParams.x));\n\
             }\n",
        );
    }
    if used.front_color {
        out.push_str("vec4 iris_FrontColor;\n");
    }

    if stage == Stage::Vertex {
        out.push_str(target.vertex_prelude);
        out.push('\n');
        if used.ftransform {
            out.push_str(
                "vec4 iris_ftransform() {\n    \
                 return iris_ModelViewProjectionMatrix * vec4(iris_position(), 1.0);\n\
                 }\n",
            );
        }
    }

    let attached = attached(interface, target);
    for n in 0..interface.outputs {
        out.push_str(&format!("vec4 iris_FragData{n};\n"));
        if n < attached {
            out.push_str(&format!("layout(location = {n}) out vec4 iris_Out{n};\n"));
        }
    }

    let version = tokens
        .iter()
        .position(lexer::is_version)
        .map_or(0, |at| at + 1);
    let mut after = version;
    for (at, token) in tokens.iter().enumerate().skip(version) {
        if lexer::is_extension(token) {
            after = at + 1;
        } else if !(token.is_trivia() || token.kind == Kind::Directive) {
            break;
        }
    }
    tokens.insert(after, Token::raw(&out));
}
