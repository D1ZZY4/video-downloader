use adw::prelude::*;

use crate::window::Window;

/// Ask the user for a playlist download confirmation.
pub fn request(parent: &Window) -> bool {
    let (tx, rx) = std::sync::mpsc::channel();
    let parent = parent.clone();
    gtk::glib::MainContext::default().invoke(move || {
        let dialog = adw::AlertDialog::builder()
            .heading("Download Playlist?")
            .body("The video is part of a playlist.")
            .build();
        dialog.add_response("no", "Single Video");
        dialog.add_response("yes", "All Videos in Playlist");
        dialog.set_close_response("no");
        let tx = tx.clone();
        dialog.connect_response(None, move |d: &adw::AlertDialog, response: &str| {
            let _ = tx.send(response == "yes");
            d.close();
        });
        dialog.present(Some(&parent));
    });
    rx.recv().unwrap_or(false)
}
