//! Metadata extraction for the start page and quality dialog.

use yt_dlp::model::Video;

/// Metadata shown on the start-page card and in the quality dialog.
#[derive(Debug, Clone)]
pub struct FetchedMetadata {
    pub title: String,
    pub channel: String,
    pub duration: i64,
    pub thumbnail_url: String,
    pub video_options: Vec<VideoOption>,
    pub audio_options: Vec<AudioOption>,
}

/// One selectable video format combination.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoOption {
    pub height: u32,
    pub ext: String,
    pub label: String,
}

/// One selectable audio format container.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioOption {
    pub ext: String,
    pub label: String,
}

/// Format seconds as `m:ss` or `h:mm:ss`.
pub fn format_duration(secs: i64) -> String {
    if secs < 0 {
        return String::new();
    }
    let h = secs / 3600;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{}:{:02}", m, s)
    }
}

fn fmt_ext(ext: &str) -> String {
    ext.to_ascii_uppercase()
}

/// All video combinations (every height x container the URL has).
pub fn video_options(video: &Video) -> Vec<VideoOption> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for f in &video.formats {
        if f.format_type().is_storyboard() {
            continue;
        }
        let Some(h) = f.video_resolution.height else {
            continue;
        };
        if h == 0 {
            continue;
        }
        let ext = f.download_info.ext.as_str().to_ascii_lowercase();
        let ext = if ext == "bin" || ext.is_empty() {
            "mp4".to_string()
        } else {
            ext
        };
        if !seen.insert((h, ext.clone())) {
            continue;
        }
        out.push(VideoOption {
            label: format!("{}p ({})", h, fmt_ext(&ext)),
            height: h,
            ext,
        });
    }
    out.sort_by(|a, b| b.height.cmp(&a.height).then(a.ext.cmp(&b.ext)));
    out
}

/// All audio containers the URL has.
pub fn audio_options(video: &Video) -> Vec<AudioOption> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for f in &video.formats {
        if f.format_type().is_storyboard() {
            continue;
        }
        let has_audio = f.is_audio()
            || f.rates_info.audio_rate.is_some()
            || f.codec_info.audio_codec.is_some()
            || f.format_type().is_audio_and_video();
        if !has_audio {
            continue;
        }
        let ext = f.download_info.ext.as_str().to_ascii_lowercase();
        if ext == "bin" || ext.is_empty() || !seen.insert(ext.clone()) {
            continue;
        }
        let label = match ext.as_str() {
            "mp3" => "MP3".to_string(),
            "opus" | "ogg" => "Opus".to_string(),
            "m4a" | "aac" => "AAC".to_string(),
            "flac" => "FLAC".to_string(),
            _ => fmt_ext(&ext),
        };
        out.push(AudioOption { ext, label });
    }
    if out.is_empty() {
        // Direct MP4 HLS etc. still carry audio; fall back to the container set.
        for f in &video.formats {
            if f.format_type().is_storyboard() {
                continue;
            }
            let ext = f.download_info.ext.as_str().to_ascii_lowercase();
            if ext == "bin" || ext.is_empty() || !seen.insert(ext.clone()) {
                continue;
            }
            out.push(AudioOption {
                label: fmt_ext(&ext),
                ext,
            });
        }
        out.sort_by(|a, b| a.label.cmp(&b.label));
    }
    out
}

/// Extract display metadata from a fetched video.
pub fn from_video(video: &Video) -> FetchedMetadata {
    FetchedMetadata {
        title: video.title.clone(),
        channel: video.channel.clone().unwrap_or_default(),
        duration: video.duration.unwrap_or(-1),
        thumbnail_url: video
            .best_thumbnail()
            .map(|t| t.url.clone())
            .unwrap_or_default(),
        video_options: video_options(video),
        audio_options: audio_options(video),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_formats() {
        assert_eq!(format_duration(65), "1:05");
        assert_eq!(format_duration(3605), "1:00:05");
        assert_eq!(format_duration(-1), "");
    }
}
