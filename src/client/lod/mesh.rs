use super::blocks::Material;
use super::pos::WIDTH;
use super::render_point as rp;
use super::render_source::RenderSource;

const DOWN: usize = 0;
const UP: usize = 1;
const NORTH: usize = 2;
const SOUTH: usize = 3;
const WEST: usize = 4;
const EAST: usize = 5;

const COMPASS_FACE: [usize; 4] = [NORTH, SOUTH, WEST, EAST];
const COMPASS_NORMAL: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

const MAX_LIGHT: i8 = 15;
const SKYLIGHT_COVERED: i8 = -1;
const CAVE_CULLING_HEIGHT: i32 = 60;
const MAX_QUAD_WIDTH: i32 = 2048;
const DIRT_COLOR: u32 = 0xFF97_6D4D;

const DIRECTION_VERTEX_IBO_QUAD: [[[u8; 2]; 4]; 6] = [
    [[1, 0], [1, 1], [0, 1], [0, 0]],
    [[0, 0], [0, 1], [1, 1], [1, 0]],
    [[0, 0], [0, 1], [1, 1], [1, 0]],
    [[1, 0], [1, 1], [0, 1], [0, 0]],
    [[0, 0], [1, 0], [1, 1], [0, 1]],
    [[0, 1], [1, 1], [1, 0], [0, 0]],
];

#[derive(Default)]
pub struct MeshBuffers {
    pub positions: Vec<[f32; 3]>,
    pub colors: Vec<[u8; 4]>,
    pub packed: Vec<u32>,
    pub indices: Vec<u32>,
}

pub struct LodMesh {
    pub opaque: MeshBuffers,
    pub transparent: MeshBuffers,
}

impl LodMesh {
    pub fn is_empty(&self) -> bool {
        self.opaque.indices.is_empty() && self.transparent.indices.is_empty()
    }
}

pub fn build(
    center: &RenderSource,
    adjacent: [Option<&RenderSource>; 4],
    shade: &[f32; 6],
) -> LodMesh {
    let detail = center.detail();
    let mut builder = QuadBuilder::new(*shade, center.y_offset);
    let mut ctx = BoxContext {
        block_width: 1i32 << detail,
        cave_culling_max_y: CAVE_CULLING_HEIGHT - center.y_offset,
        segments: Vec::new(),
        new_segments: Vec::new(),
    };

    for rel_x in 0..WIDTH {
        for rel_z in 0..WIDTH {
            let column = center.column(rel_x, rel_z);
            if column.is_empty() || !rp::exists(column[0]) || rp::has_zero_height(column[0]) {
                continue;
            }
            let void = center.void_point(rel_x, rel_z);

            let mut adj = [AdjColumn::EMPTY; 4];
            for (dir, slot) in adj.iter_mut().enumerate() {
                let (nx, nz) = COMPASS_NORMAL[dir];
                let (x_adj, z_adj) = (rel_x + nx, rel_z + nz);
                let crosses = !(0..WIDTH).contains(&x_adj) || !(0..WIDTH).contains(&z_adj);
                *slot = if !crosses {
                    AdjColumn::of(center, x_adj, z_adj)
                } else {
                    match adjacent[dir] {
                        Some(src) if src.detail() == detail => {
                            AdjColumn::of(src, x_adj.rem_euclid(WIDTH), z_adj.rem_euclid(WIDTH))
                        }
                        _ => AdjColumn::EMPTY,
                    }
                };
            }

            for i in 0..column.len() {
                let data = column[i];
                if rp::has_zero_height(data) || !rp::exists(data) {
                    break;
                }
                let top = if i > 0 { column[i - 1] } else { rp::EMPTY };
                let mut bottom = if i + 1 < column.len() {
                    column[i + 1]
                } else {
                    void
                };
                if bottom == rp::EMPTY {
                    bottom = void;
                }
                add_render_point(
                    &mut builder,
                    &mut ctx,
                    data,
                    top,
                    bottom,
                    &adj,
                    rel_x,
                    rel_z,
                );
            }
        }
    }

    builder.merge_quads();
    builder.finish()
}

#[derive(Clone, Copy)]
struct AdjColumn<'a> {
    data: &'a [u64],
    void: u64,
}

impl<'a> AdjColumn<'a> {
    const EMPTY: AdjColumn<'static> = AdjColumn {
        data: &[],
        void: rp::EMPTY,
    };

    fn of(src: &'a RenderSource, x: i32, z: i32) -> Self {
        Self {
            data: src.column(x, z),
            void: src.void_point(x, z),
        }
    }
}

struct BoxContext {
    block_width: i32,
    cave_culling_max_y: i32,
    segments: Vec<Segment>,
    new_segments: Vec<Segment>,
}

#[derive(Clone, Copy)]
struct Segment {
    start: i32,
    end: i32,
    sky: i8,
    covered: bool,
}

#[allow(clippy::too_many_arguments)]
fn add_render_point(
    builder: &mut QuadBuilder,
    ctx: &mut BoxContext,
    data: u64,
    top: u64,
    bottom: u64,
    adj: &[AdjColumn; 4],
    rel_x: i32,
    rel_z: i32,
) {
    let min_y = rp::y_min(data);
    let height = rp::y_max(data) - min_y;
    if height == 0 {
        return;
    }
    debug_assert!(height > 0, "negative y size for a render data point");
    if height < 0 {
        return;
    }
    let width = ctx.block_width;
    add_box_quads(
        builder,
        ctx,
        width,
        height,
        rel_x * width,
        min_y,
        rel_z * width,
        rp::color(data),
        rp::material(data),
        rp::sky_light(data),
        rp::block_light(data),
        top,
        bottom,
        adj,
    );
}

#[allow(clippy::too_many_arguments)]
fn add_box_quads(
    builder: &mut QuadBuilder,
    ctx: &mut BoxContext,
    width: i32,
    height: i32,
    min_x: i32,
    min_y: i32,
    min_z: i32,
    mut color: u32,
    material: u8,
    sky_light: u8,
    block_light: u8,
    top: u64,
    bottom: u64,
    adj: &[AdjColumn; 4],
) {
    let raw_color = color;
    let max_x = min_x + width;
    let max_y = min_y + height;
    let max_z = min_z + width;
    let sky_bottom = if rp::exists(bottom) {
        rp::sky_light(bottom)
    } else {
        0
    };

    let is_transparent = argb_alpha(color) < 255;
    let is_top_transparent = rp::alpha(top) < 255;
    let is_bottom_transparent = rp::alpha(bottom) < 255;

    if !rp::exists(bottom) {
        color = set_argb_alpha(color, 255);
    }

    if material == Material::Air as u8 {
        return;
    }

    let skip_top = rp::exists(top)
        && rp::y_min(top) == max_y
        && (!is_top_transparent
            || (is_transparent && is_same_transparent_block(top, raw_color, material)));
    if !skip_top {
        builder.add_quad_up(
            min_x,
            max_y,
            min_z,
            width,
            apply_shade(color, builder.shade[UP]),
            material,
            sky_light,
            block_light,
        );
    }

    let skip_bottom = rp::exists(bottom)
        && rp::y_max(bottom) == min_y
        && (!is_bottom_transparent
            || (is_transparent && is_same_transparent_block(bottom, raw_color, material)))
        && !(rp::is_fluid(bottom) && rp::material(bottom) != material);
    if !skip_bottom {
        builder.add_quad_down(
            min_x,
            min_y,
            min_z,
            width,
            apply_shade(color, builder.shade[DOWN]),
            material,
            sky_bottom,
            block_light,
        );
    }

    let origins = [
        (min_x, min_z),
        (min_x, max_z),
        (min_x, min_z),
        (max_x, min_z),
    ];
    for (compass, &(x, z)) in origins.iter().enumerate() {
        make_adj_vertical_quad(
            builder,
            ctx,
            adj[compass],
            COMPASS_FACE[compass],
            x,
            min_y,
            z,
            width,
            height,
            color,
            raw_color,
            material,
            block_light,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn make_adj_vertical_quad(
    builder: &mut QuadBuilder,
    ctx: &mut BoxContext,
    adj: AdjColumn,
    dir: usize,
    x: i32,
    y_min: i32,
    z: i32,
    width: i32,
    y_size: i32,
    color: u32,
    raw_color: u32,
    material: u8,
    block_light: u8,
) {
    let color = apply_shade(color, builder.shade[dir]);

    if adj.data.is_empty() || rp::has_zero_height(adj.data[0]) {
        builder.add_quad_adj(
            dir,
            x,
            y_min,
            z,
            width,
            y_size,
            color,
            material,
            MAX_LIGHT as u8,
            block_light,
        );
        return;
    }

    let input_transparent = argb_alpha(color) < 255;
    let y_max = y_min + y_size;
    let count = adj.data.len();

    ctx.segments.clear();
    ctx.segments.push(Segment {
        start: y_min,
        end: y_max,
        sky: MAX_LIGHT,
        covered: false,
    });

    for adj_index in -1..count as isize {
        let adj_point = if adj_index == -1 {
            adj.void
        } else {
            adj.data[adj_index as usize]
        };
        let adj_min_y = rp::y_min(adj_point);
        let mut adj_max_y = rp::y_max(adj_point);

        if !rp::exists(adj_point) || rp::has_zero_height(adj_point) || y_max <= adj_min_y {
            continue;
        }

        let adj_above = if adj_index > 0 {
            adj.data[adj_index as usize - 1]
        } else {
            rp::EMPTY
        };
        let mut adj_below = if adj_index != -1 && (adj_index as usize + 1) < count {
            adj.data[adj_index as usize + 1]
        } else {
            adj.void
        };
        if adj_below == rp::EMPTY {
            adj_below = adj.void;
        }

        let adj_over_void = !rp::exists(adj_below);
        let adj_transparent = !adj_over_void && rp::alpha(adj_point) < 255;
        let adj_sky = rp::sky_light(adj_point) as i8;
        let mut covers_range = true;

        let is_same_fluid = rp::is_fluid(adj_point) && rp::material(adj_point) == material;

        if !adj_transparent && !is_same_fluid && rp::is_surface_lowered(adj_point, adj_above) {
            adj_max_y -= 1;
        }

        let light = if !adj_transparent {
            let section_width = width * WIDTH;
            let on_border = match dir {
                WEST => x == 0,
                NORTH => z == 0,
                EAST => x == section_width,
                SOUTH => z == section_width,
                _ => false,
            };
            let is_lit = rp::sky_light(adj_point) != 0 || rp::block_light(adj_point) != 0;
            let use_adj_lighting = on_border
                && is_lit
                && rp::y_max(adj_point) >= ctx.cave_culling_max_y
                && !is_same_fluid;
            if use_adj_lighting {
                adj_sky
            } else {
                SKYLIGHT_COVERED
            }
        } else if input_transparent && is_same_transparent_block(adj_point, raw_color, material) {
            SKYLIGHT_COVERED
        } else {
            covers_range = false;
            rp::sky_light(adj_below) as i8
        };

        apply_light_to_range(ctx, adj_min_y, adj_max_y, light, covers_range);

        let adj_above_min_y = rp::y_min(adj_above);
        if adj_max_y < adj_above_min_y {
            apply_light_to_range(ctx, adj_max_y, adj_above_min_y, adj_sky, false);
        }
    }

    for seg in &ctx.segments {
        try_add_vertical_face(
            builder,
            dir,
            x,
            z,
            width,
            color,
            material,
            block_light,
            *seg,
            input_transparent,
        );
    }
}

fn apply_light_to_range(ctx: &mut BoxContext, start: i32, end: i32, light: i8, covers_range: bool) {
    ctx.new_segments.clear();
    for &seg in &ctx.segments {
        if seg.end <= start || seg.start >= end {
            ctx.new_segments.push(seg);
            continue;
        }
        if seg.start < start {
            ctx.new_segments.push(Segment {
                start: seg.start,
                end: start,
                ..seg
            });
        }
        ctx.new_segments.push(Segment {
            start: seg.start.max(start),
            end: seg.end.min(end),
            sky: light.min(seg.sky),
            covered: seg.covered || covers_range,
        });
        if seg.end > end {
            ctx.new_segments.push(Segment {
                start: end,
                end: seg.end,
                ..seg
            });
        }
    }
    std::mem::swap(&mut ctx.segments, &mut ctx.new_segments);
}

#[allow(clippy::too_many_arguments)]
fn try_add_vertical_face(
    builder: &mut QuadBuilder,
    dir: usize,
    x: i32,
    z: i32,
    width: i32,
    color: u32,
    material: u8,
    block_light: u8,
    seg: Segment,
    input_transparent: bool,
) {
    if seg.sky < 0 {
        return;
    }
    if input_transparent && seg.covered {
        return;
    }
    let height = seg.end - seg.start;
    if height <= 0 {
        return;
    }
    builder.add_quad_adj(
        dir,
        x,
        seg.start,
        z,
        width,
        height,
        color,
        material,
        seg.sky as u8,
        block_light,
    );
}

fn is_same_transparent_block(adj: u64, raw_color: u32, material: u8) -> bool {
    rp::material(adj) == material
        && (material == Material::Water as u8 || rp::color(adj) == raw_color)
}

#[inline]
fn argb_alpha(c: u32) -> u32 {
    c >> 24
}

#[inline]
fn set_argb_alpha(c: u32, a: u32) -> u32 {
    (c & 0x00FF_FFFF) | (a & 0xFF) << 24
}

fn apply_shade(c: u32, shade: f32) -> u32 {
    let ch = |shift: u32| {
        let v = ((c >> shift) & 0xFF) as f32 * shade;
        (v.clamp(0.0, 255.0) as u32) << shift
    };
    (c & 0xFF00_0000) | ch(16) | ch(8) | ch(0)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
    Z,
}

#[inline]
fn axis(dir: usize) -> Axis {
    match dir {
        DOWN | UP => Axis::Y,
        NORTH | SOUTH => Axis::Z,
        _ => Axis::X,
    }
}

#[derive(Clone, Copy)]
struct Quad {
    x: i32,
    y: i32,
    z: i32,
    width_east_west: i32,
    width_north_south: i32,
    color: u32,
    material: u8,
    sky_light: u8,
    block_light: u8,
    dir: usize,
    sort_key_east_west: i64,
    sort_key_north_south: i64,
}

impl Quad {
    #[allow(clippy::too_many_arguments)]
    fn new(
        x: i32,
        y: i32,
        z: i32,
        width_east_west: i32,
        width_north_south: i32,
        color: u32,
        material: u8,
        sky_light: u8,
        block_light: u8,
        dir: usize,
    ) -> Self {
        debug_assert!(
            width_east_west > 0 && width_north_south > 0,
            "empty or negative quad"
        );
        Self {
            x,
            y,
            z,
            width_east_west,
            width_north_south,
            color,
            material,
            sky_light,
            block_light,
            dir,
            sort_key_east_west: sort_key(dir, x, y, z, true),
            sort_key_north_south: sort_key(dir, x, y, z, false),
        }
    }

    fn try_merge(&mut self, other: &Quad, east_west: bool) -> bool {
        if self.dir != other.dir {
            return false;
        }
        if (east_west && self.y != other.y) || (!east_west && self.x != other.x) {
            return false;
        }

        let (this_perp, this_par, other_perp, other_par) = match (axis(self.dir), east_west) {
            (Axis::X, true) => (self.z, self.x, other.z, other.x),
            (Axis::X, false) => (self.y, self.z, other.y, other.z),
            (Axis::Y, true) => (self.x, self.z, other.x, other.z),
            (Axis::Y, false) => (self.z, self.y, other.z, other.y),
            (Axis::Z, true) => (self.x, self.z, other.x, other.z),
            (Axis::Z, false) => (self.y, self.z, other.y, other.z),
        };
        let (this_perp_w, this_par_w, other_perp_w, other_par_w) = if east_west {
            (
                self.width_east_west,
                self.width_north_south,
                other.width_east_west,
                other.width_north_south,
            )
        } else {
            (
                self.width_north_south,
                self.width_east_west,
                other.width_north_south,
                other.width_east_west,
            )
        };

        if this_perp_w >= MAX_QUAD_WIDTH {
            return false;
        }
        if other_perp.div_euclid(MAX_QUAD_WIDTH) != this_perp.div_euclid(MAX_QUAD_WIDTH) {
            return false;
        }

        if this_perp + this_perp_w < other_perp || this_par != other_par {
            return false;
        } else if this_perp + this_perp_w > other_perp {
            return false;
        }

        if this_par_w != other_par_w {
            return false;
        }
        if self.color != other.color
            || self.material != other.material
            || self.sky_light != other.sky_light
            || self.block_light != other.block_light
        {
            return false;
        }

        if east_west {
            self.width_east_west += other.width_east_west;
        } else {
            self.width_north_south += other.width_north_south;
        }
        true
    }
}

fn sort_key(dir: usize, x: i32, y: i32, z: i32, east_west: bool) -> i64 {
    let (a, b, c) = match (axis(dir), east_west) {
        (Axis::X, true) => (x, y, z),
        (Axis::Y, true) => (y, z, x),
        (Axis::Z, true) => (z, y, x),
        (Axis::X, false) => (x, z, y),
        (Axis::Y, false) => (y, x, z),
        (Axis::Z, false) => (z, x, y),
    };
    (a as i64) << 48 | (b as i64) << 32 | (c as i64) << 16
}

struct QuadBuilder {
    opaque: [Vec<Quad>; 6],
    transparent: [Vec<Quad>; 6],
    shade: [f32; 6],
    y_offset: i32,
}

impl QuadBuilder {
    fn new(shade: [f32; 6], y_offset: i32) -> Self {
        Self {
            opaque: Default::default(),
            transparent: Default::default(),
            shade,
            y_offset,
        }
    }

    fn list(&mut self, dir: usize, color: u32) -> &mut Vec<Quad> {
        if argb_alpha(color) < 255 {
            &mut self.transparent[dir]
        } else {
            &mut self.opaque[dir]
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_quad_adj(
        &mut self,
        dir: usize,
        x: i32,
        y: i32,
        z: i32,
        width: i32,
        height: i32,
        color: u32,
        material: u8,
        sky_light: u8,
        block_light: u8,
    ) {
        debug_assert!(dir != DOWN && dir != UP, "add_quad_adj is for side faces");
        let quad = Quad::new(
            x,
            y,
            z,
            width,
            height,
            color,
            material,
            sky_light,
            block_light,
            dir,
        );
        let list = self.list(dir, color);
        if let Some(last) = list.last_mut()
            && (last.try_merge(&quad, true) || last.try_merge(&quad, false))
        {
            return;
        }
        list.push(quad);
    }

    #[allow(clippy::too_many_arguments)]
    fn add_quad_up(
        &mut self,
        min_x: i32,
        max_y: i32,
        min_z: i32,
        width: i32,
        color: u32,
        material: u8,
        sky_light: u8,
        block_light: u8,
    ) {
        let quad = Quad::new(
            min_x,
            max_y,
            min_z,
            width,
            width,
            color,
            material,
            sky_light,
            block_light,
            UP,
        );
        self.list(UP, color).push(quad);
    }

    #[allow(clippy::too_many_arguments)]
    fn add_quad_down(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        width: i32,
        color: u32,
        material: u8,
        sky_light: u8,
        block_light: u8,
    ) {
        let quad = Quad::new(
            x,
            y,
            z,
            width,
            width,
            color,
            material,
            sky_light,
            block_light,
            DOWN,
        );
        self.list(DOWN, color).push(quad);
    }

    fn merge_quads(&mut self) {
        let total: usize = self
            .opaque
            .iter()
            .chain(self.transparent.iter())
            .map(Vec::len)
            .sum();
        if total <= 1 {
            return;
        }
        for dir in 0..6 {
            merge_quads_internal(&mut self.opaque[dir], true);
            merge_quads_internal(&mut self.transparent[dir], true);
            if dir == UP || dir == DOWN {
                merge_quads_internal(&mut self.opaque[dir], false);
                merge_quads_internal(&mut self.transparent[dir], false);
            }
        }
    }

    fn finish(self) -> LodMesh {
        let mut mesh = LodMesh {
            opaque: MeshBuffers::default(),
            transparent: MeshBuffers::default(),
        };
        self.make_vertex_buffers(&self.opaque, &mut mesh.opaque);
        self.make_vertex_buffers(&self.transparent, &mut mesh.transparent);
        mesh
    }

    fn make_vertex_buffers(&self, lists: &[Vec<Quad>; 6], out: &mut MeshBuffers) {
        let quads: usize = lists.iter().map(Vec::len).sum();
        out.positions.reserve(quads * 4);
        out.colors.reserve(quads * 4);
        out.packed.reserve(quads * 4);
        out.indices.reserve(quads * 6);
        for list in lists {
            for quad in list {
                self.put_quad(quad, out);
            }
        }
    }

    fn put_quad(&self, quad: &Quad, out: &mut MeshBuffers) {
        let base = out.positions.len() as u32;
        let pick = |on: bool, width: i32| if on { width } else { 0 };
        let sign = |on: bool| if on { 1 } else { -1 };
        for corner in &DIRECTION_VERTEX_IBO_QUAD[quad.dir] {
            let (u, v) = (corner[0] == 1, corner[1] == 1);
            let (dx, dy, dz, mx, my, mz) = match axis(quad.dir) {
                Axis::X => (
                    0,
                    pick(v, quad.width_north_south),
                    pick(u, quad.width_east_west),
                    0,
                    sign(v),
                    sign(u),
                ),
                Axis::Y => (
                    pick(u, quad.width_east_west),
                    0,
                    pick(v, quad.width_north_south),
                    sign(u),
                    0,
                    sign(v),
                ),
                Axis::Z => (
                    pick(u, quad.width_east_west),
                    pick(v, quad.width_north_south),
                    0,
                    sign(u),
                    sign(v),
                    0,
                ),
            };

            let mut color = quad.color;
            if quad.material == Material::Grass as u8 && quad.dir != UP && (!v || quad.dir == DOWN)
            {
                color = apply_shade(DIRT_COLOR, self.shade[quad.dir]);
            }

            out.positions.push([
                (quad.x + dx) as f32,
                (quad.y + dy + self.y_offset) as f32,
                (quad.z + dz) as f32,
            ]);
            out.colors.push([
                (color >> 16) as u8,
                (color >> 8) as u8,
                color as u8,
                (color >> 24) as u8,
            ]);
            let micro = micro_offset(mx) | micro_offset(my) << 2 | micro_offset(mz) << 4;
            out.packed.push(
                (quad.sky_light as u32 & 0xF)
                    | (quad.block_light as u32 & 0xF) << 4
                    | (quad.dir as u32) << 8
                    | micro << 11,
            );
        }
        out.indices
            .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
    }
}

#[inline]
fn micro_offset(m: i32) -> u32 {
    match m {
        0 => 0,
        m if m > 0 => 0b01,
        _ => 0b11,
    }
}

fn merge_quads_internal(list: &mut Vec<Quad>, east_west: bool) {
    if list.len() <= 1 {
        return;
    }
    if east_west {
        list.sort_by_key(|q| q.sort_key_east_west);
    } else {
        list.sort_by_key(|q| q.sort_key_north_south);
    }
    let mut kept: Vec<Quad> = Vec::with_capacity(list.len());
    for quad in list.drain(..) {
        if let Some(current) = kept.last_mut()
            && current.try_merge(&quad, east_west)
        {
            continue;
        }
        kept.push(quad);
    }
    *list = kept;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::lod::pos::SectionPos;

    const FLAT: [f32; 6] = [1.0; 6];
    const NORMALS: [[f32; 3]; 6] = [
        [0.0, -1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, 1.0],
        [-1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
    ];

    fn source() -> RenderSource {
        let mut s = RenderSource::new(SectionPos::new(0, 0, 0), 4, -64);
        s.mark_not_empty();
        s
    }

    fn slab(max_y: i32, min_y: i32, argb: u32, material: Material) -> u64 {
        rp::create(max_y, min_y, argb, 15, 0, material as u8)
    }

    fn normal_of(b: &MeshBuffers, quad: usize) -> usize {
        ((b.packed[quad * 4] >> 8) & 0x7) as usize
    }

    fn quads_facing(b: &MeshBuffers, dir: usize) -> Vec<usize> {
        (0..b.positions.len() / 4)
            .filter(|&q| normal_of(b, q) == dir)
            .collect()
    }

    fn assert_windings(b: &MeshBuffers) {
        assert_eq!(b.indices.len(), b.positions.len() / 4 * 6);
        for q in 0..b.positions.len() / 4 {
            let p = |k: usize| b.positions[b.indices[q * 6 + k] as usize];
            let (a, c, d) = (p(0), p(1), p(2));
            let e1 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let e2 = [d[0] - a[0], d[1] - a[1], d[2] - a[2]];
            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let want = NORMALS[normal_of(b, q)];
            let dot = n[0] * want[0] + n[1] * want[1] + n[2] * want[2];
            assert!(
                dot > 0.0,
                "quad {q} winds against normal {}",
                normal_of(b, q)
            );
        }
    }

    #[test]
    fn single_column_has_six_faces_wound_outwards() {
        let mut s = source();
        s.column_mut(10, 10)[0] = slab(70, 64, 0xFF80_8080, Material::Stone);
        let mesh = build(&s, [None; 4], &FLAT);
        assert!(mesh.transparent.indices.is_empty());
        assert_eq!(mesh.opaque.positions.len(), 24);
        for dir in 0..6 {
            assert_eq!(quads_facing(&mesh.opaque, dir).len(), 1, "direction {dir}");
        }
        assert_windings(&mesh.opaque);
        assert!(
            mesh.opaque
                .positions
                .iter()
                .all(|p| p[1] == 0.0 || p[1] == 6.0)
        );
        assert!(
            mesh.opaque
                .positions
                .iter()
                .all(|p| (10.0..=11.0).contains(&p[0]))
        );
    }

    #[test]
    fn two_equal_columns_merge_and_hide_the_shared_face() {
        let mut s = source();
        s.column_mut(10, 10)[0] = slab(70, 64, 0xFF80_8080, Material::Stone);
        s.column_mut(11, 10)[0] = slab(70, 64, 0xFF80_8080, Material::Stone);
        let mesh = build(&s, [None; 4], &FLAT);
        let b = &mesh.opaque;
        assert_eq!(b.positions.len() / 4, 6);
        let up = quads_facing(b, UP);
        assert_eq!(up.len(), 1);
        let xs: Vec<f32> = (0..4).map(|k| b.positions[up[0] * 4 + k][0]).collect();
        assert_eq!(xs.iter().cloned().fold(f32::MAX, f32::min), 10.0);
        assert_eq!(xs.iter().cloned().fold(f32::MIN, f32::max), 12.0);
        for q in quads_facing(b, EAST) {
            assert!((0..4).all(|k| b.positions[q * 4 + k][0] == 12.0));
        }
        assert_windings(b);
    }

    #[test]
    fn water_next_to_water_has_no_wall() {
        let mut s = source();
        for x in [10, 11] {
            let col = s.column_mut(x, 10);
            col[0] = slab(63, 62, 0x8020_40C0, Material::Water);
            col[1] = slab(62, 0, 0xFF80_8080, Material::Stone);
        }
        let mesh = build(&s, [None; 4], &FLAT);
        let t = &mesh.transparent;
        assert_eq!(t.positions.len() / 4, 5);
        assert!(quads_facing(t, DOWN).is_empty());
        assert_eq!(quads_facing(t, UP).len(), 1);
        assert!(t.colors.iter().all(|c| c[3] < 255));
        assert_windings(t);
    }

    #[test]
    fn grass_sides_fade_to_dirt() {
        let mut s = source();
        s.column_mut(5, 5)[0] = slab(70, 64, 0xFF30_A030, Material::Grass);
        let mesh = build(&s, [None; 4], &FLAT);
        let b = &mesh.opaque;
        let dirt = [0x97, 0x6D, 0x4D, 0xFF];
        let grass = [0x30, 0xA0, 0x30, 0xFF];
        let north = quads_facing(b, NORTH);
        assert_eq!(north.len(), 1);
        for k in 0..4 {
            let v = north[0] * 4 + k;
            let want = if b.positions[v][1] == 0.0 {
                dirt
            } else {
                grass
            };
            assert_eq!(b.colors[v], want);
        }
        for &q in &quads_facing(b, DOWN) {
            assert!((0..4).all(|k| b.colors[q * 4 + k] == dirt));
        }
        for &q in &quads_facing(b, UP) {
            assert!((0..4).all(|k| b.colors[q * 4 + k] == grass));
        }
    }

    #[test]
    fn border_face_culled_by_same_detail_neighbour() {
        let mut s = source();
        s.column_mut(63, 10)[0] = slab(70, 64, 0xFF80_8080, Material::Stone);
        let mut east = RenderSource::new(SectionPos::new(0, 1, 0), 4, -64);
        east.mark_not_empty();
        east.column_mut(0, 10)[0] = rp::create(70, 64, 0xFF80_8080, 0, 0, Material::Stone as u8);
        let with = build(&s, [None, None, None, Some(&east)], &FLAT);
        assert!(quads_facing(&with.opaque, EAST).is_empty());
        let without = build(&s, [None; 4], &FLAT);
        assert_eq!(quads_facing(&without.opaque, EAST).len(), 1);
    }
}
