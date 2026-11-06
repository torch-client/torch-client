#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::view,
    forward_io::{Vertex, VertexOutput},
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let hand_near = bitcast<f32>(#{HAND_NEAR_BITS}u);
    let hand_fov = bitcast<f32>(#{HAND_FOV_BITS}u);
    let hand_band = bitcast<f32>(#{HAND_BAND_BITS}u);

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);

#ifdef VERTEX_NORMALS
    let view_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex.instance_index
    );
    out.world_normal = normalize((view.world_from_view * vec4<f32>(view_normal, 0.0)).xyz);
#endif

#ifdef VERTEX_POSITIONS
    let p = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0)).xyz;
    out.world_position = view.world_from_view * vec4<f32>(p, 1.0);
    let aspect = view.viewport.z / view.viewport.w;
    let f = 1.0 / tan(hand_fov * 0.5);
    let w = -p.z;
    out.position = vec4<f32>(p.x * f / aspect, p.y * f, w * (1.0 - hand_band) + hand_band * hand_near, w);
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif

#ifdef VERTEX_TANGENTS
    let view_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex.instance_index
    );
    out.world_tangent = vec4<f32>((view.world_from_view * vec4<f32>(view_tangent.xyz, 0.0)).xyz, view_tangent.w);
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif

#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index, world_from_local[3]);
#endif

    return out;
}
