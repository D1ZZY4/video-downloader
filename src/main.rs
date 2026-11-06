mod dialogs {
    #[path = "about.rs"]
    pub mod about;
    #[path = "folder.rs"]
    pub mod folder;
    #[path = "login.rs"]
    pub mod login;
    #[path = "password.rs"]
    pub mod password;
    #[path = "playlist.rs"]
    pub mod playlist;
    #[path = "shortcuts.rs"]
    pub mod shortcuts;
}
#[path = "downloader/engine.rs"]
mod downloader;
mod i18n;
mod model;
#[path = "utils/helpers.rs"]
mod utils;
#[path = "window/view.rs"]
mod window;

use adw::prelude::*;
use window::Window;

fn main() {
    i18n::init();
    glib::set_application_name(&i18n::tr("Video Downloader"));
    // Ensure gtk/libadwaita init
    let app = adw::Application::builder()
        .application_id("com.github.unrud.VideoDownloader")
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
        if let Some(url) = options.lookup_value("url", Some(&glib::VariantType::new("s").unwrap())) {
            if let Some(s) = url.get::<String>() {
                let _ = app.register(Option::<&gio::Cancellable>::None);
                app.activate_action("new-window", Some(&s.to_variant()));
                return std::ops::ControlFlow::Break(glib::ExitCode::from(0));
            }
        }
        std::ops::ControlFlow::Continue(())
    });
    app.connect_activate(|app| {
        new_window(app, "");
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

    app.run();
}

fn new_window(app: &adw::Application, url: &str) {
    let window = Window::new(app);
    window.set_icon_name(Some("com.github.unrud.VideoDownloader"));
    {
        let model = window.model();
        let mut m = model.lock().unwrap();
        if let Some(m) = m.as_mut() {
            m.url = url.to_string();
        }
    }
    window.sync_from_model();
    window.present();
}
