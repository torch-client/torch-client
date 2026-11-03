use std::sync::Arc;

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
                let Some(world) = ctx.world(what)? else {
                    return Ok(Value::Nil);
                };
                let state = world.read().get_block_state(pos);
                let Some(state) = state else {
                    return Ok(Value::Nil);
                };
                let t = info(lua, state)?;
                if let Some((block, sky)) = light_at(pos.x, pos.y, pos.z) {
                    t.raw_set("blocklight", block)?;
                    t.raw_set("skylight", sky)?;
                }
                Ok(Value::Table(t))
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
                let Some(world) = ctx.world(what)? else {
                    return Ok(false);
                };
                let shapes = get_block_collisions(&world.read(), &Aabb { min, max });
                Ok(shapes.iter().any(|s| !s.is_empty()))
            },
        )?,
    )?;
    Ok(())
}

fn light_at(x: i32, y: i32, z: i32) -> Option<(u8, u8)> {
    use crate::lighting::SectionPos;
    let map = crate::client::worldsync::light_map().read();
    map.is_lit(SectionPos::of_block(x, y, z).column())
        .then(|| map.at(x, y, z))
}

fn info(lua: &Lua, state: BlockState) -> mlua::Result<Table> {
    let kind = state.as_block_kind();
    let block = Box::<dyn BlockTrait>::from(state);
    let id = block.id();
    let t = lua.create_table()?;
    t.raw_set("id", kind.to_str())?;
    let mut props: Vec<(&str, &str)> = block.property_map().into_iter().collect();
    props.sort_unstable();
    if props.is_empty() {
        t.raw_set("state", kind.to_str())?;
    } else {
        let inner: Vec<String> = props.iter().map(|(k, v)| format!("{k}={v}")).collect();
        t.raw_set("state", format!("minecraft:{id}[{}]", inner.join(",")))?;
    }
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

fn replaceable(kind: BlockKind, block: &dyn BlockTrait) -> bool {
    if kind == BlockKind::Snow {
        return block.get_property("layers") == Some("1");
    }
    REPLACEABLE.contains(&kind)
}
