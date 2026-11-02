struct SlotMeta {
    min: vec3<f32>,
    first_index: u32,
    max: vec3<f32>,
    index_count: u32,
    origin: vec3<i32>,
    base_vertex: u32,
    solid_count: u32,
    flags: u32,
    pool: u32,
    cutout_first: u32,
    faces: array<vec4<u32>, 2>,
}

struct DrawIndexedIndirectArgs {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

struct CullView {
    planes: array<vec4<f32>, 13>,
    within_min: vec4<f32>,
    within_max: vec4<f32>,
    always_min: vec4<f32>,
    always_max: vec4<f32>,
    fog: vec4<f32>,
    eye: vec4<f32>,
    slot_cap: u32,
    view_index: u32,
    pool_count: u32,
    compact: u32,
    flags: u32,
    plane_count: u32,
    pad0: u32,
    pad1: u32,
    origin: vec4<i32>,
}

const STREAM_SOLID: u32 = 0u;
const STREAM_CUTOUT: u32 = 1u;
const STREAMS: u32 = 3u;
const META_LIVE: u32 = 1u;
const META_WATER: u32 = 2u;
const VIEW_UNOCCLUDED: u32 = 2u;
const VIEW_WITHIN: u32 = 4u;
const VIEW_ALWAYS: u32 = 8u;
const VIEW_FOG: u32 = 16u;
const VIEW_FACES: u32 = 32u;
const FACE_RUNS: u32 = 4u;
const FACE_GROUPS: u32 = 7u;
const ALL_GROUPS: u32 = 0x7Fu;
const RANK_GROUP = array<u32, 7>(0u, 4u, 2u, 6u, 3u, 5u, 1u);
const FILL_QUADS: u32 = 32u;

@group(0) @binding(0) var<storage, read> metas: array<SlotMeta>;
@group(0) @binding(1) var<storage, read> vis: array<u32>;
@group(0) @binding(2) var<storage, read_write> commands: array<DrawIndexedIndirectArgs>;
@group(0) @binding(3) var<storage, read_write> counts: array<atomic<u32>>;
@group(0) @binding(4) var<uniform> cull: CullView;

fn in_frustum(lo: vec3<f32>, hi: vec3<f32>, section: vec3<f32>) -> bool {
    if (cull.flags & VIEW_WITHIN) != 0u
        && (any(hi < cull.within_min.xyz) || any(lo > cull.within_max.xyz)) {
        return false;
    }
    if (cull.flags & VIEW_FOG) != 0u {
#ifdef LOD_REAL_CUT
        let d = min(max(cull.fog.xyz, section), section + vec3(16.0)) - cull.fog.xyz;
#else
        let d = min(max(cull.fog.xyz, lo), hi) - cull.fog.xyz;
#endif
        if (max(length(d.xz), abs(d.y)) > cull.fog.w) {
            return false;
        }
    }
    if (cull.flags & VIEW_ALWAYS) != 0u
        && all(hi >= cull.always_min.xyz) && all(lo <= cull.always_max.xyz) {
        return true;
    }
    for (var i = 0u; i < cull.plane_count; i = i + 1u) {
        let p = cull.planes[i];
        let v = select(lo, hi, p.xyz > vec3<f32>(0.0));
        if (dot(p.xyz, v) + p.w < 0.0) {
            return false;
        }
    }
    return true;
}

fn region_stride() -> u32 {
    return cull.slot_cap * FACE_RUNS;
}

fn write_zero(region: u32, k: u32) {
    commands[region * region_stride() + k] = DrawIndexedIndirectArgs(0u, 1u, 0u, 0i, 0u);
}

fn write_cmd(region: u32, k: u32, first_index: u32, count: u32, base_vertex: u32, slot: u32) {
    commands[region * region_stride() + k] = DrawIndexedIndirectArgs(
        count, 1u, first_index, i32(base_vertex), slot,
    );
}

fn face_count(m: SlotMeta, stream: u32, group: u32) -> u32 {
    let k = stream * FACE_GROUPS + group;
    let word = k / 2u;
    var faces = m.faces;
    return (faces[word / 4u][word % 4u] >> ((k % 2u) * 16u)) & 0xFFFFu;
}

fn face_mask(lo: vec3<f32>, hi: vec3<f32>) -> u32 {
    if (cull.flags & VIEW_FACES) == 0u {
        return ALL_GROUPS;
    }
    let eye = cull.eye.xyz;
    var mask = 1u << 6u;
    for (var a = 0u; a < 3u; a = a + 1u) {
        if (eye[a] < hi[a] + 1.0) {
            mask = mask | (1u << (a * 2u));
        }
        if (eye[a] > lo[a] - 1.0) {
            mask = mask | (1u << (a * 2u + 1u));
        }
    }
    return mask;
}

fn emit_run(region: u32, slot: u32, run: u32, first: u32, count: u32, base_vertex: u32) {
    var k = slot * FACE_RUNS + run;
    if (cull.compact != 0u) {
        k = atomicAdd(&counts[region], 1u);
    }
    write_cmd(region, k, first, count, base_vertex, slot);
}

fn emit_stream(region: u32, m: SlotMeta, stream: u32, first: u32, count: u32, mask: u32, slot: u32) {
    var total = 0u;
    for (var g = 0u; g < FACE_GROUPS; g = g + 1u) {
        total = total + face_count(m, stream, g) * 6u;
    }
    var runs = 0u;
    if (total != count || (total != 0u && total < FILL_QUADS * 6u)) {
        emit_run(region, slot, 0u, first, count, m.base_vertex);
        runs = 1u;
    } else {
        var order = RANK_GROUP;
        var at = first;
        var run_first = first;
        var run_count = 0u;
        var gap = 0u;
        for (var r = 0u; r < FACE_GROUPS; r = r + 1u) {
            let g = order[r];
            let n = face_count(m, stream, g) * 6u;
            if (n == 0u) {
                continue;
            }
            if (((mask >> g) & 1u) != 0u) {
                if (run_count == 0u) {
                    run_first = at;
                } else {
                    run_count = run_count + gap;
                }
                gap = 0u;
                run_count = run_count + n;
            } else if (run_count != 0u) {
                if (gap + n <= FILL_QUADS * 6u) {
                    gap = gap + n;
                } else {
                    emit_run(region, slot, runs, run_first, run_count, m.base_vertex);
                    runs = runs + 1u;
                    run_count = 0u;
                    gap = 0u;
                }
            }
            at = at + n;
        }
        if (run_count != 0u) {
            emit_run(region, slot, runs, run_first, run_count, m.base_vertex);
            runs = runs + 1u;
        }
    }
    if (cull.compact == 0u) {
        for (var r = runs; r < FACE_RUNS; r = r + 1u) {
            write_zero(region, slot * FACE_RUNS + r);
        }
    }
}

fn zero_stream(region: u32, slot: u32) {
    for (var r = 0u; r < FACE_RUNS; r = r + 1u) {
        write_zero(region, slot * FACE_RUNS + r);
    }
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let slot = id.x;
    if (slot >= cull.slot_cap) {
        return;
    }

    let compact = cull.compact != 0u;

    var live = false;
    var visible = false;
    var m: SlotMeta;
    let word = slot / 32u;
    if (cull.flags & VIEW_UNOCCLUDED) != 0u || word >= arrayLength(&vis) || ((vis[word] >> (slot % 32u)) & 1u) != 0u {
        visible = true;
    }
    if (slot < arrayLength(&metas)) {
        m = metas[slot];
        if ((m.flags & META_LIVE) != 0u) {
            live = true;
        }
    }

    var inside = false;
    var mask = ALL_GROUPS;
    if (live && visible) {
        let base = vec3<f32>(m.origin - cull.origin.xyz);
        let lo = base + m.min;
        let hi = base + m.max;
        inside = in_frustum(lo, hi, base);
        mask = face_mask(lo, hi);
    }

    let draws = live && visible && inside;
    let is_water = draws && (m.flags & META_WATER) != 0u;
    let has_solid = draws && !is_water && m.solid_count > 0u;
    let has_cutout = draws && !is_water && m.index_count > m.solid_count;

    if (!compact) {
        for (var pool = 0u; pool < cull.pool_count; pool = pool + 1u) {
            let base = ((cull.view_index * cull.pool_count) + pool) * STREAMS;
            if (has_solid && m.pool == pool) {
                emit_stream(base + STREAM_SOLID, m, STREAM_SOLID, m.first_index, m.solid_count, mask, slot);
            } else {
                zero_stream(base + STREAM_SOLID, slot);
            }
            if (has_cutout && m.pool == pool) {
                emit_stream(base + STREAM_CUTOUT, m, STREAM_CUTOUT, m.cutout_first,
                    m.index_count - m.solid_count, mask, slot);
            } else {
                zero_stream(base + STREAM_CUTOUT, slot);
            }
        }
        return;
    }

    if (!draws) {
        return;
    }
    let base = ((cull.view_index * cull.pool_count) + m.pool) * STREAMS;
    if (has_solid) {
        emit_stream(base + STREAM_SOLID, m, STREAM_SOLID, m.first_index, m.solid_count, mask, slot);
    }
    if (has_cutout) {
        emit_stream(base + STREAM_CUTOUT, m, STREAM_CUTOUT, m.cutout_first,
            m.index_count - m.solid_count, mask, slot);
    }
}
