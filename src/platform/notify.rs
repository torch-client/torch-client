#[cfg(target_os = "android")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "android")]
static BACKGROUNDED: AtomicBool = AtomicBool::new(false);

pub(crate) fn connected(address: &str) {
    #[cfg(target_os = "android")]
    android::call(android::Call::Start(address));
    #[cfg(not(target_os = "android"))]
    let _ = address;
}

pub(crate) fn transferred(address: &str) {
    #[cfg(target_os = "android")]
    if !BACKGROUNDED.load(Ordering::Relaxed) {
        android::call(android::Call::Start(address));
    }
    #[cfg(not(target_os = "android"))]
    let _ = address;
}

pub(crate) fn backgrounded() -> bool {
    #[cfg(target_os = "android")]
    {
        BACKGROUNDED.load(Ordering::Relaxed)
    }
    #[cfg(not(target_os = "android"))]
    {
        false
    }
}

pub(crate) fn disconnected(reason: Option<&str>) {
    #[cfg(target_os = "android")]
    android::call(android::Call::Stop(
        reason.filter(|_| BACKGROUNDED.load(Ordering::Relaxed)),
    ));
    #[cfg(not(target_os = "android"))]
    let _ = reason;
}

pub(crate) fn suspended() {
    #[cfg(target_os = "android")]
    BACKGROUNDED.store(true, Ordering::Relaxed);
}

pub(crate) fn resumed() {
    #[cfg(target_os = "android")]
    {
        BACKGROUNDED.store(false, Ordering::Relaxed);
        android::call(android::Call::Clear);
    }
}

#[cfg(target_os = "android")]
mod android {
    use jni::{Env, JValue, errors::Result, jni_sig, jni_str, objects::JObject};

    pub(super) enum Call<'a> {
        Start(&'a str),
        Stop(Option<&'a str>),
        Clear,
    }

    impl Call<'_> {
        fn name(&self) -> &'static str {
            match self {
                Call::Start(_) => "start",
                Call::Stop(_) => "stop",
                Call::Clear => "clear",
            }
        }
    }

    pub(super) fn call(call: Call<'_>) {
        match crate::platform::java::with_activity(|env, activity| invoke(env, activity, &call)) {
            None => crate::log_warn!("net", "connection service: no AndroidApp yet"),
            Some(Err(e)) => {
                crate::log_warn!("net", "connection service: {} failed: {e}", call.name())
            }
            Some(Ok(())) => {}
        }
    }

    fn invoke(env: &mut Env<'_>, activity: &JObject<'_>, call: &Call<'_>) -> Result<()> {
        let class = crate::platform::java::app_class(
            env,
            activity,
            "com.torchclient.game.ConnectionService",
        )?;
        match call {
            Call::Start(address) => {
                let address: JObject = env.new_string(address)?.into();
                env.call_static_method(
                    &class,
                    jni_str!("start"),
                    jni_sig!("(Landroid/app/Activity;Ljava/lang/String;)V"),
                    &[JValue::Object(activity), JValue::Object(&address)],
                )?;
            }
            Call::Stop(reason) => {
                let reason: JObject = match reason {
                    Some(reason) => env.new_string(reason)?.into(),
                    None => JObject::null(),
                };
                env.call_static_method(
                    &class,
                    jni_str!("stop"),
                    jni_sig!("(Landroid/app/Activity;Ljava/lang/String;)V"),
                    &[JValue::Object(activity), JValue::Object(&reason)],
                )?;
            }
            Call::Clear => {
                env.call_static_method(
                    &class,
                    jni_str!("clear"),
                    jni_sig!("(Landroid/app/Activity;)V"),
                    &[JValue::Object(activity)],
                )?;
            }
        }
        Ok(())
    }
}
