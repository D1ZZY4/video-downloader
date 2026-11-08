use adw::prelude::*;
use std::time::Duration;

use crate::window::Window;

/// Ask the user for a playlist download confirmation.
pub fn request(parent: &Window) -> bool {
    let (tx, rx) = std::sync::mpsc::channel();
    let parent = parent.clone();
    gtk::glib::MainContext::default().invoke(move || {
        let dialog = adw::AlertDialog::builder()
            .heading(crate::i18n::tr("Download Playlist?"))
            .body(crate::i18n::tr("The video is part of a playlist."))
            .build();
        dialog.add_response("no", &crate::i18n::tr("Single Video"));
        dialog.add_response("yes", &crate::i18n::tr("All Videos in Playlist"));
        dialog.set_close_response("no");
        dialog.set_default_response(Some("no"));
        let tx_resp = tx.clone();
        dialog.connect_response(None, move |d: &adw::AlertDialog, response: &str| {
            let _ = tx_resp.send(response == "yes");
            d.close();
        });
        // Esc / X without a response must still unblock the worker.
        let tx_closed = tx.clone();
        dialog.connect_closed(move |_| {
            let _ = tx_closed.send(false);
        });
        dialog.present(Some(&parent));
    });
    crate::utils::recv_timeout(&rx, Duration::from_secs(600)).unwrap_or(false)
}
