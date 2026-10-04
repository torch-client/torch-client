use std::sync::Mutex;

use azalea::Client;
use azalea::ecs::entity::Entity as AzEntity;
use azalea::ecs::query::{QueryState, Without};
use azalea::ecs::world::{World, WorldId};
use azalea::entity::metadata::Health;
use azalea::entity::{EntityKindComponent, LocalEntity, Physics};
use azalea::world::WorldName;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_registry::builtin::EntityKind;

pub struct Seen<'a> {
    pub entity: AzEntity,
    pub id: i32,
    pub kind: EntityKind,
    pub physics: &'a Physics,
    pub health: Option<f32>,
    pub world: &'a World,
}

type Everyone = QueryState<
    (
        AzEntity,
        &'static MinecraftEntityId,
        &'static EntityKindComponent,
        &'static Physics,
        &'static WorldName,
        Option<&'static Health>,
    ),
    Without<LocalEntity>,
>;

static CACHE: Mutex<Option<(WorldId, Everyone)>> = Mutex::new(None);

pub fn each(bot: &Client, mut visit: impl FnMut(Seen<'_>)) {
    let world_name = {
        let Ok(name) = bot.component::<WorldName>() else {
            return;
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
            let state = ecs.query_filtered::<(
                AzEntity,
                &MinecraftEntityId,
                &EntityKindComponent,
                &Physics,
                &WorldName,
                Option<&Health>,
            ), Without<LocalEntity>>();
            *cache = Some((ecs.id(), state));
        }
    }

    let ecs = bot.ecs.read();
    let (_, query) = cache.as_mut().expect("filled above");
    for (entity, id, kind, physics, name, health) in query.iter(&ecs) {
        if *name != world_name {
            continue;
        }
        visit(Seen {
            entity,
            id: id.0,
            kind: kind.0,
            physics,
            health: health.map(|h| h.0),
            world: &ecs,
        });
    }
}
