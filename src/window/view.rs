#![allow(deprecated)]
use adw::prelude::*;
use gio;
use gtk::glib;
use gtk::subclass::prelude::*;

use crate::model;

mod imp {
    use super::*;
    use adw::subclass::application_window::AdwApplicationWindowImpl;
    use gtk::CompositeTemplate;

    #[derive(CompositeTemplate)]
    #[template(file = "../../ui/window.ui")]
    pub struct VideoDownloaderWindow {
        #[template_child]
        pub main_stack_wdg: TemplateChild<gtk::Stack>,
        #[template_child]
        pub audio_url_wdg: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub video_url_wdg: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub resolution_wdg: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub prefer_mpeg_wdg: TemplateChild<gtk::Switch>,
        #[template_child]
        pub audio_video_stack_wdg: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub audio_download_wdg: TemplateChild<gtk::Button>,
        #[template_child]
        pub video_download_wdg: TemplateChild<gtk::Button>,
        #[template_child]
        pub download_cancel_wdg: TemplateChild<gtk::Button>,
        #[template_child]
        pub finished_download_dir_wdg: TemplateChild<gtk::Button>,
        #[template_child]
        pub finished_download_titles_wdg: TemplateChild<gtk::Label>,
        #[template_child]
        pub download_page_title_wdg: TemplateChild<gtk::Label>,
        #[template_child]
        pub download_title_wdg: TemplateChild<gtk::Label>,
        #[template_child]
        pub download_progress_wdg: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub download_info_wdg: TemplateChild<gtk::Label>,
        #[template_child]
        pub download_images_wdg: TemplateChild<gtk::Stack>,
        #[template_child]
        pub error_detail_wdg: TemplateChild<gtk::Label>,
        #[template_child]
        pub success_detail_wdg: TemplateChild<gtk::Label>,
        #[template_child]
        pub error_back_wdg: TemplateChild<gtk::Button>,
        #[template_child]
        pub success_back_wdg: TemplateChild<gtk::Button>,
        #[template_child]
        #[allow(deprecated)]
        pub audio_video_switcher_title_wdg: TemplateChild<adw::ViewSwitcherTitle>,
        #[template_child]
        pub error_details_expander_wdg: TemplateChild<gtk::Expander>,
        #[template_child]
        pub error_details_revealer_wdg: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub success_details_expander_wdg: TemplateChild<gtk::Expander>,
        #[template_child]
        pub success_details_revealer_wdg: TemplateChild<gtk::Revealer>,
        pub model: std::sync::Arc<std::sync::Mutex<Option<model::Model>>>,
    }

    impl Default for VideoDownloaderWindow {
        fn default() -> Self {
            Self {
                main_stack_wdg: Default::default(),
                audio_url_wdg: Default::default(),
                video_url_wdg: Default::default(),
                resolution_wdg: Default::default(),
                prefer_mpeg_wdg: Default::default(),
                audio_video_stack_wdg: Default::default(),
                audio_download_wdg: Default::default(),
                video_download_wdg: Default::default(),
                download_cancel_wdg: Default::default(),
                finished_download_dir_wdg: Default::default(),
                finished_download_titles_wdg: Default::default(),
                download_page_title_wdg: Default::default(),
                download_title_wdg: Default::default(),
                download_progress_wdg: Default::default(),
                download_info_wdg: Default::default(),
                download_images_wdg: Default::default(),
                error_detail_wdg: Default::default(),
                success_detail_wdg: Default::default(),
                error_back_wdg: Default::default(),
                success_back_wdg: Default::default(),
                audio_video_switcher_title_wdg: Default::default(),
                error_details_expander_wdg: Default::default(),
                error_details_revealer_wdg: Default::default(),
                success_details_expander_wdg: Default::default(),
                success_details_revealer_wdg: Default::default(),
                model: std::sync::Arc::new(std::sync::Mutex::new(None)),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VideoDownloaderWindow {
        const NAME: &'static str = "VideoDownloaderWindow";
        type Type = super::VideoDownloaderWindow;
        type ParentType = adw::ApplicationWindow;
        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for VideoDownloaderWindow {}
    impl WidgetImpl for VideoDownloaderWindow {}
    impl WindowImpl for VideoDownloaderWindow {}
    impl ApplicationWindowImpl for VideoDownloaderWindow {}
    impl AdwApplicationWindowImpl for VideoDownloaderWindow {}
}

glib::wrapper! {
    pub struct VideoDownloaderWindow(ObjectSubclass<imp::VideoDownloaderWindow>)
    @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager, gio::ActionGroup, gio::ActionMap;
}

impl VideoDownloaderWindow {
    pub fn new(app: &adw::Application) -> Self {
        let obj: Self = glib::Object::builder().property("application", app).build();
        let download_folder = std::env::var("HOME")
            .map(|h| format!("{}/Downloads", h))
            .unwrap_or_else(|_| "Downloads".to_string());
        let mut m = model::Model::new(&obj, download_folder);
        // Load settings if schema available
        if let Ok(source) =
            gio::SettingsSchemaSource::from_directory(env!("SCHEMA_DIR"), None, true)
        {
            if let Some(schema) = source.lookup("com.github.unrud.VideoDownloader", true) {
                let settings =
                    gio::Settings::new_full(&schema, Option::<&gio::SettingsBackend>::None, None);
                let v: String = settings.get("download-folder");
                m.download_folder = crate::utils::expand_path(&v);
                m.prefer_mpeg = settings.get("prefer-mpeg");
                let v: Vec<String> = settings.get("automatic-subtitles");
                m.automatic_subtitles = v;
                let v: String = settings.get("mode");
                m.mode = v;
                m.resolution = settings.get("resolution");
                m.settings = Some(settings);
            }
        }
        *obj.imp().model.lock().unwrap() = Some(m);
        obj.setup_actions();
        obj.sync_from_model();
        obj
    }

    pub fn model(&self) -> std::sync::Arc<std::sync::Mutex<Option<model::Model>>> {
        self.imp().model.clone()
    }

    fn show_thumbnail(&self, thumbnail: &str) {
        const IMAGE_SIZE: i32 = 128;
        const MAX_RATIO: f64 = 2.39;
        let w = (IMAGE_SIZE as f64 * MAX_RATIO).ceil() as i32;
        match gdk_pixbuf::Pixbuf::from_file_at_scale(thumbnail, w, IMAGE_SIZE, true) {
            Ok(pixbuf) => {
                let pic = gtk::Picture::for_pixbuf(&pixbuf);
                pic.set_height_request(IMAGE_SIZE);
                pic.set_content_fit(gtk::ContentFit::Contain);
                self.imp().download_images_wdg.add_child(&pic);
                self.imp().download_images_wdg.set_visible_child(&pic);
            }
            Err(_) => {
                let img = gtk::Image::builder()
                    .icon_name("video-x-generic")
                    .pixel_size(IMAGE_SIZE)
                    .height_request(IMAGE_SIZE)
                    .build();
                self.imp().download_images_wdg.add_child(&img);
                self.imp().download_images_wdg.set_visible_child(&img);
            }
        }
    }

    fn update_focus_and_default(&self) {
        let state = self
            .imp()
            .main_stack_wdg
            .visible_child_name()
            .unwrap_or_else(|| "start".into())
            .to_string();
        let mode = self
            .imp()
            .audio_video_stack_wdg
            .visible_child_name()
            .unwrap_or_else(|| "audio".into())
            .to_string();
        match state.as_str() {
            "start" => match mode.as_str() {
                "audio" => {
                    self.imp().audio_url_wdg.grab_focus();
                    self.set_default_widget(Some(
                        &self
                            .imp()
                            .audio_download_wdg
                            .clone()
                            .upcast::<gtk::Widget>(),
                    ));
                }
                "video" => {
                    self.imp().video_url_wdg.grab_focus();
                    self.set_default_widget(Some(
                        &self
                            .imp()
                            .video_download_wdg
                            .clone()
                            .upcast::<gtk::Widget>(),
                    ));
                }
                _ => {}
            },
            "download" | "cancel" => {
                self.imp().download_cancel_wdg.grab_focus();
            }
            "error" => {
                self.imp().error_back_wdg.grab_focus();
            }
            "success" => {
                self.imp().finished_download_dir_wdg.grab_focus();
            }
            _ => {}
        }
    }

    fn update_notification(&self, state: &str) {
        // Withdraw any existing notification.
        let app_id = self
            .application()
            .map(|a| {
                a.application_id()
                    .unwrap_or_else(|| "com.github.unrud.VideoDownloader".into())
            })
            .unwrap_or_else(|| "com.github.unrud.VideoDownloader".into());
        let _ = app_id;
        // Only notify when window is not active.
        if self.is_active() {
            return;
        }
        let (title, body, action) = match state {
            "error" => (
                crate::i18n::tr("Download failed"),
                String::new(),
                "app.notification-error",
            ),
            "success" => {
                let body = {
                    let binding = self.model();
                    let m = binding.lock().unwrap();
                    m.as_ref()
                        .map(|m| m.download_titles.join(" | "))
                        .unwrap_or_default()
                };
                (
                    crate::i18n::tr("Download finished"),
                    body,
                    "app.notification-success",
                )
            }
            _ => return,
        };
        if let Some(app) = self.application() {
            let notification = gio::Notification::new(&title);
            if !body.is_empty() {
                notification.set_body(Some(&body));
            }
            notification.set_default_action(action);
            if state == "success" {
                notification.add_button(
                    "Open Download Location",
                    "app.notification-open-finished-download-dir",
                );
            }
            app.send_notification(Some("video-downloader-notification"), &notification);
        }
    }

    fn setup_actions(&self) {
        // close
        let a = gio::SimpleAction::new("close", None);
        let w = self.clone();
        a.connect_activate(move |_, _| w.close());
        self.add_action(&a);

        let a = gio::SimpleAction::new("about", None);
        let w = self.clone();
        a.connect_activate(move |_, _| {
            crate::dialogs::about::show_about(&w);
        });
        self.add_action(&a);

        let a = gio::SimpleAction::new("shortcuts", None);
        let w = self.clone();
        a.connect_activate(move |_, _| {
            crate::dialogs::shortcuts::show(&w);
        });
        self.add_action(&a);

        let a = gio::SimpleAction::new("change-download-folder", None);
        let w = self.clone();
        a.connect_activate(move |_, _| {
            crate::dialogs::folder::change_download_folder(&w);
        });
        self.add_action(&a);

        let a = gio::SimpleAction::new(
            "set-audio-video-page",
            Some(&glib::VariantType::new("s").unwrap()),
        );
        let w = self.clone();
        a.connect_activate(move |_, param| {
            if let Some(p) = param {
                if let Some(s) = p.get::<String>() {
                    w.imp().audio_video_stack_wdg.set_visible_child_name(&s);
                }
            }
        });
        self.add_action(&a);

        let a = gio::SimpleAction::new("download", None);
        let w = self.clone();
        a.connect_activate(move |_, _| {
            if let Some(m) = w.imp().model.lock().unwrap().as_mut() {
                m.set_state("prepare");
            }
            w.sync_from_model();
        });
        self.add_action(&a);

        let a = gio::SimpleAction::new("cancel", None);
        let w = self.clone();
        a.connect_activate(move |_, _| {
            if let Some(m) = w.imp().model.lock().unwrap().as_mut() {
                m.set_state("cancel");
            }
            w.sync_from_model();
        });
        self.add_action(&a);

        let a = gio::SimpleAction::new("back", None);
        let w = self.clone();
        a.connect_activate(move |_, _| {
            if let Some(m) = w.imp().model.lock().unwrap().as_mut() {
                m.set_state("start");
            }
            w.sync_from_model();
        });
        self.add_action(&a);

        let a = gio::SimpleAction::new("open-finished-download-dir", None);
        let w = self.clone();
        a.connect_activate(move |_, _| {
            let binding = w.model();
            let m = binding.lock().unwrap();
            if let Some(m) = m.as_ref() {
                crate::utils::open_in_file_manager(&m.get_download_dir());
            }
        });
        self.add_action(&a);

        // Bidirectional UI <-> model bindings.
        let w = self.clone();
        self.imp()
            .audio_url_wdg
            .connect_notify(Some("text"), move |_, _| {
                let text = w.imp().audio_url_wdg.text().to_string();
                let binding = w.model();
                let mut m = binding.lock().unwrap();
                if let Some(m) = m.as_mut() {
                    m.url = text;
                }
            });
        let w = self.clone();
        self.imp()
            .video_url_wdg
            .connect_notify(Some("text"), move |_, _| {
                let text = w.imp().video_url_wdg.text().to_string();
                let binding = w.model();
                let mut m = binding.lock().unwrap();
                if let Some(m) = m.as_mut() {
                    m.url = text;
                }
            });
        let w = self.clone();
        self.imp()
            .resolution_wdg
            .connect_notify(Some("selected"), move |_, _| {
                let idx = w.imp().resolution_wdg.selected() as usize;
                let binding = w.model();
                let mut m = binding.lock().unwrap();
                if let Some(m) = m.as_mut() {
                    m.set_resolution_index(idx);
                }
            });
        let w = self.clone();
        self.imp()
            .prefer_mpeg_wdg
            .connect_notify(Some("active"), move |_, _| {
                let active = w.imp().prefer_mpeg_wdg.is_active();
                let binding = w.model();
                let mut m = binding.lock().unwrap();
                if let Some(m) = m.as_mut() {
                    m.prefer_mpeg = active;
                }
            });
        let w = self.clone();
        self.imp()
            .audio_video_stack_wdg
            .connect_notify(Some("visible-child-name"), move |_, _| {
                if let Some(name) = w.imp().audio_video_stack_wdg.visible_child_name() {
                    let binding = w.model();
                    let mut m = binding.lock().unwrap();
                    if let Some(m) = m.as_mut() {
                        m.mode = name.to_string();
                    }
                }
            });
        // success details activate_link -> open file manager for that file.
        let w = self.clone();
        self.imp()
            .success_detail_wdg
            .connect_activate_link(move |_, uri| {
                let binding = w.model();
                let dir = {
                    let m = binding.lock().unwrap();
                    m.as_ref().map(|m| m.get_download_dir()).unwrap_or_default()
                };
                if !dir.is_empty() {
                    let files = vec![uri.to_string()];
                    crate::utils::open_in_file_manager_with_files(&dir, &files);
                }
                glib::Propagation::Stop
            });
    }

    pub fn sync_from_model(&self) {
        let model = self.imp().model.lock().unwrap();
        let Some(m) = model.as_ref() else {
            return;
        };

        // main stack
        let child = match m.state.as_str() {
            "prepare" | "cancel" => "download",
            other => other,
        };
        self.imp().main_stack_wdg.set_visible_child_name(child);

        // url entries
        self.imp().audio_url_wdg.set_text(&m.url);
        self.imp().video_url_wdg.set_text(&m.url);

        // audio/video stack
        self.imp()
            .audio_video_stack_wdg
            .set_visible_child_name(&m.mode);

        // resolution combo
        if let Some(model_list) = self.imp().resolution_wdg.model() {
            if model_list.n_items() == 0 {
                if let Some(sl) = model_list.downcast_ref::<gtk::StringList>() {
                    for (_, desc) in &m.resolutions {
                        sl.append(desc);
                    }
                }
            }
        }
        self.imp()
            .resolution_wdg
            .set_selected(m.resolution_index() as u32);

        // prefer mpeg
        self.imp().prefer_mpeg_wdg.set_active(m.prefer_mpeg);

        // download page title with playlist position
        let page_title = if m.download_playlist_count > 1 {
            format!(
                "{} ({} of {})",
                crate::i18n::tr("Downloading"),
                m.download_playlist_index + 1,
                m.download_playlist_count
            )
        } else {
            crate::i18n::tr("Downloading")
        };
        self.imp().download_page_title_wdg.set_label(&page_title);

        // download title (fallback while loading)
        let title = if m.download_title.is_empty() {
            "…".to_string()
        } else {
            m.download_title.clone()
        };
        self.imp().download_title_wdg.set_label(&title);

        // progress bar
        if m.download_progress < 0.0 {
            self.imp().download_progress_wdg.pulse();
        } else {
            self.imp()
                .download_progress_wdg
                .set_fraction(m.download_progress.clamp(0.0, 1.0));
        }

        // download info: "H:MM:SS - X of Y (speed)"
        self.imp()
            .download_info_wdg
            .set_label(&format_download_info(m));

        // error / success details
        self.imp().error_detail_wdg.set_label(&m.error);
        let success_markup = m
            .finished_download_filenames
            .iter()
            .map(|s| {
                let esc = crate::utils::markup_escape(s);
                format!(
                    "<span baseline_shift=\"{}\"><a href=\"{}\">{}</a></span>",
                    -22 * 1024,
                    esc,
                    esc
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.imp().success_detail_wdg.set_use_markup(true);
        self.imp().success_detail_wdg.set_label(&success_markup);

        self.imp()
            .finished_download_titles_wdg
            .set_label(&m.download_titles.join(" | "));

        // thumbnail
        if !m.download_thumbnail.is_empty() {
            self.show_thumbnail(&m.download_thumbnail);
        }

        // tooltip for finished dir (shorten ~/...)
        let tooltip = format_path_for_tooltip(&m.finished_download_dir);
        self.imp()
            .finished_download_dir_wdg
            .set_tooltip_text(Some(&tooltip));

        // focus + default widget
        self.update_focus_and_default();

        // notifications
        self.update_notification(&m.state);

        m.save_settings();
    }
}

pub type Window = VideoDownloaderWindow;

unsafe impl Send for VideoDownloaderWindow {}
unsafe impl Sync for VideoDownloaderWindow {}

fn filesize_fmt(num: i64, suffix: &str) -> String {
    let mut n = num as f64;
    const UNITS: [&str; 9] = ["", "k", "M", "G", "T", "P", "E", "Z", "Y"];
    let mut unit = "";
    for u in UNITS {
        if n.abs() < 1000.0 {
            unit = u;
            break;
        }
        n /= 1000.0;
        unit = u;
    }
    format!("{:.1}\u{00A0}{}{}", n, unit, suffix)
}

fn format_download_info(m: &model::Model) -> String {
    let eta = m.download_eta;
    let time_msg = if eta >= 0 {
        let h = eta / 3600;
        let min = (eta / 60) % 60;
        let s = eta % 60;
        format!("{}∶{:02}:{:02}", h, min, s)
    } else {
        String::new()
    };
    let size_msg = if m.download_bytes >= 0 || m.download_bytes_total >= 0 {
        let a = if m.download_bytes >= 0 {
            filesize_fmt(m.download_bytes, "B")
        } else {
            "unknown".to_string()
        };
        let b = if m.download_bytes_total >= 0 {
            filesize_fmt(m.download_bytes_total, "B")
        } else {
            "unknown".to_string()
        };
        format!("{} of {}", a, b)
    } else {
        String::new()
    };
    let speed_msg = if m.download_speed >= 0 {
        filesize_fmt(m.download_speed, "B/\u{2060}s")
    } else {
        String::new()
    };
    let mut out = String::new();
    if !time_msg.is_empty() {
        out.push_str(&time_msg);
    }
    if !time_msg.is_empty() && (!size_msg.is_empty() || !speed_msg.is_empty()) {
        out.push_str("\u{00A0}- ");
    }
    out.push_str(&size_msg);
    if !size_msg.is_empty() && !speed_msg.is_empty() {
        out.push_str(&format!(" ({})", speed_msg));
    } else {
        out.push_str(&speed_msg);
    }
    out
}

fn format_path_for_tooltip(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    if let Ok(home) = std::env::var("HOME") {
        if path == home {
            return "~".to_string();
        }
        if path.starts_with(&format!("{}/", home)) {
            return format!("~{}", &path[home.len()..]);
        }
    }
    path.to_string()
}

/// Helper used by the model when the download folder is invalid.
pub fn show_download_folder_error(parent: &Window, message: &str, path: &str) {
    crate::dialogs::folder::show_error(
        parent,
        "Invalid download folder",
        message,
        Some(path),
        true,
    );
}
