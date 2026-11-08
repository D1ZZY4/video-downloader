use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use gtk::glib;
use yt_dlp::Downloader as YtDlpDownloader;
use yt_dlp::model::Video;
use yt_dlp::model::selector::{AudioQuality, VideoCodecPreference, VideoQuality};

use crate::window::Window;

/// Settings needed to configure the yt-dlp builder for a single video.
pub(crate) struct DownloadSettings {
    pub(crate) mode: String,
    pub(crate) resolution: u32,
    pub(crate) audio_quality: String,
    pub(crate) audio_format_ext: String,
    pub(crate) video_container: String,
    pub(crate) prefer_mpeg: bool,
    pub(crate) subtitles: Vec<String>,
}

/// Map a model quality string (`best` or bitrate kbps) to the crate enum.
fn crate_audio_quality(q: &str) -> AudioQuality {
    crate::downloader::quality::crate_audio_quality(q)
}

/// Best-known total size for `video`: selected video format size plus
/// selected audio format size (`filesize` or `filesize_approx`), from the
/// fetched formats. Falls back to `tbr * duration` when the site reports
/// no sizes but bitrates (e.g. HLS-only sites). Used so progress shows
/// `X of Y` instead of unknown.
fn estimated_total_bytes(video: &Video, settings: &DownloadSettings) -> Option<i64> {
    fn size(f: &yt_dlp::model::format::Format, video: &Video) -> Option<i64> {
        if let Some(s) = f.file_info.filesize.or(f.file_info.filesize_approx) {
            return Some(s);
        }
        // tbr (kbps) * duration (s) / 8 = bytes.
        let duration = video.duration? as f64;
        let tbr = f
            .rates_info
            .total_rate
            .map(|r| *r)
            .or_else(|| f.rates_info.video_rate.map(|r| *r))
            .or_else(|| f.rates_info.audio_rate.map(|r| *r))?;
        if tbr > 1.0 && duration > 0.0 {
            Some((tbr * 1000.0 * duration / 8.0) as i64)
        } else {
            None
        }
    }
    let videos: Vec<_> = video
        .formats
        .iter()
        .filter(|f| !f.format_type().is_storyboard())
        .filter(|f| f.video_resolution.height.is_some() || f.is_video())
        .collect();
    let audios: Vec<_> = video
        .formats
        .iter()
        .filter(|f| !f.format_type().is_storyboard())
        .filter(|f| f.rates_info.audio_rate.is_some() || f.is_audio())
        .collect();
    let best_audio = audios
        .iter()
        .max_by_key(|f| f.rates_info.audio_rate.map(|r| *r as i64).unwrap_or(0))
        .and_then(|f| size(f, video));
    if settings.mode == "audio" {
        return best_audio;
    }
    let want_h = settings.resolution.min(4320);
    let mut best_video: Option<&yt_dlp::model::format::Format> = None;
    let mut best_h = 0u32;
    for f in &videos {
        let h = f.video_resolution.height.unwrap_or(0);
        if h <= want_h && h >= best_h {
            best_h = h;
            best_video = Some(f);
        }
    }
    if best_video.is_none() {
        best_video = videos.iter().copied().max_by_key(|f| {
            f.video_resolution.height.unwrap_or(0)
                + f.rates_info.video_rate.map(|r| *r as u32).unwrap_or(0) / 1000
        });
    }
    let mut total: i64 = 0;
    let mut any = false;
    if let Some(f) = best_video
        && let Some(s) = size(f, video)
    {
        total += s;
        any = true;
    }
    if let Some(s) = best_audio {
        total += s;
        any = true;
    }
    if any && total > 0 { Some(total) } else { None }
}

/// Execute the download for a single video.
///
/// Handles existing-file detection, thumbnail notification and sidecar
/// download, builder configuration, progress callbacks and the file-size
/// poller thread. Returns `Err("Cancelled")` when cancellation is
/// observed, and the yt-dlp execution error wrapped as
/// `Download failed: ...` otherwise.
pub(crate) async fn download_one(
    downloader: &YtDlpDownloader,
    video: &Video,
    output_dir: &Path,
    output_title: &str,
    settings: &DownloadSettings,
    window: &Window,
    cancel: &Arc<Mutex<bool>>,
) -> Result<(), String> {
    // Thumbnail is notified once the local JPEG is ready (no remote-URL
    // placeholder): the URL races the finished local path and overwrites it.
    // Skip if already downloaded.
    if let Some(existing) = find_existing_download(output_dir, output_title, &settings.mode) {
        let w = window.clone();
        let f = existing.clone();
        glib::MainContext::default().invoke(move || {
            let binding = w.model();
            {
                let mut m = crate::utils::lock_or_recover(&binding);
                if let Some(m) = m.as_mut() {
                    m.on_download_finished(&f);
                }
            }
            w.sync_from_model();
        });
        return Ok(());
    }
    let ext = if settings.mode == "audio" {
        settings.audio_format_ext.as_str()
    } else {
        "mp4"
    };
    let filename = format!("{}.{}", output_title, ext);
    let output_path = output_dir.join(&filename);

    let window_progress = window.clone();
    let filename_progress = std::sync::Arc::new(filename.clone());
    // Seed the total size so progress shows `X of Y` from the start.
    if let Some(total) = estimated_total_bytes(video, settings) {
        let w0 = window.clone();
        let f0 = filename.clone();
        glib::MainContext::default().invoke(move || {
            let binding = w0.model();
            {
                let mut m = crate::utils::lock_or_recover(&binding);
                if let Some(m) = m.as_mut() {
                    let (b, prog, eta, sp) = (
                        m.download_bytes,
                        m.download_progress,
                        m.download_eta,
                        m.download_speed,
                    );
                    m.on_progress(&f0, prog, b, total, eta, sp);
                }
            }
            w0.sync_from_model();
        });
    }
    // Thumbnail sidecar: download best thumbnail for preview (best-effort).
    // Stored under the temp dir, not the download folder, so downloads
    // stay clean (legacy used the per-video temp dir for this too).
    if video.best_thumbnail().is_some() {
        let w = window.clone();
        let dir = std::env::temp_dir().join("video-downloader-thumbs");
        let video = video.clone();
        let dl = downloader.clone();
        std::thread::spawn(move || {
            if let Some(to_show) =
                crate::downloader::thumbnail::download_thumbnail(&dl, &video, &dir)
            {
                let w2 = w.clone();
                let path = to_show.to_string_lossy().to_string();
                glib::MainContext::default().invoke(move || {
                    let binding = w2.model();
                    {
                        let mut m = crate::utils::lock_or_recover(&binding);
                        if let Some(m) = m.as_mut() {
                            m.on_download_thumbnail(&path);
                        }
                    }
                    w2.sync_from_model();
                });
            }
        });
    }
    let output_path_poll = output_path.clone();
    // Poll file size to derive bytes/eta/speed (yt-dlp builder only gives fraction).
    let poller_stop = Arc::new(AtomicBool::new(false));
    let poller_handle = {
        let w = window.clone();
        let fname = filename_progress.clone();
        let pth = output_path_poll.clone();
        let cancel_poll = cancel.clone();
        let stop_flag = poller_stop.clone();
        thread::spawn(move || {
            let mut last_size: i64 = 0;
            let mut last_t = std::time::Instant::now();
            loop {
                if stop_flag.load(Ordering::Relaxed) || *crate::utils::lock_or_recover(&cancel_poll)
                {
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
                if stop_flag.load(Ordering::Relaxed) || *crate::utils::lock_or_recover(&cancel_poll)
                {
                    break;
                }
                // Sum partials too: HLS merges only at the end, so the
                // final file stays 0 until completion.
                let cur = {
                    if let Ok(meta) = std::fs::metadata(&pth) {
                        meta.len() as i64
                    } else if let Some(dir) = pth.parent() {
                        let stem = pth.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        let mut total: i64 = 0;
                        if let Ok(rd) = std::fs::read_dir(dir) {
                            for e in rd.flatten() {
                                if let Some(name) = e.file_name().to_str()
                                    && name.starts_with(stem)
                                {
                                    total += e.metadata().map(|m| m.len() as i64).unwrap_or(0);
                                }
                            }
                        }
                        total
                    } else {
                        0
                    }
                };
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
                    {
                        let mut m = crate::utils::lock_or_recover(&binding);
                        if let Some(m) = m.as_mut() {
                            // Preserve the seeded total; derive fraction/ETA
                            // from it while the builder/binary reports none.
                            let prev_total = m.download_bytes_total;
                            let prev_prog = m.download_progress;
                            let (eff_prog, eff_eta) = if prev_total > 0 && cur > 0 {
                                let frac = (cur as f64 / prev_total as f64).clamp(0.0, 1.0);
                                let eta = if speed > 0 {
                                    ((prev_total - cur) / speed).max(0)
                                } else {
                                    -1
                                };
                                (frac, eta)
                            } else {
                                (prev_prog, m.download_eta)
                            };
                            m.on_progress(&f2, eff_prog, cur, prev_total, eff_eta, speed);
                        }
                    }
                    w2.sync_from_model();
                });
                last_size = cur;
                last_t = now;
            }
        })
    };
    let last_invoke = std::sync::Arc::new(std::sync::Mutex::new(std::time::Instant::now()));
    let progress_cb = move |p: f64| {
        // Throttle: the crate reports per chunk; unthrottled invocations
        // flood the main loop (seconds of work) and freeze the UI.
        if last_invoke
            .lock()
            .ok()
            .map(|g| g.elapsed() < std::time::Duration::from_millis(100))
            .unwrap_or(false)
        {
            return;
        }
        if let Ok(mut g) = last_invoke.lock() {
            *g = std::time::Instant::now();
        }
        let w = window_progress.clone();
        let fname = filename_progress.clone();
        glib::MainContext::default().invoke(move || {
            let binding = w.model();
            {
                let mut m = crate::utils::lock_or_recover(&binding);
                if let Some(m) = m.as_mut() {
                    // Keep bytes from poller if already set.
                    let (b, bt, eta, sp) = (
                        m.download_bytes,
                        m.download_bytes_total,
                        m.download_eta,
                        m.download_speed,
                    );
                    m.on_progress(&fname, p, b, bt, eta, sp);
                }
            }
            w.sync_from_model();
        });
    };

    let mut builder = downloader.download(video, &output_path);
    let aq = crate_audio_quality(&settings.audio_quality);
    if settings.mode == "audio" {
        builder = builder.audio_only();
        builder = builder.video_quality(VideoQuality::Best).audio_quality(aq);
    } else {
        builder = builder.video_quality(VideoQuality::CustomHeight(settings.resolution.min(4320)));
        builder = builder.audio_quality(aq);
        if settings.prefer_mpeg || settings.video_container == "mp4" {
            builder = builder.video_codec(VideoCodecPreference::AVC1);
        } else if settings.video_container == "webm" {
            builder = builder.video_codec(VideoCodecPreference::VP9);
        }
    }
    builder = builder.with_progress(progress_cb);

    let mut result: Result<PathBuf, String> = builder.execute().await.map_err(|e| e.to_string());

    // The crate only selects separate video/audio streams; sites that
    // serve only muxed/HLS formats (e.g. HLS `Manifest` formats with
    // `manifest_url`, or direct URLs without codec fields) always report
    // `FormatNotAvailable`. Fall back to the yt-dlp binary, which handles
    // them, before giving up. Poller keeps running during the fallback.
    if let Err(e) = &result {
        eprintln!("[download] crate failed title={} err={}", output_title, e);
        let msg = e.to_lowercase();
        if msg.contains("format available") || msg.contains("formatnotavailable") {
            eprintln!("[download] fallback to yt-dlp title={}", output_title);
            result = download_with_ytdlp(
                downloader,
                video,
                output_dir,
                output_title,
                &output_path,
                settings,
                window,
                cancel,
            )
            .await;
        }
    }

    poller_stop.store(true, Ordering::Relaxed);
    let _ = poller_handle.join();

    if *crate::utils::lock_or_recover(cancel) {
        cleanup_partial(output_dir, output_title, &filename);
        return Err("Cancelled".to_string());
    }

    match result {
        Ok(_) => {
            // Subtitles: download requested languages as sidecars, converted
            // to VTT (best-effort; a subtitle failure must not fail the video).
            download_subtitles(
                downloader,
                video,
                output_dir,
                output_title,
                &settings.subtitles,
            )
            .await;
            // Convert any remaining dfxp/ttml/tt/srt sidecars for this video.
            convert_subtitles_in_dir(output_dir, output_title);
            let w = window.clone();
            let fname = filename.clone();
            glib::MainContext::default().invoke(move || {
                let binding = w.model();
                {
                    let mut m = crate::utils::lock_or_recover(&binding);
                    if let Some(m) = m.as_mut() {
                        m.on_download_finished(&fname);
                    }
                }
                w.sync_from_model();
            });
        }
        Err(e) => {
            return Err(format!("Download failed: {}", e));
        }
    }
    Ok(())
}

/// Fallback download via the yt-dlp binary, for formats the crate's native
/// selector cannot see (muxed HLS `Manifest` formats, direct URLs without
/// codec fields). Format selection mirrors the legacy slave:
/// audio = `bestaudio/best` to MP3 192k, video = `bestvideo+bestaudio/best`
/// capped by height, merged to MP4.
#[allow(clippy::too_many_arguments)]
async fn download_with_ytdlp(
    downloader: &YtDlpDownloader,
    video: &Video,
    output_dir: &Path,
    output_title: &str,
    output_path: &Path,
    settings: &DownloadSettings,
    window: &Window,
    cancel: &Arc<Mutex<bool>>,
) -> Result<PathBuf, String> {
    let url = video
        .webpage_url
        .clone()
        .filter(|u| !u.trim().is_empty())
        .ok_or_else(|| "Download failed: no page URL for fallback".to_string())?;
    let exe = downloader.libraries().youtube.clone();
    let expected_ext = if settings.mode == "audio" {
        "mp3"
    } else {
        "mp4"
    };
    let mut cmd_args = vec!["--no-progress".to_string()];
    cmd_args.extend(downloader.args().iter().cloned());
    cmd_args.extend(["--embed-metadata".to_string(), "--embed-subs".to_string()]);
    if settings.mode == "audio" {
        cmd_args.extend([
            "-f".to_string(),
            "bestaudio/best".to_string(),
            "--extract-audio".to_string(),
            "--audio-format".to_string(),
            "mp3".to_string(),
            "--audio-quality".to_string(),
            "192".to_string(),
        ]);
    } else {
        let selector = if settings.resolution >= 65532 {
            if settings.prefer_mpeg {
                "bestvideo[vcodec=h264]+bestaudio/best".to_string()
            } else {
                "bestvideo+bestaudio/best".to_string()
            }
        } else {
            let h = settings.resolution.min(4320);
            if settings.prefer_mpeg {
                format!("bestvideo[vcodec=h264][height<={}]+bestaudio/best", h)
            } else {
                format!("bestvideo[height<={}]+bestaudio/best", h)
            }
        };
        cmd_args.extend([
            "-f".to_string(),
            selector,
            "--merge-output-format".to_string(),
            "mp4".to_string(),
        ]);
    }
    let tmpl = format!("{}/{}.%(ext)s", output_dir.to_string_lossy(), output_title);
    cmd_args.extend(["-o".to_string(), tmpl, url]);
    // Inherit stdout/stderr: piping without draining would block the child
    // on a full 64 KiB pipe buffer and hang the download forever.
    eprintln!("[download] ytdlp start title={}", output_title);
    let mut child = tokio::process::Command::new(&exe)
        .args(&cmd_args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;
    // Poll for completion so Cancel kills the child promptly instead of
    // waiting for the full download to finish.
    let status = loop {
        if *crate::utils::lock_or_recover(cancel) {
            let _ = child.kill().await;
            let _ = child.wait().await;
            cleanup_partial(output_dir, output_title, "");
            return Err("Cancelled".to_string());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => tokio::time::sleep(std::time::Duration::from_millis(200)).await,
            Err(e) => return Err(format!("Failed to run yt-dlp: {}", e)),
        }
    };
    eprintln!(
        "[download] ytdlp done title={} status={}",
        output_title, status
    );
    if !status.success() {
        return Err("Download failed: yt-dlp fallback failed".to_string());
    }
    // yt-dlp picks the final extension; reconcile with the expected file.
    if !output_path.exists() {
        let stem = output_dir.join(output_title);
        if let Some(found) = std::fs::read_dir(output_dir).ok().and_then(|rd| {
            rd.flatten().find(|e| {
                e.path()
                    .file_stem()
                    .is_some_and(|s| s == stem.file_stem().unwrap_or_default())
            })
        }) {
            let _ = std::fs::rename(found.path(), output_path);
        }
    }
    if !output_path.exists() {
        let _ = expected_ext;
        return Err("Download failed: fallback produced no file".to_string());
    }
    let w = window.clone();
    let fname = output_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    glib::MainContext::default().invoke(move || {
        let binding = w.model();
        {
            let mut m = crate::utils::lock_or_recover(&binding);
            if let Some(m) = m.as_mut() {
                let (b, bt, eta, sp) = (
                    m.download_bytes,
                    m.download_bytes_total,
                    m.download_eta,
                    m.download_speed,
                );
                m.on_progress(&fname, 1.0, b, bt, eta, sp);
            }
        }
        w.sync_from_model();
    });
    Ok(output_path.to_path_buf())
}

/// Delete leftover partials for a cancelled download: the target file,
/// `.part` / `.ytdl` temporaries, and any temp-download fragments with the
/// same stem. Never touches successfully finished files (call only on
/// cancel / failed downloads).
pub(crate) fn cleanup_partial(dir: &Path, output_title: &str, filename: &str) {
    // The final target may still be incomplete on cancel.
    if !filename.is_empty() {
        let _ = std::fs::remove_file(dir.join(filename));
    }
    let stem = std::ffi::OsStr::new(output_title);
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let path = e.path();
            if !path.is_file() {
                continue;
            }
            let is_stem = path.file_stem().is_some_and(|s| s == stem);
            let is_partial_ext = path.extension().is_some_and(|ext| {
                matches!(
                    ext.to_str()
                        .map(|s| s.to_ascii_lowercase())
                        .unwrap_or_default()
                        .as_str(),
                    "part" | "ytdl" | "temp" | "tmp" | "aria2"
                )
            });
            if is_stem && is_partial_ext {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

pub(crate) fn short_filename(name: &str, max_bytes: usize) -> String {
    // Truncate by chars to keep byte length below limit, appending ellipsis when truncated.
    let mut out = sanitize(name);
    if out.trim().trim_matches(['_', '.']).is_empty() {
        return "video".to_string();
    }
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

fn find_existing_download(dir: &Path, title: &str, mode: &str) -> Option<String> {
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

fn convert_subtitles_in_dir(dir: &Path, output_title: &str) {
    let _ = convert_subtitles_inner(dir, output_title);
}

fn convert_subtitles_inner(dir: &Path, output_title: &str) -> std::io::Result<()> {
    let rd = std::fs::read_dir(dir)?;
    for entry in rd.flatten() {
        let path = entry.path();
        // Only touch sidecars belonging to the current download.
        let fname = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        if !fname.starts_with(output_title) {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !["dfxp", "ttml", "tt", "srt"].contains(&ext.as_str()) {
            continue;
        }
        let base = path.with_extension("");
        let vtt = base.with_extension("vtt");
        let _ = Command::new("ffmpeg")
            .args([
                "-y",
                "-v",
                "error",
                "-i",
                path.to_string_lossy().as_ref(),
                "-f",
                "webvtt",
                vtt.to_string_lossy().as_ref(),
            ])
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

fn is_vtt_timestamp(line: &str) -> bool {
    // `00:00:01.000 --> 00:00:02.000` (digits, dots, colons around arrow).
    let Some((a, b)) = line.split_once(" --> ") else {
        return false;
    };
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == ':')
    };
    ok(a) && ok(b)
}

fn fix_webvtt(text: &str) -> String {
    // Fix the broken WEBVTT emitted by ffmpeg 4.4: after a timestamp line
    // it inserts a blank line, whitespace-only lines, and leading spaces on
    // the next cue line. Legacy regex:
    //   `(?<=\n\n)(ts\n)\n(?: +\n)* *` -> `\1`
    // i.e. drop the blank line(s) following a timestamp and strip leading
    // spaces of the cue text that follows.
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        out.push(line.to_string());
        if is_vtt_timestamp(line) && i + 1 < lines.len() && lines[i + 1].is_empty() {
            i += 1; // skip blank line after timestamp
            while i + 1 < lines.len() && lines[i + 1].trim().is_empty() {
                i += 1;
            }
            if i + 1 < lines.len() {
                i += 1;
                out.push(lines[i].trim_start().to_string());
            }
        }
        i += 1;
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') && !joined.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string()
}

/// Download subtitle sidecars (`<title>.<lang>.vtt`) with the crate helper.
/// All manual subtitles are downloaded; automatic captions are filtered by
/// `requested` (`all` matches everything, otherwise exact lang or translated
/// `requested-<variant>`). Failures are best-effort and only log.
async fn download_subtitles(
    downloader: &YtDlpDownloader,
    video: &Video,
    output_dir: &Path,
    output_title: &str,
    requested: &[String],
) {
    let wants_all = requested.iter().any(|r| r == "all");
    let mut langs: Vec<(String, bool)> = Vec::new();
    for lang in video.subtitles.keys() {
        langs.push((lang.clone(), false));
    }
    for lang in video.automatic_captions.keys() {
        if video.subtitles.contains_key(lang) {
            continue;
        }
        let hit = wants_all
            || requested
                .iter()
                .any(|r| r == lang || lang.starts_with(&format!("{}-", r)));
        if hit {
            langs.push((lang.clone(), true));
        }
    }
    for (lang, is_auto) in langs {
        let part = output_dir.join(format!("{}.{}.part", output_title, lang));
        let part_str = part.to_string_lossy().to_string();
        match downloader
            .download_subtitle(video, &lang, &part_str, is_auto)
            .await
        {
            Ok(_) => finalize_subtitle(&part, output_dir, output_title, &lang),
            Err(e) => {
                // Stderr-style debug line for G_MESSAGES_DEBUG=yt-dlp users.
                eprintln!("[yt-dlp] subtitle {} skipped: {}", lang, e);
            }
        }
    }
}

fn finalize_subtitle(part: &Path, dir: &Path, title: &str, lang: &str) {
    let body = std::fs::read(part).unwrap_or_default();
    let is_vtt = body.starts_with(b"WEBVTT");
    let vtt = dir.join(format!("{}.{}.vtt", title, lang));
    if is_vtt {
        let _ = std::fs::rename(part, &vtt);
        return;
    }
    // Source is SRT (crate prefers SRT) -> convert to VTT via ffmpeg.
    let part_str = part.to_string_lossy().to_string();
    let vtt_str = vtt.to_string_lossy().to_string();
    let status = Command::new("ffmpeg")
        .args([
            "-y", "-v", "error", "-i", &part_str, "-f", "webvtt", &vtt_str,
        ])
        .status();
    if status.is_ok_and(|s| s.success()) && vtt.exists() {
        if let Ok(text) = std::fs::read_to_string(&vtt) {
            let _ = std::fs::write(&vtt, fix_webvtt(&text));
        }
        let _ = std::fs::remove_file(part);
    } else {
        // Keep the downloaded file as .srt so users still get captions.
        let srt = dir.join(format!("{}.{}.srt", title, lang));
        let _ = std::fs::rename(part, srt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_filename_empty_falls_back() {
        assert_eq!(short_filename("!!!", 200), "video");
    }

    #[test]
    fn short_filename_truncates_with_ellipsis() {
        let s = short_filename(&"a".repeat(300), 200);
        assert!(s.len() <= 200);
        assert!(s.ends_with('\u{2026}'));
    }

    #[test]
    fn fix_webvtt_removes_blank_and_spaces() {
        let broken = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n\n  hello\n";
        let fixed = fix_webvtt(broken);
        assert_eq!(fixed, "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nhello\n");
    }

    #[test]
    fn fix_webvtt_keeps_clean_file() {
        let clean = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nhello\n";
        assert_eq!(fix_webvtt(clean), clean);
    }
}
