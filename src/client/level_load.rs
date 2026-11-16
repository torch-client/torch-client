use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use azalea::entity::LocalEntity;
use azalea::loading::LevelLoadTracker;

use crate::SHARED;
use crate::session::LevelLoading;

#[derive(Default)]
pub(crate) struct Published {
    loading: Option<LevelLoading>,
    callback: Option<Arc<AtomicBool>>,
}

pub(crate) fn publish(
    trackers: azalea::ecs::prelude::Query<
        &LevelLoadTracker,
        azalea::ecs::prelude::With<LocalEntity>,
    >,
    mut last: azalea::ecs::prelude::Local<Published>,
) {
    let tracker = trackers.iter().next();
    let loading = tracker.map(|t| LevelLoading {
        id: t.id(),
        reason: t.reason(),
        server_started: t.server_started(),
    });
    let callback = tracker.and_then(LevelLoadTracker::player_compiled_section_callback);
    let same_callback = match (&last.callback, &callback) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    };
    if loading == last.loading && same_callback {
        return;
    }
    let Some(shared) = SHARED.get() else {
        return;
    };
    {
        let mut s = shared.lock().unwrap();
        if s.session_ended {
            return;
        }
        s.session.level_loading = loading;
        s.session.player_section_ready = callback.clone();
    }
    match (last.loading, loading) {
        (prev, Some(now)) if prev.map(|p| p.id) != Some(now.id) => crate::log_info!(
            "level",
            "loading a new level ({:?}); waiting for LEVEL_CHUNKS_LOAD_START",
            now.reason
        ),
        (Some(prev), Some(now)) if !prev.server_started && now.server_started => {
            crate::log_info!(
                "level",
                "server started the level; waiting for the camera's section"
            )
        }
        (Some(_), None) => crate::log_info!("level", "level loaded; PlayerLoaded sent"),
        _ => {}
    }
    *last = Published { loading, callback };
}

pub(crate) fn note_player_section(
    view: bevy::prelude::Res<crate::renderer::frame_view::FrameView>,
    index: bevy::prelude::Res<crate::renderer::systems::ChunkIndex>,
) {
    let Some(flag) = view.player_section_ready.as_ref() else {
        return;
    };
    if flag.load(Ordering::Relaxed) {
        return;
    }
    let camera = view.camera_world;
    let (x, y, z) = (
        camera.x.floor() as i32,
        camera.y.floor() as i32,
        camera.z.floor() as i32,
    );
    if index.has_section(x >> 4, z >> 4, y.div_euclid(16) * 16) {
        flag.store(true, Ordering::Release);
        crate::log_info!("level", "camera section at {x},{y},{z} is built");
    }
}
