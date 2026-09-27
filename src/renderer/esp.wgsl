#import bevy_pbr::mesh_functions
#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::view_transformations::position_world_to_clip

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#endif

struct EspParams {
    line: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: EspParams;

fn shade_of(face: u32) -> f32 {
    if face == 0u { return 1.0; }
    if face == 1u { return 0.5; }
    if face == 2u || face == 3u { return 0.8; }
    return 0.6;
}

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) packed: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) edges: u32,
};

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let cutoff = c <= vec3(0.04045);
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3(2.4));
    return select(high, low, cutoff);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_position =
        mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));
    out.clip_position = position_world_to_clip(world_position.xyz);

    let corner = vertex.packed & 3u;
    out.uv = vec2(f32(corner & 1u), f32((corner >> 1u) & 1u));
    out.edges = (vertex.packed >> 2u) & 15u;
    let face = (vertex.packed >> 6u) & 7u;

    out.color = vec4(srgb_to_linear(vertex.color.rgb) * shade_of(face), vertex.color.a);
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var d = 1.0e9;
    if (in.edges & 1u) != 0u { d = min(d, in.uv.x); }
    if (in.edges & 2u) != 0u { d = min(d, 1.0 - in.uv.x); }
    if (in.edges & 4u) != 0u { d = min(d, in.uv.y); }
    if (in.edges & 8u) != 0u { d = min(d, 1.0 - in.uv.y); }

    let w = clamp(fwidth(d) * params.line.x, 1.0e-6, 0.25);
    let edge = 1.0 - smoothstep(0.0, w, d);

    let bright = min(in.color.rgb * 1.35 + vec3(0.08), vec3(1.0));
    var color = vec4(mix(in.color.rgb, bright, edge), mix(in.color.a, 1.0, edge));

    if color.a < 0.004 {
        discard;
    }
#ifdef TONEMAP_IN_SHADER
    color = tone_mapping(color, view.color_grading);
#endif
    return color;
}
