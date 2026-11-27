use std::collections::HashMap;
use std::sync::Arc;

use azalea::Client;
use azalea::block::BlockState;
use azalea::entity::inventory::Inventory;
use azalea::pathfinder::PlaceRules;
use azalea::pathfinder::mining::MiningCache;
use azalea::physics::collision::BlockWithShape;
use azalea_core::game_type::GameMode;
use azalea_core::position::BlockPos;
use azalea_inventory::ItemStack;
use azalea_inventory::components::EquipmentSlot;
use azalea_registry::builtin::ItemKind;

use super::plan::{Plan, dist2};
use crate::blocks::state_is_air;
use crate::modules::pathfinder::PLACE_COST;
use crate::modules::place::stock;

const WRONG_PLACE_PENALTY: f32 = 2.0;

const MISSING_PLACE_PENALTY: f32 = 1.5;

const BREAK_CORRECT_PENALTY: f32 = 10.0;

const BREAK_KEPT_PENALTY: f32 = 5.0;

#[derive(Clone, Debug, Default)]
pub(super) struct Stock {
    pub(super) creative: bool,
    counts: HashMap<ItemKind, u32>,
}

impl Stock {
    pub(super) fn read(bot: &Client) -> Stock {
        let creative = bot
            .component::<GameMode>()
            .is_ok_and(|gm| *gm == GameMode::Creative);
        let mut counts = HashMap::new();
        if let Ok(inv) = bot.component::<Inventory>() {
            let menu = &inv.inventory_menu;
            let slots = menu
                .player_slots_without_hotbar_range()
                .chain(menu.hotbar_slots_range());
            let stacks = slots
                .filter_map(|i| menu.slot(i))
                .chain(inv.get_equipment(EquipmentSlot::Offhand));
            for stack in stacks {
                add(&mut counts, stack);
            }
        }
        Stock { creative, counts }
    }

    #[cfg(test)]
    pub(super) fn of(creative: bool, items: &[(ItemKind, u32)]) -> Stock {
        Stock {
            creative,
            counts: items.iter().copied().collect(),
        }
    }

    pub(super) fn has(&self, item: ItemKind) -> bool {
        self.creative || self.counts.get(&item).is_some_and(|&n| n > 0)
    }

    pub(super) fn count(&self, item: ItemKind) -> u32 {
        self.counts.get(&item).copied().unwrap_or(0)
    }
}

fn add(counts: &mut HashMap<ItemKind, u32>, stack: &ItemStack) {
    if stack.is_present() {
        *counts.entry(stack.kind()).or_default() += stack.count().max(0) as u32;
    }
}

thread_local! {
    static MINING: MiningCache = MiningCache::new(None);
}

pub(super) fn falls(state: BlockState) -> bool {
    MINING.with(|m| m.is_falling_block(BlockState::from(state.as_block_kind())))
}

pub(super) fn route_block(want: BlockState, stock: &Stock) -> bool {
    !state_is_air(want)
        && want.is_collision_shape_full()
        && !falls(want)
        && stock::item_for(want).is_some_and(|item| stock.has(item))
}

pub(super) fn place_cost(want: Option<BlockState>, stock: &Stock, throwaway: bool) -> f32 {
    let Some(want) = want else {
        return if throwaway { PLACE_COST } else { f32::INFINITY };
    };
    if route_block(want, stock) {
        return 0.0;
    }
    if !throwaway {
        return f32::INFINITY;
    }
    let missing = !state_is_air(want) && stock::item_for(want).is_none_or(|item| !stock.has(item));
    let penalty = if missing {
        WRONG_PLACE_PENALTY * MISSING_PLACE_PENALTY
    } else {
        WRONG_PLACE_PENALTY
    };
    PLACE_COST * penalty
}

#[derive(Debug)]
pub(super) struct Rules {
    plan: Arc<Plan>,
    stock: Stock,
    throwaway: bool,
}

impl Rules {
    pub(super) fn new(plan: Arc<Plan>, stock: Stock, throwaway: bool) -> Rules {
        Rules {
            plan,
            stock,
            throwaway,
        }
    }
}

impl PlaceRules for Rules {
    fn place_cost(&self, pos: BlockPos) -> f32 {
        place_cost(self.plan.want(pos), &self.stock, self.throwaway)
    }

    fn break_multiplier(&self, pos: BlockPos) -> f32 {
        match self.plan.locate(pos) {
            Some((spot, want)) if !state_is_air(want) && self.plan.is_correct(spot) => {
                BREAK_CORRECT_PENALTY
            }
            Some(_) => 1.0,
            None if self.plan.kept(pos) => BREAK_KEPT_PENALTY,
            None => 1.0,
        }
    }
}

pub(super) fn batch(cells: &[(BlockPos, u8)], feet: BlockPos, max: usize) -> Vec<BlockPos> {
    let Some(y) = cells.iter().map(|(p, _)| p.y).min() else {
        return Vec::new();
    };
    let mut layer: Vec<(u8, i64, BlockPos)> = cells
        .iter()
        .filter(|(p, _)| p.y == y)
        .map(|&(p, s)| (s, dist2(p, feet), p))
        .collect();
    layer.sort_unstable_by_key(|&(s, d, _)| (s, d));
    layer.into_iter().take(max).map(|(_, _, p)| p).collect()
}

#[cfg(test)]
mod tests {
    use azalea_registry::builtin::BlockKind;

    use super::super::plan::tests::{id, region};
    use super::*;

    fn state(kind: BlockKind) -> BlockState {
        BlockState::from(kind)
    }

    #[test]
    fn placing_costs() {
        let stone = state(BlockKind::Stone);
        let have = Stock::of(false, &[(ItemKind::Stone, 3)]);
        let none = Stock::of(false, &[]);
        assert_eq!(place_cost(None, &have, true), PLACE_COST);
        assert_eq!(place_cost(None, &have, false), f32::INFINITY);
        assert_eq!(place_cost(Some(stone), &have, false), 0.0);
        assert_eq!(
            place_cost(Some(BlockState::AIR), &have, true),
            PLACE_COST * 2.0
        );
        assert_eq!(
            place_cost(Some(BlockState::AIR), &have, false),
            f32::INFINITY
        );
        assert_eq!(place_cost(Some(stone), &none, true), PLACE_COST * 3.0);
        assert_eq!(place_cost(Some(stone), &Stock::of(true, &[]), false), 0.0);
        let slab = state(BlockKind::OakSlab);
        let planks = Stock::of(false, &[(ItemKind::OakSlab, 9), (ItemKind::Sand, 9)]);
        assert_eq!(place_cost(Some(slab), &planks, true), PLACE_COST * 2.0);
        assert!(!route_block(state(BlockKind::Sand), &planks));
        assert!(falls(state(BlockKind::Sand)));
        assert!(!falls(stone));
    }

    #[test]
    fn creative_has_everything() {
        let creative = Stock::of(true, &[]);
        assert!(creative.has(ItemKind::Stone) && creative.has(ItemKind::OakPlanks));
        assert!(route_block(state(BlockKind::Stone), &creative));
        assert!(!Stock::of(false, &[]).has(ItemKind::Stone));
    }

    #[test]
    fn finished_work_is_dear_to_break() {
        let palette = vec![id(BlockKind::Stone), 0];
        let plan =
            Arc::new(Plan::new(vec![region([0, 0, 0], [2, 1, 1], palette, vec![0, 1])]).unwrap());
        let rules = Rules::new(plan.clone(), Stock::default(), true);
        let at = BlockPos::new(0, 0, 0);
        assert_eq!(rules.break_multiplier(at), 1.0);
        plan.mark(plan.locate(at).unwrap().0, true);
        assert_eq!(rules.break_multiplier(at), BREAK_CORRECT_PENALTY);
        let air = BlockPos::new(1, 0, 0);
        plan.mark(plan.locate(air).unwrap().0, true);
        assert_eq!(rules.break_multiplier(air), 1.0);
        assert_eq!(rules.break_multiplier(BlockPos::new(5, 0, 0)), 1.0);
    }

    #[test]
    fn structure_void_is_dear_to_break_but_less_than_finished_work() {
        let palette = vec![id(BlockKind::StructureVoid), id(BlockKind::BubbleColumn)];
        let plan =
            Arc::new(Plan::new(vec![region([0, 0, 0], [2, 1, 1], palette, vec![0, 1])]).unwrap());
        let rules = Rules::new(plan, Stock::default(), true);
        assert_eq!(
            rules.break_multiplier(BlockPos::new(0, 0, 0)),
            BREAK_KEPT_PENALTY
        );
        assert_eq!(rules.break_multiplier(BlockPos::new(1, 0, 0)), 1.0);
        assert!(BREAK_KEPT_PENALTY < BREAK_CORRECT_PENALTY);
    }

    #[test]
    fn a_batch_is_the_lowest_layer_fewest_strikes_then_nearest() {
        let feet = BlockPos::new(0, 64, 0);
        let cells = [
            (BlockPos::new(1, 65, 0), 0),
            (BlockPos::new(9, 64, 0), 0),
            (BlockPos::new(2, 64, 0), 1),
            (BlockPos::new(4, 64, 0), 0),
            (BlockPos::new(3, 64, 0), 0),
        ];
        assert_eq!(
            batch(&cells, feet, 3),
            vec![
                BlockPos::new(3, 64, 0),
                BlockPos::new(4, 64, 0),
                BlockPos::new(9, 64, 0)
            ]
        );
        assert_eq!(batch(&cells, feet, 10).len(), 4);
        assert!(batch(&[], feet, 3).is_empty());
    }
}
