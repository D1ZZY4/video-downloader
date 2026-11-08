use adw::prelude::*;

/// Build the shared shell for the private-video dialogs (login/password).
/// Returns the dialog, its content box, and the OK/Cancel buttons.
/// UI strings are preserved exactly as before.
pub fn build_auth_dialog() -> (adw::Dialog, gtk::Box, gtk::Button, gtk::Button) {
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
        .label(crate::i18n::tr("The video is not public."))
        .xalign(0.0)
        .build();
    let btn_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .halign(gtk::Align::End)
        .build();
    let cancel_btn = gtk::Button::builder()
        .label(crate::i18n::tr("Cancel"))
        .build();
    let ok_btn = gtk::Button::builder().label(crate::i18n::tr("OK")).build();
    ok_btn.add_css_class("suggested-action");

    btn_box.append(&cancel_btn);
    btn_box.append(&ok_btn);
    vbox.append(&label);
    vbox.append(&btn_box);
    clamp.set_child(Some(&vbox));
    dialog.set_child(Some(&clamp));
    (dialog, vbox, cancel_btn, ok_btn)
}

/// Insert `entry` before the trailing button box.
pub fn insert_entry<W: IsA<gtk::Widget>>(vbox: &gtk::Box, entry: &W) {
    let btn_box = vbox
        .last_child()
        .and_then(|c| c.downcast::<gtk::Box>().ok());
    if let Some(ref bx) = btn_box {
        vbox.insert_before(entry, Some(bx));
    } else {
        vbox.append(entry);
    }
}
