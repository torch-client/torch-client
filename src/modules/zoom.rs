use super::registry::{Id, zoom as setting};
use super::store;

#[derive(Clone, Copy)]
pub struct Settings {
    pub on: bool,
    pub fov: f32,
    pub duration: f32,
}

pub fn settings() -> Settings {
    let s = store();
    Settings {
        on: s.enabled(Id::Zoom),
        fov: s.num(setting::FOV),
        duration: s.num(setting::DURATION).max(0.001),
    }
}
