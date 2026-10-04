#[cfg_attr(not(feature = "mobile_ui"), allow(dead_code))]
pub(crate) const SUPPORTED: bool = cfg!(target_os = "android");

#[cfg_attr(not(feature = "mobile_ui"), allow(dead_code))]
pub(crate) fn set_portrait(on: bool) {
    #[cfg(target_os = "android")]
    android::set_portrait(on);
    #[cfg(not(target_os = "android"))]
    let _ = on;
}

#[cfg(target_os = "android")]
mod android {
    use jni::{Env, JValue, JavaVM, errors::Result, jni_sig, jni_str, objects::JObject};

    const SENSOR_LANDSCAPE: i32 = 6;
    const SENSOR_PORTRAIT: i32 = 7;

    pub(super) fn set_portrait(on: bool) {
        let Some(app) = bevy::android::ANDROID_APP.get() else {
            crate::log_warn!("render", "orientation: no AndroidApp yet");
            return;
        };
        let orientation = if on {
            SENSOR_PORTRAIT
        } else {
            SENSOR_LANDSCAPE
        };
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
        let result = vm.attach_current_thread(|env| {
            let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
            let called = request(env, &activity, orientation);
            if called.is_err() {
                env.exception_clear();
            }
            called
        });
        match result {
            Ok(()) => crate::log_info!(
                "render",
                "orientation: asked for {}",
                if on { "portrait" } else { "landscape" }
            ),
            Err(e) => crate::log_warn!("render", "orientation: request failed: {e}"),
        }
    }

    fn request(env: &mut Env<'_>, activity: &JObject<'_>, orientation: i32) -> Result<()> {
        env.call_method(
            activity,
            jni_str!("setRequestedOrientation"),
            jni_sig!("(I)V"),
            &[JValue::Int(orientation)],
        )?;
        Ok(())
    }
}
