use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::winit::WINIT_WINDOWS;

const ICON_PNG: &[u8] = include_bytes!("../../windows/icon.png");

pub(crate) fn apply_window_icon(
    mut done: Local<bool>,
    primary: Query<Entity, With<PrimaryWindow>>,
    _main_thread: NonSendMarker,
) {
    if *done {
        return;
    }
    let Ok(entity) = primary.single() else {
        return;
    };
    WINIT_WINDOWS.with_borrow(|windows| {
        let Some(window) = windows.get_window(entity) else {
            return;
        };
        *done = true;
        match icon() {
            Ok(icon) => window.set_window_icon(Some(icon)),
            Err(e) => crate::log_warn!("window", "icon not set: {e}"),
        }
    });
}

fn icon() -> eyre::Result<winit::window::Icon> {
    let rgba = image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png)?.into_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(winit::window::Icon::from_rgba(
        rgba.into_raw(),
        width,
        height,
    )?)
}
