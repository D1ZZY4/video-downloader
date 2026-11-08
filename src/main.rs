mod dialogs {
    #[path = "common.rs"]
    pub mod common;
    #[path = "folder.rs"]
    pub mod folder;
    #[path = "login.rs"]
    pub mod login;
    #[path = "password.rs"]
    pub mod password;
    #[path = "playlist.rs"]
    pub mod playlist;
    #[path = "quality_select.rs"]
    pub mod quality_select;
}
mod downloader {
    #[path = "download.rs"]
    pub mod download;
    #[path = "extract_metadata.rs"]
    pub mod extract_metadata;
    #[path = "quality.rs"]
    pub mod quality;
    #[path = "queue.rs"]
    pub mod queue;
    #[path = "thumbnail.rs"]
    pub mod thumbnail;
    pub use queue::DownloadEngine;
}
mod i18n;
mod model;
#[path = "utils/helpers.rs"]
mod utils;
#[path = "window/view.rs"]
mod window;

use adw::prelude::*;
use window::Window;

use std::sync::{Mutex, OnceLock};

static PENDING_URL: OnceLock<Mutex<String>> = OnceLock::new();

fn pending_url() -> &'static Mutex<String> {
    PENDING_URL.get_or_init(|| Mutex::new(String::new()))
}

fn main() -> glib::ExitCode {
    i18n::init();
    glib::set_application_name(&i18n::tr("Video Downloader"));
    // Ensure gtk/libadwaita init
    let app = adw::Application::builder()
        .application_id("com.github.unrud.VideoDownloader")
        .flags(gio::ApplicationFlags::empty())
        .build();

    app.add_main_option(
        "url",
        glib::Char::try_from('u').unwrap(),
        glib::OptionFlags::NONE,
        glib::OptionArg::String,
        &i18n::tr("Prefill URL field"),
        Some("URL"),
    );
    app.connect_handle_local_options(|app, options| {
        let Some(url) = options.lookup_value("url", Some(&glib::VariantType::new("s").unwrap()))
        else {
            return std::ops::ControlFlow::Continue(());
        };
        let Some(s) = url.get::<String>() else {
            return std::ops::ControlFlow::Continue(());
        };
        if !app.is_registered()
            && let Err(e) = app.register(Option::<&gio::Cancellable>::None)
        {
            eprintln!("Failed to register: {}", e);
            return std::ops::ControlFlow::Break(glib::ExitCode::from(1));
        }
        if app.is_remote() {
            app.activate_action("new-window", Some(&s.to_variant()));
            return std::ops::ControlFlow::Break(glib::ExitCode::from(0));
        }
        if let Ok(mut slot) = pending_url().lock() {
            *slot = s;
        }
        std::ops::ControlFlow::Continue(())
    });
    app.connect_activate(|app| {
        let url = pending_url()
            .lock()
            .map(|mut g| std::mem::take(&mut *g))
            .unwrap_or_default();
        new_window(app, &url);
    });
    app.connect_window_removed(|_, win| {
        win.destroy();
    });

    // new-window action with string param
    let app_clone = app.clone();
    let action = gio::SimpleAction::new("new-window", Some(&glib::VariantType::new("s").unwrap()));
    action.connect_activate(move |_, param| {
        let url = param
            .map(|p| p.get::<String>().unwrap_or_default())
            .unwrap_or_default();
        new_window(&app_clone, &url);
    });
    app.add_action(&action);

    let quit_action = gio::SimpleAction::new("quit", None);
    let app_clone = app.clone();
    quit_action.connect_activate(move |_, _| {
        for w in app_clone.windows() {
            w.close();
        }
    });
    app.add_action(&quit_action);

    let exit = app.run();
    if exit != glib::ExitCode::SUCCESS {
        eprintln!("Application exited with code {:?}", exit);
    }
    exit
}

fn new_window(app: &adw::Application, url: &str) {
    let window = Window::new(app);
    window.set_icon_name(Some("com.github.unrud.VideoDownloader"));
    {
        let model = window.model();
        let mut m = crate::utils::lock_or_recover(&model);
        if let Some(m) = m.as_mut() {
            m.url = url.to_string();
        }
    }
    window.sync_from_model();
    window.present();
}
