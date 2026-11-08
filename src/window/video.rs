//! Video page (URL + Filename + Video Quality + Prefer MPEG + Download).

use adw::prelude::*;

/// Build and register the Video page in `stack`.
pub fn add_video_page(
    stack: &adw::ViewStack,
    url_wdg: &adw::EntryRow,
    filename_wdg: &adw::EntryRow,
    quality_wdg: &adw::ComboRow,
    prefer_mpeg_wdg: &gtk::Switch,
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
    let row = adw::ActionRow::builder()
        .title(crate::i18n::tr("Prefer MPEG format"))
        .subtitle(crate::i18n::tr("Prefer H.264 video"))
        .build();
    row.set_activatable_widget(Some(prefer_mpeg_wdg));
    row.add_suffix(prefer_mpeg_wdg);
    list.append(&row);
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_start(24)
        .margin_end(24)
        .margin_top(12)
        .margin_bottom(24)
        .build();
    content.append(&list);
    download_wdg.set_halign(gtk::Align::Fill);
    content.append(download_wdg);
    let clamp = adw::Clamp::builder()
        .valign(gtk::Align::Center)
        .vexpand(true)
        .maximum_size(720)
        .build();
    clamp.set_child(Some(&content));
    let title = crate::i18n::tr("Video");
    stack.add_titled_with_icon(&clamp, Some("video"), &title, "video-x-generic-symbolic");
}
