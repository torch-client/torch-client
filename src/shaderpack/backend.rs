use naga::back::wgsl;
use naga::front::glsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};

pub(crate) use naga::ShaderStage;

pub(crate) fn to_wgsl(source: &str, stage: ShaderStage) -> Result<String, String> {
    let mut frontend = glsl::Frontend::default();
    let module = frontend
        .parse(&glsl::Options::from(stage), source)
        .map_err(|errors| {
            errors.errors.first().map_or_else(
                || "the shader could not be parsed".to_owned(),
                |e| e.to_string(),
            )
        })?;

    let info = Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .map_err(|e| format!("the shader is not valid: {}", chain(&e)))?;

    wgsl::write_string(&module, &info, wgsl::WriterFlags::empty())
        .map_err(|e| format!("the shader could not be written as WGSL: {e}"))
}

fn chain(error: &dyn std::error::Error) -> String {
    let mut out = error.to_string();
    let mut source = error.source();
    while let Some(next) = source {
        out.push_str(": ");
        out.push_str(&next.to_string());
        source = next.source();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn which_declaration_forms_naga_accepts() {
        let cases = [
            ("empty, no profile", "#version 450\nvoid main() {}\n".to_owned()),
            ("empty, core profile", "#version 450 core\nvoid main() {}\n".to_owned()),
            (
                "one output",
                "#version 450\nlayout(location = 0) out vec4 c;\nvoid main() { c = vec4(1.0); }\n".to_owned(),
            ),
            (
                "one input",
                "#version 450\nlayout(location = 0) in vec2 uv;\nlayout(location = 0) out vec4 c;\nvoid main() { c = vec4(uv, 0.0, 1.0); }\n".to_owned(),
            ),
            (
                "bare in/out, no location",
                "#version 450\nin vec2 uv;\nout vec4 c;\nvoid main() { c = vec4(uv, 0.0, 1.0); }\n".to_owned(),
            ),
            (
                "loose sampler uniform",
                "#version 450\nlayout(location = 0) out vec4 c;\nuniform sampler2D tex;\nvoid main() { c = texture(tex, vec2(0.0)); }\n".to_owned(),
            ),
            (
                "sampler with binding",
                "#version 450\nlayout(location = 0) out vec4 c;\nlayout(set = 0, binding = 0) uniform sampler2D tex;\nvoid main() { c = texture(tex, vec2(0.0)); }\n".to_owned(),
            ),
            (
                "loose scalar uniform",
                "#version 450\nlayout(location = 0) out vec4 c;\nuniform float k;\nvoid main() { c = vec4(k); }\n".to_owned(),
            ),
            (
                "separated texture and sampler",
                "#version 450\nlayout(location = 0) out vec4 c;\nlayout(set = 0, binding = 0) uniform texture2D tex;\nlayout(set = 0, binding = 1) uniform sampler samp;\nvoid main() { c = texture(sampler2D(tex, samp), vec2(0.0)); }\n".to_owned(),
            ),
            (
                "scalars in a bound block",
                "#version 450\nlayout(location = 0) out vec4 c;\nlayout(set = 0, binding = 0) uniform U { float k; mat4 m; } u;\nvoid main() { c = u.m * vec4(u.k); }\n".to_owned(),
            ),
            (
                "anonymous uniform block",
                "#version 450\nlayout(location = 0) out vec4 c;\nlayout(set = 0, binding = 0) uniform U { float k; mat4 m; };\nvoid main() { c = m * vec4(k); }\n".to_owned(),
            ),
            (
                "sampler2D as a function parameter",
                "#version 450\nlayout(location = 0) out vec4 c;\nlayout(set = 0, binding = 0) uniform texture2D tex;\nlayout(set = 0, binding = 1) uniform sampler samp;\nvec4 fetch(sampler2D s) { return texture(s, vec2(0.0)); }\nvoid main() { c = fetch(sampler2D(tex, samp)); }\n".to_owned(),
            ),
            (
                "3D texture and sampler",
                "#version 450\nlayout(location = 0) out vec4 c;\nlayout(set = 0, binding = 0) uniform texture3D vol;\nlayout(set = 0, binding = 1) uniform sampler samp;\nvoid main() { c = texture(sampler3D(vol, samp), vec3(0.0)); }\n".to_owned(),
            ),
        ];

        let mut any = false;
        for (name, source) in &cases {
            match to_wgsl(source, ShaderStage::Fragment) {
                Ok(wgsl) => {
                    any = true;
                    println!("{name}: ok");
                    assert!(wgsl.contains("fn main"), "{wgsl}");
                }
                Err(e) => println!("{name}: {e}"),
            }
        }
        assert!(any, "naga accepted none of the forms tried");
    }

    #[test]
    fn the_version_a_pack_ships_is_refused() {
        let glsl = "#version 130\nvoid main() {}\n";
        let error = to_wgsl(glsl, ShaderStage::Fragment).unwrap_err();
        assert!(error.to_lowercase().contains("version"), "{error}");
    }

    #[test]
    fn a_compatibility_profile_builtin_is_refused() {
        let glsl = "#version 450 core\nvoid main() { gl_FragData[0] = vec4(1.0); }\n";
        assert!(to_wgsl(glsl, ShaderStage::Fragment).is_err());
    }
}
