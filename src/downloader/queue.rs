use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use gtk::glib;
use tokio::runtime::Runtime;
use yt_dlp::Downloader as YtDlpDownloader;
use yt_dlp::model::Video;

use crate::downloader::download::{DownloadSettings, download_one, short_filename};
use yt_dlp::extractor::VideoExtractor;
use yt_dlp::model::types::playlist::Playlist;

use crate::window::Window;

pub struct DownloadEngine {
    window: Window,
    cancel: Arc<Mutex<bool>>,
    shared_downloader: Arc<Mutex<Option<YtDlpDownloader>>>,
}

impl DownloadEngine {
    pub fn new(window: Window) -> Self {
        Self {
            window,
            cancel: Arc::new(Mutex::new(false)),
            shared_downloader: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start(&self) {
        let window = self.window.clone();
        let cancel = self.cancel.clone();
        let shared_downloader = self.shared_downloader.clone();
        thread::spawn(move || {
            let rt = match Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let w = window.clone();
                    glib::MainContext::default().invoke(move || {
                        let binding = w.model();
                        {
                            let mut m = crate::utils::lock_or_recover(&binding);
                            if let Some(m) = m.as_mut() {
                                m.on_error(&format!("Failed to create runtime: {}", e));
                                m.on_finished(false);
                            }
                        }
                        w.sync_from_model();
                    });
                    return;
                }
            };

            rt.block_on(async move {
                if let Err(e) = run_download(window.clone(), cancel, shared_downloader).await {
                    let w = window.clone();
                    glib::MainContext::default().invoke(move || {
                        let binding = w.model();
                        {
                            let mut m = crate::utils::lock_or_recover(&binding);
                            if let Some(m) = m.as_mut() {
                                m.on_error(&e);
                                m.on_finished(false);
                            }
                        }
                        w.sync_from_model();
                    });
                }
            });
        });
    }

    pub fn cancel(&self) {
        *crate::utils::lock_or_recover(&self.cancel) = true;
        // Ask the yt-dlp crate to abort the in-flight download too; the
        // boolean flag alone only takes effect between videos.
        if let Some(dl) = crate::utils::lock_or_recover(&self.shared_downloader).clone() {
            dl.shutdown();
        }
    }

    pub fn destroy(&self) {
        self.cancel();
    }
}

async fn run_download(
    window: Window,
    cancel: Arc<Mutex<bool>>,
    shared_downloader: Arc<Mutex<Option<YtDlpDownloader>>>,
) -> Result<(), String> {
    let (
        url,
        mode,
        resolution,
        audio_quality,
        audio_format_ext,
        video_container,
        prefer_mpeg,
        subtitles,
        output_filename,
        download_folder,
    ) = {
        let binding = window.model();
        let m = crate::utils::lock_or_recover(&binding);
        let m = m.as_ref().ok_or("No model")?;
        (
            m.get_url(),
            m.get_mode(),
            m.get_resolution(),
            m.get_audio_quality(),
            m.get_audio_format_ext(),
            m.get_video_container(),
            m.get_prefer_mpeg(),
            m.get_automatic_subtitles(),
            m.get_output_filename(),
            m.get_download_dir(),
        )
    };

    if url.trim().is_empty() {
        return Err("URL is empty".to_string());
    }

    let settings = DownloadSettings {
        mode,
        resolution,
        audio_quality,
        audio_format_ext,
        video_container,
        prefer_mpeg,
        subtitles: subtitles.clone(),
    };

    let libs_dir = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
        .join(".local/share/video-downloader/libs");
    let output_dir = PathBuf::from(&download_folder);
    let _ = std::fs::create_dir_all(&output_dir);

    let mut args = vec![
        "--write-subs".to_string(),
        "--write-auto-subs".to_string(),
        "--sub-format".to_string(),
        "vtt/best".to_string(),
    ];
    if !subtitles.is_empty() {
        // Map user languages to --sub-langs (comma separated).
        // 'all' means all languages.
        args.push("--sub-langs".to_string());
        args.push(subtitles.join(","));
    }
    let mut downloader = YtDlpDownloader::with_new_binaries(&libs_dir, &output_dir)
        .await
        .map_err(|e| format!("Failed to prepare yt-dlp: {}", e))?
        .with_args(args)
        .build()
        .await
        .map_err(|e| format!("Failed to build downloader: {}", e))?;
    *crate::utils::lock_or_recover(&shared_downloader) = Some(downloader.clone());
    if *crate::utils::lock_or_recover(&cancel) {
        return Err("Cancelled".to_string());
    }

    // Try playlist first
    let playlist = probe_playlist(&downloader, &url).await;

    let mut auth = AuthState::default();
    let mut videos: Vec<Video> = Vec::new();
    if let Some(pl) = playlist {
        let download_all = crate::dialogs::playlist::request(&window);
        if download_all {
            for entry in &pl.entries {
                if let Ok(v) =
                    fetch_video_with_auth(&mut downloader, &window, &entry.url, &mut auth).await
                {
                    videos.push(v);
                }
            }
        } else if let Some(first) = pl.entries.first()
            && let Ok(v) =
                fetch_video_with_auth(&mut downloader, &window, &first.url, &mut auth).await
        {
            videos.push(v);
        }
    }
    if videos.is_empty() {
        let v = fetch_video_with_auth(&mut downloader, &window, &url, &mut auth)
            .await
            .map_err(|e| format!("Failed to fetch video: {}", e))?;
        videos.push(v);
    }

    let count = videos.len() as i64;
    let mut failures: Vec<String> = Vec::new();
    let mut successes = 0usize;

    for (idx, video) in videos.into_iter().enumerate() {
        if *crate::utils::lock_or_recover(&cancel) {
            return Err("Cancelled".to_string());
        }

        let title = if count == 1 && !output_filename.trim().is_empty() {
            output_filename.trim().to_string()
        } else {
            video.title.clone()
        };

        // notify start
        {
            let w = window.clone();
            let t = title.clone();
            let i = idx as i64;
            let c = count;
            glib::MainContext::default().invoke(move || {
                let binding = w.model();
                {
                    let mut m = crate::utils::lock_or_recover(&binding);
                    if let Some(m) = m.as_mut() {
                        m.on_download_start(i, c, &t);
                    }
                }
                w.sync_from_model();
            });
        }

        // Download lock (prevent concurrent same-title downloads).
        let output_title = short_filename(&title, 200);
        loop {
            let w = window.clone();
            let n = output_title.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            glib::MainContext::default().invoke(move || {
                let binding = w.model();
                let mut m = crate::utils::lock_or_recover(&binding);
                if let Some(m) = m.as_mut() {
                    let _ = tx.send(m.on_download_lock(&n));
                }
            });
            if crate::utils::recv_timeout(&rx, std::time::Duration::from_secs(600)).unwrap_or(false)
            {
                break;
            }
            if *crate::utils::lock_or_recover(&cancel) {
                let w2 = window.clone();
                glib::MainContext::default().invoke(move || {
                    let binding = w2.model();
                    {
                        let mut m = crate::utils::lock_or_recover(&binding);
                        if let Some(m) = m.as_mut() {
                            m.on_finished(false);
                        }
                    }
                    w2.sync_from_model();
                });
                return Err("Cancelled".to_string());
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        // One video failing must not abort the rest of the playlist
        // (legacy used `ignoreerrors` and skipped unavailable videos).
        // Only cancellation aborts early; other errors are collected.
        match download_one(
            &downloader,
            &video,
            &output_dir,
            &output_title,
            &settings,
            &window,
            &cancel,
        )
        .await
        {
            Ok(()) => successes += 1,
            Err(e) if e == "Cancelled" => return Err(e),
            Err(e) => failures.push(format!("{}: {}", title, e)),
        }
    }

    if successes == 0 {
        return Err(if failures.is_empty() {
            "Download failed".to_string()
        } else {
            failures.join("\n")
        });
    }

    let w = window.clone();
    glib::MainContext::default().invoke(move || {
        let binding = w.model();
        {
            let mut m = crate::utils::lock_or_recover(&binding);
            if let Some(m) = m.as_mut() {
                m.on_finished(true);
            }
        }
        w.sync_from_model();
    });

    Ok(())
}

/// Shared authentication state across playlist fetches. The legacy app
/// prompted once, stored credentials in its yt-dlp options, and skipped
/// later videos without re-prompting. This mirrors that: each credential
/// type is requested at most once per download batch.
#[derive(Default)]
pub(crate) struct AuthState {
    pub(crate) tried_login: bool,
    pub(crate) tried_password: bool,
    pub(crate) skip_login: bool,
    pub(crate) skip_password: bool,
}

/// Probe whether `url` points to a playlist with multiple entries.
/// Returns `None` for single videos, fetch failures, or playlists
/// with a single entry.
pub(crate) async fn probe_playlist(downloader: &YtDlpDownloader, url: &str) -> Option<Playlist> {
    downloader
        .generic_extractor()
        .fetch_playlist(url)
        .await
        .ok()
        .filter(|p| p.entries.len() > 1)
}

/// Patch a `--dump-single-json` document so the crate's strict `Video`
/// deserialization succeeds: newer yt-dlp omits fields the crate requires
/// (`thumbnails[].preference`, `live_status`, `playable_in_embed`,
/// `age_limit`, extractor/version info, ...). Missing entries default
/// to neutral values instead of failing the whole fetch.
fn patch_video_json(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    // yt-dlp sometimes emits `null` where the crate expects a value.
    if obj.get("id").is_none_or(|v| v.is_null()) {
        obj.insert("id".into(), serde_json::Value::from(""));
    }
    if obj.get("title").is_none_or(|v| v.is_null()) {
        obj.insert("title".into(), serde_json::Value::from(""));
    }
    if obj.get("live_status").is_none_or(|v| v.is_null()) {
        obj.insert("live_status".into(), serde_json::Value::from("not_live"));
    }
    if obj.get("playable_in_embed").is_none_or(|v| v.is_null()) {
        obj.insert("playable_in_embed".into(), serde_json::Value::from(true));
    }
    if obj.get("age_limit").is_none_or(|v| v.is_null()) {
        obj.insert("age_limit".into(), serde_json::Value::from(0));
    }
    if obj.get("extractor").is_none_or(|v| v.is_null()) {
        obj.insert("extractor".into(), serde_json::Value::from("generic"));
    }
    if obj.get("extractor_key").is_none_or(|v| v.is_null()) {
        obj.insert("extractor_key".into(), serde_json::Value::from("Generic"));
    }
    let version = obj.entry("_version").or_insert_with(
        || serde_json::json!({"version": "unknown", "repository": "yt-dlp/yt-dlp"}),
    );
    if version.is_null() {
        *version = serde_json::json!({"version": "unknown", "repository": "yt-dlp/yt-dlp"});
    }
    if let Some(vobj) = version.as_object_mut() {
        vobj.entry("version")
            .or_insert_with(|| serde_json::Value::from("unknown"));
        vobj.entry("repository")
            .or_insert_with(|| serde_json::Value::from("yt-dlp/yt-dlp"));
    }
    if let Some(thumbs) = obj.get_mut("thumbnails").and_then(|t| t.as_array_mut()) {
        for thumb in thumbs.iter_mut() {
            if let Some(tobj) = thumb.as_object_mut() {
                tobj.entry("preference")
                    .or_insert(serde_json::Value::from(0));
                tobj.entry("id")
                    .or_insert_with(|| serde_json::Value::from("0"));
            }
        }
    }
}

/// Fetch video metadata by invoking the yt-dlp binary directly.
/// The crate's extractor ignores `Downloader::args` (auth flags added via
/// `add_arg` never reach it), and its `Video` parse is stricter than
/// yt-dlp's output (see `patch_video_json`). This path respects both.
async fn fetch_video_manual(downloader: &YtDlpDownloader, url: &str) -> Result<Video, String> {
    let exe = downloader.libraries().youtube.clone();
    let mut cmd_args = vec![
        "--no-progress".to_string(),
        "--dump-single-json".to_string(),
    ];
    cmd_args.extend(downloader.args().iter().cloned());
    cmd_args.push(url.to_string());
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        tokio::process::Command::new(&exe).args(&cmd_args).output(),
    )
    .await
    .map_err(|_| "Timed out fetching video".to_string())?
    .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        let combined = format!("{} {}", stdout, stderr).trim().to_string();
        return Err(if combined.is_empty() {
            "Failed to fetch video".to_string()
        } else {
            combined
        });
    }
    if stdout.trim().is_empty() {
        return Err(format!("Failed to fetch video: empty output; {}", stderr));
    }
    let mut value: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("JSON error: {}", e))?;
    patch_video_json(&mut value);
    let mut video: Video =
        serde_json::from_value(value).map_err(|e| format!("JSON error: {}", e))?;
    for format in &mut video.formats {
        format.video_id = Some(video.id.clone());
    }
    Ok(video)
}

/// Fetch with the crate first, falling back to the tolerant manual path
/// when the crate's strict `Video` parse rejects yt-dlp's output
/// (e.g. missing `preference` / `live_status` / ...). Other errors pass through.
pub(crate) async fn fetch_video_tolerant(
    downloader: &mut YtDlpDownloader,
    url: &str,
) -> Result<Video, String> {
    match downloader.generic_extractor().fetch_video(url).await {
        Ok(v) => Ok(v),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("missing field") || msg.contains("JSON error") {
                fetch_video_manual(downloader, url).await
            } else {
                Err(msg)
            }
        }
    }
}

/// Fetch a video, prompting for login credentials or a video password
/// when the extractor reports that they are required. `auth` is shared
/// across fetches so a whole playlist is prompted at most once per type;
/// empty credentials mark the batch as "skip" and subsequent fetches
/// fail fast without another dialog (legacy `_skip_authentication`).
pub(crate) async fn fetch_video_with_auth(
    downloader: &mut YtDlpDownloader,
    window: &Window,
    url: &str,
    auth: &mut AuthState,
) -> Result<Video, String> {
    loop {
        // Direct yt-dlp invocation: the crate extractor ignores
        // `Downloader::args` (auth flags never reach it) and its strict
        // `Video` parse fails on missing `preference` in thumbnails.
        match fetch_video_manual(downloader, url).await {
            Ok(v) => return Ok(v),
            Err(msg) => {
                if !auth.tried_login && (msg.contains("Sign in") || msg.contains("--username")) {
                    auth.tried_login = true;
                    if auth.skip_login {
                        return Err(msg);
                    }
                    match crate::dialogs::login::request(window) {
                        Some((u, p)) if !u.is_empty() || !p.is_empty() => {
                            downloader.add_arg(format!("--username={}", u));
                            downloader.add_arg(format!("--password={}", p));
                            continue;
                        }
                        _ => {
                            auth.skip_login = true;
                            return Err(msg);
                        }
                    }
                } else if !auth.tried_password && msg.contains("--video-password") {
                    auth.tried_password = true;
                    if auth.skip_password {
                        return Err(msg);
                    }
                    match crate::dialogs::password::request(window) {
                        Some(p) if !p.is_empty() => {
                            downloader.add_arg(format!("--video-password={}", p));
                            continue;
                        }
                        _ => {
                            auth.skip_password = true;
                            return Err(msg);
                        }
                    }
                }
                return Err(msg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_defaults_missing_preference() {
        let mut v: serde_json::Value = serde_json::json!({
            "thumbnails": [
                {"url": "https://x/1.jpg", "id": "1"},
                {"url": "https://x/2.jpg", "id": "2", "preference": 5},
                {"url": "https://x/3.jpg"}
            ]
        });
        patch_video_json(&mut v);
        let thumbs = v["thumbnails"].as_array().unwrap();
        assert_eq!(thumbs[0]["preference"], 0);
        assert_eq!(thumbs[1]["preference"], 5);
        assert_eq!(thumbs[2]["preference"], 0);
        assert_eq!(thumbs[2]["id"], "0");
    }

    #[test]
    fn patch_defaults_missing_live_status_and_embed() {
        let mut v: serde_json::Value = serde_json::json!({"id": "x", "title": "t"});
        patch_video_json(&mut v);
        assert_eq!(v["live_status"], "not_live");
        assert_eq!(v["playable_in_embed"], true);
        assert_eq!(v["age_limit"], 0);
        assert_eq!(v["extractor"], "generic");
        assert_eq!(v["_version"]["repository"], "yt-dlp/yt-dlp");
    }

    #[test]
    fn patch_replaces_null_live_status() {
        let mut v: serde_json::Value =
            serde_json::json!({"id": "x", "title": "t", "live_status": null});
        patch_video_json(&mut v);
        assert_eq!(v["live_status"], "not_live");
    }
}
