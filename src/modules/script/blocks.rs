use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use azalea::block::fluid_state::{FluidKind, FluidState};
use azalea::block::{BlockState, BlockTrait};
use azalea::physics::collision::{
    BlockWithShape, is_solid, world_collisions::get_block_collisions,
};
use azalea_core::aabb::Aabb;
use azalea_core::position::Vec3;
use azalea_registry::builtin::BlockKind;
use azalea_registry::tags::blocks::REPLACEABLE;
use mlua::{Error, Lua, Table, Value};

use super::api::Ctx;
use super::game::{block_pos, game_fn};

const COLLIDES_SPAN: f64 = 16.0;

pub(super) fn tables(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<(Table, Table)> {
    let blocks = lua.create_table()?;
    blocks.raw_set(
        "info",
        game_fn(lua, ctx, "torch.blocks.info", |lua, _, spec: Value| {
            let state = match spec {
                Value::Integer(_) | Value::Number(_) => state_of(&spec),
                _ => super::ghost::palette_state(&spec)?,
            };
            match state {
                Some(state) => info(lua, state).map(Value::Table),
                None => Ok(Value::Nil),
            }
        })?,
    )?;
    blocks.raw_set(
        "tint",
        game_fn(lua, ctx, "torch.blocks.tint", |_, _, id: Value| {
            let Some(state) = state_of(&id) else {
                return Err(Error::runtime(
                    "torch.blocks.tint takes a state id, as torch.blocks.info's `stateid`",
                ));
            };
            Ok(colors(state).tint)
        })?,
    )?;
    blocks.set_readonly(true);

    let items = lua.create_table()?;
    items.raw_set(
        "block",
        game_fn(lua, ctx, "torch.items.block", |_, _, id: String| {
            let bare = id.strip_prefix("minecraft:").unwrap_or(&id);
            Ok(crate::items::placed::placed_block(bare).map(|b| b.to_str()))
        })?,
    )?;
    items.set_readonly(true);
    Ok((blocks, items))
}

pub(super) fn add_world(lua: &Lua, ctx: &Arc<Ctx>, world: &Table) -> mlua::Result<()> {
    world.raw_set(
        "blockinfo",
        game_fn(
            lua,
            ctx,
            "torch.world.blockinfo",
            |lua, ctx, (x, y, z): (f64, f64, f64)| {
                let what = "torch.world.blockinfo";
                let pos = block_pos(x, y, z, what)?;
                let bot = ctx.bot(what)?;
                let Ok(world) = bot.world() else {
                    return Ok(Value::Nil);
                };
                let state = world.read().get_block_state(pos);
                match state {
                    Some(state) => info(lua, state).map(Value::Table),
                    None => Ok(Value::Nil),
                }
            },
        )?,
    )?;
    world.raw_set(
        "collides",
        game_fn(
            lua,
            ctx,
            "torch.world.collides",
            |_, ctx, (x0, y0, z0, x1, y1, z1): (f64, f64, f64, f64, f64, f64)| {
                let what = "torch.world.collides";
                let (min, max) = (Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1));
                let ok = [x0, y0, z0, x1, y1, z1].iter().all(|v| v.is_finite())
                    && min.x <= max.x
                    && min.y <= max.y
                    && min.z <= max.z;
                if !ok {
                    return Err(Error::runtime(format!(
                        "{what} takes finite minx, miny, minz, maxx, maxy, maxz with each min at most its max"
                    )));
                }
                if max.x - min.x > COLLIDES_SPAN || max.y - min.y > COLLIDES_SPAN || max.z - min.z > COLLIDES_SPAN {
                    return Err(Error::runtime(format!("{what}: the box is at most {COLLIDES_SPAN} blocks a side")));
                }
                let bot = ctx.bot(what)?;
                let Ok(world) = bot.world() else {
                    return Ok(false);
                };
                let shapes = get_block_collisions(&world.read(), &Aabb { min, max });
                Ok(shapes.iter().any(|s| !s.is_empty()))
            },
        )?,
    )?;
    Ok(())
}

fn info(lua: &Lua, state: BlockState) -> mlua::Result<Table> {
    let kind = state.as_block_kind();
    let block = Box::<dyn BlockTrait>::from(state);
    let id = block.id();
    let t = lua.create_table()?;
    t.raw_set("id", format!("minecraft:{id}"))?;
    let mut props: Vec<(&str, &str)> = block.property_map().into_iter().collect();
    props.sort_unstable();
    let state_text = if props.is_empty() {
        format!("minecraft:{id}")
    } else {
        let inner: Vec<String> = props.iter().map(|(k, v)| format!("{k}={v}")).collect();
        format!("minecraft:{id}[{}]", inner.join(","))
    };
    t.raw_set("state", state_text)?;
    t.raw_set("stateid", state.id())?;
    t.raw_set(
        "air",
        matches!(
            kind,
            BlockKind::Air | BlockKind::CaveAir | BlockKind::VoidAir
        ),
    )?;
    t.raw_set("replaceable", replaceable(kind, &*block))?;
    t.raw_set("solid", is_solid(state))?;
    t.raw_set("fullcube", state.is_collision_shape_full())?;
    t.raw_set("collides", !state.is_collision_shape_empty())?;
    t.raw_set("falling", crate::blocks::ambient::is_falling(id))?;
    match FluidState::from(state).kind {
        FluidKind::Water => t.raw_set("fluid", "water")?,
        FluidKind::Lava => t.raw_set("fluid", "lava")?,
        FluidKind::Empty => {}
    }
    t.raw_set("hardness", block.behavior().destroy_time)?;
    let c = colors(state);
    t.raw_set("mapcolor", c.map)?;
    t.raw_set("texturecolor", c.texture)?;
    Ok(t)
}

fn state_of(v: &Value) -> Option<BlockState> {
    let n = match v {
        Value::Integer(n) => *n as f64,
        Value::Number(n) => *n,
        _ => return None,
    };
    if n.fract() != 0.0 || !(0.0..=f64::from(u16::MAX)).contains(&n) {
        return None;
    }
    BlockState::try_from(n as u32).ok()
}

#[derive(Clone, Copy)]
struct Colors {
    map: u32,
    texture: u32,
    tint: Option<&'static str>,
}

const FULL_BRIGHTNESS: u8 = 2;

static COLORS: Mutex<(u32, Option<HashMap<u16, Colors>>)> = Mutex::new((0, None));

fn colors(state: BlockState) -> Colors {
    let blocks = crate::blocks::registry();
    {
        let cache = COLORS.lock().unwrap_or_else(PoisonError::into_inner);
        if cache.0 == blocks.generation()
            && let Some(c) = cache.1.as_ref().and_then(|m| m.get(&state.id()))
        {
            return *c;
        }
    }
    let c = resolve(state, &blocks);
    let mut cache = COLORS.lock().unwrap_or_else(PoisonError::into_inner);
    if cache.0 != blocks.generation() {
        *cache = (blocks.generation(), None);
    }
    cache
        .1
        .get_or_insert_with(HashMap::new)
        .insert(state.id(), c);
    c
}

fn resolve(state: BlockState, blocks: &crate::blocks::Blocks) -> Colors {
    use crate::renderer::tile_colors::{ColorMode, tile_of_uv};
    use crate::renderer::{BlockGeom, TintKind};
    let block = Box::<dyn BlockTrait>::from(state);
    let props = block.property_map();
    let material = crate::util::block_map_color::material(block.id(), |k| props.get(k).copied());
    let [r, g, b, a] = crate::util::map_color::rgba_from_packed_id(material << 2 | FULL_BRIGHTNESS);
    let map = if a == 0 {
        0
    } else {
        u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
    };

    let rb = crate::util::block_model::block_visual_in(state, blocks);
    let sprites = blocks.sprites();
    let rows = sprites.rows().max(1);
    let tile = match &rb.geom {
        BlockGeom::Fluid { still, .. } => Some(*still),
        BlockGeom::Model(baked) => {
            let models: Vec<_> = baked
                .parts
                .iter()
                .filter_map(|slot| slot.choices.first())
                .collect();
            let up = models
                .iter()
                .flat_map(|m| m.quads.iter())
                .find(|q| usize::from(q.face) == crate::items::model::UP)
                .map(|q| tile_of_uv(&q.uv, rows));
            let particle = || {
                models.iter().find_map(|m| m.particle_uv).map(|uv| {
                    let centre = [[(uv[0] + uv[2]) / 2.0, (uv[1] + uv[3]) / 2.0]; 4];
                    tile_of_uv(&centre, rows)
                })
            };
            up.filter(|&t| t != 0).or_else(particle)
        }
    };
    let averaged = tile
        .filter(|&t| t != 0)
        .map(|t| sprites.tile_colors().get(t, ColorMode::Default));
    let texture = match averaged {
        Some(argb) if argb >> 24 != 0 => argb & 0xFF_FFFF,
        _ => map,
    };

    let water = matches!(block.id(), "water" | "bubble_column");
    let tint = match rb.tint_kind {
        TintKind::Grass => Some("grass"),
        TintKind::Foliage => Some("foliage"),
        TintKind::Water => Some("water"),
        _ if water => Some("water"),
        _ => None,
    };
    Colors { map, texture, tint }
}

pub(super) fn biomes_table(lua: &Lua, _ctx: &Arc<Ctx>) -> mlua::Result<Table> {
    use crate::util::biome_color::{TintSource, biome_index, biome_rgb};
    let t = lua.create_table()?;
    t.raw_set(
        "color",
        lua.create_function(
            |_, (biome, kind, _x, _z): (String, String, Option<f64>, Option<f64>)| {
                let source = match kind.as_str() {
                    "grass" => TintSource::Grass,
                    "foliage" => TintSource::Foliage,
                    "water" => TintSource::Water,
                    other => {
                        return Err(Error::runtime(format!(
                            "torch.biomes.color: kind `{other}` is one of grass, foliage, water"
                        )));
                    }
                };
                let bare = biome.strip_prefix("minecraft:").unwrap_or(&biome);
                Ok(biome_index(bare).and_then(|i| biome_rgb(i, source)))
            },
        )?,
    )?;
    t.set_readonly(true);
    Ok(t)
}

fn replaceable(kind: BlockKind, block: &dyn BlockTrait) -> bool {
    if kind == BlockKind::Snow {
        return block.get_property("layers") == Some("1");
    }
    REPLACEABLE.contains(&kind)
}
