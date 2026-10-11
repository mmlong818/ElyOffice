use crate::env;

/// The locale tag: `ELY_LOCALE`, else the OS one.
pub fn system() -> String {
    if let Some(tag) = env::value("ELY_LOCALE") {
        return tag;
    }
    platform()
}

#[cfg(target_os = "macos")]
fn platform() -> String {
    use objc2_foundation::NSLocale;
    if let Some(tag) = NSLocale::preferredLanguages()
        .firstObject()
        .map(|tag| tag.to_string())
        && !tag.is_empty()
    {
        return tag;
    }
    NSLocale::currentLocale()
        .localeIdentifier()
        .to_string()
        .replace('_', "-")
}

#[cfg(target_os = "linux")]
fn platform() -> String {
    resolve_env(env::value)
}

#[cfg(target_os = "windows")]
fn platform() -> String {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
    let mut buffer = [0u16; 85];
    let length = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
    if length == 0 {
        log::warn!("locale: GetUserDefaultLocaleName failed");
        return "en".into();
    }
    String::from_utf16_lossy(&buffer[..length as usize - 1])
}

/// First non-empty variable wins; C/POSIX ends as en.
#[cfg(any(target_os = "linux", test))]
fn resolve_env(get: impl Fn(&str) -> Option<String>) -> String {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(value) = get(key) else { continue };
        if value.is_empty() {
            continue;
        }
        let tag = value
            .split(['.', '@'])
            .next()
            .unwrap_or(&value)
            .replace('_', "-");
        return match tag.as_str() {
            "C" | "POSIX" => "en".into(),
            _ => tag,
        };
    }
    "en".into()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::resolve_env;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        move |key| map.get(key).cloned()
    }

    #[test]
    fn the_first_non_empty_variable_wins() {
        assert_eq!(resolve_env(env(&[("LANG", "zh_CN.UTF-8")])), "zh-CN");
        assert_eq!(
            resolve_env(env(&[
                ("LC_MESSAGES", "fr_FR.UTF-8"),
                ("LANG", "zh_CN.UTF-8")
            ])),
            "fr-FR"
        );
        assert_eq!(
            resolve_env(env(&[("LC_ALL", ""), ("LANG", "ja_JP.UTF-8")])),
            "ja-JP"
        );
    }

    #[test]
    fn c_and_posix_resolve_to_en_and_stop() {
        assert_eq!(
            resolve_env(env(&[("LC_ALL", "C.UTF-8"), ("LANG", "zh_CN.UTF-8")])),
            "en"
        );
        assert_eq!(
            resolve_env(env(&[("LC_MESSAGES", "POSIX"), ("LANG", "zh_CN.UTF-8")])),
            "en"
        );
        assert_eq!(
            resolve_env(env(&[("LC_ALL", "C"), ("LC_MESSAGES", "zh_CN.UTF-8")])),
            "en"
        );
    }

    #[test]
    fn nothing_set_resolves_to_en() {
        assert_eq!(resolve_env(env(&[])), "en");
        assert_eq!(resolve_env(env(&[("LANG", "")])), "en");
    }
}
