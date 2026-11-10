#[cfg(feature = "click_gui")]
use super::registry::Id;
use super::registry::freecam as setting;
use super::store;

#[cfg(feature = "click_gui")]
pub fn active() -> bool {
    store().enabled(Id::Freecam)
}

#[cfg(feature = "click_gui")]
pub fn toggle() {
    store().toggle(Id::Freecam);
}

#[cfg(not(feature = "click_gui"))]
static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(not(feature = "click_gui"))]
pub fn active() -> bool {
    ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(not(feature = "click_gui"))]
pub fn toggle() {
    ACTIVE.fetch_xor(true, std::sync::atomic::Ordering::Relaxed);
}

static EYE: std::sync::Mutex<View> = std::sync::Mutex::new(View {
    eye: [0.0; 3],
    yaw: 0.0,
    pitch: 0.0,
});

#[derive(Clone, Copy)]
struct View {
    eye: [f64; 3],
    yaw: f32,
    pitch: f32,
}

pub fn publish_eye(eye: [f64; 3], yaw: f32, pitch: f32) {
    *EYE.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = View { eye, yaw, pitch };
}

pub fn eye() -> Option<[f64; 3]> {
    active().then(|| {
        EYE.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .eye
    })
}

pub fn ray() -> Option<([f64; 3], [f64; 3])> {
    if !active() {
        return None;
    }
    let v = *EYE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Some((v.eye, view_vector(v.yaw, v.pitch)))
}

pub fn view_vector(yaw: f32, pitch: f32) -> [f64; 3] {
    super::sent::view_vector(super::sent::Look {
        yaw: -yaw - 180.0,
        pitch: -pitch,
    })
}

pub fn far_hit(
    bot: &azalea::Client,
    eye: [f64; 3],
    dir: [f64; 3],
    range: f64,
    held_item: azalea_registry::builtin::ItemKind,
) -> Option<azalea_core::hit_result::BlockHitResult> {
    use azalea::physics::clip::{BlockShapeType, ClipContext, FluidPickType, clip};
    use azalea_core::position::Vec3;

    let world = bot.world().ok()?;
    let world = world.read();
    let hit = clip(
        &world.chunks,
        ClipContext {
            from: Vec3::new(eye[0], eye[1], eye[2]),
            to: Vec3::new(
                eye[0] + dir[0] * range,
                eye[1] + dir[1] * range,
                eye[2] + dir[2] * range,
            ),
            block_shape_type: BlockShapeType::Outline,
            fluid_pick_type: FluidPickType::None,
            held_item,
            entity: None,
        },
    );
    (!hit.miss).then_some(hit)
}

pub fn pause_clicks() -> bool {
    store().flag(setting::PAUSE_CLICKS)
}

pub fn speed(sprint: bool) -> f32 {
    let n = if sprint {
        setting::SPRINT_SPEED
    } else {
        setting::SPEED
    };
    store().num(n)
}
