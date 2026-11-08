//! Quality selection for audio and video (derived from the URL's formats).
//!
//! The resolution (video) and bitrate (audio) lists are built from the
//! fetched video's formats, so they adapt to the URL itself instead of a
//! static list. The ComboRows stay hidden until a fetch succeeds.

use crate::model::{BEST_HEIGHT_CAP, DEFAULT_RESOLUTION, MAX_RESOLUTION};

/// Human readable label for a height, e.g. "1080p (HD)".
pub fn resolution_label(height: u32) -> String {
    let suffix = match height {
        2160..=4320 => " (8K)",
        1440 => " (HD)",
        1080 => " (HD)",
        720 => " (HD)",
        _ => "",
    };
    format!("{}p{}", height, suffix)
}

/// Normalize raw heights: drop zero, cap at 4320, sort descending, dedupe.
pub fn normalize_heights(heights: &[u32]) -> Vec<u32> {
    let mut out: Vec<u32> = heights
        .iter()
        .copied()
        .filter(|h| *h > 0)
        .map(|h| h.min(BEST_HEIGHT_CAP))
        .collect();
    out.sort_unstable_by(|a, b| b.cmp(a));
    out.dedup();
    out
}

/// Build the Video Quality choices from heights. Always includes Best first.
pub fn build_resolutions(heights: &[u32]) -> Vec<(u32, String)> {
    let mut out = vec![(MAX_RESOLUTION, crate::i18n::tr("Best"))];
    for h in normalize_heights(heights) {
        out.push((h, resolution_label(h)));
    }
    out
}

/// Heights available in a fetched video's formats.
///
/// Uses the `height` field directly instead of `is_video()`: sites that
/// serve only muxed HLS (`Manifest` type) or direct URLs without codec
/// fields classify as non-video in the crate, but their heights are still
/// the real video qualities. Storyboards are excluded.
pub fn heights_from_video(video: &yt_dlp::model::Video) -> Vec<u32> {
    let heights: Vec<u32> = video
        .formats
        .iter()
        .filter(|f| !f.format_type().is_storyboard())
        .filter_map(|f| f.video_resolution.height)
        .collect();
    normalize_heights(&heights)
}

/// Pick the resolution to select from a newly built list.
/// Keeps `prev` when present, else 1080 when present, else Best.
pub fn choose_resolution(prev: u32, resolutions: &[(u32, String)]) -> u32 {
    if resolutions.iter().any(|(r, _)| *r == prev) {
        prev
    } else if resolutions.iter().any(|(r, _)| *r == DEFAULT_RESOLUTION) {
        DEFAULT_RESOLUTION
    } else {
        MAX_RESOLUTION
    }
}

/// Normalize a raw `audio-quality` string (schema/UI) to `best` or a
/// bitrate in kbps (e.g. `"128"`). Legacy tier names map to bitrates.
pub fn normalize_audio_quality(raw: &str) -> String {
    let t = raw.trim();
    if t == "best" {
        return "best".to_string();
    }
    match t {
        "high" => return "192".to_string(),
        "medium" => return "128".to_string(),
        "low" => return "96".to_string(),
        "worst" => return "64".to_string(),
        _ => {}
    }
    if let Ok(n) = t.parse::<u32>()
        && (8..=1024).contains(&n)
    {
        return n.to_string();
    }
    "best".to_string()
}

/// Fallback choices when no fetch is possible; normally unused because
/// the ComboRow stays hidden until a fetch succeeds.
pub fn audio_quality_options() -> Vec<(String, String)> {
    vec![("best".to_string(), crate::i18n::tr("Best"))]
}

/// Normalize raw bitrates: drop zero, round, sort descending, dedupe.
pub fn normalize_bitrates(rates: &[f64]) -> Vec<u32> {
    let mut out: Vec<u32> = rates
        .iter()
        .filter(|r| **r > 1.0)
        .map(|r| r.round() as u32)
        .filter(|b| *b > 0)
        .collect();
    out.sort_unstable_by(|a, b| b.cmp(a));
    out.dedup();
    out
}

/// Build the Audio Quality choices from bitrates. Always includes Best first.
pub fn build_audio_qualities(bitrates: &[u32]) -> Vec<(String, String)> {
    let mut out = vec![("best".to_string(), crate::i18n::tr("Best"))];
    for b in normalize_bitrates(&bitrates.iter().map(|b| *b as f64).collect::<Vec<_>>()) {
        out.push((b.to_string(), format!("{} kbps", b)));
    }
    out
}

/// Bitrates available in a fetched video's audio formats.
///
/// Uses `rates_info.audio_rate` directly instead of `is_audio()` for the
/// same reason as [`heights_from_video`]: muxed/HLS formats classify as
/// non-audio in the crate but may still carry a real audio bitrate.
pub fn bitrates_from_video(video: &yt_dlp::model::Video) -> Vec<u32> {
    let rates: Vec<f64> = video
        .formats
        .iter()
        .filter(|f| !f.format_type().is_storyboard())
        .filter_map(|f| f.rates_info.audio_rate.map(|r| *r))
        .collect();
    normalize_bitrates(&rates)
}

/// Pick the audio quality to select from a newly built list.
/// Keeps `prev` when present, else Best.
pub fn choose_audio_quality(prev: &str, qualities: &[(String, String)]) -> String {
    let prev = normalize_audio_quality(prev);
    if qualities.iter().any(|(v, _)| *v == prev) {
        prev
    } else {
        "best".to_string()
    }
}

/// Map a model quality string (`best` or bitrate kbps) to the crate enum.
pub fn crate_audio_quality(q: &str) -> yt_dlp::model::selector::AudioQuality {
    use yt_dlp::model::selector::AudioQuality;
    match normalize_audio_quality(q).as_str() {
        "192" => AudioQuality::High,
        "128" => AudioQuality::Medium,
        "96" => AudioQuality::Low,
        "64" => AudioQuality::Worst,
        "best" => AudioQuality::Best,
        other => other
            .parse::<u32>()
            .map(AudioQuality::CustomBitrate)
            .unwrap_or(AudioQuality::Best),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_sorts_dedupes_caps() {
        assert_eq!(
            normalize_heights(&[360, 1080, 720, 1080, 0, 9999]),
            vec![4320, 1080, 720, 360]
        );
    }

    #[test]
    fn build_resolutions_always_has_best_first() {
        let res = build_resolutions(&[720, 1080]);
        assert_eq!(res[0], (MAX_RESOLUTION, crate::i18n::tr("Best")));
        assert_eq!(res[1].0, 1080);
        assert_eq!(res[2].0, 720);
    }

    #[test]
    fn labels() {
        assert_eq!(resolution_label(1080), "1080p (HD)");
        assert_eq!(resolution_label(480), "480p");
    }

    #[test]
    fn choose_preserves_valid() {
        let res = build_resolutions(&[1080, 720]);
        assert_eq!(choose_resolution(720, &res), 720);
    }

    #[test]
    fn choose_falls_back_to_best() {
        let res = build_resolutions(&[480, 360]);
        assert_eq!(choose_resolution(1080, &res), MAX_RESOLUTION);
    }

    #[test]
    fn audio_quality_normalize() {
        assert_eq!(normalize_audio_quality("high"), "192");
        assert_eq!(normalize_audio_quality("128"), "128");
        assert_eq!(normalize_audio_quality("bogus"), "best");
    }

    #[test]
    fn audio_qualities_from_bitrates() {
        let q = build_audio_qualities(&[128, 256, 128, 0]);
        assert_eq!(q[0].0, "best");
        assert_eq!(q[1].0, "256");
        assert_eq!(q[2].0, "128");
        assert_eq!(choose_audio_quality("128", &q), "128");
        assert_eq!(choose_audio_quality("64", &q), "best");
        assert_eq!(normalize_bitrates(&[128.4, 64.6, 0.0]), vec![128, 65]);
    }
}
