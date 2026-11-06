use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::entity::inventory::Inventory;
use azalea::entity::metadata::{AttachedToTarget, FallFlying, FireworkRocket};
use azalea::entity::{
    ActiveEffects, HasClientLoaded, LocalEntity, LookDirection, OnClimbable, Physics,
    PlayerAbilities, view_vector,
};
use azalea::packet::game::SendGamePacketEvent;
use azalea::physics::{PhysicsSystems, travel::travel};
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::tick::GameTick;
use azalea_inventory::ItemStack;
use azalea_inventory::components::{
    Damage, EquipmentSlot, Equippable, Glider, MaxDamage, Unbreakable,
};
use azalea_protocol::common::movements::MoveFlags;
use azalea_protocol::packets::game::s_move_player_status_only::ServerboundMovePlayerStatusOnly;
use azalea_protocol::packets::game::s_player_command::{Action, ServerboundPlayerCommand};
use azalea_registry::builtin::MobEffect;

use crate::play::flight::{JustToggledFlight, flight_pre_travel};

const SLOTS: [EquipmentSlot; 6] = [
    EquipmentSlot::Mainhand,
    EquipmentSlot::Offhand,
    EquipmentSlot::Feet,
    EquipmentSlot::Legs,
    EquipmentSlot::Chest,
    EquipmentSlot::Head,
];

#[derive(Resource, Default)]
struct WasJumping(bool);

pub struct ElytraPlugin;

impl Plugin for ElytraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WasJumping>().add_systems(
            GameTick,
            (
                start_fall_flying
                    .in_set(PhysicsSystems)
                    .after(flight_pre_travel)
                    .before(travel),
                stop_on_climbable.in_set(PhysicsSystems).after(travel),
                firework_boost.after(azalea::movement::send_position),
            ),
        );
        #[cfg(feature = "audio")]
        app.init_resource::<GlideSound>()
            .add_systems(GameTick, glide_sound.after(firework_boost));
    }
}

fn can_glide_using(stack: &ItemStack, slot: EquipmentSlot) -> bool {
    if stack.get_component::<Glider>().is_none() {
        return false;
    }
    let Some(equippable) = stack.get_component::<Equippable>() else {
        return false;
    };
    equippable.slot == slot && !next_damage_will_break(stack)
}

pub(crate) fn next_damage_will_break(stack: &ItemStack) -> bool {
    let (Some(max), Some(damage)) = (
        stack.get_component::<MaxDamage>(),
        stack.get_component::<Damage>(),
    ) else {
        return false;
    };
    stack.get_component::<Unbreakable>().is_none() && damage.amount >= max.amount - 1
}

fn wearing_glider(inventory: &Inventory) -> bool {
    SLOTS.iter().any(|&slot| {
        inventory
            .get_equipment(slot)
            .is_some_and(|stack| can_glide_using(stack, slot))
    })
}

#[allow(clippy::type_complexity)]
fn start_fall_flying(
    mut query: Query<
        (
            Entity,
            &MinecraftEntityId,
            &mut FallFlying,
            &OnClimbable,
            &Physics,
            &PlayerAbilities,
            &Inventory,
            Option<&ActiveEffects>,
        ),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
    just_toggled: Res<JustToggledFlight>,
    mut was_jumping: ResMut<WasJumping>,
    mut commands: Commands,
) {
    for (entity, id, mut fall_flying, on_climbable, physics, abilities, inventory, effects) in
        &mut query
    {
        let jump = just_toggled.jump;
        let pressed = jump && !was_jumping.0;
        was_jumping.0 = jump;
        if !pressed || just_toggled.toggled || **on_climbable {
            continue;
        }

        let levitating = effects.is_some_and(|e| e.has(MobEffect::Levitation));
        if **fall_flying
            || abilities.flying
            || physics.is_in_water()
            || physics.on_ground()
            || crate::client::tracking::local_vehicle_id().is_some()
            || levitating
            || !wearing_glider(inventory)
        {
            continue;
        }

        **fall_flying = true;
        if crate::modules::no_fall::enabled() {
            commands.trigger(SendGamePacketEvent::new(
                entity,
                ServerboundMovePlayerStatusOnly {
                    flags: MoveFlags {
                        on_ground: false,
                        horizontal_collision: physics.horizontal_collision,
                    },
                },
            ));
        }
        commands.trigger(SendGamePacketEvent::new(
            entity,
            ServerboundPlayerCommand {
                id: *id,
                action: Action::StartFallFlying,
                data: 0,
            },
        ));
    }
}

fn stop_on_climbable(
    mut query: Query<(&mut FallFlying, &OnClimbable), (With<LocalEntity>, With<HasClientLoaded>)>,
) {
    for (mut fall_flying, on_climbable) in &mut query {
        if **fall_flying && **on_climbable {
            **fall_flying = false;
        }
    }
}

#[allow(clippy::type_complexity)]
fn firework_boost(
    rockets: Query<&AttachedToTarget, With<FireworkRocket>>,
    mut players: Query<
        (
            &MinecraftEntityId,
            &FallFlying,
            &LookDirection,
            &mut Physics,
        ),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
) {
    for (id, fall_flying, direction, mut physics) in &mut players {
        if !**fall_flying {
            continue;
        }
        let look = view_vector(*direction);
        for attached in &rockets {
            if attached.0.0.is_none_or(|target| target as i32 != id.0) {
                continue;
            }
            let movement = physics.velocity;
            physics.velocity.x += look.x * 0.1 + (look.x * 1.5 - movement.x) * 0.5;
            physics.velocity.y += look.y * 0.1 + (look.y * 1.5 - movement.y) * 0.5;
            physics.velocity.z += look.z * 0.1 + (look.z * 1.5 - movement.z) * 0.5;
        }
    }
}

#[cfg(feature = "audio")]
#[derive(Resource, Default)]
struct GlideSound {
    playing: Option<(crate::audio::LoopId, u32)>,
    was_fall_flying: bool,
}

#[cfg(feature = "audio")]
impl Drop for GlideSound {
    fn drop(&mut self) {
        if let Some((id, _)) = self.playing.take() {
            crate::audio::end_loop(id);
        }
    }
}

#[cfg(feature = "audio")]
fn glide_sound(
    query: Query<(&FallFlying, &Physics), (With<LocalEntity>, With<HasClientLoaded>)>,
    mut sound: ResMut<GlideSound>,
) {
    const DELAY: u32 = 20;

    let Ok((fall_flying, physics)) = query.single() else {
        if let Some((id, _)) = sound.playing.take() {
            crate::audio::end_loop(id);
        }
        sound.was_fall_flying = false;
        return;
    };
    let flying = **fall_flying;

    if flying && !sound.was_fall_flying {
        if let Some((id, _)) = sound.playing.take() {
            crate::audio::end_loop(id);
        }
        sound.playing = crate::audio::start_loop(
            "item.elytra.flying",
            crate::audio::SoundCategory::Players,
            0.0,
            1.0,
        )
        .map(|id| (id, 0));
    }
    sound.was_fall_flying = flying;

    let Some((id, time)) = sound.playing.as_mut() else {
        return;
    };
    *time += 1;
    if *time > DELAY && !flying {
        let id = *id;
        sound.playing = None;
        crate::audio::end_loop(id);
        return;
    }

    let speed = physics.velocity.length_squared() as f32;
    let mut volume = if speed >= 1.0e-7 {
        (speed / 4.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if *time < DELAY {
        volume = 0.0;
    } else if *time < DELAY * 2 {
        volume *= (*time - DELAY) as f32 / DELAY as f32;
    }
    let pitch = if volume > 0.8 {
        1.0 + (volume - 0.8)
    } else {
        1.0
    };
    crate::audio::update_loop(*id, volume, pitch);
}
