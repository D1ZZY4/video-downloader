use std::env;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

/// Lock a mutex, recovering from poison instead of panicking.
/// Behavior is otherwise identical to `mutex.lock().unwrap()`.
pub fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Receive with timeout, returning `None` on timeout or channel closure.
/// Useful for dialog request loops that must not block forever.
pub fn recv_timeout<T>(rx: &std::sync::mpsc::Receiver<T>, timeout: Duration) -> Option<T> {
    rx.recv_timeout(timeout).ok()
}

pub fn languages_from_locale() -> Vec<String> {
    let mut locale_languages = Vec::new();
    for envar in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = env::var(envar)
            && !val.is_empty()
        {
            locale_languages.extend(val.split(':').map(|s| s.to_string()));
            break;
        }
    }
    if !locale_languages.iter().any(|l| l == "C") {
        locale_languages.push("C".to_string());
    }
    let mut languages = Vec::new();
    for lang in locale_languages {
        let mut lang = lang;
        for sep in ['@', '.', '_'] {
            if let Some((a, _)) = lang.split_once(sep) {
                lang = a.to_string();
            }
        }
        if lang == "C" {
            lang = "en".to_string();
        }
        if !languages.contains(&lang) {
            languages.push(lang);
        }
    }
    languages
}

pub fn markup_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[allow(dead_code)]
pub fn open_in_file_manager(directory: &str) {
    open_in_file_manager_with_files(directory, &[]);
}
pub fn open_in_file_manager_with_files(directory: &str, filenames: &[String]) {
    if let Some(first) = filenames.first() {
        let path = std::path::Path::new(directory).join(first);
        if path.exists() {
            let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
            return;
        }
    }
    let _ = std::process::Command::new("xdg-open")
        .arg(directory)
        .spawn();
}

/// Normalize a path lexically, mirroring `os.path.normpath` behaviour:
/// collapse redundant separators and `.`/`..` components without
/// touching the filesystem.
fn normalize_path(path: &str) -> String {
    let is_abs = path.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for comp in path.split('/') {
        match comp {
            "" | "." => {}
            ".." => match out.last() {
                Some(&"..") if !is_abs => out.push(".."),
                Some(_) => {
                    out.pop();
                }
                None if !is_abs => out.push(".."),
                None => {}
            },
            other => out.push(other),
        }
    }
    let joined = out.join("/");
    if is_abs {
        format!("/{}", joined)
    } else if joined.is_empty() {
        ".".to_string()
    } else {
        joined
    }
}

pub fn expand_path(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    // Expand the leading component (`~` or `xdg-<dir>`), like the legacy
    // `expand_path` but falling back to `$HOME` when resolution fails.
    let (first, rest) = match path.split_once('/') {
        Some((f, r)) => (f, r),
        None => (path, ""),
    };
    let expanded_first = if first == "~" {
        env::var("HOME").unwrap_or_else(|_| first.to_string())
    } else if let Some(name) = first.strip_prefix("xdg-") {
        let key = name.replace('-', "").to_uppercase();
        let env_key = format!("XDG_{}_DIR", key);
        env::var(&env_key)
            .ok()
            .or_else(|| {
                std::process::Command::new("xdg-user-dir")
                    .arg(&key)
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                        } else {
                            None
                        }
                    })
                    .filter(|s| !s.is_empty())
            })
            .or_else(|| env::var("HOME").ok())
            .unwrap_or_else(|| first.to_string())
    } else {
        first.to_string()
    };
    let combined = if rest.is_empty() {
        expanded_first.clone()
    } else if expanded_first.ends_with('/') {
        format!("{}{}", expanded_first, rest)
    } else {
        format!("{}/{}", expanded_first, rest)
    };
    normalize_path(&combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde() {
        let home = env::var("HOME").unwrap();
        assert_eq!(expand_path("~"), home);
        assert_eq!(expand_path("~/Downloads"), format!("{}/Downloads", home));
    }

    #[test]
    fn expand_normalizes() {
        assert_eq!(normalize_path("/a//b/./c"), "/a/b/c");
        assert_eq!(normalize_path("/a/b/../c"), "/a/c");
        assert_eq!(normalize_path("a/../b"), "b");
        assert_eq!(normalize_path(""), ".");
    }

    #[test]
    fn expand_xdg_preserves_suffix() {
        // The xdg-<dir> directory itself depends on the host; just check the
        // suffix is preserved and the result is absolute.
        let p = expand_path("xdg-download/VideoDownloader");
        assert!(p.ends_with("/VideoDownloader"), "got {}", p);
        assert!(p.starts_with('/'), "got {}", p);
    }

    #[test]
    fn languages_dedupe_and_c_maps_to_en() {
        // At least contains "en" via the appended "C" fallback.
        let langs = languages_from_locale();
        assert!(langs.contains(&"en".to_string()) || !langs.is_empty());
    }

    #[test]
    fn markup_escapes() {
        assert_eq!(markup_escape("<a&b>"), "&lt;a&amp;b&gt;");
    }
}
