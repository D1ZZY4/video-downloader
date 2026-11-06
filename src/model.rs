use std::collections::HashSet;
use std::sync::Mutex;

use gio;
use gio::prelude::*;
use gtk::glib;
use gtk::glib::prelude::*;
use once_cell::sync::Lazy;

use crate::window::Window;

pub const MAX_RESOLUTION: u32 = (1 << 16) - 1;

pub struct Model {
    pub state: String,
    pub mode: String,
    pub url: String,
    pub error: String,
    pub resolution: u32,
    pub download_folder: String,
    pub finished_download_dir: String,
    pub finished_download_filenames: Vec<String>,
    pub automatic_subtitles: Vec<String>,
    pub prefer_mpeg: bool,
    pub download_playlist_index: i64,
    pub download_playlist_count: i64,
    pub download_filename: String,
    pub download_title: String,
    pub download_titles: Vec<String>,
    pub download_thumbnail: String,
    pub download_progress: f64,
    pub download_bytes: i64,
    pub download_bytes_total: i64,
    pub download_speed: i64,
    pub download_eta: i64,
    pub resolutions: Vec<(u32, String)>,
    pub window: glib::WeakRef<Window>,
    prev_state: Option<String>,
    pub active_download_lock: Option<String>,
    pub downloader: Option<crate::downloader::DownloadEngine>,
    pub settings: Option<gio::Settings>,
}

static GLOBAL_DOWNLOAD_LOCK: Lazy<Mutex<HashSet<String>>> =
    Lazy::new(|| Mutex::new(HashSet::new()));

impl Model {
    pub fn new(window: &Window, download_folder: String) -> Self {
        let mut resolutions = Vec::new();
        resolutions.push((MAX_RESOLUTION, "Best".to_string()));
        for (r, l) in [
            (4320u32, "4320p (8K)"),
            (2160, "2160p (4K)"),
            (1440, "1440p (HD)"),
            (1080, "1080p (HD)"),
            (720, "720p (HD)"),
            (480, "480p"),
            (360, "360p"),
            (240, "240p"),
            (144, "144p"),
        ] {
            resolutions.push((r, l.to_string()));
        }
        Self {
            state: "start".to_string(),
            mode: "audio".to_string(),
            url: String::new(),
            error: String::new(),
            resolution: 1080,
            download_folder,
            finished_download_dir: String::new(),
            finished_download_filenames: Vec::new(),
            automatic_subtitles: Vec::new(),
            prefer_mpeg: false,
            download_playlist_index: 0,
            download_playlist_count: 0,
            download_filename: String::new(),
            download_title: String::new(),
            download_titles: Vec::new(),
            download_thumbnail: String::new(),
            download_progress: -1.0,
            download_bytes: -1,
            download_bytes_total: -1,
            download_speed: -1,
            download_eta: -1,
            resolutions,
            window: window.downgrade(),
            prev_state: Some("start".to_string()),
            active_download_lock: None,
            downloader: None,
            settings: None,
        }
    }

    pub fn is_valid_state_transition(prev: Option<&str>, next: &str) -> bool {
        match next {
            "start" => !matches!(prev, Some("start") | Some("download")),
            "prepare" => prev == Some("start"),
            "download" => prev == Some("prepare"),
            "cancel" | "success" | "error" => prev == Some("download"),
            _ => false,
        }
    }

    pub fn set_state(&mut self, state: &str) {
        if self.prev_state.as_deref() == Some(state) {
            return;
        }
        if !Self::is_valid_state_transition(self.prev_state.as_deref(), state) {
            // revert
            if let Some(p) = &self.prev_state {
                self.state = p.clone();
            }
            return;
        }
        self.prev_state = Some(state.to_string());
        self.state = state.to_string();

        match state {
            "prepare" => {
                self.error.clear();
                self.download_playlist_index = 0;
                self.download_playlist_count = 0;
                self.download_filename.clear();
                self.download_title.clear();
                self.download_titles.clear();
                self.download_thumbnail.clear();
                self.download_progress = -1.0;
                self.download_bytes = -1;
                self.download_bytes_total = -1;
                self.download_speed = -1;
                self.download_eta = -1;
                self.finished_download_filenames.clear();
                self.finished_download_dir.clear();
                self.try_start_download();
            }
            "download" => {
                if let Some(window) = self.window.upgrade() {
                    let dl = crate::downloader::DownloadEngine::new(window.clone());
                    dl.start();
                    self.downloader = Some(dl);
                }
            }
            "cancel" => {
                if let Some(dl) = &self.downloader {
                    dl.cancel();
                }
            }
            _ => {}
        }
    }

    pub fn try_start_download(&mut self) {
        let path = crate::utils::expand_path(&self.download_folder);
        if let Some(msg) = check_download_dir(&path, true) {
            // Validation failed: inform the window (shows error dialog).
            // The dialog offers Change/Reset; on retry the caller re-enters prepare.
            if let Some(window) = self.window.upgrade() {
                let _ = crate::window::show_download_folder_error(&window, &msg, &path);
            }
            self.state = "start".to_string();
            self.prev_state = Some("start".to_string());
            return;
        }
        self.finished_download_dir = path;
        self.set_state("download");
    }

    pub fn resolution_index(&self) -> usize {
        self.resolutions
            .iter()
            .position(|(r, _)| *r == self.resolution)
            .unwrap_or(0)
    }

    pub fn set_resolution_index(&mut self, idx: usize) {
        if let Some((r, _)) = self.resolutions.get(idx) {
            self.resolution = *r;
        }
    }

    pub fn get_download_dir(&self) -> String {
        self.finished_download_dir.clone()
    }
    pub fn get_prefer_mpeg(&self) -> bool {
        self.prefer_mpeg
    }
    pub fn get_automatic_subtitles(&self) -> Vec<String> {
        let mut v = crate::utils::languages_from_locale();
        v.extend(self.automatic_subtitles.clone());
        v
    }
    pub fn get_url(&self) -> String {
        self.url.clone()
    }
    pub fn get_mode(&self) -> String {
        self.mode.clone()
    }
    pub fn get_resolution(&self) -> u32 {
        self.resolution
    }

    pub fn save_settings(&self) {
        if let Some(s) = &self.settings {
            let _ = s.set_string("download-folder", &self.download_folder);
            let _ = s.set_boolean("prefer-mpeg", self.prefer_mpeg);
            let v: Vec<&str> = self
                .automatic_subtitles
                .iter()
                .map(|s| s.as_str())
                .collect();
            let _ = s.set_strv("automatic-subtitles", v.as_slice());
            let _ = s.set_string("mode", &self.mode);
            let _ = s.set_uint("resolution", self.resolution);
        }
    }
    pub fn on_playlist_request(&mut self) -> bool {
        false
    }
    pub fn on_login_request(&mut self) -> (String, String) {
        (String::new(), String::new())
    }
    pub fn on_password_request(&mut self) -> String {
        String::new()
    }
    pub fn on_error(&mut self, msg: &str) {
        self.error = msg.to_string();
    }
    pub fn on_progress(
        &mut self,
        filename: &str,
        progress: f64,
        bytes_: i64,
        bytes_total: i64,
        eta: i64,
        speed: i64,
    ) {
        self.download_filename = filename.to_string();
        self.download_progress = progress;
        self.download_bytes = bytes_;
        self.download_bytes_total = bytes_total;
        self.download_eta = eta;
        self.download_speed = speed;
    }
    pub fn on_download_start(&mut self, playlist_index: i64, playlist_count: i64, title: &str) {
        self.download_playlist_index = playlist_index;
        self.download_playlist_count = playlist_count;
        self.download_title = title.to_string();
        self.download_titles.push(title.to_string());
    }
    pub fn on_download_lock(&mut self, name: &str) -> bool {
        if self.active_download_lock.is_some() {
            // already holds
        }
        let mut lock = GLOBAL_DOWNLOAD_LOCK.lock().unwrap();
        if lock.contains(name) {
            return false;
        }
        lock.insert(name.to_string());
        self.active_download_lock = Some(name.to_string());
        true
    }
    pub fn on_download_thumbnail(&mut self, thumbnail: &str) {
        self.download_thumbnail = thumbnail.to_string();
    }
    pub fn on_download_finished(&mut self, filename: &str) {
        if let Some(name) = self.active_download_lock.take() {
            let mut lock = GLOBAL_DOWNLOAD_LOCK.lock().unwrap();
            lock.remove(&name);
        }
        self.finished_download_filenames.push(filename.to_string());
    }
    pub fn on_pulse(&mut self) {}
    pub fn on_finished(&mut self, success: bool) {
        self.active_download_lock = None;
        if let Some(dl) = &self.downloader {
            dl.destroy();
        }
        self.downloader = None;
        if self.state == "cancel" {
            self.set_state("start");
        } else {
            self.set_state(if success { "success" } else { "error" });
        }
    }
}

/// Validate `path`. If `create` is true, create it (like `os.makedirs`).
/// Returns `Some(message)` on error, `None` on success.
pub fn check_download_dir(path: &str, create: bool) -> Option<String> {
    if create {
        match std::fs::create_dir_all(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Some(crate::i18n::tr("Not a directory"));
            }
            Err(_) => {
                return Some(crate::i18n::tr("Permission denied"));
            }
        }
    } else if !std::path::Path::new(path).is_dir() {
        return Some(crate::i18n::tr("Not a directory"));
    }
    // Permission check via metadata (Unix mode bits).
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mode = meta.mode() & 0o777;
            // Require read+write+execute for owner.
            if mode & 0o700 != 0o700 {
                return Some(crate::i18n::tr("Permission denied"));
            }
        }
    }
    None
}
