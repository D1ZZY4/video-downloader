//! Audio page (URL + Filename + Audio Quality + Download).

use adw::prelude::*;

/// Build and register the Audio page in `stack`.
pub fn add_audio_page(
    stack: &adw::ViewStack,
    url_wdg: &adw::EntryRow,
    filename_wdg: &adw::EntryRow,
    quality_wdg: &adw::ComboRow,
    formats_btn: &gtk::Button,
    download_wdg: &gtk::Button,
) {
    let list = gtk::ListBox::builder()
        .valign(gtk::Align::Start)
        .selection_mode(gtk::SelectionMode::None)
        .build();
    list.add_css_class("boxed-list");
    list.append(url_wdg);
    list.append(filename_wdg);
    list.append(quality_wdg);
    list.append(formats_btn);
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_start(24)
        .margin_end(24)
        .margin_top(12)
        .margin_bottom(24)
        .build();
    content.append(&list);
    // Full-width download button for the redesigned start page.
    download_wdg.set_halign(gtk::Align::Fill);
    content.append(download_wdg);
    let clamp = adw::Clamp::builder()
        .valign(gtk::Align::Center)
        .vexpand(true)
        .maximum_size(720)
        .build();
    clamp.set_child(Some(&content));
    let title = crate::i18n::tr("Audio");
    stack.add_titled_with_icon(&clamp, Some("audio"), &title, "audio-x-generic-symbolic");
}
