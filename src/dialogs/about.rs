use adw::prelude::*;

use crate::window::Window;

pub fn show_about(parent: &Window) {
    let version = env!("CARGO_PKG_VERSION");
    let dialog = adw::AboutDialog::builder()
        .application_name("Video Downloader")
        .application_icon("com.github.unrud.VideoDownloader")
        .version(version)
        .developer_name("Unrud")
        .developers(vec![
            "Unrud",
            "Adrien Plazas",
            "Gustavo Peredo",
            "Dave Patrick",
            "Artem Polishchuk",
            "Óscar Fernández Díaz",
        ])
        .artists(vec!["Tobias Bernard"])
        .website("https://github.com/unrud/video-downloader")
        .issue_url("https://github.com/unrud/video-downloader/issues")
        .debug_info_filename("video-downloader.txt")
        .debug_info(debug_info(version))
        .license_type(gtk::License::Gpl30)
        .build();
    dialog.present(Some(parent));
}

fn debug_info(version: &str) -> String {
    let os_name = gtk::glib::os_info("PRETTY_NAME")
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Linux".to_string());
    let mut lines = vec![
        format!("OS: {}", os_name),
        String::new(),
        "Libraries:".to_string(),
    ];
    lines.push("\tGLib 0.22".to_string());
    lines.push("\tGtk 0.11".to_string());
    lines.push("\tLibadwaita 0.9".to_string());
    lines.push(format!("\tvideo-downloader {}", version));
    lines.push("\tyt-dlp 2.10".to_string());
    lines.push(String::new());
    lines.push("Env:".to_string());
    for name in [
        "LANGUAGE",
        "LC_ALL",
        "LC_MESSAGES",
        "LANG",
        "G_MESSAGES_DEBUG",
    ] {
        if let Ok(val) = std::env::var(name) {
            lines.push(format!("\t{}={}", name, val));
        }
    }
    lines.join("\n")
}
