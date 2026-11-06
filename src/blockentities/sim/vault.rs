use simdnbt::owned::Nbt;

use super::TickCtx;
use crate::blockentities::feed::BlockEntityData;
use crate::renderer::anim::HeldItem;
use crate::util::mth;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Data {
    pub display_item: HeldItem,
}

pub fn decode(tag: &Nbt) -> Data {
    Data {
        display_item: tag
            .compound("shared_data")
            .and_then(|shared| shared.compound("display_item"))
            .map(super::items::stack)
            .unwrap_or_default(),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vault {
    previous_spin: f32,
    current_spin: f32,
}

impl Vault {
    const ROTATION_SPEED: f32 = 10.0;

    pub fn tick(&mut self, ctx: &TickCtx) {
        self.update_display_item_spin();
        if let BlockEntityData::Vault(data) = ctx.data
            && !data.display_item.is_empty()
        {
            super::idle_sound(ctx, "block.vault.ambient");
        }
    }

    fn update_display_item_spin(&mut self) {
        self.previous_spin = self.current_spin;
        self.current_spin = mth::wrap_degrees(self.current_spin + Self::ROTATION_SPEED);
    }

    pub fn angle(&self, partial: f32) -> f32 {
        mth::rot_lerp(partial, self.previous_spin, self.current_spin)
    }
}

#[cfg(test)]
mod tests {
    use simdnbt::owned::NbtTag;

    use super::*;
    use crate::blockentities::sim::test_nbt::{compound, item, root};

    #[test]
    fn the_display_item_comes_from_shared_data() {
        let shared = compound(vec![(
            "display_item",
            NbtTag::Compound(compound(item("minecraft:diamond", 2))),
        )]);
        let data = decode(&root(vec![("shared_data", NbtTag::Compound(shared))]));
        assert_eq!(
            (data.display_item.id, data.display_item.count),
            ("diamond", 2)
        );
        assert!(decode(&root(vec![])).display_item.is_empty());
    }

    #[test]
    fn the_spin_turns_ten_degrees_a_tick_the_short_way_round() {
        let mut vault = Vault {
            previous_spin: 0.0,
            current_spin: 175.0,
        };
        vault.update_display_item_spin();
        assert_eq!(vault.current_spin, -175.0);
        assert!((vault.angle(0.5) - 180.0).abs() < 1e-3 || (vault.angle(0.5) + 180.0).abs() < 1e-3);
    }
}
