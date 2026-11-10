use azalea::physics::clip::{BlockShapeType, ClipContext, FluidPickType, clip};
use azalea_core::attribute_modifier_operation::AttributeModifierOperation as Op;
use azalea_core::position::Vec3;
use azalea_core::registry_holder::RegistryHolder;
use azalea_inventory::ItemStack;
use azalea_inventory::components::{self as comp, EquipmentSlotGroup as Group};
use azalea_registry::builtin::Attribute;
use azalea_world::ChunkStorage;

pub(crate) const CRYSTAL_POWER: f32 = 6.0;

pub(crate) fn blast(power: f32, distance: f64, exposure: f32) -> f32 {
    let reach = power as f64 * 2.0;
    let d = distance / reach;
    if d > 1.0 {
        return 0.0;
    }
    let p = (1.0 - d) * exposure as f64;
    ((p * p + p) / 2.0 * 7.0 * reach + 1.0) as f32
}

pub(crate) fn seen_percent(
    chunks: &ChunkStorage,
    center: [f64; 3],
    min: [f64; 3],
    max: [f64; 3],
) -> f32 {
    let xs = 1.0 / ((max[0] - min[0]) * 2.0 + 1.0);
    let ys = 1.0 / ((max[1] - min[1]) * 2.0 + 1.0);
    let zs = 1.0 / ((max[2] - min[2]) * 2.0 + 1.0);
    if xs < 0.0 || ys < 0.0 || zs < 0.0 {
        return 0.0;
    }
    let x_offset = (1.0 - (1.0 / xs).floor() * xs) / 2.0;
    let z_offset = (1.0 - (1.0 / zs).floor() * zs) / 2.0;
    let lerp = |t: f64, a: f64, b: f64| a + t * (b - a);
    let to = Vec3::new(center[0], center[1], center[2]);

    let (mut hits, mut count) = (0u32, 0u32);
    let mut xx = 0.0;
    while xx <= 1.0 {
        let mut yy = 0.0;
        while yy <= 1.0 {
            let mut zz = 0.0;
            while zz <= 1.0 {
                let from = Vec3::new(
                    lerp(xx, min[0], max[0]) + x_offset,
                    lerp(yy, min[1], max[1]),
                    lerp(zz, min[2], max[2]) + z_offset,
                );
                let hit = clip(
                    chunks,
                    ClipContext {
                        from,
                        to,
                        block_shape_type: BlockShapeType::Collider,
                        fluid_pick_type: FluidPickType::None,
                        held_item: azalea_registry::builtin::ItemKind::Air,
                        entity: None,
                    },
                );
                if hit.miss {
                    hits += 1;
                }
                count += 1;
                zz += zs;
            }
            yy += ys;
        }
        xx += xs;
    }
    if count == 0 {
        0.0
    } else {
        hits as f32 / count as f32
    }
}

pub(crate) fn scale_difficulty(damage: f32, difficulty: u8) -> f32 {
    match difficulty {
        0 => 0.0,
        1 => (damage / 2.0 + 1.0).min(damage),
        3 => damage * 3.0 / 2.0,
        _ => damage,
    }
}

pub(crate) fn after_armor(damage: f32, armor: f32, toughness: f32) -> f32 {
    let toughness = 2.0 + toughness / 4.0;
    let real = (armor - damage / toughness).clamp(armor * 0.2, 20.0);
    damage * (1.0 - real / 25.0)
}

pub(crate) fn after_magic(damage: f32, resistance: Option<i32>, protection: f32) -> f32 {
    let mut damage = damage;
    if let Some(amp) = resistance {
        let absorb = 25 - (amp + 1) * 5;
        damage = (damage * absorb as f32 / 25.0).max(0.0);
    }
    if damage <= 0.0 {
        return 0.0;
    }
    damage * (1.0 - protection.clamp(0.0, 20.0) / 25.0)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Worn {
    pub present: bool,
    pub armor: f32,
    pub toughness: f32,
    pub protection: f32,
    pub durability: f32,
}

impl Worn {
    pub(crate) fn of(stack: &ItemStack, slot: Group, registries: Option<&RegistryHolder>) -> Worn {
        let Some(data) = stack.as_present() else {
            return Worn::default();
        };
        let mut protection = 0.0;
        if let Some(enchants) = data.get_component::<comp::Enchantments>() {
            for (e, level) in &enchants.levels {
                match crate::play::inventory_bridge::enchantment_name(registries, e).as_str() {
                    "protection" => protection += *level as f32,
                    "blast_protection" => protection += 2.0 * *level as f32,
                    _ => {}
                }
            }
        }
        let max = data
            .get_component::<comp::MaxDamage>()
            .map_or(0, |m| m.amount);
        let used = data.get_component::<comp::Damage>().map_or(0, |d| d.amount);
        Worn {
            present: true,
            armor: modifier_total(stack, slot, Attribute::Armor) as f32,
            toughness: modifier_total(stack, slot, Attribute::ArmorToughness) as f32,
            protection,
            durability: if max > 0 {
                (1.0 - used as f32 / max as f32).clamp(0.0, 1.0)
            } else {
                1.0
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Defense {
    pub armor: f32,
    pub toughness: f32,
    pub protection: f32,
    pub resistance: Option<i32>,
}

impl Defense {
    pub(crate) fn of(worn: &[Worn; 4], resistance: Option<i32>) -> Defense {
        let sum = |f: fn(&Worn) -> f32| worn.iter().map(f).sum::<f32>();
        Defense {
            armor: sum(|w| w.armor).floor(),
            toughness: sum(|w| w.toughness),
            protection: sum(|w| w.protection),
            resistance,
        }
    }
}

pub(crate) fn explosion(
    power: f32,
    center: [f64; 3],
    feet: [f64; 3],
    exposure: f32,
    def: &Defense,
    difficulty: u8,
) -> f32 {
    let d = [
        feet[0] - center[0],
        feet[1] - center[1],
        feet[2] - center[2],
    ];
    let distance = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let raw = blast(power, distance, exposure);
    if raw <= 0.0 {
        return 0.0;
    }
    let damage = scale_difficulty(raw, difficulty);
    let damage = after_armor(damage, def.armor, def.toughness);
    after_magic(damage, def.resistance, def.protection)
}

fn admits(entry: Group, worn: Group) -> bool {
    match entry {
        Group::Any => true,
        Group::Armor => matches!(worn, Group::Head | Group::Chest | Group::Legs | Group::Feet),
        Group::Hand => matches!(worn, Group::Mainhand | Group::Offhand),
        other => other == worn,
    }
}

pub(crate) fn modifier_total(stack: &ItemStack, worn: Group, attribute: Attribute) -> f64 {
    let Some(modifiers) = stack.get_component::<comp::AttributeModifiers>() else {
        return 0.0;
    };
    modifiers
        .modifiers
        .iter()
        .filter(|e| {
            e.kind == attribute && e.modifier.operation == Op::AddValue && admits(e.slot, worn)
        })
        .map(|e| e.modifier.amount)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blast_matches_vanilla() {
        assert_eq!(blast(6.0, 0.0, 1.0), 85.0);
        assert_eq!(blast(6.0, 12.0, 1.0), 1.0);
        assert_eq!(blast(6.0, 12.01, 1.0), 0.0);
        assert_eq!(blast(4.0, 4.0, 1.0), 22.0);
        assert_eq!(blast(6.0, 0.0, 0.0), 1.0);
    }

    #[test]
    fn difficulty_scales_as_player_hurt_server() {
        assert_eq!(scale_difficulty(10.0, 0), 0.0);
        assert_eq!(scale_difficulty(10.0, 1), 6.0);
        assert_eq!(scale_difficulty(1.0, 1), 1.0);
        assert_eq!(scale_difficulty(10.0, 2), 10.0);
        assert_eq!(scale_difficulty(10.0, 3), 15.0);
    }

    #[test]
    fn full_diamond_and_blast_protection() {
        assert!((after_armor(40.0, 20.0, 8.0) - 24.0).abs() < 1e-4);
        assert!((after_magic(24.0, None, 32.0) - 4.8).abs() < 1e-4);
        assert!((after_magic(10.0, Some(0), 0.0) - 8.0).abs() < 1e-4);
        assert_eq!(after_magic(10.0, Some(4), 0.0), 0.0);
    }
}
