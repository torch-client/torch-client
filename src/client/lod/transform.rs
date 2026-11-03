use super::blocks::{BlockTable, FULLY_OPAQUE, MAX_FLUID_AMOUNT, Material, StateInfo};
use super::point;
use super::pos::{WIDTH, column_index};
use super::reduce::{ReducingList, reduce_to_one};
use super::render_point as rp;
use super::render_source::RenderSource;
use super::section::FullSection;

const CAVE_CULLING_HEIGHT: i32 = 60;
const IGNORE_NON_COLLIDING_BLOCKS: bool = true;
const COLOR_BELOW_WITH_AVOIDED_BLOCKS: bool = true;
const MIN_MC_LIGHT: u8 = 0;
const NO_FLUID_AMOUNT: u8 = 0;

#[derive(Clone, Copy)]
struct Block {
    state: u16,
    info: StateInfo,
}

struct Context {
    cave_culling: bool,
    cave_culling_max_y: i32,
    water: Block,
}

struct ColumnView<'a> {
    data: &'a mut [u64],
    void: &'a mut u64,
}

pub fn to_render_source(
    full: &FullSection,
    blocks: &mut BlockTable,
    min_y: i32,
    max_vertical: usize,
    cave_culling: bool,
) -> RenderSource {
    let mut source = RenderSource::new(full.pos, max_vertical, min_y);
    if full.is_empty {
        return source;
    }
    source.mark_not_empty();

    let water_state = blocks.water_state();
    let ctx = Context {
        cave_culling,
        cave_culling_max_y: CAVE_CULLING_HEIGHT - min_y,
        water: Block {
            state: water_state,
            info: blocks.info(water_state),
        },
    };

    let mut expanding = Vec::new();
    let mut reducing = ReducingList::new();
    for x in 0..WIDTH {
        for z in 0..WIDTH {
            update_or_replace_column(
                blocks,
                &ctx,
                full.column(x, z),
                &mut source,
                x,
                z,
                &mut expanding,
                &mut reducing,
            );
        }
    }
    source
}

#[allow(clippy::too_many_arguments)]
fn update_or_replace_column(
    blocks: &mut BlockTable,
    ctx: &Context,
    full_column: &[u64],
    source: &mut RenderSource,
    x: i32,
    z: i32,
    expanding: &mut Vec<u64>,
    reducing: &mut ReducingList,
) {
    if full_column.is_empty() {
        return;
    }

    let max_vertical = source.max_vertical;
    let column = column_index(x, z);
    let offset = column * max_vertical;
    let full_len = full_column.len();
    if full_len <= max_vertical {
        let mut view = ColumnView {
            data: &mut source.data[offset..offset + max_vertical],
            void: &mut source.void_data[column],
        };
        set_render_column_view(blocks, ctx, &mut view, full_column);
    } else {
        expanding.clear();
        expanding.resize(full_len, rp::EMPTY);
        let mut expanding_void = rp::EMPTY;
        let mut view = ColumnView {
            data: expanding.as_mut_slice(),
            void: &mut expanding_void,
        };
        set_render_column_view(blocks, ctx, &mut view, full_column);
        change_vertical_size_from(
            &mut source.data[offset..offset + max_vertical],
            &mut source.void_data[column],
            expanding.as_slice(),
            expanding_void,
            reducing,
        );
    }

    apply_top_block_data(blocks, ctx, source, x, z, full_column);
}

fn change_vertical_size_from(
    out: &mut [u64],
    out_void: &mut u64,
    source: &[u64],
    source_void: u64,
    reducing: &mut ReducingList,
) {
    if out.len() >= source.len() {
        out[..source.len()].copy_from_slice(source);
        out[source.len()..].fill(rp::EMPTY);
    } else {
        merge_multi_data(source, reducing, out);
    }
    *out_void = source_void;
}

fn merge_multi_data(source: &[u64], reducing: &mut ReducingList, out: &mut [u64]) {
    match out.len() {
        0 => {}
        1 => out[0] = reduce_to_one(source),
        target => {
            reducing.populate(source);
            reducing.reduce(target as i32);
            reducing.copy_to(out);
        }
    }
}

fn apply_top_block_data(
    blocks: &mut BlockTable,
    ctx: &Context,
    source: &mut RenderSource,
    x: i32,
    z: i32,
    full_column: &[u64],
) {
    for render_index in 0..source.max_vertical {
        let render = source.point(x, z, render_index);
        if !rp::exists(render) || rp::has_zero_height(render) {
            break;
        }

        let top_block_y = rp::y_max(render) - 1;
        let material = rp::material(render);
        let mut fluid_amount = if rp::is_fluid(render) {
            MAX_FLUID_AMOUNT
        } else {
            NO_FLUID_AMOUNT
        };
        let mut waterlogged = false;
        for (full_index, &full) in full_column.iter().enumerate() {
            let bottom_y = point::bottom_y(full);
            if bottom_y > top_block_y {
                continue;
            }
            if top_block_y < bottom_y + point::height(full) {
                let mut block = state_info(blocks, full);
                if material == Material::Snow as u8
                    && block.material as u8 != material
                    && full_index > 0
                {
                    let above = state_info(blocks, full_column[full_index - 1]);
                    if !above.is_air {
                        block = above;
                    }
                }
                if is_replaced_with_water(blocks, full_column, full_index)
                    || is_submerged_replacement(blocks, full_column, full_index)
                {
                    block = ctx.water.info;
                }
                if block.fluid_amount != NO_FLUID_AMOUNT {
                    fluid_amount = block.fluid_amount;
                }
                waterlogged = block.waterlogged;
            }
            break;
        }
        source.set_attributes(x, z, render_index, fluid_amount, waterlogged);
    }
}

#[inline]
fn state_info(blocks: &mut BlockTable, full: u64) -> StateInfo {
    blocks.info(point::id_state(point::id(full)))
}

fn is_replaced_with_water(blocks: &mut BlockTable, full_column: &[u64], full_index: usize) -> bool {
    if !state_info(blocks, full_column[full_index]).water_subsurface_replacement {
        return false;
    }
    if full_index == 0 {
        return true;
    }
    let above = state_info(blocks, full_column[full_index - 1]);
    above.is_air || above.water_surface_replacement
}

fn is_submerged_replacement(
    blocks: &mut BlockTable,
    full_column: &[u64],
    full_index: usize,
) -> bool {
    if full_index == 0 || !state_info(blocks, full_column[full_index]).water_subsurface_replacement
    {
        return false;
    }
    let above = state_info(blocks, full_column[full_index - 1]);
    above.water_subsurface_replacement || (above.material == Material::Water && !above.waterlogged)
}

#[inline]
fn block_color(blocks: &mut BlockTable, block: Block, biome: u8) -> u32 {
    blocks.color(point::pack_id(block.state, biome))
}

#[inline]
fn alpha_of(argb: u32) -> u32 {
    argb >> 24
}

#[inline]
fn set_alpha(argb: u32, alpha: u32) -> u32 {
    (argb & 0x00FF_FFFF) | alpha << 24
}

fn set_render_column_view(
    blocks: &mut BlockTable,
    ctx: &Context,
    view: &mut ColumnView,
    full_column: &[u64],
) {
    let mut is_column_void = true;

    let mut last_color: u32 = 0;
    let mut last_bottom: i32 = -10_000;
    let mut last_block: Option<Block> = None;
    let mut air_after_last_block = false;

    let mut color_to_apply: Option<u32> = None;
    let mut block_to_apply: Option<Block> = None;
    let mut skylight_to_apply: Option<u8> = None;
    let mut blocklight_to_apply: Option<u8> = None;

    let mut render_index = 0usize;
    let last_full_index = full_column.len() - 1;

    for (full_index, &full) in full_column.iter().enumerate() {
        let bottom_y = point::bottom_y(full);
        let mut block_height = point::height(full);
        let top_y = bottom_y + block_height;
        let id = point::id(full);
        let biome = point::id_biome(id);
        let mut block_light = point::block_light(full);
        let mut sky_light = point::sky_light(full);
        let state = point::id_state(id);
        let mut block = Block {
            state,
            info: blocks.info(state),
        };

        if block.info.is_air {
            color_to_apply = None;
            block_to_apply = None;
            skylight_to_apply = None;
            blocklight_to_apply = None;
            air_after_last_block = true;
        }

        if block.info.material == Material::Water && !air_after_last_block {
            if let Some(last) = last_block.filter(|b| b.info.is_ice) {
                block = last;
            }
        }

        let mut replaced_block_color = 0u32;
        if is_replaced_with_water(blocks, full_column, full_index) {
            replaced_block_color = block_color(blocks, block, biome);
            block = ctx.water;
        } else if is_submerged_replacement(blocks, full_column, full_index) {
            block = ctx.water;
        }

        let ignore_block = block.info.ignored;
        let cave_block = block.info.cave_ignored;
        if cave_block
            || !block.info.is_solid
            || block.info.is_liquid
            || block.info.opacity < FULLY_OPAQUE
        {
            if ctx.cave_culling
                && sky_light == MIN_MC_LIGHT
                && top_y < ctx.cave_culling_max_y
                && render_index != 0
                && full_index != 0
                && full_index + 1 < full_column.len()
            {
                let next_sky_light = point::sky_light(full_column[full_index + 1]);
                if next_sky_light == MIN_MC_LIGHT && alpha_of(last_color) == 255 {
                    let above = view.data[render_index - 1];
                    view.data[render_index - 1] = rp::set_y_min(above, bottom_y);
                }
                continue;
            }

            if ignore_block {
                if full_index == last_full_index {
                    set_void_air(
                        view,
                        render_index,
                        bottom_y,
                        block_height,
                        sky_light,
                        block_light,
                    );
                }
                continue;
            }
        } else if ignore_block {
            skylight_to_apply = Some(sky_light);
            continue;
        }

        let mut ignore_non_solid_block = IGNORE_NON_COLLIDING_BLOCKS
            && !block.info.is_solid
            && !block.info.is_liquid
            && block.info.opacity != FULLY_OPAQUE;

        let is_snow_layer = block.info.is_snow_layer;
        let is_water_surface_replacement = block.info.water_surface_replacement;
        if is_snow_layer || is_water_surface_replacement {
            let mut surface_block_color = 0u32;
            if is_water_surface_replacement {
                surface_block_color = block_color(blocks, block, biome);
                block = ctx.water;
            }

            block_height -= 1;
            if block_height == 0 {
                ignore_non_solid_block = true;
                if is_snow_layer {
                    let snow_color = block_color(blocks, block, biome);
                    color_to_apply = Some(set_alpha(snow_color, 255));
                    block_to_apply = Some(block);
                } else {
                    let water_color = block_color(blocks, block, biome);
                    color_to_apply = Some(tint_water_color(water_color, surface_block_color));
                }
            }
        }

        if ignore_non_solid_block {
            let ignored_color = block_color(blocks, block, biome);
            let ignored_alpha = alpha_of(ignored_color);

            if COLOR_BELOW_WITH_AVOIDED_BLOCKS
                && !is_water_surface_replacement
                && ignored_alpha != 0
            {
                color_to_apply = Some(set_alpha(ignored_color, 255));
            }
            if ignored_alpha != 0 {
                skylight_to_apply = Some(sky_light);
                blocklight_to_apply = Some(block_light);
            }

            if full_index == last_full_index {
                set_void_air(
                    view,
                    render_index,
                    bottom_y,
                    block_height,
                    sky_light,
                    block_light,
                );
            }
            continue;
        }

        let color = match color_to_apply.take() {
            Some(carried) => carried,
            None => {
                let own = block_color(blocks, block, biome);
                if replaced_block_color != 0 {
                    tint_water_color(own, replaced_block_color)
                } else {
                    own
                }
            }
        };
        if let Some(s) = skylight_to_apply.take() {
            sky_light = s;
        }
        if let Some(b) = blocklight_to_apply.take() {
            block_light = b;
        }
        if let Some(b) = block_to_apply.take() {
            block = b;
        }

        if color == last_color && bottom_y + block_height == last_bottom && render_index > 0 {
            let above = view.data[render_index - 1];
            view.data[render_index - 1] = rp::set_y_min(above, bottom_y);
        } else {
            is_column_void = false;
            view.data[render_index] = rp::create(
                bottom_y + block_height,
                bottom_y,
                color,
                sky_light,
                block_light,
                block.info.material as u8,
            );
            render_index += 1;
        }
        last_bottom = bottom_y;
        last_color = color;
        last_block = Some(block);
        air_after_last_block = false;
    }

    if is_column_void {
        view.data[0] = rp::EMPTY;
    }
}

fn tint_water_color(water_color: u32, block_color: u32) -> u32 {
    if alpha_of(block_color) == 0 {
        return water_color;
    }
    let channel = |shift: u32| {
        ((((block_color >> shift) & 0xFF) + ((water_color >> shift) & 0xFF)) / 2) << shift
    };
    (water_color & 0xFF00_0000) | channel(16) | channel(8) | channel(0)
}

fn set_void_air(
    view: &mut ColumnView,
    render_index: usize,
    bottom_y: i32,
    block_height: i32,
    sky_light: u8,
    block_light: u8,
) {
    let last = if render_index > 0 {
        view.data[render_index - 1]
    } else {
        rp::EMPTY
    };
    let max_y = if last != rp::EMPTY {
        rp::y_min(last)
    } else {
        bottom_y + block_height
    };
    *view.void = rp::create(max_y, 0, 0, sky_light, block_light, Material::Air as u8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_tint_averages_rgb_and_keeps_water_alpha() {
        let water = 0x8010_2030;
        assert_eq!(tint_water_color(water, 0xFF30_4050), 0x8020_3040);
        assert_eq!(tint_water_color(water, 0x00FF_FFFF), water);
    }

    #[test]
    fn void_air_hangs_from_the_slab_above() {
        let mut data = [rp::create(80, 70, 0xFF00_0000, 15, 0, 2), rp::EMPTY];
        let mut void = rp::EMPTY;
        let mut view = ColumnView {
            data: &mut data,
            void: &mut void,
        };
        set_void_air(&mut view, 1, 10, 5, 12, 3);
        assert_eq!(rp::y_max(void), 70);
        assert_eq!(rp::y_min(void), 0);
        assert_eq!(rp::sky_light(void), 12);
        assert_eq!(rp::block_light(void), 3);
        assert_eq!(rp::material(void), Material::Air as u8);

        let mut view = ColumnView {
            data: &mut data,
            void: &mut void,
        };
        set_void_air(&mut view, 0, 10, 5, 12, 3);
        assert_eq!(rp::y_max(void), 15);
    }

    #[test]
    fn change_vertical_size_reduces_and_carries_the_void() {
        let source = [
            rp::create(64, 60, 0xFF00_0001, 15, 0, 2),
            rp::create(60, 30, 0xFF00_0002, 15, 0, 2),
            rp::create(30, 0, 0xFF00_0003, 15, 0, 2),
        ];
        let source_void = rp::create(5, 0, 0, 7, 0, Material::Air as u8);
        let mut out = [rp::EMPTY; 1];
        let mut out_void = rp::EMPTY;
        let mut reducing = ReducingList::new();
        change_vertical_size_from(&mut out, &mut out_void, &source, source_void, &mut reducing);
        assert_eq!((rp::y_max(out[0]), rp::y_min(out[0])), (64, 0));
        assert_eq!(out_void, source_void);

        let mut out = [rp::EMPTY; 2];
        change_vertical_size_from(&mut out, &mut out_void, &source, source_void, &mut reducing);
        assert_eq!(rp::y_max(out[0]), 64);
        assert_eq!(rp::y_min(out[1]), 0);
    }

    fn state_named(name: &str) -> u16 {
        use azalea::block::BlockState;
        (0..=u16::MAX)
            .find(|&id| {
                BlockState::try_from(id as u32).is_ok_and(|s| crate::blocks::block_name(s) == name)
            })
            .expect(name)
    }

    #[test]
    fn kelp_under_water_draws_as_the_water_slab() {
        let mut blocks = BlockTable::new();
        let water_state = blocks.water_state();
        let water = Block {
            state: water_state,
            info: blocks.info(water_state),
        };
        let ctx = Context {
            cave_culling: false,
            cave_culling_max_y: 0,
            water,
        };
        let run = |state: u16, height: i32, min_y: i32| {
            point::encode(point::pack_id(state, 0), height, min_y, 0, 15)
        };
        let full_column = [
            run(water_state, 13, 50),
            run(state_named("kelp"), 1, 49),
            run(state_named("kelp_plant"), 9, 40),
            run(state_named("sand"), 40, 0),
        ];
        let mut data = [rp::EMPTY; 4];
        let mut void = rp::EMPTY;
        let mut view = ColumnView {
            data: &mut data,
            void: &mut void,
        };
        set_render_column_view(&mut blocks, &ctx, &mut view, &full_column);
        assert_eq!((rp::y_max(data[0]), rp::y_min(data[0])), (63, 40));
        assert_eq!(rp::material(data[0]), Material::Water as u8);
        assert_eq!(rp::color(data[0]), block_color(&mut blocks, water, 0));
        assert_eq!((rp::y_max(data[1]), rp::y_min(data[1])), (40, 0));
    }
}
