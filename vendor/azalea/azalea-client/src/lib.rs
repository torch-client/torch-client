#![feature(error_generic_member_access)]

pub mod account;
mod client;
pub mod local_player;
pub mod ping;
pub mod player;
mod plugins;

#[cfg(feature = "log")]
#[doc(hidden)]
#[cfg(not(target_arch = "wasm32"))]
pub mod test_utils;

#[deprecated = "moved to `account::Account`."]
pub type Account = account::Account;

pub use azalea_physics::client_movement::{ClientMovementState, SprintDirection, WalkDirection};
#[deprecated = "renamed to `ClientMovementState`."]
pub type PhysicsState = ClientMovementState;

pub use azalea_protocol::common::client_information::ClientInformation;
pub use bevy_tasks;
pub use client::{
    InConfigState, InGameState, JoinedClientBundle, LocalPlayerBundle, start_ecs_runner,
};
pub use movement::{StartSprintEvent, StartWalkEvent};
pub use plugins::*;

#[cfg(not(target_arch = "wasm32"))]
pub fn compat<F: Future>(future: F) -> async_compat::Compat<F> {
    async_compat::Compat::new(future)
}

#[cfg(target_arch = "wasm32")]
pub fn compat<F: Future>(future: F) -> F {
    future
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn sleep(duration: std::time::Duration) {
    tokio::time::sleep(duration).await;
}

#[cfg(target_arch = "wasm32")]
pub async fn sleep(duration: std::time::Duration) {
    use wasm_bindgen::{JsCast, JsValue};

    let ms = duration.as_millis().min(i32::MAX as u128) as f64;
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let global = js_sys::global();
        match js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout")) {
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
