#![allow(deprecated)]
//! Focused builder: widget construction and actions.
use adw::prelude::*;
use gtk::glib;
use gtk::subclass::prelude::*;

use crate::model;

pub mod imp {
    use super::*;
    use adw::subclass::application_window::AdwApplicationWindowImpl;

    pub struct VideoDownloaderWindow {
        pub main_stack_wdg: gtk::Stack,
        pub audio_url_wdg: adw::EntryRow,
        pub audio_filename_wdg: adw::EntryRow,
        pub video_url_wdg: adw::EntryRow,
        pub video_filename_wdg: adw::EntryRow,
        pub resolution_wdg: adw::ComboRow,
        pub audio_quality_wdg: adw::ComboRow,
        pub video_formats_wdg: gtk::Button,
        pub audio_formats_wdg: gtk::Button,
        pub prefer_mpeg_wdg: gtk::Switch,
        pub audio_video_stack_wdg: adw::ViewStack,
        pub audio_download_wdg: gtk::Button,
        pub video_download_wdg: gtk::Button,
        pub download_cancel_wdg: gtk::Button,
        pub finished_download_dir_wdg: gtk::Button,
        pub finished_download_titles_wdg: gtk::Label,
        pub download_page_title_wdg: gtk::Label,
        pub download_title_wdg: gtk::Label,
        pub download_progress_wdg: gtk::ProgressBar,
        pub download_info_wdg: gtk::Label,
        pub download_images_wdg: gtk::Stack,
        pub error_detail_wdg: gtk::Label,
        pub success_detail_wdg: gtk::Label,
        pub error_back_wdg: gtk::Button,
        pub success_back_wdg: gtk::Button,
        #[allow(deprecated)]
        pub audio_video_switcher_title_wdg: adw::ViewSwitcherTitle,
        pub error_details_expander_wdg: adw::ExpanderRow,
        pub success_details_expander_wdg: adw::ExpanderRow,
        #[allow(clippy::arc_with_non_send_sync)]
        pub model: std::sync::Arc<std::sync::Mutex<Option<model::Model>>>,
        pub last_thumbnail: std::sync::Mutex<Option<String>>,
        pub notification_id: String,
        pub prefetch_seq: std::sync::Mutex<u64>,
        pub prefetch_scheduled_seq: std::sync::Mutex<u64>,
    }

    impl Default for VideoDownloaderWindow {
        #[allow(clippy::arc_with_non_send_sync)]
        fn default() -> Self {
            Self {
                main_stack_wdg: gtk::Stack::new(),
                audio_url_wdg: adw::EntryRow::new(),
                audio_filename_wdg: crate::window::filename::filename_row(),
                video_url_wdg: adw::EntryRow::new(),
                video_filename_wdg: crate::window::filename::filename_row(),
                resolution_wdg: adw::ComboRow::new(),
                audio_quality_wdg: adw::ComboRow::new(),
                video_formats_wdg: gtk::Button::new(),
                audio_formats_wdg: gtk::Button::new(),
                prefer_mpeg_wdg: gtk::Switch::new(),
                audio_video_stack_wdg: adw::ViewStack::new(),
                audio_download_wdg: gtk::Button::new(),
                video_download_wdg: gtk::Button::new(),
                download_cancel_wdg: gtk::Button::new(),
                finished_download_dir_wdg: gtk::Button::new(),
                finished_download_titles_wdg: gtk::Label::new(None),
                download_page_title_wdg: gtk::Label::new(None),
                download_title_wdg: gtk::Label::new(None),
                download_progress_wdg: gtk::ProgressBar::new(),
                download_info_wdg: gtk::Label::new(None),
                download_images_wdg: gtk::Stack::new(),
                error_detail_wdg: gtk::Label::new(None),
                success_detail_wdg: gtk::Label::new(None),
                error_back_wdg: gtk::Button::new(),
                success_back_wdg: gtk::Button::new(),
                audio_video_switcher_title_wdg: adw::ViewSwitcherTitle::new(),
                error_details_expander_wdg: adw::ExpanderRow::new(),
                success_details_expander_wdg: adw::ExpanderRow::new(),
                model: std::sync::Arc::new(std::sync::Mutex::new(None)),
                last_thumbnail: std::sync::Mutex::new(None),
                notification_id: glib::uuid_string_random().to_string(),
                prefetch_seq: std::sync::Mutex::new(0),
                prefetch_scheduled_seq: std::sync::Mutex::new(0),
            }
        }
    }

    fn bold_attributes() -> gtk::pango::AttrList {
        let list = gtk::pango::AttrList::new();
        list.insert(gtk::pango::AttrInt::new_weight(gtk::pango::Weight::Bold));
        list.insert(gtk::pango::AttrFloat::new_scale(1.125_f64));
        list
    }

    fn make_shortcut(
        trigger_str: &str,
        action_str: &str,
        args: Option<&glib::Variant>,
    ) -> gtk::Shortcut {
        let trigger = gtk::ShortcutTrigger::parse_string(trigger_str);
        let action = gtk::ShortcutAction::parse_string(action_str);
        let shortcut = gtk::Shortcut::new(trigger, action);
        if args.is_some() {
            shortcut.set_arguments(args);
        }
        shortcut
    }

    fn make_shortcut_controller(
        scope: gtk::ShortcutScope,
        shortcuts: Vec<gtk::Shortcut>,
    ) -> gtk::ShortcutController {
        let controller = gtk::ShortcutController::new();
        controller.set_scope(scope);
        for shortcut in shortcuts {
            controller.add_shortcut(shortcut);
        }
        controller
    }

    fn configure_entry_row(row: &adw::EntryRow) {
        row.set_title(&crate::i18n::tr("URL"));
        row.set_activates_default(true);
        row.set_input_hints(
            gtk::InputHints::NO_SPELLCHECK | gtk::InputHints::NO_EMOJI | gtk::InputHints::NONE,
        );
        row.set_input_purpose(gtk::InputPurpose::Url);
        // Paste suffix (clipboard), like the Filename row.
        let btn = gtk::Button::builder()
            .icon_name("edit-paste-symbolic")
            .tooltip_text(crate::i18n::tr("Paste"))
            .build();
        btn.add_css_class("flat");
        btn.set_can_focus(false);
        let entry = row.clone();
        btn.connect_clicked(move |_| {
            let entry = entry.clone();
            entry
                .clipboard()
                .read_text_async(Option::<&gio::Cancellable>::None, move |res| {
                    if let Ok(Some(text)) = res {
                        entry.set_text(&text);
                    }
                });
        });
        row.add_suffix(&btn);
    }

    fn configure_download_button(button: &gtk::Button) {
        button.set_label(&crate::i18n::tr("Download"));
        button.set_receives_default(true);
        button.set_halign(gtk::Align::Center);
        button.set_action_name(Some("win.download"));
        button.add_css_class("suggested-action");
        button.add_css_class("pill");
    }

    fn configure_back_button(button: &gtk::Button) {
        button.set_action_name(Some("win.back"));
        let back_text = crate::i18n::tr("Back");
        button.set_tooltip_text(Some(&back_text));
        button.update_property(&[gtk::accessible::Property::Label(&back_text)]);
        let controller = make_shortcut_controller(
            gtk::ShortcutScope::Managed,
            vec![make_shortcut("<Alt>Left|Back", "activate", None)],
        );
        button.add_controller(controller);
        let image = gtk::Image::builder()
            .icon_name("go-previous-symbolic")
            .icon_size(gtk::IconSize::Normal)
            .build();
        button.set_child(Some(&image));
        button.add_css_class("image-button");
    }

    fn build_menu_model() -> gio::Menu {
        let menu = gio::Menu::new();
        let first_section = gio::Menu::new();
        let new_window_label = crate::i18n::tr("New Window");
        let empty = glib::Variant::from("");
        let new_window_item = gio::MenuItem::new(Some(&new_window_label), Some("app.new-window"));
        new_window_item.set_action_and_target_value(Some("app.new-window"), Some(&empty));
        first_section.append_item(&new_window_item);
        menu.append_section(None, &first_section);
        let second_section = gio::Menu::new();
        let change_label = crate::i18n::tr("Change Download Location");
        second_section.append_item(&gio::MenuItem::new(
            Some(&change_label),
            Some("win.change-download-folder"),
        ));
        let shortcuts_label = crate::i18n::tr("Keyboard Shortcuts");
        second_section.append_item(&gio::MenuItem::new(
            Some(&shortcuts_label),
            Some("win.shortcuts"),
        ));
        let about_label = crate::i18n::tr("About Video Downloader");
        second_section.append_item(&gio::MenuItem::new(Some(&about_label), Some("win.about")));

        menu.append_section(None, &second_section);
        menu
    }

    fn build_global_shortcut_controller() -> gtk::ShortcutController {
        make_shortcut_controller(
            gtk::ShortcutScope::Global,
            vec![
                make_shortcut(
                    "<Primary>N",
                    "action(app.new-window)",
                    Some(&glib::Variant::from("")),
                ),
                make_shortcut("<Primary>W", "action(win.close)", None),
                make_shortcut("<Primary>Q", "action(app.quit)", None),
                make_shortcut("<Primary>question", "action(win.shortcuts)", None),
            ],
        )
    }

    fn build_details_section(
        expander: &adw::ExpanderRow,
        detail_label: &gtk::Label,
        with_link_class: bool,
    ) {
        expander.set_title(&crate::i18n::tr("Details"));
        expander.set_enable_expansion(true);
        expander.set_expanded(false);
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .has_frame(false)
            .vexpand(true)
            .build();
        detail_label.add_css_class("body");
        if with_link_class {
            detail_label.add_css_class("link");
        }
        detail_label.set_selectable(true);
        detail_label.set_xalign(0.0);
        detail_label.set_yalign(0.0);
        detail_label.set_wrap(true);
        detail_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        detail_label.set_margin_bottom(6);
        detail_label.set_margin_start(6);
        detail_label.set_margin_end(6);
        detail_label.set_margin_top(6);
        scrolled.set_child(Some(detail_label));
        let row = gtk::ListBoxRow::builder().build();
        row.set_child(Some(&scrolled));
        expander.add_row(&row);
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VideoDownloaderWindow {
        const NAME: &'static str = "VideoDownloaderWindow";
        type Type = super::VideoDownloaderWindow;
        type ParentType = adw::ApplicationWindow;
    }

    impl ObjectImpl for VideoDownloaderWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            self.construct_ui(&obj);
        }
    }
    impl WidgetImpl for VideoDownloaderWindow {}
    impl WindowImpl for VideoDownloaderWindow {}
    impl ApplicationWindowImpl for VideoDownloaderWindow {}
    impl AdwApplicationWindowImpl for VideoDownloaderWindow {}
    impl VideoDownloaderWindow {
        fn construct_ui(&self, obj: &super::VideoDownloaderWindow) {
            let window_title = crate::i18n::tr("Video Downloader");
            obj.set_title(Some(&window_title));
            obj.set_default_width(720);
            obj.set_content(Some(&self.main_stack_wdg));
            self.main_stack_wdg
                .set_transition_type(gtk::StackTransitionType::SlideLeftRight);
            obj.add_controller(build_global_shortcut_controller());

            // Configure standalone widgets.
            configure_entry_row(&self.audio_url_wdg);
            configure_entry_row(&self.video_url_wdg);
            configure_download_button(&self.audio_download_wdg);
            configure_download_button(&self.video_download_wdg);
            self.resolution_wdg
                .set_title(&crate::i18n::tr("Video Quality"));
            self.resolution_wdg
                .set_model(Some(&gtk::StringList::new(&[])));
            // Hidden/disabled until the URL has been fetched successfully.
            self.resolution_wdg.set_visible(false);
            self.resolution_wdg.set_sensitive(false);
            // Audio quality adapts to the URL; hidden until fetched.
            self.audio_quality_wdg
                .set_title(&crate::i18n::tr("Audio Quality"));
            self.audio_quality_wdg
                .set_model(Some(&gtk::StringList::new(&[])));
            self.audio_quality_wdg.set_visible(false);
            self.audio_quality_wdg.set_sensitive(false);
            // Advanced all-formats pickers (grid dialog), hidden until fetch.
            self.video_formats_wdg
                .set_label(&crate::i18n::tr("All formats..."));
            self.video_formats_wdg.set_visible(false);
            self.video_formats_wdg.set_sensitive(false);
            self.video_formats_wdg.add_css_class("pill");
            self.audio_formats_wdg
                .set_label(&crate::i18n::tr("All formats..."));
            self.audio_formats_wdg.set_visible(false);
            self.audio_formats_wdg.set_sensitive(false);
            self.audio_formats_wdg.add_css_class("pill");
            self.prefer_mpeg_wdg.set_valign(gtk::Align::Center);
            self.audio_video_stack_wdg.set_vexpand(true);
            self.audio_video_switcher_title_wdg
                .set_stack(Some(&self.audio_video_stack_wdg));
            // Python template: switcher-title.title <- window.title.
            let title_binding = obj
                .bind_property("title", &self.audio_video_switcher_title_wdg, "title")
                .sync_create()
                .build();
            std::mem::forget(title_binding);
            self.download_cancel_wdg
                .set_label(&crate::i18n::tr("Cancel"));
            self.download_cancel_wdg.set_action_name(Some("win.cancel"));
            self.download_page_title_wdg.set_label("Downloading XXX");
            self.download_page_title_wdg
                .set_attributes(Some(&bold_attributes()));
            self.download_title_wdg.set_label("Title XXX");
            self.download_title_wdg
                .set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            self.download_images_wdg
                .set_transition_type(gtk::StackTransitionType::SlideUpDown);
            self.download_info_wdg.set_valign(gtk::Align::Center);
            self.download_info_wdg.set_wrap(true);
            self.download_info_wdg
                .set_justify(gtk::Justification::Center);
            self.download_info_wdg.set_label("Info XXX");
            self.download_info_wdg.add_css_class("dim-label");
            configure_back_button(&self.error_back_wdg);
            configure_back_button(&self.success_back_wdg);
            self.finished_download_titles_wdg.set_label("Title XXX");
            self.finished_download_titles_wdg
                .set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            self.finished_download_dir_wdg
                .set_label(&crate::i18n::tr("Open Download Location"));
            self.finished_download_dir_wdg
                .set_halign(gtk::Align::Center);
            self.finished_download_dir_wdg
                .set_action_name(Some("win.open-finished-download-dir"));
            self.success_detail_wdg.set_use_markup(true);

            // Start page.
            let start_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .build();
            let start_shortcuts = make_shortcut_controller(
                gtk::ShortcutScope::Managed,
                vec![
                    make_shortcut(
                        "<Alt>A",
                        "action(win.set-audio-video-page)",
                        Some(&glib::Variant::from("audio")),
                    ),
                    make_shortcut(
                        "<Alt>V",
                        "action(win.set-audio-video-page)",
                        Some(&glib::Variant::from("video")),
                    ),
                ],
            );
            start_box.add_controller(start_shortcuts);

            let start_header = adw::HeaderBar::builder()
                .centering_policy(adw::CenteringPolicy::Strict)
                .build();
            start_header.set_title_widget(Some(&self.audio_video_switcher_title_wdg));
            let menu_model = build_menu_model();
            let popover = gtk::PopoverMenu::builder().menu_model(&menu_model).build();
            let main_menu_text = crate::i18n::tr("Main Menu");
            let menu_button = gtk::MenuButton::builder()
                .tooltip_text(main_menu_text)
                .direction(gtk::ArrowType::None)
                .primary(true)
                .popover(&popover)
                .build();
            start_header.pack_end(&menu_button);
            start_box.append(&start_header);

            crate::window::audio::add_audio_page(
                &self.audio_video_stack_wdg,
                &self.audio_url_wdg,
                &self.audio_filename_wdg,
                &self.audio_quality_wdg,
                &self.audio_formats_wdg,
                &self.audio_download_wdg,
            );
            crate::window::video::add_video_page(
                &self.audio_video_stack_wdg,
                &self.video_url_wdg,
                &self.video_filename_wdg,
                &self.resolution_wdg,
                &self.prefer_mpeg_wdg,
                &self.video_formats_wdg,
                &self.video_download_wdg,
            );
            start_box.append(&self.audio_video_stack_wdg);
            let switcher_bar = adw::ViewSwitcherBar::builder()
                .stack(&self.audio_video_stack_wdg)
                .build();
            // Python template: switcher-bar.reveal <- title.title-visible.
            // `title-visible` is read-only, so it must be the binding source
            // (binding to it as target leaks a null GBinding -> abort).
            let reveal_binding = self
                .audio_video_switcher_title_wdg
                .bind_property("title-visible", &switcher_bar, "reveal")
                .sync_create()
                .build();
            std::mem::forget(reveal_binding);
            start_box.append(&switcher_bar);

            // Download page.
            let download_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .build();
            let download_header = adw::HeaderBar::builder().build();
            download_header.pack_start(&self.download_cancel_wdg);
            download_box.append(&download_header);
            let download_content = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(24)
                .margin_start(18)
                .margin_end(18)
                .margin_top(18)
                .margin_bottom(18)
                .build();
            download_content.append(&self.download_page_title_wdg);
            let download_details = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(18)
                .build();
            download_details.append(&self.download_title_wdg);
            let images_clamp = adw::Clamp::builder()
                .orientation(gtk::Orientation::Vertical)
                .maximum_size(0)
                .build();
            images_clamp.set_child(Some(&self.download_images_wdg));
            download_details.append(&images_clamp);
            let progress_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(6)
                .build();
            progress_box.append(&self.download_progress_wdg);
            progress_box.append(&self.download_info_wdg);
            download_details.append(&progress_box);
            download_content.append(&download_details);
            let download_clamp = adw::Clamp::builder()
                .valign(gtk::Align::Center)
                .vexpand(true)
                .build();
            download_clamp.set_child(Some(&download_content));
            download_box.append(&download_clamp);

            // Error page.
            let error_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .build();
            let error_header = adw::HeaderBar::builder().build();
            error_header.pack_start(&self.error_back_wdg);
            error_box.append(&error_header);
            let error_content = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(24)
                .margin_start(18)
                .margin_end(18)
                .margin_top(18)
                .margin_bottom(18)
                .build();
            let error_status = adw::StatusPage::builder()
                .icon_name("dialog-warning-symbolic")
                .title(crate::i18n::tr("Download failed"))
                .description(crate::i18n::tr(
                    "Something went wrong. Check the details below.",
                ))
                .vexpand(true)
                .build();
            error_content.append(&error_status);
            let error_details_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(8)
                .vexpand(true)
                .build();
            error_details_box.append(&self.error_details_expander_wdg);
            build_details_section(
                &self.error_details_expander_wdg,
                &self.error_detail_wdg,
                false,
            );
            error_status.set_child(Some(&error_details_box));
            let error_clamp = adw::Clamp::builder().vexpand(true).build();
            error_clamp.set_child(Some(&error_content));
            error_box.append(&error_clamp);

            // Success page.
            let success_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .build();
            let success_header = adw::HeaderBar::builder().build();
            success_header.pack_start(&self.success_back_wdg);
            success_box.append(&success_header);
            let success_content = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(24)
                .margin_start(18)
                .margin_end(18)
                .margin_top(18)
                .margin_bottom(18)
                .build();
            let success_status = adw::StatusPage::builder()
                .icon_name("document-save-symbolic")
                .title(crate::i18n::tr("Download finished"))
                .description(crate::i18n::tr("Your files are ready to open."))
                .vexpand(true)
                .build();
            success_content.append(&success_status);
            let success_details_outer = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(18)
                .build();
            success_details_outer.append(&self.finished_download_titles_wdg);
            success_details_outer.append(&self.finished_download_dir_wdg);
            self.finished_download_dir_wdg
                .set_halign(gtk::Align::Center);
            let success_details_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(8)
                .vexpand(true)
                .build();
            success_details_box.append(&self.success_details_expander_wdg);
            build_details_section(
                &self.success_details_expander_wdg,
                &self.success_detail_wdg,
                true,
            );
            success_details_outer.append(&success_details_box);
            success_status.set_child(Some(&success_details_outer));
            let success_clamp = adw::Clamp::builder().vexpand(true).build();
            success_clamp.set_child(Some(&success_content));
            success_box.append(&success_clamp);

            self.main_stack_wdg.add_named(&start_box, Some("start"));
            self.main_stack_wdg
                .add_named(&download_box, Some("download"));
            self.main_stack_wdg.add_named(&error_box, Some("error"));
            self.main_stack_wdg.add_named(&success_box, Some("success"));
        }
    }
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
        let source = gio::SettingsSchemaSource::from_directory(env!("SCHEMA_DIR"), None, true)
            .ok()
            .or_else(gio::SettingsSchemaSource::default);
        if let Some(source) = source
            && let Some(schema) = source.lookup("com.github.unrud.VideoDownloader", true)
        {
            let settings =
                gio::Settings::new_full(&schema, Option::<&gio::SettingsBackend>::None, None);
            let v: String = settings.get("download-folder");
            m.download_folder = crate::utils::expand_path(&v);
            m.prefer_mpeg = settings.get("prefer-mpeg");
            let v: Vec<String> = settings.get("automatic-subtitles");
            m.automatic_subtitles = v;
            let v: String = settings.get("mode");
            m.mode = v;
            // `output-filename` is empty on older schemas: default_value guard.
            if settings.default_value("output-filename").is_some() {
                let v: String = settings.get("output-filename");
                m.output_filename = v;
            }
            if settings.default_value("audio-format").is_some() {
                let v: String = settings.get("audio-format");
                m.audio_format_ext = v;
            }
            // `video-quality` is now a string (`best` or px), like audio.
            // Older installs stored a uint; accept both without panicking.
            let vq = settings.value("video-quality");
            if vq.type_().as_str() == "u" {
                m.resolution = vq.get::<u32>().unwrap_or(model::MAX_RESOLUTION);
            } else if let Some(s) = vq.get::<String>() {
                m.resolution = model::Model::parse_video_quality(&s);
            } else {
                m.resolution = model::MAX_RESOLUTION;
            }
            // Guard: system schemas from older installs lack `audio-quality`.
            if settings.default_value("audio-quality").is_some() {
                let v: String = settings.get("audio-quality");
                m.audio_quality = model::Model::normalize_audio_quality(&v);
            }
            m.settings = Some(settings);
        }
        *crate::utils::lock_or_recover(&obj.imp().model) = Some(m);
        obj.setup_actions();
        obj.sync_from_model();
        obj
    }

    #[allow(clippy::arc_with_non_send_sync)]
    pub fn model(&self) -> std::sync::Arc<std::sync::Mutex<Option<model::Model>>> {
        self.imp().model.clone()
    }

    /// Register per-window notification actions on the application so the
    /// notification buttons do something (present the window / open the
    /// folder). Mirrors legacy `app.notification-*-<uuid>`.
    fn register_notification_actions(&self) {
        let Some(app) = self.application() else {
            return;
        };
        let id = self.imp().notification_id.clone();
        let weak = self.downgrade();
        let w = weak.clone();
        let a = gio::SimpleAction::new(&format!("notification-success--{}", id), None);
        a.connect_activate(move |_, _| {
            if let Some(win) = w.upgrade() {
                win.present();
            }
        });
        app.add_action(&a);
        let w = weak.clone();
        let a = gio::SimpleAction::new(&format!("notification-error--{}", id), None);
        a.connect_activate(move |_, _| {
            if let Some(win) = w.upgrade() {
                win.present();
            }
        });
        app.add_action(&a);
        let w = weak;
        let a = gio::SimpleAction::new(
            &format!("notification-open-finished-download-dir--{}", id),
            None,
        );
        a.connect_activate(move |_, _| {
            if let Some(win) = w.upgrade() {
                let binding = win.model();
                let (dir, files) = {
                    let m = crate::utils::lock_or_recover(&binding);
                    match m.as_ref() {
                        Some(m) => (m.get_download_dir(), m.finished_download_filenames.clone()),
                        None => (String::new(), Vec::new()),
                    }
                };
                if !dir.is_empty() {
                    crate::utils::open_in_file_manager_with_files(&dir, &files);
                }
            }
        });
        app.add_action(&a);
    }

    fn setup_actions(&self) {
        self.register_notification_actions();
        self.connect_close_request(move |w| {
            if let Some(app) = w.application() {
                let id = w.imp().notification_id.clone();
                app.withdraw_notification(&id);
            }
            glib::Propagation::Proceed
        });
        // close
        let a = gio::SimpleAction::new("close", None);
        let w = self.clone();
        a.connect_activate(move |_, _| w.close());
        self.add_action(&a);

        let a = gio::SimpleAction::new("about", None);
        let w = self.clone();
        a.connect_activate(move |_, _| {
            crate::window::about::show_about(&w);
        });
        self.add_action(&a);

        let a = gio::SimpleAction::new("shortcuts", None);
        let w = self.clone();
        a.connect_activate(move |_, _| {
            crate::window::shortcuts::show(&w);
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
            if let Some(p) = param
                && let Some(s) = p.get::<String>()
            {
                w.imp().audio_video_stack_wdg.set_visible_child_name(&s);
            }
        });
        self.add_action(&a);

        let a = gio::SimpleAction::new("download", None);
        let w = self.clone();
        a.connect_activate(move |_, _| {
            if let Some(m) = crate::utils::lock_or_recover(&w.imp().model).as_mut() {
                m.set_state("prepare");
            }
            w.sync_from_model();
        });
        self.add_action(&a);

        let a = gio::SimpleAction::new("cancel", None);
        let w = self.clone();
        a.connect_activate(move |_, _| {
            if let Some(m) = crate::utils::lock_or_recover(&w.imp().model).as_mut() {
                m.set_state("cancel");
            }
            w.sync_from_model();
        });
        self.add_action(&a);

        let a = gio::SimpleAction::new("back", None);
        let w = self.clone();
        a.connect_activate(move |_, _| {
            if let Some(m) = crate::utils::lock_or_recover(&w.imp().model).as_mut() {
                m.set_state("start");
            }
            w.sync_from_model();
        });
        self.add_action(&a);

        let a = gio::SimpleAction::new("open-finished-download-dir", None);
        let w = self.clone();
        a.connect_activate(move |_, _| {
            let binding = w.model();
            let (dir, files) = {
                let m = crate::utils::lock_or_recover(&binding);
                match m.as_ref() {
                    Some(m) => (m.get_download_dir(), m.finished_download_filenames.clone()),
                    None => (String::new(), Vec::new()),
                }
            };
            if !dir.is_empty() {
                crate::utils::open_in_file_manager_with_files(&dir, &files);
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
                let mut m = crate::utils::lock_or_recover(&binding);
                if let Some(m) = m.as_mut() {
                    if text != m.url {
                        m.on_fetch_failed();
                    }
                    m.url = text;
                }
                drop(m);
                // Debounced auto-fetch so qualities appear without Enter.
                crate::window::prefetch::schedule_refresh(&w);
            });
        let w = self.clone();
        self.imp().audio_url_wdg.connect_activate(move |_| {
            crate::window::prefetch::refresh_resolutions(&w);
        });
        let w = self.clone();
        self.imp()
            .audio_filename_wdg
            .connect_notify(Some("text"), move |_, _| {
                let text = w.imp().audio_filename_wdg.text().to_string();
                let binding = w.model();
                let mut m = crate::utils::lock_or_recover(&binding);
                if let Some(m) = m.as_mut() {
                    m.output_filename = text;
                }
            });
        let w = self.clone();
        self.imp()
            .video_filename_wdg
            .connect_notify(Some("text"), move |_, _| {
                let text = w.imp().video_filename_wdg.text().to_string();
                let binding = w.model();
                let mut m = crate::utils::lock_or_recover(&binding);
                if let Some(m) = m.as_mut() {
                    m.output_filename = text;
                }
            });
        let w = self.clone();
        self.imp()
            .video_url_wdg
            .connect_notify(Some("text"), move |_, _| {
                let text = w.imp().video_url_wdg.text().to_string();
                let binding = w.model();
                let mut m = crate::utils::lock_or_recover(&binding);
                if let Some(m) = m.as_mut() {
                    if text != m.url {
                        m.on_fetch_failed();
                    }
                    m.url = text;
                }
                drop(m);
                // Debounced auto-fetch so resolutions appear without Enter.
                crate::window::prefetch::schedule_refresh(&w);
            });
        let w = self.clone();
        self.imp().video_url_wdg.connect_activate(move |_| {
            crate::window::prefetch::refresh_resolutions(&w);
        });
        let w = self.clone();
        self.imp()
            .resolution_wdg
            .connect_notify(Some("selected"), move |_, _| {
                let idx = w.imp().resolution_wdg.selected() as usize;
                let binding = w.model();
                let mut m = crate::utils::lock_or_recover(&binding);
                if let Some(m) = m.as_mut() {
                    m.set_resolution_index(idx);
                }
            });
        // Advanced all-formats pickers (grid dialog).
        let w = self.clone();
        self.imp().video_formats_wdg.connect_clicked(move |_| {
            let (meta, current) = {
                let binding = w.model();
                let m = crate::utils::lock_or_recover(&binding);
                match m.as_ref() {
                    Some(m) if m.resolution_fetched && !m.fetched_video_options.is_empty() => {
                        let meta = crate::downloader::extract_metadata::FetchedMetadata {
                            title: m.fetched_title.clone(),
                            channel: m.fetched_channel.clone(),
                            duration: m.fetched_duration,
                            thumbnail_url: m.fetched_thumbnail.clone(),
                            video_options: m.fetched_video_options.clone(),
                            audio_options: m.fetched_audio_options.clone(),
                        };
                        let cur = m.resolution;
                        (meta, cur)
                    }
                    _ => return,
                }
            };
            let labels: Vec<String> = meta.video_options.iter().map(|o| o.label.clone()).collect();
            let video_opts = meta.video_options.clone();
            let wcb = w.clone();
            crate::dialogs::quality_select::show_grid(
                &w,
                &crate::i18n::tr("Choose video format"),
                &meta,
                labels,
                Vec::new(),
                move |chosen| {
                    if let Some(chosen_label) = chosen
                        && let Some(opt) = video_opts.iter().find(|o| o.label == chosen_label)
                    {
                        let binding = wcb.model();
                        let mut m = crate::utils::lock_or_recover(&binding);
                        if let Some(m) = m.as_mut() {
                            m.resolution = opt.height;
                            m.video_container = opt.ext.clone();
                            m.prefer_mpeg = opt.ext == "mp4";
                        }
                        drop(m);
                        wcb.sync_from_model();
                    }
                },
            );
            let _ = current;
        });
        let w = self.clone();
        self.imp().audio_formats_wdg.connect_clicked(move |_| {
            let meta = {
                let binding = w.model();
                let m = crate::utils::lock_or_recover(&binding);
                match m.as_ref() {
                    Some(m) if m.audio_quality_fetched && !m.fetched_audio_options.is_empty() => {
                        crate::downloader::extract_metadata::FetchedMetadata {
                            title: m.fetched_title.clone(),
                            channel: m.fetched_channel.clone(),
                            duration: m.fetched_duration,
                            thumbnail_url: m.fetched_thumbnail.clone(),
                            video_options: m.fetched_video_options.clone(),
                            audio_options: m.fetched_audio_options.clone(),
                        }
                    }
                    _ => return,
                }
            };
            let labels: Vec<String> = meta.audio_options.iter().map(|o| o.label.clone()).collect();
            let audio_opts = meta.audio_options.clone();
            let wcb = w.clone();
            crate::dialogs::quality_select::show_grid(
                &w,
                &crate::i18n::tr("Choose audio format"),
                &meta,
                Vec::new(),
                labels,
                move |chosen| {
                    if let Some(chosen_label) = chosen
                        && let Some(opt) = audio_opts.iter().find(|o| o.label == chosen_label)
                    {
                        let binding = wcb.model();
                        let mut m = crate::utils::lock_or_recover(&binding);
                        if let Some(m) = m.as_mut() {
                            m.audio_format_ext = opt.ext.clone();
                        }
                        drop(m);
                        wcb.sync_from_model();
                    }
                },
            );
        });
        let w = self.clone();
        self.imp()
            .audio_quality_wdg
            .connect_notify(Some("selected"), move |_, _| {
                let idx = w.imp().audio_quality_wdg.selected() as usize;
                let binding = w.model();
                let mut m = crate::utils::lock_or_recover(&binding);
                if let Some(m) = m.as_mut() {
                    m.set_audio_quality_index(idx);
                }
            });
        let w = self.clone();
        self.imp()
            .prefer_mpeg_wdg
            .connect_notify(Some("active"), move |_, _| {
                let active = w.imp().prefer_mpeg_wdg.is_active();
                let binding = w.model();
                let mut m = crate::utils::lock_or_recover(&binding);
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
                    let mut m = crate::utils::lock_or_recover(&binding);
                    if let Some(m) = m.as_mut() {
                        m.mode = name.to_string();
                    }
                }
                crate::window::prefetch::schedule_refresh(&w);
            });
        // success details activate_link -> open file manager for that file.
        let w = self.clone();
        self.imp()
            .success_detail_wdg
            .connect_activate_link(move |_, uri| {
                let binding = w.model();
                let dir = {
                    let m = crate::utils::lock_or_recover(&binding);
                    m.as_ref().map(|m| m.get_download_dir()).unwrap_or_default()
                };
                if !dir.is_empty() {
                    let files = vec![uri.to_string()];
                    crate::utils::open_in_file_manager_with_files(&dir, &files);
                }
                glib::Propagation::Stop
            });
    }
}
