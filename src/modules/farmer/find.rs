use std::collections::HashSet;

use azalea::Client;
use azalea::entity::inventory::Inventory;
use azalea::entity::metadata::ItemItem;
use azalea_core::position::{BlockPos, ChunkPos};
use azalea_inventory::ItemStack;
use azalea_registry::builtin::{EntityKind, ItemKind};

use super::Crop;
use crate::modules::esp::table::Merged;

pub(super) struct Sweep {
    pub(super) targets: Vec<BlockPos>,
    pub(super) unloaded: HashSet<ChunkPos>,
    pub(super) capped: bool,
}

pub(super) fn table(crop: Crop) -> Merged {
    Merged::of_groups(1, move |state| crop.is_plant(state).then_some(0))
}

fn chunk_radius(range: i32) -> i32 {
    range.div_euclid(16) + 2
}

pub(super) fn in_range(p: BlockPos, home: BlockPos, range: i32) -> bool {
    let (dx, dz) = (i64::from(p.x - home.x), i64::from(p.z - home.z));
    dx * dx + dz * dz <= i64::from(range) * i64::from(range)
}

pub(super) fn column(p: BlockPos) -> ChunkPos {
    ChunkPos::new(p.x.div_euclid(16), p.z.div_euclid(16))
}

fn column_in_range(c: ChunkPos, home: BlockPos, range: i32) -> bool {
    let near = BlockPos::new(
        home.x.clamp(c.x * 16, c.x * 16 + 15),
        home.y,
        home.z.clamp(c.z * 16, c.z * 16 + 15),
    );
    in_range(near, home, range)
}

pub(super) fn sweep(
    bot: &Client,
    table: &Merged,
    crop: Crop,
    home: BlockPos,
    range: i32,
) -> Option<Sweep> {
    let radius = chunk_radius(range);
    let mut found = Vec::new();
    let capped = crate::modules::esp::sweep(
        bot,
        table,
        [home.x, home.y, home.z],
        Some(radius),
        &mut found,
    );
    let plants: Vec<BlockPos> = found.iter().map(|f| BlockPos::new(f.x, f.y, f.z)).collect();
    let targets = targets_in(&plants, crop, home, range);
    let unloaded = {
        let world = bot.world().ok()?;
        let w = world.read();
        let centre = column(home);
        let mut out = HashSet::new();
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let c = ChunkPos::new(centre.x + dx, centre.z + dz);
                if column_in_range(c, home, range) && w.chunks.get(&c).is_none() {
                    out.insert(c);
                }
            }
        }
        out
    };
    Some(Sweep {
        targets,
        unloaded,
        capped,
    })
}

pub(super) fn targets_in(
    plants: &[BlockPos],
    crop: Crop,
    home: BlockPos,
    range: i32,
) -> Vec<BlockPos> {
    let set: HashSet<BlockPos> = plants.iter().copied().collect();
    let mut out: Vec<BlockPos> = plants
        .iter()
        .copied()
        .filter(|p| in_range(*p, home, range))
        .filter(|p| crop.target(|d| set.contains(&BlockPos::new(p.x, p.y - d, p.z))))
        .collect();
    out.sort_unstable_by_key(|p| (dist2(*p, home), p.x, p.y, p.z));
    out.dedup();
    out
}

pub(super) fn dist2(a: BlockPos, b: BlockPos) -> i64 {
    let d = [a.x - b.x, a.y - b.y, a.z - b.z].map(i64::from);
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Dropped {
    pub(super) id: i32,
    pub(super) pos: [f64; 3],
    pub(super) block: BlockPos,
}

pub(super) fn drops(
    bot: &Client,
    crop: Crop,
    home: BlockPos,
    range: i32,
    feet: BlockPos,
    tried: &HashSet<i32>,
) -> Vec<Dropped> {
    let item = crop.drop_item();
    let mut out = Vec::new();
    crate::modules::nearby::each(bot, |e| {
        if e.kind != EntityKind::Item || tried.contains(&e.id) {
            return;
        }
        if !e
            .world
            .get::<ItemItem>(e.entity)
            .is_some_and(|s| s.0.kind() == item)
        {
            return;
        }
        let b = &e.physics.bounding_box;
        let pos = [
            (b.min.x + b.max.x) * 0.5,
            b.min.y,
            (b.min.z + b.max.z) * 0.5,
        ];
        let block = BlockPos::new(
            pos[0].floor() as i32,
            pos[1].floor() as i32,
            pos[2].floor() as i32,
        );
        if in_range(block, home, range) {
            out.push(Dropped {
                id: e.id,
                pos,
                block,
            });
        }
    });
    out.sort_unstable_by_key(|d| (dist2(d.block, feet), d.id));
    out
}

pub(super) fn room_for(bot: &Client, item: ItemKind) -> bool {
    let Ok(inv) = bot.component::<Inventory>() else {
        return false;
    };
    let menu = &inv.inventory_menu;
    menu.player_slots_without_hotbar_range()
        .chain(menu.hotbar_slots_range())
        .any(|i| match menu.slot(i) {
            Some(ItemStack::Present(d)) => {
                d.kind == item && d.count < azalea_inventory::bundle::max_stack_size(d)
            }
            Some(_) => true,
            None => false,
        })
}

pub(super) fn held_count(bot: &Client, item: ItemKind) -> i32 {
    let Ok(inv) = bot.component::<Inventory>() else {
        return 0;
    };
    let menu = &inv.inventory_menu;
    menu.player_slots_without_hotbar_range()
        .chain(menu.hotbar_slots_range())
        .map(|i| match menu.slot(i) {
            Some(ItemStack::Present(d)) if d.kind == item => d.count,
            _ => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column_of(x: i32, z: i32, base: i32, height: i32) -> Vec<BlockPos> {
        (0..height).map(|i| BlockPos::new(x, base + i, z)).collect()
    }

    #[test]
    fn targets_are_the_second_block_of_tall_stalks_in_range() {
        let home = BlockPos::new(0, 64, 0);
        let mut plants = Vec::new();
        plants.extend(column_of(1, 0, 64, 1));
        plants.extend(column_of(2, 0, 64, 2));
        plants.extend(column_of(3, 0, 64, 3));
        plants.extend(column_of(4, 0, 64, 4));
        plants.extend(column_of(40, 0, 64, 3));
        let t = targets_in(&plants, Crop::SugarCane, home, 32);
        assert_eq!(
            t,
            vec![
                BlockPos::new(2, 65, 0),
                BlockPos::new(3, 65, 0),
                BlockPos::new(4, 65, 0)
            ]
        );
    }

    #[test]
    fn the_sweep_radius_covers_the_range_from_any_offset_in_the_chunk() {
        for range in [4, 16, 32, 100] {
            let r = chunk_radius(range);
            for home in [
                BlockPos::new(0, 64, 0),
                BlockPos::new(15, 64, 15),
                BlockPos::new(-1, 64, 7),
            ] {
                for (dx, dz) in [(range, 0), (0, -range), (range * 7 / 10, range * 7 / 10)] {
                    let p = BlockPos::new(home.x + dx, 64, home.z + dz);
                    let (cx, cz) = (column(p).x - column(home).x, column(p).z - column(home).z);
                    assert!(
                        cx * cx + cz * cz <= r * r,
                        "range {range} home {home:?} offset {dx} {dz}"
                    );
                }
            }
        }
    }
}
