use super::TERRAIN_SET;
use crate::renderer::terrain_pool::ORIGIN_ROW;
use crate::renderer::terrain_pool::draw::SLOT_TABLE_BINDING;

pub(crate) fn terrain_prelude(direct: bool) -> String {
    let origin = if direct {
        format!(
            "layout(set = {TERRAIN_SET}, binding = {SLOT_TABLE_BINDING}) uniform itexture2D iris_Origins;\n\
             ivec3 iris_origin() {{\n    \
                 int slot = iris_RawPosition.w & 0xFFFF;\n    \
                 return texelFetch(iris_Origins, ivec2(slot % {ORIGIN_ROW}, slot / {ORIGIN_ROW}), 0).xyz;\n\
             }}\n"
        )
    } else {
        format!(
            "layout(std430, set = {TERRAIN_SET}, binding = {SLOT_TABLE_BINDING}) readonly buffer IrisSlotTable {{\n    \
                 ivec4 iris_SlotWords[];\n\
             }};\n\
             ivec3 iris_origin() {{\n    \
                 return iris_SlotWords[gl_InstanceIndex * 4 + 2].xyz;\n\
             }}\n"
        )
    };
    format!(
        "layout(location = 0) in ivec4 iris_RawPosition;\n\
         layout(location = 1) in vec2 iris_RawUV0;\n\
         layout(location = 2) in vec4 iris_RawLight;\n\
         layout(location = 3) in vec4 iris_RawColor;\n\
         {origin}\
         ivec3 iris_Origin;\n\
         vec3 iris_position() {{\n    \
             vec3 camera_block = floor(iris_CameraPosition.xyz);\n    \
             return vec3(iris_Origin - ivec3(camera_block)) - (iris_CameraPosition.xyz - camera_block)\n        \
                 + vec3(iris_RawPosition.xyz) / 256.0;\n\
         }}\n\
         vec2 iris_uv0() {{ return iris_RawUV0; }}\n\
         vec2 iris_light() {{ return iris_RawLight.xy * 255.0; }}\n\
         vec4 iris_color() {{ return iris_RawColor; }}\n\
         {PACK_STREAM}"
    )
}

pub(crate) const PACK_QUADS_SET: u32 = 2;

const PACK_STREAM: &str = concat!(
    "layout(set = 2, binding = 0) uniform utexture2D iris_PackQuads;\n",
    "// Decoded once, before the pack's `main` (`TERRAIN_BEGIN`).\n",
    "uvec4 iris_Quad;\n",
    "uvec4 iris_pack_quad() {\n",
    "    int quad = int(gl_VertexIndex) >> 2;\n",
    "    return texelFetch(iris_PackQuads, ivec2(quad % 2048, quad / 2048), 0);\n",
    "}\n",
    "vec3 iris_snorm3(uint v) {\n",
    "    ivec3 b = ivec3(int(v << 24u) >> 24, int(v << 16u) >> 24, int(v << 8u) >> 24);\n",
    "    return max(vec3(b) / 127.0, vec3(-1.0));\n",
    "}\n",
    "vec3 iris_normal() {\n",
    "    vec3 n = iris_snorm3(iris_Quad.x);\n",
    "    return dot(n, n) > 0.25 ? normalize(n) : vec3(0.0, 1.0, 0.0);\n",
    "}\n",
    "vec4 iris_tangent() {\n",
    "    uint word = iris_Quad.y;\n",
    "    vec3 t = iris_snorm3(word);\n",
    "    return dot(t, t) > 0.25 ? vec4(normalize(t), int(word) < 0 ? -1.0 : 1.0) : vec4(1.0, 0.0, 0.0, 1.0);\n",
    "}\n",
    "vec4 iris_mid_uv() {\n",
    "    uint word = iris_Quad.z;\n",
    "    return vec4(vec2(float(word & 65535u), float(word >> 16u)) / 65535.0, 0.0, 1.0);\n",
    "}\n",
    "vec4 iris_entity() {\n",
    "    uint id = iris_Quad.w & 65535u;\n",
    "    return vec4(id == 65535u ? -1.0 : float(id), 0.0, 0.0, 1.0);\n",
    "}\n",
    "vec4 iris_mid_block() {\n",
    "    uint word = iris_Quad.w;\n",
    "    vec3 cell = vec3(float((word >> 16u) & 15u), float((word >> 20u) & 15u), float((word >> 24u) & 15u));\n",
    "    return vec4((cell + 0.5 - vec3(iris_RawPosition.xyz) / 256.0) * 64.0, float(word >> 28u));\n",
    "}\n",
);

pub(super) const TERRAIN_BEGIN: &str =
    "iris_Origin = iris_origin();\n    iris_Quad = iris_pack_quad();\n    ";

const _: () = assert!(PACK_QUADS_SET == 2 && crate::renderer::packvertex::QUAD_ROW == 2048);

pub(super) const ENTITY_PRELUDE: &str = "layout(location = 0) in vec3 iris_EntityPosition;\n\
layout(location = 1) in vec3 iris_EntityNormal;\n\
layout(location = 2) in vec2 iris_EntityUV;\n\
layout(location = 3) in vec4 iris_EntityColor;\n\
layout(location = 4) in vec4 iris_ModelRow0;\n\
layout(location = 5) in vec4 iris_ModelRow1;\n\
layout(location = 6) in vec4 iris_ModelRow2;\n\
layout(location = 7) in vec4 iris_EntityTint;\n\
layout(location = 8) in vec4 entityColor;\n\
layout(location = 9) in vec2 iris_EntityLight;\n\
layout(location = 10) in int entityId;\n\
layout(location = 11) in int blockEntityId;\n\
layout(location = 12) in int currentRenderedItemId;\n\
layout(location = 13) in int renderStage;\n\
layout(location = 14) in uint iris_EntityFlags;\n\
vec3 iris_model(vec4 v) { return vec3(dot(iris_ModelRow0, v), dot(iris_ModelRow1, v), dot(iris_ModelRow2, v)); }\n\
vec3 iris_position() { return iris_model(vec4(iris_EntityPosition, 1.0)); }\n\
vec2 iris_uv0() { return iris_EntityUV; }\n\
vec2 iris_light() { return iris_EntityLight; }\n\
vec4 iris_color() {\n    \
    vec4 color = iris_EntityColor;\n    \
    if ((iris_EntityFlags & 1u) != 0u) { color.a = 1.0; }\n    \
    return color * iris_EntityTint;\n\
}\n\
vec3 iris_normal() {\n    \
    vec3 n = iris_model(vec4(iris_EntityNormal, 0.0));\n    \
    return dot(n, n) > 1e-8 ? normalize(n) : vec3(0.0, 1.0, 0.0);\n\
}\n\
vec4 iris_entity() { return vec4(-1.0, 0.0, 0.0, 1.0); }\n\
vec4 iris_tangent() { return vec4(1.0, 0.0, 0.0, 1.0); }\n\
vec4 iris_mid_uv() { return vec4(iris_EntityUV, 0.0, 1.0); }\n\
vec4 iris_mid_block() { return vec4(0.0); }\n";

pub(super) const ENTITY_UNIFORMS: [(&str, &str); 5] = [
    ("entityColor", "vec4"),
    ("entityId", "int"),
    ("blockEntityId", "int"),
    ("currentRenderedItemId", "int"),
    ("renderStage", "int"),
];

pub(super) fn entity_varyings(fragment: &str, first: u32) -> (String, String, String) {
    let tokens = crate::shaderpack::transform::lexer::lex(fragment);
    let (mut inputs, mut outputs, mut copies) = (String::new(), String::new(), String::new());
    let named = ENTITY_UNIFORMS
        .iter()
        .filter(|(name, _)| tokens.iter().any(|t| t.is_word(name)));
    for (location, (name, ty)) in (first..).zip(named) {
        inputs.push_str(&format!(
            "layout(location = {location}) flat in {ty} {name};\n"
        ));
        outputs.push_str(&format!(
            "layout(location = {location}) flat out {ty} iris_Flat_{name};\n"
        ));
        copies.push_str(&format!("iris_Flat_{name} = {name};\n    "));
    }
    (inputs, outputs, copies)
}

const _: () = assert!(crate::renderer::packvertex::UNMAPPED == 65535);

pub(crate) const SCREEN_PRELUDE: &str = "vec2 iris_corner() {\n    \
    return vec2(float((gl_VertexIndex << 1) & 2), float(gl_VertexIndex & 2));\n\
}\n\
vec3 iris_position() { return vec3(iris_corner(), 0.0); }\n\
vec2 iris_uv0() { return iris_corner(); }\n\
vec2 iris_light() { return vec2(240.0); }\n\
vec4 iris_color() { return vec4(1.0); }\n\
vec3 iris_normal() { return vec3(0.0, 0.0, 1.0); }\n";

#[cfg(test)]
mod tests {
    use super::super::PACK_SET;
    use super::*;
    use crate::shaderpack::backend;
    use crate::shaderpack::transform::{self, Stage};

    #[test]
    fn entity_values_reach_the_fragment_stage() {
        let vsh = "#version 130\nuniform int entityId;\nvarying vec2 texcoord;\n\
                   void main() { gl_Position = ftransform(); texcoord = gl_MultiTexCoord0.xy + float(entityId); }\n";
        let fsh = "#version 130\nuniform vec4 entityColor;\nuniform int entityId;\nvarying vec2 texcoord;\n\
                   void main() { gl_FragData[0] = mix(vec4(texcoord, 0.0, 1.0), entityColor, entityColor.a) * float(entityId); }\n";
        let mut shared = transform::Shared::of(vsh, fsh);
        shared
            .uniforms
            .retain(|u| !ENTITY_UNIFORMS.iter().any(|(name, _)| u.name == *name));
        let (inputs, outputs, copies) = entity_varyings(fsh, shared.varying_end);
        assert!(
            inputs.contains("layout(location = 1) flat in vec4 entityColor;"),
            "{inputs}"
        );
        assert!(
            inputs.contains("layout(location = 2) flat in int entityId;"),
            "{inputs}"
        );
        let prelude = format!("{ENTITY_PRELUDE}{outputs}");
        let target = transform::Target {
            set: PACK_SET,
            vertex_prelude: &prelude,
            fragment_prelude: &inputs,
            vertex_end: &copies,
            outputs: Some(1),
            gl_clip: true,
            attribute_defaults: true,
            ..Default::default()
        };
        for (source, stage, naga_stage) in [
            (vsh, Stage::Vertex, backend::ShaderStage::Vertex),
            (fsh, Stage::Fragment, backend::ShaderStage::Fragment),
        ] {
            let out = transform::transform(source, stage, &shared, &target);
            let wgsl = backend::to_wgsl(&out.source, naga_stage, &[])
                .unwrap_or_else(|e| panic!("{e}\n\n{}", out.source));
            assert!(wgsl.contains("@location(2) @interpolate(flat)"), "{wgsl}");
        }
    }

    #[test]
    fn every_prelude_compiles() {
        let vsh = "#version 130\nattribute vec4 mc_Entity;\nvarying vec2 texcoord;\nvarying vec2 lmcoord;\nvarying vec4 tint;\n\
                   void main() { gl_Position = ftransform(); texcoord = (gl_TextureMatrix[0] * gl_MultiTexCoord0).xy;\n\
                   lmcoord = (gl_TextureMatrix[1] * gl_MultiTexCoord1).xy; tint = gl_Color * mc_Entity.x;\n\
                   vec3 n = gl_NormalMatrix * gl_Normal; tint.rgb *= max(n.y, 0.5); }\n";
        let shared = transform::Shared::of(vsh, "");
        for (prelude, geometry) in [
            (terrain_prelude(false), true),
            (terrain_prelude(true), true),
            (SCREEN_PRELUDE.to_owned(), false),
        ] {
            let target = transform::Target {
                set: PACK_SET,
                vertex_prelude: &prelude,
                gl_clip: true,
                attribute_defaults: geometry,
                ..Default::default()
            };
            let out = transform::transform(vsh, Stage::Vertex, &shared, &target);
            let wgsl = backend::to_wgsl(&out.source, backend::ShaderStage::Vertex, &[])
                .unwrap_or_else(|e| panic!("{e}\n\n{}", out.source));
            if geometry {
                assert!(
                    wgsl.contains(&format!(
                        "@group({TERRAIN_SET}) @binding({SLOT_TABLE_BINDING})"
                    )),
                    "{wgsl}"
                );
            }
        }
    }
}
