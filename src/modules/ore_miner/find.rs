use std::collections::{HashSet, VecDeque};

use azalea::Client;
use azalea_core::position::BlockPos;

use super::{Halt, OreSource, Want};
use crate::modules::esp::scan::Found;
use crate::modules::esp::table::Merged;
use crate::modules::registry::{Id, ore_miner as setting};
use crate::modules::{Store, store};
use crate::session::SharedMutex;

const SWEEPS: [Option<i32>; 4] = [Some(20), Some(64), Some(160), None];

pub(super) fn dist2(a: BlockPos, b: BlockPos) -> i64 {
    let d = [a.x - b.x, a.y - b.y, a.z - b.z].map(i64::from);
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

pub(super) fn sort_nearest(list: &mut [BlockPos], feet: BlockPos) {
    list.sort_unstable_by_key(|p| (dist2(*p, feet), p.x, p.y, p.z));
}

#[allow(clippy::too_many_arguments)]
pub(super) fn candidates(
    bot: &Client,
    shared: &SharedMutex,
    s: &Store,
    want: &Want,
    feet: BlockPos,
    bad: &HashSet<BlockPos>,
    said: &mut bool,
    cap: usize,
) -> Result<Vec<BlockPos>, Halt> {
    let (lo, hi) = super::y_limits(s);
    let mask = match want {
        Want::Ores(mask) => *mask,
        Want::Blocks { table, .. } => return Ok(blocks(bot, table, feet, (lo, hi), bad, cap)),
    };
    let range = s.num(setting::RANGE).max(0.0) as i32;
    let seed_on = store().enabled(Id::SeedXray);
    let mut out = Vec::new();
    match s.choice(setting::SOURCE) {
        OreSource::SeedXray => {
            if !seed_on || !crate::modules::seed_xray::predicted(mask, feet, range, &mut out) {
                return Err(Halt::NeedSeed);
            }
        }
        OreSource::Xray => from_frame(bot, shared, want, feet, range, &mut out)?,
        OreSource::Auto => {
            if !(seed_on && crate::modules::seed_xray::predicted(mask, feet, range, &mut out)) {
                if seed_on && !*said {
                    *said = true;
                    super::say(shared, crate::lang::tr!("ore_miner.no_seed_fallback"));
                }
                from_frame(bot, shared, want, feet, range, &mut out)?;
            } else if out.is_empty() && (s.enabled(Id::Xray) || s.enabled(Id::OreEsp)) {
                from_frame(bot, shared, want, feet, range, &mut out)?;
            }
        }
    }
    out.retain(|p| (lo..=hi).contains(&p.y) && !bad.contains(p));
    sort_nearest(&mut out, feet);
    out.dedup();
    out.truncate(cap);
    Ok(out)
}

fn blocks(
    bot: &Client,
    table: &Merged,
    feet: BlockPos,
    y: (i32, i32),
    bad: &HashSet<BlockPos>,
    cap: usize,
) -> Vec<BlockPos> {
    let centre = [feet.x, feet.y, feet.z];
    let mut found = Vec::new();
    for range in SWEEPS {
        match range {
            Some(r) => crate::modules::esp::sweep(
                bot,
                &table.with_range(r),
                centre,
                Some(r / 16 + 1),
                &mut found,
            ),
            None => crate::modules::esp::sweep(bot, table, centre, None, &mut found),
        };
        let (tier, mut out) = first_tier(&found, y, bad);
        if !out.is_empty() && (tier == 0 || range.is_none()) {
            sort_nearest(&mut out, feet);
            out.dedup();
            out.truncate(cap);
            return out;
        }
    }
    Vec::new()
}

fn first_tier(found: &[Found], y: (i32, i32), bad: &HashSet<BlockPos>) -> (u16, Vec<BlockPos>) {
    let keep =
        |f: &&Found| (y.0..=y.1).contains(&f.y) && !bad.contains(&BlockPos::new(f.x, f.y, f.z));
    let Some(tier) = found.iter().filter(keep).map(|f| f.slot).min() else {
        return (0, Vec::new());
    };
    (
        tier,
        found
            .iter()
            .filter(keep)
            .filter(|f| f.slot == tier)
            .map(|f| BlockPos::new(f.x, f.y, f.z))
            .collect(),
    )
}

fn from_frame(
    bot: &Client,
    shared: &SharedMutex,
    want: &Want,
    feet: BlockPos,
    range: i32,
    out: &mut Vec<BlockPos>,
) -> Result<(), Halt> {
    let s = store();
    if !(s.enabled(Id::Xray) || s.enabled(Id::OreEsp)) {
        return Err(Halt::NeedEsp);
    }
    let frame = shared.lock().unwrap().session.esp.clone();
    let Ok(world) = bot.world() else {
        return Ok(());
    };
    let w = world.read();
    let range2 = i64::from(range) * i64::from(range);
    for f in &frame.boxes {
        let p = BlockPos::new(f.x, f.y, f.z);
        if dist2(p, feet) > range2 {
            continue;
        }
        if w.get_block_state(p).and_then(|st| want.group(st)).is_some() {
            out.push(p);
        }
    }
    Ok(())
}

pub(super) fn vein(
    bot: &Client,
    from: BlockPos,
    want: &Want,
    group: u8,
    bad: &HashSet<BlockPos>,
    y: (i32, i32),
    cap: usize,
) -> Vec<BlockPos> {
    let Ok(world) = bot.world() else {
        return Vec::new();
    };
    let w = world.read();
    let mut seen: HashSet<BlockPos> = HashSet::from([from]);
    let mut queue: VecDeque<BlockPos> = VecDeque::from([from]);
    let mut out = Vec::new();
    while let Some(p) = queue.pop_front() {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let q = BlockPos::new(p.x + dx, p.y + dy, p.z + dz);
                    if !seen.insert(q) || bad.contains(&q) || !(y.0..=y.1).contains(&q.y) {
                        continue;
                    }
                    if w.get_block_state(q).and_then(|st| want.group(st)) == Some(group) {
                        out.push(q);
                        if out.len() >= cap {
                            return out;
                        }
                        queue.push_back(q);
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_sort_nearest_first() {
        let feet = BlockPos::new(0, 64, 0);
        let mut list = vec![
            BlockPos::new(5, 64, 0),
            BlockPos::new(1, 64, 0),
            BlockPos::new(0, 60, 0),
            BlockPos::new(1, 64, 0),
        ];
        sort_nearest(&mut list, feet);
        list.dedup();
        assert_eq!(
            list,
            vec![
                BlockPos::new(1, 64, 0),
                BlockPos::new(0, 60, 0),
                BlockPos::new(5, 64, 0)
            ]
        );
    }

    fn found(x: i32, y: i32, z: i32, slot: u16) -> Found {
        Found {
            x,
            y,
            z,
            neigh: 0,
            slot,
        }
    }

    #[test]
    fn the_lowest_tier_with_a_block_wins() {
        let all = [found(1, 70, 0, 1), found(2, 64, 0, 0), found(3, 64, 0, 1)];
        let none = HashSet::new();
        assert_eq!(
            first_tier(&all, (-64, 320), &none),
            (0, vec![BlockPos::new(2, 64, 0)])
        );
        assert_eq!(
            first_tier(&all, (65, 320), &none).1,
            vec![BlockPos::new(1, 70, 0)]
        );
        let bad = HashSet::from([BlockPos::new(2, 64, 0)]);
        assert_eq!(
            first_tier(&all, (-64, 320), &bad),
            (1, vec![BlockPos::new(1, 70, 0), BlockPos::new(3, 64, 0)])
        );
        assert!(first_tier(&all, (100, 320), &none).1.is_empty());
    }
}
