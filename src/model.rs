use std::collections::HashSet;
use std::sync::Mutex;

use gio::prelude::*;
use gtk::glib;
use once_cell::sync::Lazy;

use crate::window::Window;

pub const MAX_RESOLUTION: u32 = (1 << 16) - 1;
/// Height cap used for the synthetic "Best" entry (4320 is the largest quality known).
pub const BEST_HEIGHT_CAP: u32 = 4320;
/// Preferred fallback height when the previous selection is no longer available.
pub const DEFAULT_RESOLUTION: u32 = 1080;

pub struct Model {
    pub state: String,
    pub mode: String,
    pub url: String,
    /// Custom output filename (base name). Empty = use the video title.
    pub output_filename: String,
    pub error: String,
    pub resolution: u32,
    pub download_folder: String,
    pub finished_download_dir: String,
    pub finished_download_filenames: Vec<String>,
    pub automatic_subtitles: Vec<String>,
    pub prefer_mpeg: bool,
    /// Audio quality tier: best/high/medium/low/worst.
    pub audio_quality: String,
    /// Selected audio container (mp3/opus/flac/...). `mp3` = legacy behaviour.
    pub audio_format_ext: String,
    /// Selected video container preference (`mp4`/`webm`/`""` = any).
    pub video_container: String,
    /// Fetched metadata card (title/duration/thumb) for the start page.
    pub fetched_title: String,
    pub fetched_channel: String,
    pub fetched_duration: i64,
    pub fetched_thumbnail: String,
    /// All format combinations the fetched URL has (grid dialog source).
    pub fetched_video_options: Vec<crate::downloader::extract_metadata::VideoOption>,
    pub fetched_audio_options: Vec<crate::downloader::extract_metadata::AudioOption>,
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
    /// Resolution choices shown in the ComboRow. Empty until a fetch succeeds.
    pub resolutions: Vec<(u32, String)>,
    /// Heights reported by the fetched video formats, sorted descending.
    pub available_heights: Vec<u32>,
    /// Audio quality choices (value = `best` or bitrate kbps, label for UI).
    /// Empty until a fetch succeeds; derived from the URL's actual formats.
    pub audio_qualities: Vec<(String, String)>,
    /// Bitrates reported by the fetched video formats, sorted descending.
    pub available_bitrates: Vec<u32>,
    /// True only after a video fetch has succeeded.
    pub resolution_fetched: bool,
    /// True only after a video fetch has succeeded (audio choices present).
    pub audio_quality_fetched: bool,
    pub window: glib::WeakRef<Window>,
    prev_state: Option<String>,
    pub active_download_lock: Option<String>,
    pub downloader: Option<crate::downloader::DownloadEngine>,
    pub settings: Option<gio::Settings>,
    /// Set when a download was blocked by an invalid folder; the next
    /// successful folder change/reset should retry starting the download.
    pub pending_folder_retry: bool,
}

static GLOBAL_DOWNLOAD_LOCK: Lazy<Mutex<HashSet<String>>> =
    Lazy::new(|| Mutex::new(HashSet::new()));

impl Model {
    pub fn new(window: &Window, download_folder: String) -> Self {
        Self {
            state: "start".to_string(),
            mode: "audio".to_string(),
            url: String::new(),
            output_filename: String::new(),
            error: String::new(),
            resolution: MAX_RESOLUTION,
            download_folder,
            finished_download_dir: String::new(),
            finished_download_filenames: Vec::new(),
            automatic_subtitles: Vec::new(),
            prefer_mpeg: false,
            audio_quality: "best".to_string(),
            audio_format_ext: "mp3".to_string(),
            video_container: String::new(),
            fetched_title: String::new(),
            fetched_channel: String::new(),
            fetched_duration: -1,
            fetched_thumbnail: String::new(),
            fetched_video_options: Vec::new(),
            fetched_audio_options: Vec::new(),
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
            resolutions: Vec::new(),
            available_heights: Vec::new(),
            audio_qualities: Vec::new(),
            available_bitrates: Vec::new(),
            resolution_fetched: false,
            audio_quality_fetched: false,
            window: window.downgrade(),
            prev_state: Some("start".to_string()),
            active_download_lock: None,
            downloader: None,
            settings: None,
            pending_folder_retry: false,
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
            self.pending_folder_retry = true;
            if let Some(window) = self.window.upgrade() {
                crate::window::show_download_folder_error(&window, &msg, &path);
            }
            self.state = "start".to_string();
            self.prev_state = Some("start".to_string());
            return;
        }
        self.pending_folder_retry = false;
        self.finished_download_dir = path;
        self.set_state("download");
    }

    /// Retry a download that was blocked by an invalid folder, if pending.
    /// Returns true when a retry was started.
    pub fn retry_pending_folder_download(&mut self) -> bool {
        if !self.pending_folder_retry {
            return false;
        }
        self.pending_folder_retry = false;
        self.set_state("prepare");
        true
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

    pub fn audio_quality_options() -> Vec<(String, String)> {
        crate::downloader::quality::audio_quality_options()
    }

    pub fn audio_quality_index(&self) -> usize {
        let list = if self.audio_qualities.is_empty() {
            Self::audio_quality_options()
        } else {
            self.audio_qualities.clone()
        };
        list.iter()
            .position(|(v, _)| *v == self.audio_quality)
            .unwrap_or(0)
    }

    pub fn set_audio_quality_index(&mut self, idx: usize) {
        let list = if self.audio_qualities.is_empty() {
            Self::audio_quality_options()
        } else {
            self.audio_qualities.clone()
        };
        if let Some((v, _)) = list.get(idx) {
            self.audio_quality = v.clone();
        }
    }

    /// Normalize a raw `audio-quality` string (schema/UI) to `best` or a
    /// bitrate in kbps (e.g. `"128"`). Legacy tier names map to bitrates.
    pub fn normalize_audio_quality(raw: &str) -> String {
        crate::downloader::quality::normalize_audio_quality(raw)
    }

    /// Parse a stored `video-quality` string (`best` or a height in px)
    /// to the internal height. `best` maps to `MAX_RESOLUTION`.
    pub fn parse_video_quality(raw: &str) -> u32 {
        let t = raw.trim();
        if t == "best" {
            return MAX_RESOLUTION;
        }
        t.parse::<u32>().unwrap_or(MAX_RESOLUTION)
    }

    /// Format an internal height for storage as `video-quality`.
    pub fn video_quality_string(height: u32) -> String {
        if height == MAX_RESOLUTION {
            "best".to_string()
        } else {
            height.to_string()
        }
    }

    /// Human readable label for a height, e.g. "1080p (HD)".
    pub fn resolution_label(height: u32) -> String {
        crate::downloader::quality::resolution_label(height)
    }

    /// Normalize raw heights: drop zero, cap at 4320, sort descending, dedupe.
    pub fn normalize_heights(heights: &[u32]) -> Vec<u32> {
        crate::downloader::quality::normalize_heights(heights)
    }

    /// Build the ComboRow choices from heights. Always includes Best first.
    pub fn build_resolutions(heights: &[u32]) -> Vec<(u32, String)> {
        crate::downloader::quality::build_resolutions(heights)
    }

    /// Heights available in a fetched video's formats.
    pub fn heights_from_video(video: &yt_dlp::model::Video) -> Vec<u32> {
        crate::downloader::quality::heights_from_video(video)
    }

    /// Pick the resolution to select from a newly built list.
    /// Keeps `prev` when present, else 1080 when present, else Best.
    pub fn choose_resolution(prev: u32, resolutions: &[(u32, String)]) -> u32 {
        crate::downloader::quality::choose_resolution(prev, resolutions)
    }

    /// Populate resolutions from the heights reported by a fetched video.
    /// Called from the downloader/queue callback after a successful fetch.
    /// Keeps the current selection when still available, else falls back to
    /// 1080 when present, else Best (index 0).
    pub fn set_available_heights(&mut self, heights: Vec<u32>) {
        let prev = self.resolution;
        self.available_heights = Self::normalize_heights(&heights);
        self.resolutions = Self::build_resolutions(&self.available_heights);
        self.resolution_fetched = true;
        self.resolution = Self::choose_resolution(prev, &self.resolutions);
    }

    /// Normalize raw bitrates: drop zero, round, sort descending, dedupe.
    pub fn normalize_bitrates(rates: &[f64]) -> Vec<u32> {
        crate::downloader::quality::normalize_bitrates(rates)
    }

    /// Build the ComboRow choices from bitrates. Always includes Best first.
    pub fn build_audio_qualities(bitrates: &[u32]) -> Vec<(String, String)> {
        crate::downloader::quality::build_audio_qualities(bitrates)
    }

    /// Bitrates available in a fetched video's audio formats.
    pub fn bitrates_from_video(video: &yt_dlp::model::Video) -> Vec<u32> {
        crate::downloader::quality::bitrates_from_video(video)
    }

    /// Pick the audio quality to select from a newly built list.
    /// Keeps `prev` when present, else Best.
    pub fn choose_audio_quality(prev: &str, qualities: &[(String, String)]) -> String {
        crate::downloader::quality::choose_audio_quality(prev, qualities)
    }

    /// Populate audio qualities from the bitrates reported by a fetched video.
    /// Keeps the current selection when still available, else Best.
    pub fn set_available_audio_bitrates(&mut self, bitrates: Vec<u32>) {
        let prev = self.audio_quality.clone();
        self.available_bitrates =
            Self::normalize_bitrates(&bitrates.iter().map(|b| *b as f64).collect::<Vec<_>>());
        self.audio_qualities = Self::build_audio_qualities(&self.available_bitrates);
        self.audio_quality_fetched = true;
        self.audio_quality = Self::choose_audio_quality(&prev, &self.audio_qualities);
    }

    /// Callback for the downloader/queue after a video was fetched.
    /// Populates both video resolutions and audio qualities from the URL.
    pub fn on_video_fetched(&mut self, video: &yt_dlp::model::Video) {
        let heights = Self::heights_from_video(video);
        self.set_available_heights(heights);
        let bitrates = Self::bitrates_from_video(video);
        self.set_available_audio_bitrates(bitrates);
        let meta = crate::downloader::extract_metadata::from_video(video);
        self.fetched_title = meta.title;
        self.fetched_channel = meta.channel;
        self.fetched_duration = meta.duration;
        self.fetched_thumbnail = meta.thumbnail_url;
        self.fetched_video_options = meta.video_options;
        self.fetched_audio_options = meta.audio_options;
    }

    /// Clear fetched qualities (fetch failed or URL changed).
    /// Stays hidden/disabled; the error path is shown as before.
    pub fn on_fetch_failed(&mut self) {
        self.resolutions.clear();
        self.available_heights.clear();
        self.resolution_fetched = false;
        self.audio_qualities.clear();
        self.available_bitrates.clear();
        self.audio_quality_fetched = false;
        self.fetched_title.clear();
        self.fetched_channel.clear();
        self.fetched_duration = -1;
        self.fetched_thumbnail.clear();
        self.fetched_video_options.clear();
        self.fetched_audio_options.clear();
    }

    /// Whether the resolution ComboRow should be visible.
    pub fn resolutions_visible(&self) -> bool {
        self.resolution_fetched && !self.resolutions.is_empty()
    }

    /// Whether the audio quality ComboRow should be visible.
    pub fn audio_qualities_visible(&self) -> bool {
        self.audio_quality_fetched && !self.audio_qualities.is_empty()
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
    pub fn get_output_filename(&self) -> String {
        self.output_filename.clone()
    }
    pub fn get_resolution(&self) -> u32 {
        self.resolution
    }
    pub fn get_audio_quality(&self) -> String {
        Self::normalize_audio_quality(&self.audio_quality)
    }
    pub fn get_video_container(&self) -> String {
        self.video_container.clone()
    }
    pub fn get_audio_format_ext(&self) -> String {
        let e = self.audio_format_ext.to_ascii_lowercase();
        if e.is_empty() || e == "bin" {
            "mp3".to_string()
        } else {
            e
        }
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
            let _ = s.set_string(
                "video-quality",
                &Self::video_quality_string(self.resolution),
            );
            let _ = s.set_string(
                "audio-quality",
                &Self::normalize_audio_quality(&self.audio_quality),
            );
            let _ = s.set_string("output-filename", &self.output_filename);
            let _ = s.set_string("audio-format", &self.audio_format_ext);
        }
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
        let mut lock = crate::utils::lock_or_recover(&GLOBAL_DOWNLOAD_LOCK);
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
            crate::utils::lock_or_recover(&GLOBAL_DOWNLOAD_LOCK).remove(&name);
        }
        self.finished_download_filenames.push(filename.to_string());
    }
    pub fn on_finished(&mut self, success: bool) {
        if let Some(name) = self.active_download_lock.take() {
            crate::utils::lock_or_recover(&GLOBAL_DOWNLOAD_LOCK).remove(&name);
        }
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
#[allow(unreachable_code)]
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
    // Effective-access check (R/W/X), like Python's `os.access` with
    // effective IDs. Falls back to mode bits when the syscall is unavailable.
    #[cfg(unix)]
    {
        if let Ok(cstr) = std::ffi::CString::new(path) {
            // SAFETY: cstr is NUL-free; eaccess/access only read the string.
            let denied =
                unsafe { libc::access(cstr.as_ptr(), libc::R_OK | libc::W_OK | libc::X_OK) != 0 };
            if denied {
                return Some(crate::i18n::tr("Permission denied"));
            }
            return None;
        }
        // Path contains a NUL byte; can't query, treat as invalid.
        return Some(crate::i18n::tr("Not a directory"));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_with_resolutions() {
        // Model::new needs a window; test via pure helpers for the instance logic.
        let _ = Model::normalize_heights(&[1080, 720]);
    }

    #[test]
    fn normalize_sorts_dedupes_caps() {
        assert_eq!(
            Model::normalize_heights(&[360, 1080, 720, 1080, 0, 9999]),
            vec![4320, 1080, 720, 360]
        );
    }

    #[test]
    fn build_resolutions_always_has_best_first() {
        let res = Model::build_resolutions(&[720, 1080]);
        assert_eq!(res[0].0, MAX_RESOLUTION);
        assert_eq!(res[1].0, 1080);
        assert_eq!(res[2].0, 720);
    }

    #[test]
    fn build_resolutions_empty_gives_best_only() {
        let res = Model::build_resolutions(&[]);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].0, MAX_RESOLUTION);
    }

    #[test]
    fn labels() {
        assert_eq!(Model::resolution_label(1080), "1080p (HD)");
        assert_eq!(Model::resolution_label(480), "480p");
    }

    #[test]
    fn helper_compiles() {
        model_with_resolutions();
    }

    #[test]
    fn choose_preserves_valid() {
        let res = Model::build_resolutions(&[1080, 720]);
        assert_eq!(Model::choose_resolution(720, &res), 720);
    }

    #[test]
    fn choose_falls_back_to_1080() {
        let res = Model::build_resolutions(&[1080, 720]);
        assert_eq!(Model::choose_resolution(1440, &res), 1080);
    }

    #[test]
    fn choose_falls_back_to_best() {
        let res = Model::build_resolutions(&[480, 360]);
        assert_eq!(Model::choose_resolution(1080, &res), MAX_RESOLUTION);
    }

    #[test]
    fn state_transitions_match_legacy() {
        assert!(Model::is_valid_state_transition(Some("start"), "prepare"));
        assert!(Model::is_valid_state_transition(
            Some("prepare"),
            "download"
        ));
        assert!(Model::is_valid_state_transition(Some("download"), "cancel"));
        assert!(Model::is_valid_state_transition(
            Some("download"),
            "success"
        ));
        assert!(Model::is_valid_state_transition(Some("download"), "error"));
        assert!(Model::is_valid_state_transition(Some("error"), "start"));
        assert!(Model::is_valid_state_transition(Some("success"), "start"));
        assert!(Model::is_valid_state_transition(Some("cancel"), "start"));
        assert!(!Model::is_valid_state_transition(Some("start"), "download"));
        assert!(!Model::is_valid_state_transition(Some("start"), "start"));
        assert!(!Model::is_valid_state_transition(None, "prepare"));
        assert!(!Model::is_valid_state_transition(Some("start"), "cancel"));
    }

    #[test]
    fn audio_quality_normalize_and_options() {
        assert_eq!(Model::normalize_audio_quality("high"), "192");
        assert_eq!(Model::normalize_audio_quality("128"), "128");
        assert_eq!(Model::normalize_audio_quality("bogus"), "best");
        let opts = Model::audio_quality_options();
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].0, "best");
    }

    #[test]
    fn video_quality_parse() {
        assert_eq!(Model::parse_video_quality("best"), MAX_RESOLUTION);
        assert_eq!(Model::parse_video_quality("1080"), 1080);
        assert_eq!(Model::parse_video_quality("bogus"), MAX_RESOLUTION);
        assert_eq!(Model::video_quality_string(MAX_RESOLUTION), "best");
        assert_eq!(Model::video_quality_string(720), "720");
    }

    #[test]
    fn audio_qualities_from_bitrates() {
        let q = Model::build_audio_qualities(&[128, 256, 128, 0]);
        assert_eq!(q[0].0, "best");
        assert_eq!(q[1].0, "256");
        assert_eq!(q[2].0, "128");
        assert_eq!(Model::choose_audio_quality("128", &q), "128");
        assert_eq!(Model::choose_audio_quality("64", &q), "best");
        assert_eq!(
            Model::normalize_bitrates(&[128.4, 64.6, 0.0]),
            vec![128, 65]
        );
    }

    #[test]
    fn check_dir_tmp_roundtrip() {
        let dir = std::env::temp_dir().join("vd-test-check-dir");
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.to_string_lossy().to_string();
        assert!(check_download_dir(&p, true).is_none());
        assert!(check_download_dir(&p, false).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_dir_missing_no_create_is_not_a_directory() {
        let p = "/definitely/not/a/real/path/xyz123";
        assert!(check_download_dir(p, false).is_some());
    }
}
