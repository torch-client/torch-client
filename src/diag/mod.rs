macro_rules! counted {
    (
        $(#[$m:meta])*
        $vis:vis enum $name:ident / $count:ident { $($(#[$vm:meta])* $v:ident,)* }
    ) => {
        $(#[$m])*
        #[derive(Clone, Copy)]
        #[repr(usize)]
        $vis enum $name { $($(#[$vm])* $v,)* }

        const $count: usize = [$($name::$v),*].len();
    };
}

pub(crate) mod alloc;
pub(crate) mod budget;
mod layer;
pub(crate) mod panic_report;
#[cfg(feature = "profiling")]
pub(crate) mod profiling;
mod runtime;
mod stats;
mod watchdog;

pub use layer::custom_layers;
pub use runtime::{CPU_METRICS, MEMORY_METRICS, Runtime, runtime};
pub use stats::{Counts, Stat, add, bump, counts, get};
pub use watchdog::{
    Phase, TickGuard, light_thread_died, note_bot_stopped, note_bot_update, note_cache_center,
    note_chunk_packet, note_event, note_frame, note_packet, note_packet_detail, note_sent_packet,
    note_view, on_chunk_received, on_connected, on_disconnected, on_spawned, set_watching,
    start_watchdog, timed, trace_enabled, tracing_packets,
};

#[cfg_attr(not(feature = "profiling"), allow(unused_imports))]
pub use runtime::{ThreadCpu, thread_cpu};

use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Debug => "debug",
            Level::Info => "info ",
            Level::Warn => "warn ",
            Level::Error => "error",
        }
    }
}

pub fn line(level: Level, tag: &str, msg: &str) {
    let mut out = String::with_capacity(msg.len() + 16);
    for (i, part) in msg.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(level.label());
        out.push(' ');
        out.push_str(&format!("{tag:<7}"));
        out.push(' ');
        out.push_str(part);
    }
    emit(level, &out);
}

#[cfg(target_arch = "wasm32")]
fn emit(level: Level, out: &str) {
    let text = wasm_bindgen::JsValue::from_str(out);
    match level {
        Level::Error => web_sys::console::error_1(&text),
        Level::Warn => web_sys::console::warn_1(&text),
        Level::Debug | Level::Info => web_sys::console::log_1(&text),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn emit(_level: Level, out: &str) {
    use std::io::Write as _;
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(out.as_bytes());
    let _ = err.write_all(b"\n");
}

pub fn debug_on(tag: &str) -> bool {
    static TAGS: OnceLock<Vec<String>> = OnceLock::new();
    let tags = TAGS.get_or_init(|| {
        let mut tags: Vec<String> = std::env::var("MC_DEBUG")
            .unwrap_or_default()
            .split(',')
            .map(|t| t.trim().to_ascii_lowercase())
            .filter(|t| !t.is_empty())
            .collect();
        for (var, tag) in [("MC_DEBUG_CHUNKS", "chunks"), ("MC_DEBUG_LIGHT", "light")] {
            if std::env::var_os(var).is_some() {
                tags.push(tag.to_string());
            }
        }
        tags
    });
    tags.iter().any(|t| t == "all" || t == tag)
}

#[macro_export]
macro_rules! log_info {
    ($tag:expr, $($arg:tt)*) => {
        $crate::diag::line($crate::diag::Level::Info, $tag, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($tag:expr, $($arg:tt)*) => {
        $crate::diag::line($crate::diag::Level::Warn, $tag, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_error {
    ($tag:expr, $($arg:tt)*) => {
        $crate::diag::line($crate::diag::Level::Error, $tag, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_debug {
    ($tag:expr, $($arg:tt)*) => {
        if $crate::diag::debug_on($tag) {
            $crate::diag::line($crate::diag::Level::Debug, $tag, &format!($($arg)*));
        }
    };
}
