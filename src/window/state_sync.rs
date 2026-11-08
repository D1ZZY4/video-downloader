//! Model to window synchronization.

use adw::prelude::*;
use gtk::subclass::prelude::*;

use crate::window::format_info::{format_download_info, format_path_for_tooltip};
use crate::window::window_builder::VideoDownloaderWindow;

impl VideoDownloaderWindow {
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

    fn sync_qualities(&self, qualities: &[(String, String)], quality_index: usize) {
        crate::dialogs::quality_select::sync_combo_row(
            &self.imp().audio_quality_wdg,
            qualities,
            quality_index,
        );
    }

    fn sync_resolutions(&self, resolutions: &[(u32, String)], resolution_index: usize) {
        let combo = &self.imp().resolution_wdg;
        if resolutions.is_empty() {
            // Hidden/disabled until a fetch succeeds.
            if let Some(model_list) = combo.model()
                && let Some(list) = model_list.downcast_ref::<gtk::StringList>()
            {
                // Rebuild as empty.
                combo.set_model(Some(&gtk::StringList::new(&[])));
                let _ = list;
            }
            combo.set_visible(false);
            combo.set_sensitive(false);
            return;
        }
        combo.set_visible(true);
        combo.set_sensitive(true);
        // Rebuild only when contents changed.
        let desired: Vec<&str> = resolutions.iter().map(|(_, d)| d.as_str()).collect();
        let needs_rebuild = match combo.model() {
            Some(model_list) => {
                if model_list.n_items() as usize != desired.len() {
                    true
                } else if let Some(list) = model_list.downcast_ref::<gtk::StringList>() {
                    desired.iter().enumerate().any(|(i, want)| {
                        list.string(i as u32).map(|s| s.to_string()) != Some((*want).to_string())
                    })
                } else {
                    true
                }
            }
            None => true,
        };
        if needs_rebuild {
            combo.set_model(Some(&gtk::StringList::new(&desired)));
        }
        combo.set_selected(resolution_index as u32);
    }

    pub fn sync_from_model(&self) {
        // Snapshot model fields under lock, then drop the guard before touching
        // widgets. Holding the guard across widget setters deadlocks because
        // notify callbacks (e.g. visible-child-name) re-acquire the same mutex.
        struct Snapshot {
            state: String,
            mode: String,
            url: String,
            resolutions: Vec<(u32, String)>,
            resolution_index: usize,
            audio_qualities: Vec<(String, String)>,
            audio_quality_index: usize,
            output_filename: String,
            prefer_mpeg: bool,
            download_playlist_index: i64,
            download_playlist_count: i64,
            download_title: String,
            download_progress: f64,
            download_bytes: i64,
            download_bytes_total: i64,
            download_eta: i64,
            download_speed: i64,
            error: String,
            finished_download_filenames: Vec<String>,
            download_titles: Vec<String>,
            download_thumbnail: String,
            finished_download_dir: String,
            settings_changed: bool,
        }
        let snapshot = {
            let guard = crate::utils::lock_or_recover(&self.imp().model);
            let Some(m) = guard.as_ref() else {
                return;
            };
            // Compare current settings against GSettings to gate save_settings:
            // avoid 5 dconf writes on every sync. Skip during progress-only
            // syncs (prepare/download/cancel) where settings cannot change.
            // (Skipping the reads also keeps the UI responsive while a
            // download syncs progress twice per second.)
            let settings_changed = if matches!(m.state.as_str(), "prepare" | "download" | "cancel")
            {
                false
            } else {
                match &m.settings {
                    Some(s) => {
                        let cur_folder: String = s.get("download-folder");
                        let cur_folder = crate::utils::expand_path(&cur_folder);
                        let cur_mpeg: bool = s.get("prefer-mpeg");
                        let cur_subs: Vec<String> = s.get("automatic-subtitles");
                        let cur_mode: String = s.get("mode");
                        let cur_res: u32 = {
                            let vq = s.value("video-quality");
                            if vq.type_().as_str() == "u" {
                                vq.get::<u32>().unwrap_or(crate::model::MAX_RESOLUTION)
                            } else if let Some(st) = vq.get::<String>() {
                                crate::model::Model::parse_video_quality(&st)
                            } else {
                                crate::model::MAX_RESOLUTION
                            }
                        };
                        // Guard: older system schemas lack `audio-quality`.
                        let audio_changed = if s.default_value("audio-quality").is_some() {
                            let cur_aq: String = s.get("audio-quality");
                            crate::model::Model::normalize_audio_quality(&cur_aq)
                                != crate::model::Model::normalize_audio_quality(&m.audio_quality)
                        } else {
                            false
                        };
                        cur_folder != m.download_folder
                            || cur_mpeg != m.prefer_mpeg
                            || cur_subs != m.automatic_subtitles
                            || cur_mode != m.mode
                            || cur_res != m.resolution
                            || audio_changed
                    }
                    None => false,
                }
            };
            Snapshot {
                state: m.state.clone(),
                mode: m.mode.clone(),
                url: m.url.clone(),
                resolutions: m.resolutions.clone(),
                resolution_index: m.resolution_index(),
                audio_qualities: m.audio_qualities.clone(),
                audio_quality_index: m.audio_quality_index(),
                output_filename: m.output_filename.clone(),
                prefer_mpeg: m.prefer_mpeg,
                download_playlist_index: m.download_playlist_index,
                download_playlist_count: m.download_playlist_count,
                download_title: m.download_title.clone(),
                download_progress: m.download_progress,
                download_bytes: m.download_bytes,
                download_bytes_total: m.download_bytes_total,
                download_eta: m.download_eta,
                download_speed: m.download_speed,
                error: m.error.clone(),
                finished_download_filenames: m.finished_download_filenames.clone(),
                download_titles: m.download_titles.clone(),
                download_thumbnail: m.download_thumbnail.clone(),
                finished_download_dir: m.finished_download_dir.clone(),
                settings_changed,
            }
        };

        // main stack
        let child = match snapshot.state.as_str() {
            "prepare" | "cancel" => "download",
            other => other,
        };
        self.imp().main_stack_wdg.set_visible_child_name(child);

        // url entries
        self.imp().audio_url_wdg.set_text(&snapshot.url);
        self.imp().video_url_wdg.set_text(&snapshot.url);
        // filename entries (custom output name)
        self.imp()
            .audio_filename_wdg
            .set_text(&snapshot.output_filename);
        self.imp()
            .video_filename_wdg
            .set_text(&snapshot.output_filename);

        // audio/video stack
        self.imp()
            .audio_video_stack_wdg
            .set_visible_child_name(&snapshot.mode);

        // resolution combo (hidden/disabled until fetched)
        self.sync_resolutions(&snapshot.resolutions, snapshot.resolution_index);

        // audio quality combo (hidden/disabled until fetched; URL-adaptive)
        self.sync_qualities(&snapshot.audio_qualities, snapshot.audio_quality_index);

        // all-formats buttons mirror the fetched state.
        let show_video = !snapshot.resolutions.is_empty();
        self.imp().video_formats_wdg.set_visible(show_video);
        self.imp().video_formats_wdg.set_sensitive(show_video);
        let show_audio = !snapshot.audio_qualities.is_empty();
        self.imp().audio_formats_wdg.set_visible(show_audio);
        self.imp().audio_formats_wdg.set_sensitive(show_audio);

        // prefer mpeg
        self.imp().prefer_mpeg_wdg.set_active(snapshot.prefer_mpeg);

        // download page title with playlist position
        let page_title = if snapshot.download_playlist_count > 1 {
            format!(
                "{} ({} of {})",
                crate::i18n::tr("Downloading"),
                snapshot.download_playlist_index + 1,
                snapshot.download_playlist_count
            )
        } else {
            crate::i18n::tr("Downloading")
        };
        self.imp().download_page_title_wdg.set_label(&page_title);

        // download title (fallback while loading)
        let title = if snapshot.download_title.is_empty() {
            "\u{2026}".to_string()
        } else {
            snapshot.download_title.clone()
        };
        self.imp().download_title_wdg.set_label(&title);

        // progress bar
        if snapshot.download_progress < 0.0 {
            self.imp().download_progress_wdg.pulse();
        } else {
            self.imp()
                .download_progress_wdg
                .set_fraction(snapshot.download_progress.clamp(0.0, 1.0));
        }

        // download info: "H:MM:SS - X of Y (speed)"
        self.imp()
            .download_info_wdg
            .set_label(&format_download_info(
                snapshot.download_bytes,
                snapshot.download_bytes_total,
                snapshot.download_eta,
                snapshot.download_speed,
            ));

        // error / success details
        self.imp().error_detail_wdg.set_label(&snapshot.error);
        let success_markup = snapshot
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
            .set_label(&snapshot.download_titles.join(" | "));

        // thumbnail
        if !snapshot.download_thumbnail.is_empty() {
            self.show_thumbnail(&snapshot.download_thumbnail);
        }

        // tooltip for finished dir (shorten ~/...)
        let tooltip = format_path_for_tooltip(&snapshot.finished_download_dir);
        self.imp()
            .finished_download_dir_wdg
            .set_tooltip_text(Some(&tooltip));

        // Action enablement, mirroring the legacy Model bindings:
        // download needs a URL + `start` state, cancel needs `download`,
        // back needs a valid `start` transition, open-finished needs success.
        let url_ok = !snapshot.url.trim().is_empty();
        let can_download = url_ok
            && crate::model::Model::is_valid_state_transition(Some(&snapshot.state), "prepare");
        let can_cancel =
            crate::model::Model::is_valid_state_transition(Some(&snapshot.state), "cancel");
        let can_back =
            crate::model::Model::is_valid_state_transition(Some(&snapshot.state), "start");
        let can_open = snapshot.state == "success";
        if let Some(a) = self.lookup_action("download")
            && let Some(s) = a.downcast_ref::<gio::SimpleAction>()
        {
            s.set_enabled(can_download);
        }
        if let Some(a) = self.lookup_action("cancel")
            && let Some(s) = a.downcast_ref::<gio::SimpleAction>()
        {
            s.set_enabled(can_cancel);
        }
        if let Some(a) = self.lookup_action("back")
            && let Some(s) = a.downcast_ref::<gio::SimpleAction>()
        {
            s.set_enabled(can_back);
        }
        if let Some(a) = self.lookup_action("open-finished-download-dir")
            && let Some(s) = a.downcast_ref::<gio::SimpleAction>()
        {
            s.set_enabled(can_open);
        }

        // focus + default widget
        self.update_focus_and_default();

        // notifications
        self.update_notification(&snapshot.state);

        // Persist settings only when they changed and we are not in a
        // progress-only sync (prepare/download/cancel).
        if snapshot.settings_changed
            && !matches!(snapshot.state.as_str(), "prepare" | "download" | "cancel")
        {
            let guard = crate::utils::lock_or_recover(&self.imp().model);
            if let Some(m) = guard.as_ref() {
                m.save_settings();
            }
        }
    }
}
