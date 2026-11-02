use std::collections::{HashMap, hash_map};

use azalea_core::attribute_modifier_operation::AttributeModifierOperation;
use azalea_inventory::components::AttributeModifier;
use azalea_registry::{builtin::Attribute, identifier::Identifier};
use thiserror::Error;

#[cfg_attr(feature = "bevy_ecs", derive(bevy_ecs::component::Component))]
#[derive(Clone, Debug)]
pub struct Attributes {
    pub movement_speed: AttributeInstance,
    pub sneaking_speed: AttributeInstance,
    pub attack_speed: AttributeInstance,
    pub water_movement_efficiency: AttributeInstance,
    pub movement_efficiency: AttributeInstance,
    pub gravity: AttributeInstance,
    pub mining_efficiency: AttributeInstance,
    pub block_break_speed: AttributeInstance,
    pub submerged_mining_speed: AttributeInstance,

    pub block_interaction_range: AttributeInstance,
    pub entity_interaction_range: AttributeInstance,

    pub step_height: AttributeInstance,
    pub scale: AttributeInstance,
}

impl Attributes {
    pub fn get_mut(&mut self, attribute: Attribute) -> Option<&mut AttributeInstance> {
        let value = match attribute {
            Attribute::MovementSpeed => &mut self.movement_speed,
            Attribute::SneakingSpeed => &mut self.sneaking_speed,
            Attribute::AttackSpeed => &mut self.attack_speed,
            Attribute::WaterMovementEfficiency => &mut self.water_movement_efficiency,
            Attribute::MovementEfficiency => &mut self.movement_efficiency,
            Attribute::Gravity => &mut self.gravity,
            Attribute::MiningEfficiency => &mut self.mining_efficiency,
            Attribute::BlockInteractionRange => &mut self.block_interaction_range,
            Attribute::EntityInteractionRange => &mut self.entity_interaction_range,
            Attribute::StepHeight => &mut self.step_height,
            Attribute::BlockBreakSpeed => &mut self.block_break_speed,
            Attribute::SubmergedMiningSpeed => &mut self.submerged_mining_speed,
            Attribute::Scale => &mut self.scale,
            _ => return None,
        };
        Some(value)
    }
}

#[derive(Clone, Debug)]
pub struct AttributeInstance {
    pub base: f64,
    modifiers_by_id: HashMap<Identifier, AttributeModifier>,
}

#[derive(Clone, Debug, Error)]
#[error("A modifier with this ID is already present.")]
pub struct AlreadyPresentError;

impl AttributeInstance {
    pub fn new(base: f64) -> Self {
        Self {
            base,
            modifiers_by_id: HashMap::new(),
        }
    }

    pub fn calculate(&self) -> f64 {
        let mut base = self.base;
        for modifier in self.modifiers_by_id.values() {
            if modifier.operation == AttributeModifierOperation::AddValue {
                base += modifier.amount;
            }
        }
        let mut total = base;
        for modifier in self.modifiers_by_id.values() {
            if modifier.operation == AttributeModifierOperation::AddMultipliedBase {
                total += base * modifier.amount;
            }
        }
        for modifier in self.modifiers_by_id.values() {
            if modifier.operation == AttributeModifierOperation::AddMultipliedTotal {
                total *= 1. + modifier.amount;
            }
        }
        total
    }

    pub fn apply_snapshot(&mut self, base: f64, modifiers: &[AttributeModifier]) {
        self.base = base;
        self.modifiers_by_id.clear();
        for modifier in modifiers {
            self.insert(modifier.clone());
        }
    }

    pub fn insert(&mut self, modifier: AttributeModifier) -> Option<AttributeModifier> {
        self.modifiers_by_id.insert(modifier.id.clone(), modifier)
    }

    pub fn try_insert(&mut self, modifier: AttributeModifier) -> Result<(), AlreadyPresentError> {
        match self.modifiers_by_id.entry(modifier.id.clone()) {
            hash_map::Entry::Occupied(_) => Err(AlreadyPresentError),
            hash_map::Entry::Vacant(entry) => {
                entry.insert(modifier);
                Ok(())
            }
        }
    }

    pub fn remove(&mut self, id: &Identifier) -> Option<AttributeModifier> {
        self.modifiers_by_id.remove(id)
    }
}

pub fn sprinting_modifier() -> AttributeModifier {
    AttributeModifier {
        id: Identifier::new("sprinting"),
        amount: 0.3f32 as f64,
        operation: AttributeModifierOperation::AddMultipliedTotal,
    }
}
pub fn base_attack_speed_modifier(amount: f64) -> AttributeModifier {
    AttributeModifier {
        id: Identifier::new("base_attack_speed"),
        amount,
        operation: AttributeModifierOperation::AddValue,
    }
}
pub fn creative_block_interaction_range_modifier() -> AttributeModifier {
    AttributeModifier {
        id: Identifier::new("creative_mode_block_range"),
        amount: 0.5,
        operation: AttributeModifierOperation::AddValue,
    }
}

pub fn creative_entity_interaction_range_modifier() -> AttributeModifier {
    AttributeModifier {
        id: Identifier::new("creative_mode_entity_range"),
        amount: 2.0,
        operation: AttributeModifierOperation::AddValue,
    }
}
