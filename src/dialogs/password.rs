use adw::prelude::*;

use crate::window::Window;

/// Ask the user for a video password.
pub fn request(parent: &Window) -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let parent = parent.clone();
    gtk::glib::MainContext::default().invoke(move || {
        let dialog = adw::Dialog::builder().build();
        let clamp = adw::Clamp::builder().build();
        let vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();
        let label = gtk::Label::builder()
            .label("The video is not public.")
            .xalign(0.0)
            .build();
        let pass_entry = adw::PasswordEntryRow::builder().title("Password").build();
        let btn_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .halign(gtk::Align::End)
            .build();
        let cancel_btn = gtk::Button::builder().label("Cancel").build();
        let ok_btn = gtk::Button::builder().label("OK").build();
        ok_btn.add_css_class("suggested-action");

        btn_box.append(&cancel_btn);
        btn_box.append(&ok_btn);
        vbox.append(&label);
        vbox.append(&pass_entry);
        vbox.append(&btn_box);
        clamp.set_child(Some(&vbox));
        dialog.set_child(Some(&clamp));

        let tx_clone = tx.clone();
        let dialog_cancel = dialog.clone();
        cancel_btn.connect_clicked(move |_| {
            let _ = tx_clone.send(None);
            dialog_cancel.close();
        });
        let pass_c = pass_entry.clone();
        let dialog_ok = dialog.clone();
        ok_btn.connect_clicked(move |_| {
            let _ = tx.send(Some(pass_c.text().to_string()));
            dialog_ok.close();
        });

        dialog.present(Some(&parent));
    });
    rx.recv().unwrap_or(None)
}
