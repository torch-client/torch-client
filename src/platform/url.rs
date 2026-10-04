pub(crate) fn is_web_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && !url.chars().any(|c| c.is_control() || c == ' ')
}

pub(crate) fn open(url: &str) {
    if !is_web_url(url) {
        return;
    }
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window.open_with_url_and_target(url, "_blank");
        }
    }
    #[cfg(target_os = "android")]
    let _ = url;
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    native(url);
}

#[cfg(resource_packs)]
pub(crate) fn open_folder(path: &std::path::Path) {
    if let Err(e) = std::fs::create_dir_all(path) {
        crate::log_warn!("url", "could not create {}: {e}", path.display());
        return;
    }
    native(path.as_os_str());
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn native(url: impl AsRef<std::ffi::OsStr>) {
    let (program, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("open", &[])
    } else if cfg!(target_os = "windows") {
        ("cmd", &["/C", "start", ""])
    } else {
        ("xdg-open", &[])
    };
    let _ = std::process::Command::new(program)
        .args(args)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_http_urls_are_opened() {
        assert!(is_web_url("https://example.com/a?b=c"));
        assert!(is_web_url("http://example.com"));
        assert!(!is_web_url("file:///etc/passwd"));
        assert!(!is_web_url("javascript:alert(1)"));
        assert!(!is_web_url("https://example.com/a b"));
        assert!(!is_web_url("https://example.com/\na"));
    }
}
