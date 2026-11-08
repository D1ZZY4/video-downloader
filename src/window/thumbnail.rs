//! Thumbnail display for the download page.

use adw::prelude::*;
use gtk::subclass::prelude::*;

use crate::window::window_builder::VideoDownloaderWindow;

impl VideoDownloaderWindow {
    pub(crate) fn show_thumbnail(&self, thumbnail: &str) {
        // Skip reloading when the path is unchanged (called on every sync).
        if let Ok(cache) = self.imp().last_thumbnail.lock()
            && cache.as_deref() == Some(thumbnail)
        {
            return;
        }
        const IMAGE_SIZE: i32 = 128;
        // Remove any previous thumbnail child to avoid leaking stack children.
        while let Some(child) = self.imp().download_images_wdg.first_child() {
            self.imp().download_images_wdg.remove(&child);
        }
        eprintln!("[window] show_thumbnail {}", thumbnail);
        match gtk::gdk::Texture::from_filename(thumbnail) {
            Ok(texture) => {
                eprintln!("[window] thumbnail loaded {}", thumbnail);
                let pic = gtk::Picture::for_paintable(&texture);
                pic.set_height_request(IMAGE_SIZE);
                pic.set_content_fit(gtk::ContentFit::Contain);
                self.imp().download_images_wdg.add_child(&pic);
                self.imp().download_images_wdg.set_visible_child(&pic);
            }
            Err(_) => {
                let img = gtk::Image::builder()
                    .icon_name("video-x-generic")
                    .pixel_size(IMAGE_SIZE)
                    .height_request(IMAGE_SIZE)
                    .build();
                self.imp().download_images_wdg.add_child(&img);
                self.imp().download_images_wdg.set_visible_child(&img);
            }
        }
        if let Ok(mut cache) = self.imp().last_thumbnail.lock() {
            *cache = Some(thumbnail.to_string());
        }
    }
}
