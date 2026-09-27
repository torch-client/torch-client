use azalea_client::attack::{
    AttackEvent, AttackStrengthScale, TicksSinceLastAttack, get_attack_strength_delay,
};
use azalea_entity::Attributes;
use bevy_ecs::entity::Entity;

use crate::Client;

impl Client {
    pub fn attack(&self, entity: Entity) {
        self.ecs.write().write_message(AttackEvent {
            entity: self.entity,
            target: entity,
        });
    }

    pub fn has_attack_cooldown(&self) -> bool {
        let Ok(attack_strength_scale) = self.component::<AttackStrengthScale>() else {
            return false;
        };
        **attack_strength_scale < 1.0
    }

    pub fn attack_cooldown_remaining_ticks(&self) -> usize {
        let ecs = self.ecs.read();

        let Some(attributes) = ecs.get::<Attributes>(self.entity) else {
            return 0;
        };
        let Some(ticks_since_last_attack) = ecs.get::<TicksSinceLastAttack>(self.entity) else {
            return 0;
        };

        let attack_strength_delay = get_attack_strength_delay(attributes);
        let remaining_ticks = attack_strength_delay - **ticks_since_last_attack as f32;

        remaining_ticks.max(0.).ceil() as usize
    }
}
