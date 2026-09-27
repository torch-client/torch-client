use std::{mem, ops::Deref, sync::Arc};

use azalea_brigadier::prelude::*;
use bevy_app::App;
use bevy_ecs::{prelude::*, system::RunSystemOnce};
use parking_lot::RwLock;

#[test]
fn bevy_app() {
    let mut app = App::new();

    app.init_resource::<DispatchStorage>();

    if let Err(err) = app
        .world_mut()
        .run_system_once(DispatchStorage::bevy_process_commands)
    {
        panic!("Failed to process commands: {err}");
    }

    if let Err(err) = app
        .world_mut()
        .run_system_once(DispatchStorage::verify_spawned_entities)
    {
        panic!("Failed to verify spawned entities: {err}");
    }
}

#[derive(Resource)]
struct DispatchStorage {
    dispatch: CommandDispatcher<WorldAccessor>,
    world: WorldAccessor,
}

impl FromWorld for DispatchStorage {
    fn from_world(_: &mut World) -> Self {
        let mut dispatch = CommandDispatcher::new();

        {
            dispatch
                .register(literal("spawn_entity").executes(DispatchStorage::command_spawn_entity));

            dispatch.register(literal("spawn_entity_num").then(
                argument("entities", integer()).executes(DispatchStorage::command_spawn_entity_num),
            ));
        }

        Self {
            dispatch,
            world: WorldAccessor::empty(),
        }
    }
}

impl DispatchStorage {
    fn bevy_process_commands(world: &mut World) {
        world.resource_scope::<Self, _>(|bevy_world, mut storage| {
            storage.world.swap(bevy_world);

            let source = storage.world.clone();

            {
                println!("Testing 'spawn_entity' command");
                let result = storage.dispatch.execute("spawn_entity", source.clone());

                assert_eq!(result.unwrap(), 0);

                let mut world = source.write();
                let mut query = world.query_filtered::<(), With<SpawnedEntity>>();

                let count = query.iter(&world).count();
                println!("Spawned entities: {count}");
                assert_eq!(count, 1);
            }

            {
                println!("Testing 'spawn_entity_num' command");
                let result = storage
                    .dispatch
                    .execute("spawn_entity_num 3", source.clone());

                assert_eq!(result.unwrap(), 0);

                let mut world = source.write();
                let mut query = world.query_filtered::<(), With<SpawnedEntity>>();

                let count = query.iter(&world).count();
                println!("Spawned entities: {count}");
                assert_eq!(count, 4);
            }

            storage.world.swap(bevy_world);
        });
    }

    fn command_spawn_entity(context: &CommandContext<WorldAccessor>) -> i32 {
        context.source.write().spawn(SpawnedEntity);

        0
    }

    fn command_spawn_entity_num(context: &CommandContext<WorldAccessor>) -> i32 {
        let num = get_integer(context, "entities").unwrap();

        for _ in 0..num {
            context.source.write().spawn(SpawnedEntity);
        }

        0
    }

    fn verify_spawned_entities(query: Query<(), With<SpawnedEntity>>) {
        assert_eq!(query.iter().count(), 4);
    }
}

#[derive(Clone)]
struct WorldAccessor {
    world: Arc<RwLock<World>>,
}

impl WorldAccessor {
    fn empty() -> Self {
        Self {
            world: Arc::new(RwLock::new(World::new())),
        }
    }

    fn swap(&mut self, world: &mut World) {
        mem::swap(&mut *self.write(), world);
    }
}

#[derive(Clone, Component, Copy, Debug, Default, Eq, Hash, PartialEq)]
struct SpawnedEntity;

impl Deref for WorldAccessor {
    type Target = Arc<RwLock<World>>;
    fn deref(&self) -> &Self::Target {
        &self.world
    }
}
