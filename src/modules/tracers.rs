use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::Relaxed};

use crate::modules::registry::handle::{Enum, Slider, Toggle};
use crate::modules::registry::{Category, Id, Mode, ModuleIx};
use crate::modules::store::{store, Store};

static ENABLED: AtomicBool = AtomicBool::new(false);

options! {
    pub enum Origin {
        Eye = "Eye",
        Camera = "Camera",
    }
}

pub mod tracer {
    use super::*;
    handles!(Id::Tracers; 0;
        ENABLED Toggle;
        RANGE Slider;
        WIDTH Slider;
        COLOR Enum<ColorMode>;
        ORIGIN Enum<Origin>;
        ONLY_WHEN_LOOKING Toggle;
    );
}

pub fn enabled() -> bool {
    ENABLED.load(Relaxed)
}

pub fn range() -> f32 {
    tracer::RANGE.get(store())
}

pub fn width() -> f32 {
    tracer::WIDTH.get(store())
}

pub fn color_mode() -> ColorMode {
    tracer::COLOR.get(store())
}

pub fn origin() -> Origin {
    tracer::ORIGIN.get(store())
}

pub fn only_when_looking() -> bool {
    tracer::ONLY_WHEN_LOOKING.get(store())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ColorMode {
    EspColor = 0,
    Rainbow = 1,
    Single = 2,
}

impl crate::modules::registry::Options for ColorMode {
    const NAMES: &'static [&'static str] = &["ESP Color", "Rainbow", "Single"];
    fn from_index(i: u8) -> Self {
        const ALL: &[ColorMode] = &[ColorMode::EspColor, ColorMode::Rainbow, ColorMode::Single];
        ALL[(i as usize).min(ALL.len() - 1)]
    }
}

static SINGLE_COLOR: AtomicU8 = AtomicU8::new(0);

pub fn single_color() -> [u8; 3] {
    let c = SINGLE_COLOR.load(Relaxed);
    [(c >> 16) as u8, (c >> 8) as u8, c as u8]
}

pub fn set_single_color(rgb: [u8; 3]) {
    let packed = ((rgb[0] as u32) << 16) | ((rgb[1] as u32) << 8) | (rgb[2] as u32);
    SINGLE_COLOR.store(packed as u8, Relaxed);
}