pub(crate) fn system() -> Vec<String> {
    raw().iter().filter_map(|name| normalise(name)).collect()
}

fn normalise(raw: &str) -> Option<String> {
    let code = raw.split(['.', '@']).next().unwrap_or("").trim();
    if code.is_empty() || code == "C" || code == "POSIX" {
        return None;
    }
    Some(code.replace('-', "_").to_ascii_lowercase())
}

#[cfg(all(unix, not(target_os = "android")))]
fn raw() -> Vec<String> {
    let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    let Some(locale) = var("LC_ALL")
        .or_else(|| var("LC_MESSAGES"))
        .or_else(|| var("LANG"))
    else {
        crate::log_warn!("locale", "none of LC_ALL, LC_MESSAGES or LANG is set");
        return Vec::new();
    };
    if normalise(&locale).is_none() {
        return Vec::new();
    }
    let mut names: Vec<String> = var("LANGUAGE")
        .map(|list| {
            list.split(':')
                .filter(|entry| !entry.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    names.push(locale);
    names
}

#[cfg(windows)]
fn raw() -> Vec<String> {
    const MUI_LANGUAGE_NAME: u32 = 0x8;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserPreferredUILanguages(
            flags: u32,
            count: *mut u32,
            buffer: *mut u16,
            buffer_len: *mut u32,
        ) -> i32;
    }

    let mut count = 0u32;
    let mut len = 0u32;
    let ok = unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut count,
            std::ptr::null_mut(),
            &mut len,
        )
    };
    if ok == 0 {
        crate::log_warn!(
            "locale",
            "GetUserPreferredUILanguages: {}",
            std::io::Error::last_os_error()
        );
        return Vec::new();
    }
    let mut buffer = vec![0u16; len as usize];
    let ok = unsafe {
        GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, buffer.as_mut_ptr(), &mut len)
    };
    if ok == 0 {
        crate::log_warn!(
            "locale",
            "GetUserPreferredUILanguages: {}",
            std::io::Error::last_os_error()
        );
        return Vec::new();
    }
    let names: Vec<String> = buffer
        .split(|&unit| unit == 0)
        .take_while(|name| !name.is_empty())
        .map(String::from_utf16_lossy)
        .collect();
    if names.is_empty() {
        crate::log_warn!("locale", "Windows lists no preferred UI language");
    }
    names
}

#[cfg(target_arch = "wasm32")]
fn raw() -> Vec<String> {
    let Some(navigator) = web_sys::window().map(|window| window.navigator()) else {
        crate::log_warn!("locale", "no window to ask for the language");
        return Vec::new();
    };
    let mut names: Vec<String> = navigator
        .languages()
        .iter()
        .filter_map(|name| name.as_string())
        .collect();
    if names.is_empty() {
        names.extend(navigator.language());
    }
    if names.is_empty() {
        crate::log_warn!("locale", "the browser reports no navigator.languages");
    }
    names
}

#[cfg(target_os = "android")]
fn raw() -> Vec<String> {
    use jni::{jni_sig, jni_str, objects::JString};

    let result = crate::platform::java::with_activity(|env, _activity| {
        let locale = env
            .call_static_method(
                jni_str!("java/util/Locale"),
                jni_str!("getDefault"),
                jni_sig!("()Ljava/util/Locale;"),
                &[],
            )?
            .l()?;
        let tag = env
            .call_method(
                &locale,
                jni_str!("toLanguageTag"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            )?
            .l()?;
        env.cast_local::<JString>(tag)?.try_to_string(env)
    });
    match result {
        Some(Ok(tag)) => vec![tag],
        Some(Err(e)) => {
            crate::log_warn!("locale", "Locale.getDefault: {e}");
            Vec::new()
        }
        None => {
            crate::log_warn!("locale", "no activity to ask for the locale yet");
            Vec::new()
        }
    }
}

#[cfg(not(any(unix, windows, target_arch = "wasm32")))]
fn raw() -> Vec<String> {
    crate::log_warn!("locale", "no system language source on this target");
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::normalise;

    #[test]
    fn locale_names_become_mojang_codes() {
        assert_eq!(normalise("ru_RU.UTF-8").as_deref(), Some("ru_ru"));
        assert_eq!(normalise("ru-RU").as_deref(), Some("ru_ru"));
        assert_eq!(normalise("ru").as_deref(), Some("ru"));
        assert_eq!(normalise("sr_RS@latin").as_deref(), Some("sr_rs"));
        assert_eq!(normalise("C.UTF-8"), None);
        assert_eq!(normalise("POSIX"), None);
        assert_eq!(normalise(""), None);
    }
}
