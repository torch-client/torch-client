pub(crate) use bevy::platform::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_arch = "wasm32")]
pub(crate) use web_time::{SystemTime, UNIX_EPOCH};

pub(crate) fn epoch() -> Instant {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn sleep(duration: std::time::Duration) {
    tokio::time::sleep(duration).await;
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn sleep(duration: std::time::Duration) {
    use wasm_bindgen::{JsCast, JsValue};

    let ms = duration.as_millis().min(i32::MAX as u128) as f64;
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let global = js_sys::global();
        let set_timeout = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"));
        match set_timeout {
            Ok(f) if f.is_function() => {
                let f: js_sys::Function = f.unchecked_into();
                if f.call2(&global, &resolve, &JsValue::from_f64(ms)).is_ok() {
                    return;
                }
            }
            _ => {}
        }
        let _ = resolve.call0(&JsValue::NULL);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn timeout<F: Future>(
    duration: std::time::Duration,
    future: F,
) -> Result<F::Output, ()> {
    use std::{pin::Pin, task::Poll};

    let mut future = Box::pin(future);
    let mut deadline = Box::pin(sleep(duration));
    std::future::poll_fn(move |cx| {
        if let Poll::Ready(value) = Pin::new(&mut future).poll(cx) {
            return Poll::Ready(Ok(value));
        }
        if Pin::new(&mut deadline).poll(cx).is_ready() {
            return Poll::Ready(Err(()));
        }
        Poll::Pending
    })
    .await
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn timeout<F: Future>(
    duration: std::time::Duration,
    future: F,
) -> Result<F::Output, ()> {
    tokio::time::timeout(duration, future).await.map_err(|_| ())
}
