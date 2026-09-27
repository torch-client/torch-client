pub(crate) fn install_panic_hook() {
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let msg = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("<non-string panic payload>");
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("<unnamed>").to_string();
        let location = match info.location() {
            Some(l) => format!("{}:{}:{}", l.file(), l.line(), l.column()),
            None => "<unknown location>".to_string(),
        };

        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let report = format!(
            "\n\
             ==================== PANIC ====================\n\
             thread : {name} ({:?})\n\
             at     : {location}\n\
             message: {msg}\n\
             \n{}\n\
             ===============================================\n",
            thread.id(),
            backtrace.trim_end(),
        );
        #[cfg(target_arch = "wasm32")]
        {
            web_sys::console::error_1(&report.as_str().into());
            print_js_stack();
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::io::Write as _;
            let mut err = std::io::stderr().lock();
            let _ = err.write_all(report.as_bytes());
            let _ = err.flush();
        }
    }));
}

#[cfg(target_arch = "wasm32")]
fn print_js_stack() {
    const NOISE: [&str; 8] = [
        "panic_report",
        "console_error_panic_hook",
        "::panicking",
        "panic_fmt",
        "rust_begin_unwind",
        "__rust_end_short_backtrace",
        "panic_with_hook",
        "panic_already_borrowed",
    ];

    let error = js_sys::Error::new("");
    let Some(stack) = js_sys::Reflect::get(&error, &"stack".into())
        .ok()
        .and_then(|stack| stack.as_string())
    else {
        return;
    };
    let mut printed = 0;
    for frame in stack.lines() {
        if NOISE.iter().any(|noise| frame.contains(noise)) {
            continue;
        }
        let frame = frame.split("@http").next().unwrap_or(frame);
        let frame = match frame.rfind(".wasm.") {
            Some(i) => &frame[i + ".wasm.".len()..],
            None => frame,
        };
        if frame.trim().is_empty() {
            continue;
        }
        web_sys::console::error_1(&format!("  at {frame}").as_str().into());
        printed += 1;
        if printed == 25 {
            return;
        }
    }
}
