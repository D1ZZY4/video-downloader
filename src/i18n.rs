pub const DOMAIN: &str = "video-downloader";

pub fn init() {
    // Set locale from environment (setlocale(LC_ALL, "")).
    extern "C" {
        fn setlocale(category: i32, locale: *const i8) -> *const i8;
    }
    unsafe {
        setlocale(6, std::ptr::null());
    }
    let locale_dir = std::env::var("LOCALEDIR").unwrap_or_else(|_| "/usr/share/locale".to_string());
    gettextrs::bind_textdomain_codeset(DOMAIN, "UTF-8")
        .map(|_| ())
        .unwrap_or(());
    let _ = gettextrs::bindtextdomain(DOMAIN, &locale_dir);
    let _ = gettextrs::textdomain(DOMAIN);
}

/// Translate `msgid` using the current textdomain.
/// Falls back to the input string when no translation is found.
pub fn tr(msgid: &str) -> String {
    gettextrs::gettext(msgid)
}
