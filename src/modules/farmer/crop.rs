use azalea::block::BlockState;
use azalea_registry::builtin::{BlockKind, ItemKind};

use crate::modules::registry::options;

options! {
    pub enum Crop {
        SugarCane = "Sugar cane",
    }
}

impl Crop {
    pub(super) fn is_plant(self, state: BlockState) -> bool {
        match self {
            Crop::SugarCane => BlockKind::from(state) == BlockKind::SugarCane,
        }
    }

    pub(super) fn target(self, plant: impl Fn(i32) -> bool) -> bool {
        match self {
            Crop::SugarCane => plant(0) && plant(1) && !plant(2),
        }
    }

    pub(super) fn safe_to_break(self, plant: impl Fn(i32) -> bool) -> bool {
        match self {
            Crop::SugarCane => plant(0) && plant(1),
        }
    }

    pub(super) fn half_width(self) -> f64 {
        match self {
            Crop::SugarCane => 6.0 / 16.0,
        }
    }

    pub(super) fn drop_item(self) -> ItemKind {
        match self {
            Crop::SugarCane => ItemKind::SugarCane,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Crop::SugarCane => crate::lang::tr!("farmer.crop.sugar_cane"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets(height: i32) -> Vec<i32> {
        let cane = |y: i32| (0..height).contains(&y);
        (-2..height + 2)
            .filter(|&y| Crop::SugarCane.target(|d| cane(y - d)))
            .collect()
    }

    #[test]
    fn the_second_block_of_a_stalk_is_the_only_target() {
        assert_eq!(
            targets(1),
            Vec::<i32>::new(),
            "a stalk one high has nothing to harvest"
        );
        assert_eq!(targets(2), vec![1]);
        assert_eq!(targets(3), vec![1]);
        assert_eq!(targets(4), vec![1]);
    }

    #[test]
    fn the_base_is_never_a_target_nor_safe_to_break() {
        for height in 1..=4 {
            let cane = |y: i32| (0..height).contains(&y);
            assert!(!Crop::SugarCane.target(|d| cane(-d)), "height {height}");
            assert!(
                !Crop::SugarCane.safe_to_break(|d| cane(-d)),
                "height {height}"
            );
        }
        let cane = |y: i32| (0..4).contains(&y);
        for y in 1..4 {
            assert!(Crop::SugarCane.safe_to_break(|d| cane(y - d)));
        }
    }
}
