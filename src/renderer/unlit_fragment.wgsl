#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}
#import client::unlit::{unlit_base_color, unlit_finish}

#ifdef VISIBILITY_RANGE_DITHER
#import bevy_pbr::pbr_functions::visibility_range_dither
#endif

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
#ifdef VISIBILITY_RANGE_DITHER
    visibility_range_dither(in.position, in.visibility_range_dither);
#endif

    var out: FragmentOutput;
    out.color = unlit_finish(in, unlit_base_color(in));
    return out;
}
