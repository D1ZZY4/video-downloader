//! Background fetch of video metadata to populate resolutions.
//!
//! The resolution ComboRow stays hidden/disabled until the URL has been
//! fetched successfully. On success the model is updated via
//! `Model::on_video_fetched` (the same callback used by the downloader/queue)
//! and the view syncs the new choices. On failure the model is cleared and
//! the ComboRow stays hidden/disabled; the existing error path is unchanged.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use gtk::glib;
use gtk::subclass::prelude::*;
use yt_dlp::Downloader as YtDlpDownloader;

use crate::downloader::queue::fetch_video_tolerant;
use crate::window::window_builder::VideoDownloaderWindow;

/// Schedule a debounced resolution refresh (700 ms after the last call so
/// typing the URL does not spawn a fetch per keystroke).
pub(crate) fn schedule_refresh(window: &VideoDownloaderWindow) {
    // Generation guard instead of `SourceId::remove`: `remove` panics when
    // the timer already fired, which crashed the app on fast typing (e.g.
    // pasting a URL). Stale timers simply no-op when they fire.
    let my = {
        let mut g = crate::utils::lock_or_recover(&window.imp().prefetch_scheduled_seq);
        *g += 1;
        *g
    };
    let w = window.clone();
    glib::timeout_add_local(Duration::from_millis(700), move || {
        if *crate::utils::lock_or_recover(&w.imp().prefetch_scheduled_seq) != my {
            return glib::ControlFlow::Break;
        }
        refresh_resolutions(&w);
        glib::ControlFlow::Break
    });
}

/// Trigger a background fetch for the current video URL.
/// No-op when the URL is empty. Single-flight: only the latest fetch's
/// result is applied, so rapid retriggers do not race.
pub(crate) fn refresh_resolutions(window: &VideoDownloaderWindow) {
    let url = {
        let binding = window.model();
        let m = crate::utils::lock_or_recover(&binding);
        m.as_ref().map(|m| m.url.clone()).unwrap_or_default()
    };
    if url.trim().is_empty() {
        return;
    }
    let seq = {
        let mut g = crate::utils::lock_or_recover(&window.imp().prefetch_seq);
        *g += 1;
        *g
    };
    let w = window.clone();
    let url2 = url.clone();
    thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(_) => {
                return;
            }
        };
        rt.block_on(async move {
            let libs_dir = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join(".local/share/video-downloader/libs");
            let output_dir =
                PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                    .join("Downloads");
            let result = async {
                let mut downloader = YtDlpDownloader::with_new_binaries(&libs_dir, &output_dir)
                    .await
                    .map_err(|e| e.to_string())?
                    .build()
                    .await
                    .map_err(|e| e.to_string())?;
                fetch_video_tolerant(&mut downloader, &url2).await
            }
            .await;
            let w2 = w.clone();
            let outcome = result;
            match &outcome {
                Ok(v) => eprintln!(
                    "[prefetch] ok url={} id={} heights={} bitrates={}",
                    url2,
                    v.id,
                    crate::downloader::quality::heights_from_video(v).len(),
                    crate::downloader::quality::bitrates_from_video(v).len()
                ),
                Err(e) => eprintln!("[prefetch] err url={} err={}", url2, e),
            }
            glib::MainContext::default().invoke(move || {
                // Drop stale results: a newer fetch started meanwhile.
                if *crate::utils::lock_or_recover(&w2.imp().prefetch_seq) != seq {
                    return;
                }
                let binding = w2.model();
                {
                    let mut m = crate::utils::lock_or_recover(&binding);
                    if let Some(model) = m.as_mut() {
                        // Only apply if the URL still matches the fetched one.
                        if model.url == url2 {
                            match outcome {
                                Ok(video) => model.on_video_fetched(&video),
                                Err(_) => model.on_fetch_failed(),
                            }
                        }
                    }
                }
                w2.sync_from_model();
            });
        });
    });
}
