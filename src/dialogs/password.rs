use adw::prelude::*;
use std::time::Duration;

use crate::dialogs::common;
use crate::window::Window;

/// Ask the user for a video password.
pub fn request(parent: &Window) -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let parent = parent.clone();
    gtk::glib::MainContext::default().invoke(move || {
        let (dialog, vbox, cancel_btn, ok_btn) = common::build_auth_dialog();
        let pass_entry = adw::PasswordEntryRow::builder()
            .title(crate::i18n::tr("Password"))
            .build();
        common::insert_entry(&vbox, &pass_entry);

        let tx_clone = tx.clone();
        let dialog_cancel = dialog.clone();
        cancel_btn.connect_clicked(move |_| {
            let _ = tx_clone.send(None);
            dialog_cancel.close();
        });
        let pass_c = pass_entry.clone();
        let dialog_ok = dialog.clone();
        let tx_ok = tx.clone();
        ok_btn.connect_clicked(move |_| {
            let _ = tx_ok.send(Some(pass_c.text().to_string()));
            dialog_ok.close();
        });
        // Esc / X without clicking a button must still unblock the worker.
        let tx_closed = tx.clone();
        dialog.connect_closed(move |_| {
            let _ = tx_closed.send(None);
        });

        dialog.present(Some(&parent));
    });
    crate::utils::recv_timeout(&rx, Duration::from_secs(600)).unwrap_or(None)
}
