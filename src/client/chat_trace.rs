use std::sync::Mutex;

use crate::platform::time::Instant;

static PENDING: Mutex<Option<(String, Instant)>> = Mutex::new(None);

pub(crate) fn queued(line: &str) {
    if let Ok(mut pending) = PENDING.lock() {
        *pending = Some((line.trim_start_matches('/').to_owned(), Instant::now()));
    }
}

pub(crate) fn written() {
    if let Ok(pending) = PENDING.lock()
        && let Some((_, at)) = pending.as_ref()
    {
        crate::log_info!(
            "chat",
            "trace: packet written {:.0} ms after Enter",
            ms(*at)
        );
    }
}

pub(crate) fn received(text: &str) {
    let Ok(mut pending) = PENDING.lock() else {
        return;
    };
    let Some((line, at)) = pending.as_ref() else {
        return;
    };
    if !line.is_empty() && text.contains(line.as_str()) {
        crate::log_info!("chat", "trace: echo shown {:.0} ms after Enter", ms(*at));
        *pending = None;
    }
}

fn ms(since: Instant) -> f32 {
    since.elapsed().as_secs_f32() * 1000.0
}
