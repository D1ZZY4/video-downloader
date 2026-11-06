use std::env;

pub fn languages_from_locale() -> Vec<String> {
    let mut locale_languages = Vec::new();
    for envar in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = env::var(envar) {
            if !val.is_empty() {
                locale_languages.extend(val.split(':').map(|s| s.to_string()));
                break;
            }
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
    let _ = std::process::Command::new("xdg-open").arg(directory).spawn();
}

pub fn expand_path(path: &str) -> String {
    let mut parts: Vec<String> = path.split('/').map(|s| s.to_string()).collect();
    if parts.is_empty() {
        return path.to_string();
    }
    if parts[0] == "~" {
        if let Ok(home) = env::var("HOME") {
            parts[0] = home;
        }
    } else if let Some(name) = parts[0].strip_prefix("xdg-") {
        let key = name.replace('-', "").to_uppercase();
        let env_key = format!("XDG_{}_DIR", key);
        let resolved = env::var(&env_key).ok().or_else(|| {
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
        });
        if let Some(val) = resolved {
            parts[0] = val;
        }
    }
    parts.join("/")
}
