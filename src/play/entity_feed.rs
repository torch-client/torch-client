use std::collections::{HashMap, HashSet};

use azalea::ecs::entity::Entity as AzEntity;
use azalea::prelude::*;
use azalea_core::{
    aabb::Aabb,
    direction::Direction as AzDirection,
    position::{BlockPos as AzBlockPos, Vec3 as AzVec3},
};

use crate::client::tick::held_item;
use crate::client::tracking::{
    add_entity_data, anim_states, entity_anims, equipment, game_time, has_map, head_yaws,
    meta_index_18, on_grounds, passengers, velocities,
};
use crate::entities;

pub(crate) mod entity_event {
    pub const JUMP: u8 = 1;
    pub const START_ATTACKING: u8 = 4;
    pub const SHAKE_WETNESS: u8 = 8;
    pub const EAT_GRASS: u8 = 10;
    pub const OFFER_FLOWER: u8 = 11;
    pub const SQUID_ANIM_SYNCH: u8 = 19;
    pub const PERMISSION_LEVEL_ALL: u8 = 24;
    pub const PERMISSION_LEVEL_OWNERS: u8 = 28;
    pub const ARMORSTAND_WOBBLE: u8 = 32;
    pub const STOP_OFFER_FLOWER: u8 = 34;
    pub const RAVAGER_STUNNED: u8 = 39;
    pub const CANCEL_SHAKE_WETNESS: u8 = 56;
    pub const START_RAM: u8 = 58;
    pub const END_RAM: u8 = 59;
    pub const TENDRILS_SHIVER: u8 = 61;
}

pub(crate) const TICKS_REQUIRED_TO_FREEZE: i32 = 140;

pub(crate) const METADATA_INDEX_18: u8 = 18;

const ENTITY_RANGE: f64 = 64.0;

fn flag<C>(eref: &EntityView) -> bool
where
    C: azalea::ecs::component::Component + std::ops::Deref<Target = bool>,
{
    meta::<C>(eref).unwrap_or(false)
}

fn is_living(kind: azalea_registry::builtin::EntityKind) -> bool {
    use azalea_registry::builtin::EntityKind;
    !matches!(
        kind,
        EntityKind::Tnt
            | EntityKind::FallingBlock
            | EntityKind::Item
            | EntityKind::ItemDisplay
            | EntityKind::BlockDisplay
            | EntityKind::TextDisplay
            | EntityKind::ExperienceOrb
            | EntityKind::Arrow
            | EntityKind::SpectralArrow
            | EntityKind::Trident
            | EntityKind::Snowball
            | EntityKind::Egg
            | EntityKind::EnderPearl
            | EntityKind::ExperienceBottle
            | EntityKind::SplashPotion
            | EntityKind::LingeringPotion
            | EntityKind::WindCharge
            | EntityKind::BreezeWindCharge
            | EntityKind::EyeOfEnder
            | EntityKind::Fireball
            | EntityKind::SmallFireball
            | EntityKind::DragonFireball
            | EntityKind::WitherSkull
            | EntityKind::ShulkerBullet
            | EntityKind::LlamaSpit
            | EntityKind::FireworkRocket
            | EntityKind::AcaciaBoat
            | EntityKind::BambooRaft
            | EntityKind::BirchBoat
            | EntityKind::CherryBoat
            | EntityKind::DarkOakBoat
            | EntityKind::JungleBoat
            | EntityKind::MangroveBoat
            | EntityKind::OakBoat
            | EntityKind::PaleOakBoat
            | EntityKind::SpruceBoat
            | EntityKind::AcaciaChestBoat
            | EntityKind::BambooChestRaft
            | EntityKind::BirchChestBoat
            | EntityKind::CherryChestBoat
            | EntityKind::DarkOakChestBoat
            | EntityKind::JungleChestBoat
            | EntityKind::MangroveChestBoat
            | EntityKind::OakChestBoat
            | EntityKind::PaleOakChestBoat
            | EntityKind::SpruceChestBoat
            | EntityKind::Minecart
            | EntityKind::ChestMinecart
            | EntityKind::CommandBlockMinecart
            | EntityKind::FurnaceMinecart
            | EntityKind::HopperMinecart
            | EntityKind::SpawnerMinecart
            | EntityKind::TntMinecart
            | EntityKind::ArmorStand
            | EntityKind::ItemFrame
            | EntityKind::GlowItemFrame
            | EntityKind::Painting
            | EntityKind::LeashKnot
            | EntityKind::EndCrystal
            | EntityKind::FishingBobber
            | EntityKind::AreaEffectCloud
            | EntityKind::Marker
            | EntityKind::Interaction
            | EntityKind::LightningBolt
            | EntityKind::EvokerFangs
            | EntityKind::OminousItemSpawner
    )
}

struct EntityView<'w>(azalea::ecs::world::EntityRef<'w>);

impl EntityView<'_> {
    fn get_component<T: azalea::ecs::component::Component>(&self) -> Option<&T> {
        self.0.get::<T>()
    }
}

fn instance_of<C: azalea::ecs::component::Component>(eref: &EntityView) -> bool {
    eref.get_component::<C>().is_some()
}

fn meta<C>(eref: &EntityView) -> Option<<C as std::ops::Deref>::Target>
where
    C: azalea::ecs::component::Component + std::ops::Deref,
    <C as std::ops::Deref>::Target: Clone,
{
    eref.get_component::<C>().map(|c| (**c).clone())
}

fn variant_key<C>(eref: &EntityView) -> Option<(&'static str, u32)>
where
    C: azalea::ecs::component::Component + std::ops::Deref,
    <C as std::ops::Deref>::Target: azalea_registry::DataRegistry,
{
    use azalea_registry::DataRegistry;
    type Value<C> = <C as std::ops::Deref>::Target;
    let value = meta::<C>(eref)?;
    Some((<Value<C> as DataRegistry>::NAME, value.protocol_id()))
}

fn resolve_variant(
    registries: Option<&azalea_core::registry_holder::RegistryHolder>,
    key: Option<(&'static str, u32)>,
) -> Option<String> {
    let (name, id) = key?;
    registries?
        .protocol_id_to_identifier(azalea::Identifier::from(name), id)
        .map(|ident| ident.to_string())
}

fn block_id(state: azalea::block::BlockState) -> Option<String> {
    use azalea::block::BlockTrait;
    let block: Box<dyn BlockTrait> = state.into();
    let id = block.id();
    (id != "air").then(|| id.to_string())
}

fn block_state_id(state: azalea::block::BlockState) -> Option<u32> {
    (!state.is_air()).then(|| state.id() as u32)
}

fn entity_pose(pose: azalea::entity::Pose) -> entities::Pose {
    use azalea::entity::Pose as A;
    use entities::Pose as P;
    match pose {
        A::Standing => P::Standing,
        A::FallFlying => P::FallFlying,
        A::Sleeping => P::Sleeping,
        A::Swimming => P::Swimming,
        A::SpinAttack => P::SpinAttack,
        A::Crouching => P::Crouching,
        A::LongJumping => P::LongJumping,
        A::Dying => P::Dying,
        A::Croaking => P::Croaking,
        A::UsingTongue => P::UsingTongue,
        A::Sitting => P::Sitting,
        A::Roaring => P::Roaring,
        A::Sniffing => P::Sniffing,
        A::Emerging => P::Emerging,
        A::Digging => P::Digging,
        A::Sliding => P::Sliding,
        A::Shooting => P::Shooting,
        A::Inhaling => P::Inhaling,
    }
}

fn entity_facing(dir: AzDirection) -> entities::Direction {
    entities::Direction::from_index(dir as u8)
}

struct MetaCache {
    kind: azalea_registry::builtin::EntityKind,
    input: entities::feed::TickInput,
    variant: Option<(&'static str, u32)>,
    head_target_ids: [i32; 2],
    sleeping_pos: Option<AzBlockPos>,
}

fn meta_cache() -> &'static std::sync::Mutex<HashMap<i32, MetaCache>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<HashMap<i32, MetaCache>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

struct EntityDeferred {
    variant: Option<(&'static str, u32)>,
    head_target_ids: [i32; 2],
    sleeping_pos: Option<AzBlockPos>,
    block_pos: AzBlockPos,
    eye_pos: AzBlockPos,
}

struct Pending {
    id: i32,
    kind: azalea_registry::builtin::EntityKind,
    input: entities::feed::TickInput,
    deferred: EntityDeferred,
    fresh: bool,
}

fn par_map<T: Sync, R: Send + 'static>(items: &[T], f: impl Fn(&T) -> R + Send + Sync) -> Vec<R> {
    let Some((pool, size)) = fan_out(items.len()) else {
        return items.iter().map(f).collect();
    };
    let parts: Vec<Vec<R>> = pool.scope(|scope| {
        for chunk in items.chunks(size) {
            let f = &f;
            scope.spawn(async move {
                let _churn = crate::diag::alloc::scope(crate::diag::alloc::Site::Entities);
                chunk.iter().map(f).collect::<Vec<R>>()
            });
        }
    });
    parts.into_iter().flatten().collect()
}

fn par_width() -> usize {
    static WIDTH: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *WIDTH.get_or_init(|| {
        std::env::var("MC_PAR")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(4)
    })
}

fn fan_out(len: usize) -> Option<(&'static bevy::tasks::ComputeTaskPool, usize)> {
    if par_width() <= 1 {
        return None;
    }

    let pool = bevy::tasks::ComputeTaskPool::try_get()?;
    let threads = par_width().min(pool.thread_num()).max(1);
    if len < threads * 2 {
        return None;
    }

    Some((pool, len.div_ceil(threads).max(1)))
}

fn par_for_each_mut<T: Send>(items: &mut [T], f: impl Fn(&mut T) + Send + Sync) {
    let Some((pool, size)) = fan_out(items.len()) else {
        items.iter_mut().for_each(f);
        return;
    };
    pool.scope(|scope| {
        for chunk in items.chunks_mut(size) {
            let f = &f;
            scope.spawn(async move {
                let _churn = crate::diag::alloc::scope(crate::diag::alloc::Site::Entities);
                chunk.iter_mut().for_each(f);
            });
        }
    });
}

fn max_fed_entities() -> usize {
    static MAX: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *MAX.get_or_init(|| {
        std::env::var("MC_MAX_FED")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(10_000)
    })
}

fn max_simulated_entities() -> usize {
    static MAX: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *MAX.get_or_init(|| {
        std::env::var("MC_MAX_SIMULATED")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(12_000)
    })
}

fn nearby_entities(bot: &Client, origin: AzVec3, cap: usize) -> Vec<(AzEntity, AzVec3, i32)> {
    use azalea::ecs::query::{QueryState, Without};
    use azalea::ecs::world::WorldId;
    use azalea::entity::{LocalEntity, Position, metadata};
    use azalea_core::entity_id::MinecraftEntityId;
    use azalea_world::WorldName;

    type NearbyQuery = QueryState<
        (
            AzEntity,
            &'static WorldName,
            &'static Position,
            &'static MinecraftEntityId,
        ),
        (Without<LocalEntity>, Without<metadata::Player>),
    >;
    static CACHE: std::sync::Mutex<Option<(WorldId, NearbyQuery)>> = std::sync::Mutex::new(None);

    let world_name = {
        let Ok(name) = bot.component::<WorldName>() else {
            return Vec::new();
        };
        name.clone()
    };

    let mut cache = CACHE.lock().unwrap();
    {
        let ecs = bot.ecs.read();
        let stale = cache.as_ref().is_none_or(|(id, _)| *id != ecs.id());
        drop(ecs);
        if stale {
            let mut ecs = bot.ecs.write();
            let state = ecs.query_filtered::<
                (AzEntity, &WorldName, &Position, &MinecraftEntityId),
                (Without<LocalEntity>, Without<metadata::Player>),
            >();
            *cache = Some((ecs.id(), state));
        }
    }
    let ecs = bot.ecs.read();
    let (_, query) = cache.as_mut().expect("filled above");
    let mut all: Vec<(AzEntity, AzVec3, i32)> = query
        .iter(&ecs)
        .filter(|(_, name, _, _)| **name == world_name)
        .map(|(entity, _, pos, id)| (entity, **pos, id.0))
        .collect();
    drop(ecs);
    drop(cache);

    if all.len() > cap {
        all.select_nth_unstable_by_key(cap, |(_, pos, _)| {
            pos.distance_squared_to(origin).to_bits()
        });
    }
    all
}

fn is_bare_object(kind: azalea_registry::builtin::EntityKind) -> bool {
    use azalea_registry::builtin::EntityKind;
    matches!(
        kind,
        EntityKind::Tnt | EntityKind::FallingBlock | EntityKind::ExperienceOrb
    )
}

pub(crate) fn collect_entities(
    bot: &Client,
    seen_players: &HashSet<i32>,
) -> std::sync::Arc<Vec<entities::feed::EntityAnim>> {
    use azalea::entity::dimensions::EntityDimensions;
    use azalea::entity::{EntityKindComponent, metadata};
    use azalea::entity::{LookDirection, Position};
    use azalea_core::entity_id::MinecraftEntityId;
    use azalea_registry::builtin::EntityKind;
    use entities::Pose;
    use entities::feed::{EntityAnim, TickInput, walk_factor};

    let Ok(origin) = bot.position() else {
        return std::sync::Arc::default();
    };

    let heads_guard = head_yaws().lock().unwrap();
    let equip_guard = equipment().lock().unwrap();
    let meta18_guard = meta_index_18().lock().unwrap();
    let grounded_guard = on_grounds().lock().unwrap();
    let add_entity_data_guard = add_entity_data().lock().unwrap();
    let velocity_guard = velocities().lock().unwrap();
    let heads = &*heads_guard;
    let equip = &*equip_guard;
    let meta18 = &*meta18_guard;
    let grounded = &*grounded_guard;
    let add_entity_data_map = &*add_entity_data_guard;
    let velocity_map = &*velocity_guard;
    let (riders, vehicles, mounts): (HashSet<i32>, HashSet<i32>, HashMap<i32, i32>) = {
        let map = passengers().lock().unwrap();
        (
            map.values().flatten().copied().collect(),
            map.iter()
                .filter(|(_, list)| !list.is_empty())
                .map(|(id, _)| *id)
                .collect(),
            map.iter()
                .flat_map(|(vehicle, list)| list.iter().map(move |rider| (*rider, *vehicle)))
                .collect(),
        )
    };
    let now = game_time();

    let feed_cap = max_fed_entities();
    let refs = {
        let _t = crate::diag::timed(crate::diag::Phase::Nearest);
        nearby_entities(bot, origin, feed_cap)
    };

    let mut seen: HashSet<i32> = HashSet::with_capacity(refs.len());
    let mut live_ids: HashSet<i32> = HashSet::with_capacity(refs.len());

    let _meta_timer = crate::diag::timed(crate::diag::Phase::Meta);
    let ecs = bot.ecs.read();
    let mut accepted: Vec<(azalea::ecs::world::EntityRef<'_>, i32, EntityKind, AzVec3)> =
        Vec::with_capacity(refs.len().min(feed_cap));
    for &(entity, pos, id) in refs.iter() {
        let Ok(entity_ref) = ecs.get_entity(entity) else {
            continue;
        };
        let view = EntityView(entity_ref);
        live_ids.insert(id);
        if pos.distance_squared_to(origin) > ENTITY_RANGE * ENTITY_RANGE {
            continue;
        }
        if seen.len() >= feed_cap {
            continue;
        }
        let Some(kind) = view.get_component::<EntityKindComponent>().map(|k| **k) else {
            continue;
        };
        seen.insert(id);
        accepted.push((entity_ref, id, kind, pos));
    }

    let mut meta_cache_guard = meta_cache().lock().unwrap();
    let mut meta_dirty_guard = crate::client::tracking::meta_dirty().lock().unwrap();
    let meta_cache_ref = &*meta_cache_guard;
    let dirty_ref = &*meta_dirty_guard;

    let mut pending: Vec<Pending> = par_map(&accepted, |&(entity_ref, id, kind, pos)| {
        let eref = &EntityView(entity_ref);

        let (y_rot, x_rot) = eref
            .get_component::<LookDirection>()
            .map(|l| (l.y_rot(), l.x_rot()))
            .unwrap_or((0.0, 0.0));

        if !dirty_ref.contains_key(&id)
            && let Some(c) = meta_cache_ref.get(&id).filter(|c| c.kind == kind)
        {
            let mut input = c.input.clone();
            input.simulate = true;
            input.pos = [pos.x, pos.y, pos.z];
            input.y_rot = y_rot;
            input.x_rot = x_rot;
            input.head_rot = heads.get(&id).copied();
            input.velocity = velocity_map.get(&id).copied();
            input.on_ground = grounded.get(&id).copied().unwrap_or(false);
            input.is_passenger = riders.contains(&id);
            input.walk_halted = matches!(kind, EntityKind::Camel | EntityKind::CamelHusk)
                && (input.pose != Pose::Standing || input.shared.dash);
            input.head_targets = [None; 2];
            let deferred = EntityDeferred {
                variant: c.variant,
                head_target_ids: c.head_target_ids,
                sleeping_pos: c.sleeping_pos,
                block_pos: AzBlockPos::from(pos),
                eye_pos: AzBlockPos::from(pos.up(input.eye_height as f64)),
            };
            return Pending {
                id,
                kind,
                input,
                deferred,
                fresh: false,
            };
        }
        let living = is_living(kind);
        let pose = eref
            .get_component::<azalea::entity::Pose>()
            .map(|p| entity_pose(*p))
            .unwrap_or_default();
        let worn = equip.get(&id).cloned().unwrap_or_default();

        let mut eating = false;
        let health = if living {
            meta::<metadata::Health>(eref).unwrap_or(1.0)
        } else {
            1.0
        };
        let mut shared = crate::entities::state::ExtrasShared::default();

        if living {
            shared.is_baby = flag::<metadata::AbstractAgeableBaby>(eref)
                || flag::<metadata::ZombieBaby>(eref)
                || flag::<metadata::ZoglinBaby>(eref)
                || flag::<metadata::PiglinBaby>(eref);

            shared.powered = flag::<metadata::IsPowered>(eref);

            shared.size = meta::<metadata::SlimeSize>(eref)
                .or_else(|| meta::<metadata::PhantomSize>(eref))
                .unwrap_or(1);

            shared.sheared =
                flag::<metadata::SheepSheared>(eref) || flag::<metadata::BoggedSheared>(eref);
            if kind == EntityKind::Sheep {
                shared.wool_color = meta18
                    .get(&id)
                    .copied()
                    .map(entities::feed::sheep_wool_color)
                    .unwrap_or(0);
            }

            shared.tame = flag::<metadata::Tame>(eref) || flag::<metadata::Tamed>(eref);
            shared.sitting = flag::<metadata::InSittingPose>(eref)
                || flag::<metadata::FoxSitting>(eref)
                || flag::<metadata::PandaSitting>(eref);
            shared.collar_color = meta::<metadata::WolfCollarColor>(eref)
                .or_else(|| meta::<metadata::CatCollarColor>(eref))
                .map(|c| c as u8)
                .unwrap_or(14);

            shared.variant_id = meta::<metadata::HorseTypeVariant>(eref)
                .or_else(|| meta::<metadata::LlamaVariant>(eref))
                .or_else(|| meta::<metadata::TropicalFishTypeVariant>(eref))
                .or_else(|| meta::<metadata::RabbitKind>(eref))
                .or_else(|| meta::<metadata::FoxKind>(eref))
                .or_else(|| meta::<metadata::MooshroomKind>(eref))
                .or_else(|| meta::<metadata::AxolotlVariant>(eref))
                .or_else(|| meta::<metadata::SalmonKind>(eref))
                .or_else(|| meta::<metadata::ParrotVariant>(eref))
                .unwrap_or(0);

            shared.attach_face = meta::<metadata::AttachFace>(eref)
                .map(entity_facing)
                .unwrap_or(entities::Direction::Down);
            shared.color = meta::<metadata::Color>(eref).unwrap_or(16);
        }
        let peek_target = if living {
            meta::<metadata::Peek>(eref).unwrap_or(0) as f32 * 0.01
        } else {
            0.0
        };

        let open_mouth = living
            && eref
                .get_component::<metadata::AbstractHorseStanding>()
                .is_some()
            && meta18
                .get(&id)
                .copied()
                .is_some_and(entities::feed::horse_open_mouth);

        let lying = living && flag::<metadata::IsLying>(eref);
        let relax_state_one = living && flag::<metadata::RelaxStateOne>(eref);

        if !is_bare_object(kind) {
            shared.item = meta::<metadata::ItemItem>(eref)
                .or_else(|| meta::<metadata::ItemFrameItem>(eref))
                .or_else(|| meta::<metadata::ItemDisplayItemStack>(eref))
                .or_else(|| meta::<metadata::FireworksItem>(eref))
                .or_else(|| meta::<metadata::AbstractThrownItemProjectileItemStack>(eref))
                .or_else(|| meta::<metadata::EyeOfEnderItemStack>(eref))
                .or_else(|| meta::<metadata::FireballItemStack>(eref))
                .or_else(|| meta::<metadata::SmallFireballItemStack>(eref))
                .or_else(|| meta::<metadata::OminousItemSpawnerItem>(eref))
                .map(|stack| held_item(&stack))
                .unwrap_or_default();
            shared.item_rotation = meta::<metadata::Rotation>(eref).unwrap_or(0);
        }

        if matches!(kind, EntityKind::ItemFrame | EntityKind::GlowItemFrame) {
            let raw = meta::<metadata::ItemFrameDirection>(eref);
            crate::log_debug!(
                "itemframe",
                "id={id} kind={kind:?} raw_direction={raw:?} item={:?} item_rotation={:?}",
                meta::<metadata::ItemFrameItem>(eref),
                meta::<metadata::Rotation>(eref)
            );
            shared.item_frame_direction =
                raw.map(entity_facing).unwrap_or(entities::Direction::South);
        }

        if let Some(map) = shared.item.map_id
            && !has_map(map)
        {
            shared.item.map_id = None;
        }

        if living {
            if let Some(data) = meta::<metadata::VillagerVillagerData>(eref)
                .or_else(|| meta::<metadata::ZombieVillagerVillagerData>(eref))
            {
                shared.villager_kind = Some(data.kind.to_str());
                shared.villager_profession = Some(data.profession.to_str());
                shared.villager_level = data.level;
            }

            shared.interested =
                flag::<metadata::WolfInterested>(eref) || flag::<metadata::FoxInterested>(eref);
            shared.anger_ticks = meta::<metadata::WolfAngerEndTime>(eref)
                .or_else(|| meta::<metadata::BeeAngerEndTime>(eref))
                .filter(|end| *end > 0)
                .map(|end| (end - now).clamp(0, i32::MAX as i64) as i32)
                .unwrap_or(0);
        }

        shared.crouching = flag::<metadata::FoxCrouching>(eref)
            || flag::<metadata::AbstractEntityShiftKeyDown>(eref);
        if living {
            shared.sleeping = flag::<metadata::Sleeping>(eref);
            shared.pouncing = flag::<metadata::Pouncing>(eref);
            shared.faceplanted = flag::<metadata::Faceplanted>(eref);

            shared.on_back = flag::<metadata::OnBack>(eref);
            shared.panda_rolling = flag::<metadata::PandaRolling>(eref);
            shared.bee_rolling = flag::<metadata::BeeRolling>(eref);
            shared.unhappy_counter = meta::<metadata::PandaUnhappyCounter>(eref)
                .or_else(|| meta::<metadata::AbstractVillagerUnhappyCounter>(eref))
                .unwrap_or(0);

            shared.left_horn = meta::<metadata::HasLeftHorn>(eref).unwrap_or(true);
            shared.right_horn = meta::<metadata::HasRightHorn>(eref).unwrap_or(true);

            shared.standing = flag::<metadata::PolarBearStanding>(eref);
            shared.anger_level = meta::<metadata::ClientAngerLevel>(eref).unwrap_or(0);

            shared.has_pumpkin = meta::<metadata::HasPumpkin>(eref).unwrap_or(true);

            shared.charged = flag::<metadata::Charged>(eref);
            shared.moving = flag::<metadata::Moving>(eref);

            shared.resting = flag::<metadata::Resting>(eref);
            shared.dancing = flag::<metadata::Dancing>(eref) || flag::<metadata::IsDancing>(eref);
        }

        shared.saddled = worn.saddle.is_some();
        let ridden = vehicles.contains(&id);
        shared.ridden = ridden;
        shared.passenger = riders.contains(&id);
        if living {
            shared.has_chest = flag::<metadata::Chest>(eref);
            eating = flag::<metadata::Eating>(eref)
                || meta::<metadata::EatCounter>(eref).is_some_and(|c| c > 0);
            shared.rearing = flag::<metadata::AbstractHorseStanding>(eref);
            shared.dash =
                flag::<metadata::CamelDash>(eref) || flag::<metadata::AbstractNautilusDash>(eref);

            shared.sniffer_state = meta::<metadata::SnifferState>(eref)
                .map(|s| {
                    use azalea::entity::SnifferStateKind as S;
                    match s {
                        S::Idling => 0,
                        S::FeelingHappy => 1,
                        S::Scenting => 2,
                        S::Sniffing => 3,
                        S::Searching => 4,
                        S::Digging => 5,
                        S::Rising => 6,
                    }
                })
                .unwrap_or(0);
            shared.armadillo_state = meta::<metadata::ArmadilloState>(eref)
                .map(|s| {
                    use azalea::entity::ArmadilloStateKind as S;
                    match s {
                        S::Idle => 0,
                        S::Rolling => 1,
                        S::Scared => 2,
                    }
                })
                .unwrap_or(0);
            shared.copper_golem_state = meta::<metadata::CopperGolemState>(eref)
                .map(|s| {
                    use azalea::entity::CopperGolemStateKind as S;
                    match s {
                        S::Idle => 0,
                        S::GettingItem => 1,
                        S::GettingNoItem => 2,
                        S::DroppingItem => 3,
                        S::DroppingNoItem => 4,
                    }
                })
                .unwrap_or(0);
            shared.weather_state = meta::<metadata::WeatherState>(eref)
                .map(|s| {
                    use azalea::entity::WeatheringCopperStateKind as S;
                    match s {
                        S::Unaffected => 0,
                        S::Exposed => 1,
                        S::Weathered => 2,
                        S::Oxidized => 3,
                    }
                })
                .unwrap_or(0);
            shared.has_egg = flag::<metadata::HasEgg>(eref);
            shared.laying_egg = flag::<metadata::LayingEgg>(eref);
            shared.tearing_down = flag::<metadata::IsTearingDown>(eref);
            shared.active = flag::<metadata::IsActive>(eref);

            shared.puff_state = meta::<metadata::PuffState>(eref).unwrap_or(0);

            shared.carried_block = meta::<metadata::CarryState>(eref).and_then(block_id);
            shared.creepy = flag::<metadata::Creepy>(eref);

            shared.spell = meta::<metadata::SpellCasting>(eref).unwrap_or(0);
            shared.charging_crossbow = flag::<metadata::PillagerIsChargingCrossbow>(eref)
                || flag::<metadata::PiglinIsChargingCrossbow>(eref);
            shared.celebrating = flag::<metadata::IsCelebrating>(eref);

            shared.dragon_phase = meta::<metadata::Phase>(eref).unwrap_or(0);
        }

        let mut paddling_left = false;
        let mut paddling_right = false;
        if instance_of::<metadata::AbstractVehicle>(eref) {
            paddling_left = flag::<metadata::PaddleLeft>(eref);
            paddling_right = flag::<metadata::PaddleRight>(eref);
            shared.bubble_time = meta::<metadata::BubbleTime>(eref).unwrap_or(0) as f32;
            shared.hurt_time = meta::<metadata::Hurt>(eref).unwrap_or(0) as f32;
            shared.hurt_dir = meta::<metadata::Hurtdir>(eref).unwrap_or(1) as f32;
            shared.damage = meta::<metadata::Damage>(eref).unwrap_or(0.0);
            shared.display_block =
                meta::<metadata::CustomDisplayBlock>(eref).and_then(block_state_id);
            shared.display_offset = meta::<metadata::DisplayOffset>(eref).unwrap_or(0);
        }

        shared.block_state = meta::<metadata::TntBlockState>(eref)
            .and_then(block_state_id)
            .or_else(|| {
                (kind == EntityKind::FallingBlock)
                    .then(|| add_entity_data_map.get(&id).copied())
                    .flatten()
                    .and_then(|data| azalea::block::BlockState::try_from(data).ok())
                    .and_then(block_state_id)
            });
        shared.fuse = meta::<metadata::Fuse>(eref).unwrap_or(0);

        if instance_of::<metadata::AbstractArrow>(eref) {
            shared.in_ground = flag::<metadata::InGround>(eref);
            shared.arrow_tipped = meta::<metadata::EffectColor>(eref).is_some_and(|c| c > 0);
        }

        if instance_of::<metadata::ArmorStand>(eref) {
            shared.small = flag::<metadata::Small>(eref);
            shared.show_arms = flag::<metadata::ShowArms>(eref);
            shared.show_base_plate = meta::<metadata::ShowBasePlate>(eref).unwrap_or(true);
            let stand_pose = |rot: Option<azalea::entity::Rotations>| {
                rot.map(|r| [r.x.to_radians(), r.y.to_radians(), r.z.to_radians()])
                    .unwrap_or([0.0; 3])
            };
            shared.stand_pose = [
                stand_pose(meta::<metadata::HeadPose>(eref)),
                stand_pose(meta::<metadata::BodyPose>(eref)),
                stand_pose(meta::<metadata::LeftArmPose>(eref)),
                stand_pose(meta::<metadata::RightArmPose>(eref)),
                stand_pose(meta::<metadata::LeftLegPose>(eref)),
                stand_pose(meta::<metadata::RightLegPose>(eref)),
            ];
        }

        shared.main_hand = worn.main_hand;
        shared.off_hand = worn.off_hand;
        shared.helmet = worn.helmet;
        shared.chestplate = worn.chestplate;
        shared.leggings = worn.leggings;
        shared.boots = worn.boots;
        shared.body_armor = worn.body_armor;
        if living {
            shared.left_handed = flag::<metadata::LeftHanded>(eref);
        }

        let custom_name = meta::<metadata::CustomName>(eref).flatten();
        shared.name_spans = custom_name
            .as_ref()
            .map(|text| crate::client::chat_text::to_spans(text))
            .unwrap_or_default();
        shared.name = custom_name.map(|text| text.to_string());
        shared.name_visible = flag::<metadata::CustomNameVisible>(eref);

        shared.sprinting = flag::<metadata::Sprinting>(eref);
        if living {
            shared.sneezing = flag::<metadata::Sneezing>(eref);
            shared.sneeze_time = meta::<metadata::SneezeCounter>(eref).unwrap_or(0);
            shared.suffocating = flag::<metadata::Suffocating>(eref);
            shared.can_move = meta::<metadata::CanMove>(eref).unwrap_or(true);
        }

        if matches!(kind, EntityKind::Wolf) {
            shared.tail_angle = if shared.anger_ticks > 0 {
                1.539_380_4
            } else if shared.tame {
                let damaged = (40.0 - health.min(40.0)) / 40.0;
                (0.55 - damaged * 0.4) * std::f32::consts::PI
            } else {
                std::f32::consts::FRAC_PI_2
            };
        }

        if living {
            shared.has_stinger = !flag::<metadata::HasStung>(eref);
            shared.has_nectar = flag::<metadata::HasNectar>(eref);
        }

        shared.fall_flying = flag::<metadata::FallFlying>(eref);
        if living {
            shared.using_item = flag::<metadata::AbstractLivingUsingItem>(eref);
            shared.use_offhand = flag::<metadata::AbstractLivingUsingOffhand>(eref);
            shared.aggressive = flag::<metadata::Aggressive>(eref);
            shared.charging = meta::<metadata::VexFlags>(eref).is_some_and(|f| f & 0x01 != 0);
        }

        if living {
            shared.invulnerable_ticks = meta::<metadata::Inv>(eref).unwrap_or(0) as f32;
        }
        let head_target_ids = if living {
            [
                meta::<metadata::TargetB>(eref).unwrap_or(0),
                meta::<metadata::TargetC>(eref).unwrap_or(0),
            ]
        } else {
            [0, 0]
        };

        if kind == EntityKind::IronGolem {
            let fraction = health / 100.0;
            shared.crackiness = match fraction {
                f if f < 0.25 => 3,
                f if f < 0.5 => 2,
                f if f < 0.75 => 1,
                _ => 0,
            };
        }

        if kind == EntityKind::WitherSkull {
            shared.skull_dangerous = flag::<metadata::Dangerous>(eref);
        }
        if kind == EntityKind::EndCrystal {
            shared.shows_bottom = meta::<metadata::ShowBottom>(eref).unwrap_or(true);
        }
        if instance_of::<metadata::AbstractDisplay>(eref) {
            shared.brightness_override = meta::<metadata::BrightnessOverride>(eref).unwrap_or(-1);
            if let Some(block) =
                meta::<metadata::BlockDisplayBlockState>(eref).and_then(block_state_id)
            {
                shared.display_block = Some(block);
            }
            let quat = |q: azalea::entity::Quaternion| [q.x, q.y, q.z, q.w];
            let vec3 = |v: azalea_core::position::Vec3f32| [v.x, v.y, v.z];
            let d = crate::entities::state::Display::default();
            let mut display = crate::entities::state::Display {
                billboard: crate::entities::state::Billboard::from_id(
                    meta::<metadata::BillboardRenderConstraints>(eref).unwrap_or(0),
                ),
                translation: meta::<metadata::Translation>(eref)
                    .map(vec3)
                    .unwrap_or(d.translation),
                scale: meta::<metadata::Scale>(eref).map(vec3).unwrap_or(d.scale),
                left_rotation: meta::<metadata::LeftRotation>(eref)
                    .map(quat)
                    .unwrap_or(d.left_rotation),
                right_rotation: meta::<metadata::RightRotation>(eref)
                    .map(quat)
                    .unwrap_or(d.right_rotation),
                view_range: meta::<metadata::ViewRange>(eref).unwrap_or(d.view_range),
                ..d
            };
            if instance_of::<metadata::TextDisplay>(eref) {
                display.text = meta::<metadata::Text>(eref)
                    .map(|text| crate::client::chat_text::to_spans(&text))
                    .unwrap_or_default();
                display.line_width = meta::<metadata::LineWidth>(eref)
                    .map(|w| w as f32)
                    .unwrap_or(display.line_width);
                display.background = meta::<metadata::BackgroundColor>(eref)
                    .map(|c| c as u32)
                    .unwrap_or(display.background);
                display.opacity = meta::<metadata::TextOpacity>(eref).unwrap_or(display.opacity);
                display.flags = meta::<metadata::StyleFlags>(eref).unwrap_or(display.flags);
            }
            shared.display = Some(Box::new(display));
        }
        if living {
            shared.dark_ticks_remaining = meta::<metadata::DarkTicksRemaining>(eref).unwrap_or(0);
        }

        let dimensions = eref
            .get_component::<EntityDimensions>()
            .map(|d| (d.width, d.height, d.eye_height))
            .unwrap_or_else(|| {
                let d = EntityDimensions::from(kind);
                (d.width, d.height, d.eye_height)
            });

        let now_pos = [pos.x, pos.y, pos.z];
        let halted = matches!(kind, EntityKind::Camel | EntityKind::CamelHusk)
            && (pose != Pose::Standing || shared.dash);

        let is_auto_spin_attack = living && flag::<metadata::AutoSpinAttack>(eref);
        let swell_dir = if living {
            meta::<metadata::SwellDir>(eref).unwrap_or(-1)
        } else {
            -1
        };
        let ignited = living && flag::<metadata::IsIgnited>(eref);
        let playing_dead = living && flag::<metadata::PlayingDead>(eref);

        let input = TickInput {
            simulate: true,
            pos: now_pos,
            y_rot,
            head_rot: heads.get(&id).copied(),
            x_rot,
            is_living: eref.get_component::<metadata::Health>().is_some(),
            pose,
            dead: health <= 0.0,
            is_in_water: false,
            is_in_lava: false,
            velocity: velocity_map.get(&id).copied(),
            is_fully_frozen: meta::<metadata::TicksFrozen>(eref)
                .is_some_and(|t| t >= TICKS_REQUIRED_TO_FREEZE),
            is_auto_spin_attack,
            is_invisible: flag::<metadata::Invisible>(eref),
            display_fire: flag::<metadata::OnFire>(eref),
            scale: 1.0,
            bounding_box_width: dimensions.0,
            bounding_box_height: dimensions.1,
            eye_height: dimensions.2,
            bed_orientation: None,
            walk_factor: walk_factor(kind),
            walk_halted: halted,
            is_passenger: riders.contains(&id),
            swell_dir,
            ignited,
            peek_target,
            paddling_left,
            paddling_right,
            lying,
            relax_state_one,
            on_ground: grounded.get(&id).copied().unwrap_or(false),
            playing_dead,
            open_mouth,
            head_targets: [None; 2],
            shared: std::sync::Arc::new(shared),
            eating,
        };

        let deferred = EntityDeferred {
            head_target_ids,
            variant: if living {
                variant_key::<metadata::WolfVariant>(eref)
                    .or_else(|| variant_key::<metadata::CatVariant>(eref))
                    .or_else(|| variant_key::<metadata::CowVariant>(eref))
                    .or_else(|| variant_key::<metadata::PigVariant>(eref))
                    .or_else(|| variant_key::<metadata::ChickenVariant>(eref))
                    .or_else(|| variant_key::<metadata::FrogVariant>(eref))
                    .or_else(|| variant_key::<metadata::PaintingVariant>(eref))
                    .or_else(|| variant_key::<metadata::ZombieNautilusVariant>(eref))
            } else {
                variant_key::<metadata::PaintingVariant>(eref)
            },
            sleeping_pos: if living {
                meta::<metadata::SleepingPos>(eref).flatten()
            } else {
                None
            },
            block_pos: AzBlockPos::from(pos),
            eye_pos: AzBlockPos::from(pos.up(dimensions.2 as f64)),
        };
        Pending {
            id,
            kind,
            input,
            deferred,
            fresh: true,
        }
    });
    drop(ecs);
    drop(_meta_timer);
    drop(heads_guard);
    drop(equip_guard);
    drop(meta18_guard);
    drop(grounded_guard);
    drop(add_entity_data_guard);
    drop(velocity_guard);

    let cap = max_simulated_entities();
    if pending.len() > cap {
        let eye = origin;
        let key = |e: &Pending| {
            let p = e.input.pos;
            let (dx, dy, dz) = (p[0] - eye.x, p[1] - eye.y, p[2] - eye.z);
            (((dx * dx + dy * dy + dz * dz).sqrt() * 0.5) as i32, e.id)
        };
        pending.select_nth_unstable_by_key(cap, key);
        for entry in pending[cap..].iter_mut() {
            entry.input.simulate = false;
        }
    }

    if pending.iter().any(|p| p.deferred.head_target_ids != [0, 0]) {
        use azalea::ecs::query::With;
        let mut eyes: HashMap<i32, [f64; 3]> = HashMap::with_capacity(pending.len());
        for Pending { id, input, .. } in &pending {
            eyes.insert(
                *id,
                [
                    input.pos[0],
                    input.pos[1] + input.eye_height as f64,
                    input.pos[2],
                ],
            );
        }
        for eref in bot
            .nearest_entities::<With<metadata::Player>>()
            .unwrap_or_default()
            .iter()
        {
            let Some(pos) = eref.get_component::<Position>().map(|p| **p) else {
                continue;
            };
            let Some(pid) = eref.get_component::<MinecraftEntityId>().map(|i| i.0) else {
                continue;
            };
            let eye = eref
                .get_component::<EntityDimensions>()
                .map(|d| d.eye_height)
                .unwrap_or(EntityDimensions::from(EntityKind::Player).eye_height);
            eyes.insert(pid, [pos.x, pos.y + eye as f64, pos.z]);
        }
        for Pending {
            input,
            deferred: extra,
            ..
        } in &mut pending
        {
            for i in 0..2 {
                let target = extra.head_target_ids[i];
                input.head_targets[i] = (target != 0).then(|| eyes.get(&target).copied()).flatten();
            }
        }
    }

    let _world_timer = crate::diag::timed(crate::diag::Phase::World);
    {
        let world = bot.world().ok();
        let guard = world.as_ref().map(|w| w.read());
        let level = guard.as_deref();
        let registries = level.map(|g| &g.registries);
        par_for_each_mut(&mut pending, |entry| {
            let Pending {
                kind,
                input,
                deferred: extra,
                fresh,
                ..
            } = entry;
            use azalea::block::fluid_state::FluidKind;
            use azalea::entity::dimensions::EntityDimensions;
            let bed = extra
                .sleeping_pos
                .and_then(|pos| level.and_then(|g| g.get_block_state(pos)))
                .and_then(|state| {
                    use azalea::block::BlockTrait;
                    let block: Box<dyn BlockTrait> = state.into();
                    if !block.id().ends_with("_bed") {
                        return None;
                    }
                    bed_orientation(block.get_property("facing")?)
                });
            let feet = level
                .and_then(|g| g.get_fluid_state(extra.block_pos))
                .map(|f| f.kind);
            let eyes = level
                .and_then(|g| g.get_fluid_state(extra.eye_pos))
                .map(|f| f.kind);
            let variant = (*fresh || (input.shared.variant.is_none() && extra.variant.is_some()))
                .then(|| resolve_variant(registries, extra.variant));

            input.bed_orientation = bed;
            if bed.is_some() {
                input.eye_height = EntityDimensions::from(*kind).eye_height;
            }
            input.is_in_water = matches!(feet, Some(FluidKind::Water));
            input.is_in_lava = matches!(feet, Some(FluidKind::Lava));
            let on_land = !matches!(feet, Some(FluidKind::Water));
            let under_water = matches!(eyes, Some(FluidKind::Water));
            let variant_moved = variant.as_ref().is_some_and(|v| *v != input.shared.variant);
            if variant_moved
                || input.shared.on_land != on_land
                || input.shared.under_water != under_water
            {
                let shared = input.shared_mut();
                if let Some(variant) = variant {
                    shared.variant = variant;
                }
                shared.on_land = on_land;
                shared.under_water = under_water;
            }
        });
    }

    let first_seen: Vec<i32> = pending
        .iter()
        .filter(|p| p.fresh && !meta_cache_ref.contains_key(&p.id))
        .map(|p| p.id)
        .collect();

    for Pending {
        id,
        kind,
        input,
        deferred: extra,
        fresh,
    } in &pending
    {
        if *fresh {
            if input.shared.anger_ticks == 0 {
                meta_cache_guard.insert(
                    *id,
                    MetaCache {
                        kind: *kind,
                        input: input.clone(),
                        variant: extra.variant,
                        head_target_ids: extra.head_target_ids,
                        sleeping_pos: extra.sleeping_pos,
                    },
                );
            } else {
                meta_cache_guard.remove(id);
            }
            if let Some(n) = meta_dirty_guard.get_mut(id) {
                *n -= 1;
                if *n == 0 {
                    meta_dirty_guard.remove(id);
                }
            }
        }
    }
    for id in first_seen {
        meta_dirty_guard.entry(id).or_insert(1);
    }
    drop(meta_cache_guard);
    drop(meta_dirty_guard);

    drop(_world_timer);
    let _anim_timer = crate::diag::timed(crate::diag::Phase::Anim);
    let world = bot.world().ok();
    let guard = world.as_ref().map(|w| w.read());
    let chunk_level = guard.as_ref().map(|g| ChunkLevel { world: g });
    let level: &(dyn entities::physics::Level + Sync) = match &chunk_level {
        Some(level) => level,
        None => &entities::physics::Void,
    };
    let mut anims = entity_anims().lock().unwrap();
    let mut ticked: Vec<i32> = Vec::with_capacity(pending.len());
    let _tick_timer = crate::diag::timed(crate::diag::Phase::Tick);

    let mut at: HashMap<i32, u32> = HashMap::with_capacity(pending.len());
    for (i, entry) in pending.iter().enumerate() {
        let (id, kind) = (entry.id, entry.kind);
        let anim = anims.entry(id).or_insert_with(|| EntityAnim::new(id, kind));
        if anim.kind != kind {
            *anim = EntityAnim::new(id, kind);
        }
        ticked.push(id);
        at.insert(id, i as u32);
    }

    let mut work: Vec<(&mut EntityAnim, &TickInput)> = Vec::with_capacity(ticked.len());
    for (id, anim) in anims.iter_mut() {
        if let Some(&i) = at.get(id) {
            work.push((anim, &pending[i as usize].input));
        }
    }
    par_for_each_mut(&mut work, |(anim, input)| {
        anim.tick_in(input, level);
    });
    drop(work);

    drop(_tick_timer);
    let _snap_timer = crate::diag::timed(crate::diag::Phase::Snapshot);
    let pairs: HashMap<i32, (f32, f32)> = if mounts.is_empty() {
        HashMap::new()
    } else {
        let player_anims = anim_states().lock().unwrap();
        mounts
            .iter()
            .filter_map(|(rider, vehicle)| {
                let pair = anims
                    .get(vehicle)
                    .filter(|anim| anim.is_living())
                    .map(|anim| anim.body_rot_pair())
                    .or_else(|| player_anims.get(vehicle).map(|anim| anim.body_yaw_pair()))?;
                Some((*rider, pair))
            })
            .collect()
    };
    for id in &ticked {
        if let Some(anim) = anims.get_mut(id) {
            anim.set_vehicle_body_rot(pairs.get(id).copied());
        }
    }
    let live: Vec<&EntityAnim> = ticked.iter().filter_map(|id| anims.get(id)).collect();
    let out = std::sync::Arc::new(par_map(&live, |anim| (*anim).clone()));

    drop(_snap_timer);
    anims.retain(|id, _| seen.contains(id));
    drop(anims);
    meta_index_18()
        .lock()
        .unwrap()
        .retain(|id, _| live_ids.contains(id));
    add_entity_data()
        .lock()
        .unwrap()
        .retain(|id, _| live_ids.contains(id));
    meta_cache()
        .lock()
        .unwrap()
        .retain(|id, _| live_ids.contains(id));
    crate::client::tracking::meta_dirty()
        .lock()
        .unwrap()
        .retain(|id, _| live_ids.contains(id));
    velocities()
        .lock()
        .unwrap()
        .retain(|id, _| live_ids.contains(id));
    passengers()
        .lock()
        .unwrap()
        .retain(|id, _| live_ids.contains(id));
    {
        let local = crate::client::tracking::local_entity_id();
        crate::client::tracking::max_healths()
            .lock()
            .unwrap()
            .retain(|id, _| live_ids.contains(id) || seen_players.contains(id) || *id == local);
    }
    crate::client::tracking::mount_attributes()
        .lock()
        .unwrap()
        .retain(|id, _| live_ids.contains(id));
    on_grounds()
        .lock()
        .unwrap()
        .retain(|id, _| live_ids.contains(id) || seen_players.contains(id));
    let live = |id: &i32| live_ids.contains(id) || seen_players.contains(id);
    head_yaws().lock().unwrap().retain(|id, _| live(id));
    equipment().lock().unwrap().retain(|id, _| live(id));

    out
}

struct ChunkLevel<'a> {
    world: &'a azalea_world::World,
}

impl ChunkLevel<'_> {
    fn block(&self, pos: [i32; 3]) -> Option<Box<dyn azalea::block::BlockTrait>> {
        let pos = AzBlockPos::new(pos[0], pos[1], pos[2]);
        self.world.get_block_state(pos).map(|state| state.into())
    }
}

const MAX_FAST_CELLS: usize = 64;

fn collide_check() -> bool {
    static CHECK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *CHECK.get_or_init(|| std::env::var("MC_COLLIDE_CHECK").as_deref() == Ok("1"))
}

fn comp(v: AzVec3, axis: usize) -> f64 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

fn cube_collide(axis: usize, entity: &Aabb, cell: AzBlockPos, mut movement: f64) -> f64 {
    use azalea_core::math::EPSILON;

    if movement.abs() < EPSILON {
        return 0.0;
    }
    let corner = [cell.x as f64, cell.y as f64, cell.z as f64];
    let lo = |i: usize| corner[i];
    let hi = |i: usize| corner[i] + 1.0;

    for other in [(axis + 1) % 3, (axis + 2) % 3] {
        if !(comp(entity.min, other) + EPSILON < hi(other)
            && comp(entity.max, other) - EPSILON >= lo(other))
        {
            return movement;
        }
    }

    if movement > 0.0 {
        if comp(entity.max, axis) - EPSILON < lo(axis) {
            let gap = lo(axis) - comp(entity.max, axis);
            if gap >= -EPSILON {
                movement = movement.min(gap);
            }
        }
    } else if comp(entity.min, axis) + EPSILON >= hi(axis) {
        let gap = hi(axis) - comp(entity.min, axis);
        if gap <= EPSILON {
            movement = movement.max(gap);
        }
    }
    movement
}

fn sweep_cubes(axis: usize, entity: &Aabb, cells: &[AzBlockPos], mut movement: f64) -> f64 {
    use azalea_core::math::EPSILON;

    for cell in cells {
        if movement.abs() < EPSILON {
            return 0.0;
        }
        movement = cube_collide(axis, entity, *cell, movement);
    }
    movement
}

impl ChunkLevel<'_> {
    fn full_cube_cells(&self, area: &Aabb, out: &mut [AzBlockPos]) -> Option<usize> {
        use azalea_core::math::EPSILON;
        use azalea_core::position::{ChunkBlockPos, ChunkPos, ChunkSectionPos};
        use azalea_physics::collision::BlockWithShape;

        let min_y = self.world.chunks.min_y();
        let lo = [
            (area.min.x - EPSILON).floor() as i32 - 1,
            (area.min.y - EPSILON).floor() as i32 - 1,
            (area.min.z - EPSILON).floor() as i32 - 1,
        ];
        let hi = [
            (area.max.x + EPSILON).floor() as i32 + 1,
            (area.max.y + EPSILON).floor() as i32 + 1,
            (area.max.z + EPSILON).floor() as i32 + 1,
        ];

        let section = ChunkSectionPos::block_to_section_coord;
        let mut found = 0;
        for chunk_x in section(lo[0])..=section(hi[0]) {
            for chunk_z in section(lo[2])..=section(hi[2]) {
                let chunk = self.world.chunks.get(&ChunkPos::new(chunk_x, chunk_z));
                let Some(chunk) = chunk.as_deref().map(|c| c.read()) else {
                    continue;
                };
                for x in lo[0].max(chunk_x * 16)..=hi[0].min(chunk_x * 16 + 15) {
                    for z in lo[2].max(chunk_z * 16)..=hi[2].min(chunk_z * 16 + 15) {
                        for y in lo[1]..=hi[1] {
                            let pos = AzBlockPos::new(x, y, z);
                            let Some(state) =
                                chunk.get_block_state(&ChunkBlockPos::from(pos), min_y)
                            else {
                                continue;
                            };
                            if state.is_collision_shape_empty() {
                                continue;
                            }
                            if !state.is_collision_shape_full() {
                                return None;
                            }
                            if !area.intersects_aabb(&Aabb {
                                min: pos.to_vec3_floored(),
                                max: (pos + 1).to_vec3_floored(),
                            }) {
                                continue;
                            }
                            if found == out.len() {
                                return None;
                            }
                            out[found] = pos;
                            found += 1;
                        }
                    }
                }
            }
        }
        Some(found)
    }

    fn collide_shapes(&self, mut aabb: Aabb, mut movement: AzVec3) -> [f64; 3] {
        use azalea_core::direction::Axis;
        use azalea_physics::collision::{Shapes, world_collisions::get_block_collisions};

        let shapes = get_block_collisions(self.world, &aabb.expand_towards(movement));
        if shapes.is_empty() {
            return [movement.x, movement.y, movement.z];
        }

        if movement.y != 0.0 {
            movement.y = Shapes::collide(Axis::Y, &aabb, &shapes, movement.y);
            if movement.y != 0.0 {
                aabb = aabb.move_relative(AzVec3::new(0.0, movement.y, 0.0));
            }
        }
        let z_first = movement.x.abs() < movement.z.abs();
        if z_first && movement.z != 0.0 {
            movement.z = Shapes::collide(Axis::Z, &aabb, &shapes, movement.z);
            if movement.z != 0.0 {
                aabb = aabb.move_relative(AzVec3::new(0.0, 0.0, movement.z));
            }
        }
        if movement.x != 0.0 {
            movement.x = Shapes::collide(Axis::X, &aabb, &shapes, movement.x);
            if movement.x != 0.0 {
                aabb = aabb.move_relative(AzVec3::new(movement.x, 0.0, 0.0));
            }
        }
        if !z_first && movement.z != 0.0 {
            movement.z = Shapes::collide(Axis::Z, &aabb, &shapes, movement.z);
        }
        [movement.x, movement.y, movement.z]
    }
}

impl entities::physics::Level for ChunkLevel<'_> {
    fn collide(&self, min: [f64; 3], max: [f64; 3], movement: [f64; 3]) -> [f64; 3] {
        let mut movement = AzVec3::new(movement[0], movement[1], movement[2]);
        let mut aabb = Aabb {
            min: AzVec3::new(min[0], min[1], min[2]),
            max: AzVec3::new(max[0], max[1], max[2]),
        };

        let (entry_box, entry_movement) = (aabb, movement);
        let mut cells = [AzBlockPos::new(0, 0, 0); MAX_FAST_CELLS];
        let Some(found) = self.full_cube_cells(&aabb.expand_towards(movement), &mut cells) else {
            return self.collide_shapes(aabb, movement);
        };
        let cells = &cells[..found];

        if movement.y != 0.0 {
            movement.y = sweep_cubes(1, &aabb, cells, movement.y);
            if movement.y != 0.0 {
                aabb = aabb.move_relative(AzVec3::new(0.0, movement.y, 0.0));
            }
        }
        let z_first = movement.x.abs() < movement.z.abs();
        if z_first && movement.z != 0.0 {
            movement.z = sweep_cubes(2, &aabb, cells, movement.z);
            if movement.z != 0.0 {
                aabb = aabb.move_relative(AzVec3::new(0.0, 0.0, movement.z));
            }
        }
        if movement.x != 0.0 {
            movement.x = sweep_cubes(0, &aabb, cells, movement.x);
            if movement.x != 0.0 {
                aabb = aabb.move_relative(AzVec3::new(movement.x, 0.0, 0.0));
            }
        }
        if !z_first && movement.z != 0.0 {
            movement.z = sweep_cubes(2, &aabb, cells, movement.z);
        }
        let fast = [movement.x, movement.y, movement.z];
        if collide_check() {
            let slow = self.collide_shapes(entry_box, entry_movement);
            for axis in 0..3 {
                if (fast[axis] - slow[axis]).abs() > 1.0e-9 {
                    crate::log_warn!(
                        "phys",
                        "collision disagreement on axis {axis}: fast {:?} against general {:?}, \
                         box {:?}..{:?} moving {:?}",
                        fast,
                        slow,
                        entry_box.min,
                        entry_box.max,
                        entry_movement
                    );
                    break;
                }
            }
        }
        fast
    }

    fn friction(&self, pos: [i32; 3]) -> f32 {
        self.block(pos)
            .map(|block| block.behavior().friction)
            .unwrap_or(0.6)
    }

    fn speed_factor(&self, pos: [i32; 3]) -> (f32, bool) {
        use azalea::registry::builtin::BlockKind;

        let pos = AzBlockPos::new(pos[0], pos[1], pos[2]);
        let Some(kind) = self.world.get_block_state(pos).map(BlockKind::from) else {
            return (1.0, false);
        };
        (
            azalea::physics::block_speed_factor(kind),
            matches!(kind, BlockKind::Water | BlockKind::BubbleColumn),
        )
    }
}

pub(crate) fn bed_orientation_at(bot: &Client, pos: AzBlockPos) -> Option<entities::Direction> {
    use azalea::block::BlockTrait;
    let world = bot.world().ok()?;
    let guard = world.read();
    let state = guard.get_block_state(pos)?;
    let block: Box<dyn BlockTrait> = state.into();
    if !block.id().ends_with("_bed") {
        return None;
    }
    bed_orientation(block.get_property("facing")?)
}

fn bed_orientation(name: &str) -> Option<entities::Direction> {
    match name {
        "north" => Some(entities::Direction::North),
        "south" => Some(entities::Direction::South),
        "west" => Some(entities::Direction::West),
        "east" => Some(entities::Direction::East),
        _ => None,
    }
}
