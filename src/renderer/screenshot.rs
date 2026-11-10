use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

use super::systems::Shared;

pub(super) fn warm_up(mut commands: Commands) {
    commands
        .spawn(Screenshot::primary_window())
        .observe(discard);
}

fn discard(_captured: On<ScreenshotCaptured>) {}

pub(super) fn request(commands: &mut Commands) {
    commands.spawn(Screenshot::primary_window()).observe(store);
}

fn store(captured: On<ScreenshotCaptured>, shared: Res<Shared>) {
    let shared = shared.0.clone();
    let report = move |result| announce(&shared, result);
    match captured.image.clone().try_into_dynamic() {
        Ok(image) => crate::platform::screenshot::store(&image.to_rgb8(), report),
        Err(e) => report(Err(format!("could not read the frame back: {e}"))),
    }
}

fn announce(shared: &crate::session::SharedMutex, result: Result<String, String>) {
    let (text, tag, style) = match result {
        Ok(text) => (
            text,
            crate::session::ChatTag::System,
            crate::text::Style::default(),
        ),
        Err(reason) => (
            format!("Could not capture a screenshot: {reason}"),
            crate::session::ChatTag::Error,
            crate::text::Style {
                italic: true,
                ..crate::text::Style::colored(0xFF5555)
            },
        ),
    };
    crate::log_info!("render", "{text}");

    shared
        .lock()
        .unwrap()
        .session
        .chat_incoming
        .push(crate::session::ChatEntry {
            spans: vec![crate::text::Span { text, style }],
            events: None,
            tag,
            original: None,
        });
}
