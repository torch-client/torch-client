use std::sync::Arc;

use azalea::Client;
use azalea_core::direction::Direction;
use azalea_core::position::BlockPos;
use azalea_protocol::packets::game::s_player_action::{Action, ServerboundPlayerAction};
use azalea_registry::builtin::SoundEvent;

use crate::client::tracking::{LOCAL_ANIM_ID, anim_states, local_entity_id};
use crate::session::SharedMutex;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct HeldWeapon {
    pub piercing: Option<Option<SoundEvent>>,
    pub min_charge: f32,
}

impl HeldWeapon {
    pub fn cannot_attack(&self, strength: f32) -> bool {
        self.min_charge > 0.0 && strength < self.min_charge
    }
}

pub(crate) fn held_weapon(bot: &Client) -> HeldWeapon {
    let Ok(inventory) = bot.component::<azalea::entity::inventory::Inventory>() else {
        return HeldWeapon::default();
    };
    let held = inventory.held_item();
    let Some(data) = held.as_present() else {
        return HeldWeapon::default();
    };
    HeldWeapon {
        piercing: crate::items::kinetic::piercing(data),
        min_charge: crate::items::kinetic::minimum_attack_charge(data),
    }
}

pub(crate) fn stab(bot: &Client, shared: &Arc<SharedMutex>, sound: Option<SoundEvent>) {
    bot.write_packet(ServerboundPlayerAction {
        action: Action::Stab,
        pos: BlockPos::new(0, 0, 0),
        direction: Direction::Down,
        seq: 0,
    });
    crate::client::tick::request_attack_ticker_reset();
    play_at_player(shared, sound);
    crate::play::interaction::swing_main_hand(bot, shared);
}

pub(crate) fn charge_started(bot: &Client, shared: &Arc<SharedMutex>) {
    let sound = bot
        .component::<azalea::entity::inventory::Inventory>()
        .ok()
        .and_then(|inventory| {
            let held = inventory.held_item();
            crate::items::kinetic::kinetic(held.as_present()?)?.use_sound
        });
    play_at_player(shared, sound);
}

pub(crate) fn kinetic_hit(entity_id: i32) {
    let id = if entity_id == local_entity_id() {
        LOCAL_ANIM_ID
    } else {
        entity_id
    };
    let (sound, _at) = {
        let mut anims = anim_states().lock().unwrap();
        let Some(anim) = anims.get_mut(&id) else {
            return;
        };
        (anim.kinetic_hit(), anim.position(1.0))
    };
    #[cfg(feature = "audio")]
    if let Some(event) = sound {
        crate::audio::play_at(
            crate::items::kinetic::sound_name(event),
            crate::audio::SoundCategory::Players,
            _at,
            1.0,
            1.0,
        );
    }
    #[cfg(not(feature = "audio"))]
    let _ = sound;
}

fn play_at_player(_shared: &Arc<SharedMutex>, sound: Option<SoundEvent>) {
    #[cfg(feature = "audio")]
    if let Some(event) = sound {
        let at = _shared.lock().unwrap().session.player_pos;
        crate::audio::play_at(
            crate::items::kinetic::sound_name(event),
            crate::audio::SoundCategory::Players,
            at,
            1.0,
            1.0,
        );
    }
    #[cfg(not(feature = "audio"))]
    let _ = sound;
}
