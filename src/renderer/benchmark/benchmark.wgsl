#import bevy_pbr::mesh_functions
#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::view_transformations::position_world_to_clip

struct BenchParams {
    sky: vec4<f32>,
    fog: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: BenchParams;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) world_position: vec3<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));
    let sun = normalize(vec3(0.4, 1.0, 0.25));
    let light = 0.45 + 0.55 * max(dot(vertex.normal, sun), 0.0);
    out.color = vertex.color.rgb * light;
    out.world_position = world.xyz;
    out.clip_position = position_world_to_clip(world.xyz);
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let dist = length(in.world_position - view.world_position);
    let fog = clamp((dist - params.fog.x) / (params.fog.y - params.fog.x), 0.0, 1.0);
    return vec4(mix(in.color, params.sky.rgb, fog), 1.0);
}
