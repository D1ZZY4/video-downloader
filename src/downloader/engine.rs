use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use gio::prelude::*;
use gtk::glib;
use tokio::runtime::Runtime;
use yt_dlp::model::selector::{AudioQuality, VideoCodecPreference, VideoQuality};
use yt_dlp::model::types::playlist::Playlist;
use yt_dlp::Downloader as YtDlpDownloader;
use yt_dlp::extractor::VideoExtractor;

use crate::window::Window;

pub struct DownloadEngine {
    window: Window,
    cancel: Arc<Mutex<bool>>,
}

impl DownloadEngine {
    pub fn new(window: Window) -> Self {
        Self {
            window,
            cancel: Arc::new(Mutex::new(false)),
        }
    }

    pub fn start(&self) {
        let window = self.window.clone();
        let cancel = self.cancel.clone();
        thread::spawn(move || {
            let rt = match Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let w = window.clone();
                    glib::MainContext::default().invoke(move || {
                        let binding = w.model();
                        let mut m = binding.lock().unwrap();
                        if let Some(m) = m.as_mut() {
                            m.on_error(&format!("Failed to create runtime: {}", e));
                            m.on_finished(false);
                        }
                        w.sync_from_model();
                        ()
                    });
                    return;
                }
            };

            rt.block_on(async move {
                if let Err(e) = run_download(window.clone(), cancel).await {
                    let w = window.clone();
                    glib::MainContext::default().invoke(move || {
                        let binding = w.model();
                        let mut m = binding.lock().unwrap();
                        if let Some(m) = m.as_mut() {
                            m.on_error(&e);
                            m.on_finished(false);
                        }
                        w.sync_from_model();
                        ()
                    });
                }
            });
        });
    }

    pub fn cancel(&self) {
        *self.cancel.lock().unwrap() = true;
    }

    pub fn destroy(&self) {
        self.cancel();
    }
}

async fn run_download(window: Window, cancel: Arc<Mutex<bool>>) -> Result<(), String> {
    let (url, mode, resolution, prefer_mpeg, subtitles, download_folder) = {
        let binding = window.model();
        let m = binding.lock().unwrap();
        let m = m.as_ref().ok_or("No model")?;
        (
            m.get_url(),
            m.get_mode(),
            m.get_resolution(),
            m.get_prefer_mpeg(),
            m.get_automatic_subtitles(),
            m.get_download_dir(),
        )
    };

    if url.is_empty() {
        return Err("URL is empty".to_string());
    }

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

    // Try playlist first
    let playlist: Option<Playlist> = downloader
        .generic_extractor()
        .fetch_playlist(&url)
        .await
        .ok()
        .filter(|p| p.entries.len() > 1);

    let mut videos: Vec<(String, String)> = Vec::new();
    if let Some(pl) = playlist {
        let download_all = crate::dialogs::playlist::request(&window);
        if download_all {
            for entry in &pl.entries {
                if let Ok(v) = fetch_video_with_auth(&mut downloader, &window, &entry.url).await {
                    videos.push((v.title.clone(), entry.url.clone()));
                }
            }
        } else if let Some(first) = pl.entries.first() {
            if let Ok(v) = fetch_video_with_auth(&mut downloader, &window, &first.url).await {
                videos.push((v.title.clone(), first.url.clone()));
            }
        }
    }
    if videos.is_empty() {
        let v = fetch_video_with_auth(&mut downloader, &window, &url)
            .await
            .map_err(|e| format!("Failed to fetch video: {}", e))?;
        videos.push((v.title.clone(), url.clone()));
    }

    let count = videos.len() as i64;

    for (idx, (title, video_url)) in videos.into_iter().enumerate() {
        if *cancel.lock().unwrap() {
            return Err("Cancelled".to_string());
        }

        let video = fetch_video_with_auth(&mut downloader, &window, &video_url)
            .await
            .map_err(|e| format!("Failed to fetch video: {}", e))?;

        // notify start
        {
            let w = window.clone();
            let t = title.clone();
            let i = idx as i64;
            let c = count;
            glib::MainContext::default().invoke(move || {
                let binding = w.model();
                let mut m = binding.lock().unwrap();
                if let Some(m) = m.as_mut() {
                    m.on_download_start(i, c, &t);
                }
                w.sync_from_model();
                ()
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
                let mut m = binding.lock().unwrap();
                if let Some(m) = m.as_mut() {
                    let _ = tx.send(m.on_download_lock(&n));
                }
            });
            if rx.recv().unwrap_or(false) {
                break;
            }
            if *cancel.lock().unwrap() {
                let w2 = window.clone();
                glib::MainContext::default().invoke(move || {
                    let binding = w2.model();
                    let mut m = binding.lock().unwrap();
                    if let Some(m) = m.as_mut() {
                        m.on_finished(false);
                    }
                    w2.sync_from_model();
                    ()
                });
                return Err("Cancelled".to_string());
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        // Notify thumbnail if available.
        if let Some(thumb) = video.best_thumbnail() {
            let w = window.clone();
            let t = thumb.url.clone();
            glib::MainContext::default().invoke(move || {
                let binding = w.model();
                let mut m = binding.lock().unwrap();
                if let Some(m) = m.as_mut() {
                    m.on_download_thumbnail(&t);
                }
                w.sync_from_model();
                ()
            });
        }
        // Skip if already downloaded.
        if let Some(existing) = find_existing_download(&output_dir, &output_title, &mode) {
            let w = window.clone();
            let f = existing.clone();
            glib::MainContext::default().invoke(move || {
                let binding = w.model();
                let mut m = binding.lock().unwrap();
                if let Some(m) = m.as_mut() {
                    m.on_download_finished(&f);
                }
                w.sync_from_model();
                ()
            });
            continue;
        }
        let ext = if mode == "audio" { "mp3" } else { "mp4" };
        let filename = format!("{}.{}", output_title, ext);
        let output_path = output_dir.join(&filename);

        let window_progress = window.clone();
        let filename_progress = std::sync::Arc::new(filename.clone());
        let _cancel_progress = cancel.clone();
        // Thumbnail sidecar: download best thumbnail for preview (best-effort).
        if let Some(thumb) = video.best_thumbnail().cloned() {
            let w = window.clone();
            let dir = output_dir.clone();
            let video_id = video.id.clone();
            std::thread::spawn(move || {
                if let Ok(bytes) = fetch_bytes(&thumb.url) {
                    let raw = dir.join(format!("{}.thumb", video_id));
                    let _ = std::fs::write(&raw, &bytes);
                    let jpg = convert_thumbnail(&raw, &dir, &video_id);
                    let to_show = if jpg.is_some() { dir.join(format!("{}.jpg", video_id)) } else { raw };
                    let w2 = w.clone();
                    let path = to_show.to_string_lossy().to_string();
                    glib::MainContext::default().invoke(move || {
                        let binding = w2.model();
                        let mut m = binding.lock().unwrap();
                        if let Some(m) = m.as_mut() {
                            m.on_download_thumbnail(&path);
                        }
                        w2.sync_from_model();
                        ()
                    });
                }
            });
        }
        let output_path_poll = output_path.clone();
        let (stop_tx, _stop_rx) = std::sync::mpsc::channel::<()>();
        let _ = stop_tx;
        // Poll file size to derive bytes/eta/speed (yt-dlp builder only gives fraction).
        {
            let w = window.clone();
            let fname = filename_progress.clone();
            let pth = output_path_poll.clone();
            let cancel_poll = cancel.clone();
            thread::spawn(move || {
                let mut last_size: i64 = 0;
                let mut last_t = std::time::Instant::now();
                loop {
                    if *cancel_poll.lock().unwrap() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(500));
                    let cur = std::fs::metadata(&pth).map(|m| m.len() as i64).unwrap_or(0);
                    let now = std::time::Instant::now();
                    let elapsed = now.duration_since(last_t).as_secs_f64();
                    let speed = if elapsed > 0.0 && cur >= last_size {
                        ((cur - last_size) as f64 / elapsed) as i64
                    } else {
                        -1
                    };
                    // Estimate total from progress fraction of last callback is unknown;
                    // keep total -1, eta derived from speed and remaining estimate via file size
                    // when progress is known we cannot know total without format info, so eta stays -1
                    // unless we have a rough estimate. Keep -1 for eta to avoid lying.
                    let w2 = w.clone();
                    let f2 = fname.clone();
                    glib::MainContext::default().invoke(move || {
                        let binding = w2.model();
                        let mut m = binding.lock().unwrap();
                        if let Some(m) = m.as_mut() {
                            // Preserve progress fraction already set; update bytes/speed.
                            // Use 0 total, -1 eta when unknown.
                            let prog = m.download_progress;
                            m.on_progress(&f2, prog, cur, -1, -1, speed);
                        }
                        ()
                    });
                    last_size = cur;
                    last_t = now;
                }
            });
        }
        let progress_cb = move |p: f64| {
            let w = window_progress.clone();
            let fname = filename_progress.clone();
            glib::MainContext::default().invoke(move || {
                let binding = w.model();
                let mut m = binding.lock().unwrap();
                if let Some(m) = m.as_mut() {
                    // Keep bytes from poller if already set.
                    let (b, bt, eta, sp) = (m.download_bytes, m.download_bytes_total, m.download_eta, m.download_speed);
                    m.on_progress(&fname, p, b, bt, eta, sp);
                }
                w.sync_from_model();
                ()
            });
        };

        let mut builder = downloader.download(&video, &output_path);
        if mode == "audio" {
            builder = builder.audio_only();
            builder = builder.video_quality(VideoQuality::Best).audio_quality(AudioQuality::Best);
        } else {
            builder = builder.video_quality(VideoQuality::CustomHeight(resolution.min(4320)));
            builder = builder.audio_quality(AudioQuality::Best);
            if prefer_mpeg {
                builder = builder.video_codec(VideoCodecPreference::AVC1);
            }
        }
        builder = builder.with_progress(progress_cb);

        let result = builder.execute().await;

        if *cancel.lock().unwrap() {
            return Err("Cancelled".to_string());
        }

        match result {
            Ok(_) => {
                // Subtitle conversion (dfxp/ttml/tt -> srt -> vtt, best-effort via ffmpeg).
                convert_subtitles_in_dir(&output_dir);
                let w = window.clone();
                let fname = filename.clone();
                glib::MainContext::default().invoke(move || {
                    let binding = w.model();
                    let mut m = binding.lock().unwrap();
                    if let Some(m) = m.as_mut() {
                        m.on_download_finished(&fname);
                    }
                    w.sync_from_model();
                    ()
                });
            }
            Err(e) => {
                return Err(format!("Download failed: {}", e));
            }
        }
    }

    let w = window.clone();
    glib::MainContext::default().invoke(move || {
        let binding = w.model();
        let mut m = binding.lock().unwrap();
        if let Some(m) = m.as_mut() {
            m.on_finished(true);
        }
        w.sync_from_model();
        ()
    });

    Ok(())
}


async fn fetch_video_with_auth(
    downloader: &mut YtDlpDownloader,
    window: &Window,
    url: &str,
) -> Result<yt_dlp::model::Video, String> {
    let mut allow_login = true;
    let mut allow_password = true;
    loop {
        match downloader.generic_extractor().fetch_video(url).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                let msg = e.to_string();
                if allow_login && (msg.contains("Sign in") || msg.contains("--username")) {
                    allow_login = false;
                    if let Some((u, p)) = crate::dialogs::login::request(window) {
                        if !u.is_empty() || !p.is_empty() {
                            downloader.add_arg(format!("--username={}", u));
                            downloader.add_arg(format!("--password={}", p));
                            continue;
                        }
                    }
                    return Err(msg);
                } else if allow_password && msg.contains("--video-password") {
                    allow_password = false;
                    if let Some(p) = crate::dialogs::password::request(window) {
                        if !p.is_empty() {
                            downloader.add_arg(format!("--video-password={}", p));
                            continue;
                        }
                    }
                    return Err(msg);
                }
                return Err(msg);
            }
        }
    }
}


fn short_filename(name: &str, max_bytes: usize) -> String {
    // Truncate by chars to keep byte length below limit, appending ellipsis when truncated.
    let mut out = sanitize(name);
    if out.len() > max_bytes {
        let mut truncated = String::new();
        for c in out.chars() {
            if truncated.len() + c.len_utf8() > max_bytes - 3 {
                truncated.push('\u{2026}');
                break;
            }
            truncated.push(c);
        }
        out = truncated;
    }
    out
}

fn find_existing_download(dir: &std::path::Path, title: &str, mode: &str) -> Option<String> {
    let rd = std::fs::read_dir(dir).ok()?;
    for entry in rd.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let fname = path.file_name()?.to_string_lossy().to_string();
        let stem = path.file_stem()?.to_string_lossy().to_string();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if stem != title {
            continue;
        }
        if mode == "audio" && !ext.eq_ignore_ascii_case("mp3") {
            continue;
        }
        return Some(fname);
    }
    None
}


fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    // GIO download to memory.
    let file = gio::File::for_uri(url);
    let (tx, rx) = std::sync::mpsc::channel();
    file.load_contents_async(Option::<&gio::Cancellable>::None, move |res| {
        let _ = tx.send(res.map(|(b, _)| b.to_vec()).map_err(|e| e.to_string()));
    });
    rx.recv().map_err(|_| "download channel closed".to_string())?
}

fn convert_thumbnail(raw: &Path, dir: &Path, stem: &str) -> Option<PathBuf> {
    let out = dir.join(format!("{}.jpg", stem));
    let out_str = out.to_string_lossy().to_string();
    let raw_str = raw.to_string_lossy().to_string();
    let s = Command::new("ffmpeg")
        .args([
            "-y",
            "-v", "error",
            "-i", &raw_str,
            "-vf", "scale='min(1024,iw):min(1024,ih):force_original_aspect_ratio=decrease'",
            &out_str,
        ])
        .status()
        .ok()?;
    if s.success() { Some(out) } else { None }
}

fn convert_subtitles_in_dir(dir: &Path) {
    let _ = convert_subtitles_inner(dir);
}

fn convert_subtitles_inner(dir: &Path) -> std::io::Result<()> {
    let rd = std::fs::read_dir(dir)?;
    for entry in rd.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if !["dfxp", "ttml", "tt", "srt"].contains(&ext.as_str()) {
            continue;
        }
        let base = path.with_extension("");
        let vtt = base.with_extension("vtt");
        let _ = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-i", &path.to_string_lossy().to_string(), "-f", "webvtt", &vtt.to_string_lossy().to_string()])
            .status();
        if vtt.exists() {
            // Fix broken WEBVTT (ffmpeg v4.4 leading spaces) + cleanup.
            if let Ok(text) = std::fs::read_to_string(&vtt) {
                let fixed = fix_webvtt(&text);
                let _ = std::fs::write(&vtt, fixed);
            }
            if ext != "vtt" {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    Ok(())
}

fn fix_webvtt(text: &str) -> String {
    // Remove blank line after timestamp introduced by ffmpeg: "ts\n\n( *\n)* *" -> "ts\n"
    let mut out = String::new();
    let lines = text.lines().peekable();
    // Simple targeted fix: replace "\n\n(\n| *\n)* * " patterns via line scan.
    let mut prev_empty = false;
    let mut prev_is_ts = false;
    for line in lines.clone() {
        if prev_empty && (line.trim().is_empty() || (prev_is_ts && line.starts_with(' '))) {
            continue;
        }
        prev_is_ts = line.contains(" --> ");
        prev_empty = line.is_empty();
        if !out.is_empty() { out.push('\n'); }
        out.push_str(line);
    }
    if text.ends_with('\n') && !out.ends_with('\n') { out.push('\n'); }
    out
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}