//! Thumbnail fetching and conversion for the download page.
//!
//! Display lives in `crate::window::thumbnail`; this module handles
//! downloading the best thumbnail over HTTP(S) and scaling it to a
//! JPEG preview with ffmpeg.

use gio::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Download bytes for a thumbnail URL. Only http(s) is allowed so
/// metadata-supplied URLs cannot read arbitrary local files via file://.
///
/// Uses the synchronous `load_contents`: the async variant never calls back
/// on a plain worker thread (no main loop), so thumbnails never arrived.
fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("unsupported thumbnail scheme".to_string());
    }
    let file = gio::File::for_uri(url);
    file.load_contents(Option::<&gio::Cancellable>::None)
        .map(|(bytes, _etag)| bytes.to_vec())
        .map_err(|e| e.to_string())
}

pub(crate) fn convert_thumbnail(raw: &Path, dir: &Path, stem: &str) -> Option<PathBuf> {
    let out = dir.join(format!("{}.jpg", stem));
    let out_str = out.to_string_lossy().to_string();
    let raw_str = raw.to_string_lossy().to_string();
    let s = Command::new("ffmpeg")
        .args([
            "-y",
            "-v",
            "error",
            "-i",
            &raw_str,
            "-vf",
            "scale='min(1024,iw):min(1024,ih):force_original_aspect_ratio=decrease'",
            &out_str,
        ])
        .status()
        .ok()?;
    if s.success() { Some(out) } else { None }
}

/// Fetch the thumbnail via the yt-dlp binary (it sends the extractor's
/// Referer/Origin headers, which some CDNs require; GIO fetch is blocked
/// with 403 there). Best-effort: returns `None` on failure.
fn fetch_via_ytdlp(
    downloader: &yt_dlp::Downloader,
    webpage_url: &str,
    dir: &Path,
    video_id: &str,
) -> Option<PathBuf> {
    let exe = downloader.libraries().youtube.clone();
    let tmpl = format!("{}/{}.%(ext)s", dir.to_string_lossy(), video_id);
    let mut cmd_args = vec![
        "--no-progress".to_string(),
        "--skip-download".to_string(),
        "--write-thumbnail".to_string(),
        "--convert-thumbnails".to_string(),
        "jpg".to_string(),
        "--no-overwrites".to_string(),
        "-o".to_string(),
        tmpl,
    ];
    cmd_args.extend(downloader.args().iter().cloned());
    cmd_args.push(webpage_url.to_string());
    let status = Command::new(&exe).args(&cmd_args).status().ok()?;
    if !status.success() {
        eprintln!("[thumbnail] ytdlp fallback status={:?}", status);
        return None;
    }
    let jpg = dir.join(format!("{}.jpg", video_id));
    if jpg.is_file() {
        return Some(jpg);
    }
    let raw = dir.join(format!("{}.thumb", video_id));
    if raw.is_file() {
        return Some(raw);
    }
    None
}

/// Download the best thumbnail and convert it to JPEG. Returns the
/// path to display, or `None` when unavailable (caller shows icon).
pub(crate) fn download_thumbnail(
    downloader: &yt_dlp::Downloader,
    video: &yt_dlp::model::Video,
    dir: &Path,
) -> Option<PathBuf> {
    let _ = std::fs::create_dir_all(dir);
    let video_id = video.id.clone();
    let thumb = video.best_thumbnail()?;
    let url = thumb.url.clone();
    eprintln!("[thumbnail] fetching id={} url_len={}", video_id, url.len());
    if let Ok(bytes) = fetch_bytes(&url) {
        let raw = dir.join(format!("{}.thumb", video_id));
        let _ = std::fs::write(&raw, &bytes);
        let jpg = convert_thumbnail(&raw, dir, &video_id);
        let out = if jpg.is_some() {
            let _ = std::fs::remove_file(&raw);
            dir.join(format!("{}.jpg", video_id))
        } else {
            raw
        };
        eprintln!("[thumbnail] ok id={} path={}", video_id, out.display());
        return Some(out);
    }
    eprintln!("[thumbnail] gio fetch failed, trying ytdlp id={}", video_id);
    let page_url = video.webpage_url.clone().unwrap_or_default();
    if !page_url.trim().is_empty()
        && let Some(out) = fetch_via_ytdlp(downloader, &page_url, dir, &video_id)
    {
        eprintln!(
            "[thumbnail] ytdlp ok id={} path={}",
            video_id,
            out.display()
        );
        return Some(out);
    }
    eprintln!("[thumbnail] all fetches failed id={}", video_id);
    None
}
