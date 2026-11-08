//! Window facade. Implementation lives in focused submodules.
//!
//! Layout:
//! - `window_builder`: widget construction, menus, shortcuts, actions.
//! - `audio` / `video`: start-page builders per tab.
//! - `state_sync`: model to view synchronization (including qualities).
//! - `thumbnail`: download page thumbnail display.
//! - `format_info`: download info and path formatting helpers.
//! - `prefetch`: background metadata fetch that populates qualities.
//! - `about` / `shortcuts`: info dialogs shown from the main menu.

#[path = "about.rs"]
pub mod about;
#[path = "audio.rs"]
pub mod audio;
#[path = "filename.rs"]
pub(crate) mod filename;
#[path = "format_info.rs"]
pub(crate) mod format_info;
#[path = "../dialogs/notification.rs"]
mod notification;
#[path = "prefetch.rs"]
pub(crate) mod prefetch;
#[path = "shortcuts.rs"]
pub mod shortcuts;
#[path = "state_sync.rs"]
mod state_sync;
#[path = "thumbnail.rs"]
mod thumbnail;
#[path = "video.rs"]
pub mod video;
#[path = "window_builder.rs"]
mod window_builder;

pub use window_builder::VideoDownloaderWindow;
pub type Window = VideoDownloaderWindow;

// Note: Window is moved across threads in the download engine via
// MainContext::invoke. The unsafe Send/Sync impls below are retained because
// removing them breaks the build (gtk widgets are not Send). All widget
// access from worker threads goes through MainContext::invoke; the model
// mutex is the only cross-thread state.
unsafe impl Send for VideoDownloaderWindow {}
unsafe impl Sync for VideoDownloaderWindow {}

/// Helper used by the model when the download folder is invalid.
pub fn show_download_folder_error(parent: &Window, message: &str, path: &str) {
    crate::dialogs::folder::show_error(
        parent,
        "Invalid download folder",
        message,
        Some(path),
        true,
    );
}
