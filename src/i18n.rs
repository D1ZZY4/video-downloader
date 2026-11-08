pub const DOMAIN: &str = "video-downloader";

pub fn init() {
    // Set locale from environment (setlocale(LC_ALL, "")).
    unsafe {
        libc::setlocale(libc::LC_ALL, std::ptr::null());
    }
    let locale_dir = option_env!("LOCALEDIR").unwrap_or("/usr/share/locale");
    gettextrs::bind_textdomain_codeset(DOMAIN, "UTF-8")
        .map(|_| ())
        .unwrap_or(());
    let _ = gettextrs::bindtextdomain(DOMAIN, locale_dir);
    let _ = gettextrs::textdomain(DOMAIN);
}

/// Translate `msgid` using the current textdomain.
/// Falls back to the input string when no translation is found.
pub fn tr(msgid: &str) -> String {
    gettextrs::gettext(msgid)
}
