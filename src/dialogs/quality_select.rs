//! Quality selection grid dialog (all formats, adapted to the URL).
//!
//! Mirrors the Telegram-bot style: metadata card on top, then a button
//! grid with every resolution x container and audio container the URL
//! actually has, plus Best shortcuts and Cancel.

use adw::prelude::*;

use crate::downloader::extract_metadata::{FetchedMetadata, format_duration};

/// Sync a quality ComboRow with the model's choices.
/// Hides and disables the row until a fetch populates `qualities`.
pub fn sync_combo_row(combo: &adw::ComboRow, qualities: &[(String, String)], selected: usize) {
    if qualities.is_empty() {
        combo.set_model(Some(&gtk::StringList::new(&[])));
        combo.set_visible(false);
        combo.set_sensitive(false);
        return;
    }
    combo.set_visible(true);
    combo.set_sensitive(true);
    let desired: Vec<&str> = qualities.iter().map(|(_, d)| d.as_str()).collect();
    let needs_rebuild = match combo.model() {
        Some(model_list) => {
            if model_list.n_items() as usize != desired.len() {
                true
            } else if let Some(list) = model_list.downcast_ref::<gtk::StringList>() {
                desired.iter().enumerate().any(|(i, want)| {
                    list.string(i as u32).map(|s| s.to_string()) != Some((*want).to_string())
                })
            } else {
                true
            }
        }
        None => true,
    };
    if needs_rebuild {
        combo.set_model(Some(&gtk::StringList::new(&desired)));
    }
    combo.set_selected(selected as u32);
}

/// Show a scrollable grid dialog with metadata header + option buttons.
/// `on_pick` is called with `Some(label)` for a picked option, `None` on
/// cancel/close. Non-blocking: safe to call from a UI handler.
pub fn show_grid<F>(
    parent: &crate::window::Window,
    title: &str,
    meta: &FetchedMetadata,
    video_labels: Vec<String>,
    audio_labels: Vec<String>,
    on_pick: F,
) where
    F: FnOnce(Option<String>) + Send + 'static,
{
    use std::sync::{Arc, Mutex};
    let on_pick = Arc::new(Mutex::new(Some(on_pick)));
    let parent = parent.clone();
    let title = title.to_string();
    let meta = meta.clone();
    gtk::glib::MainContext::default().invoke(move || {
        let dialog = adw::Dialog::builder()
            .title(title.clone())
            .content_width(640)
            .content_height(480)
            .build();
        let outer = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();
        // Metadata header.
        let title_label = gtk::Label::builder()
            .label(meta.title.clone())
            .wrap(true)
            .halign(gtk::Align::Start)
            .build();
        title_label.add_css_class("heading");
        let dur = format_duration(meta.duration);
        let sub = gtk::Label::builder()
            .label(format!("{} · {}", meta.channel, dur))
            .halign(gtk::Align::Start)
            .build();
        sub.add_css_class("dim-label");
        outer.append(&title_label);
        outer.append(&sub);
        // Video grid.
        if !video_labels.is_empty() {
            outer.append(
                &gtk::Label::builder()
                    .label(crate::i18n::tr("Video"))
                    .halign(gtk::Align::Start)
                    .build(),
            );
            let grid = gtk::Grid::builder()
                .column_spacing(8)
                .row_spacing(8)
                .build();
            for (i, label) in video_labels.iter().enumerate() {
                let btn = gtk::Button::builder().label(label.clone()).build();
                let lbl = label.clone();
                let dlg = dialog.clone();
                let cb = on_pick.clone();
                btn.connect_clicked(move |_| {
                    if let Some(cb) = cb.lock().ok().and_then(|mut g| g.take()) {
                        cb(Some(lbl.clone()));
                    }
                    dlg.close();
                });
                grid.attach(&btn, (i % 3) as i32, (i / 3) as i32, 1, 1);
            }
            outer.append(&grid);
        }
        // Audio grid.
        if !audio_labels.is_empty() {
            outer.append(
                &gtk::Label::builder()
                    .label(crate::i18n::tr("Audio"))
                    .halign(gtk::Align::Start)
                    .build(),
            );
            let grid = gtk::Grid::builder()
                .column_spacing(8)
                .row_spacing(8)
                .build();
            for (i, label) in audio_labels.iter().enumerate() {
                let btn = gtk::Button::builder().label(label.clone()).build();
                let lbl = label.clone();
                let dlg = dialog.clone();
                let cb = on_pick.clone();
                btn.connect_clicked(move |_| {
                    if let Some(cb) = cb.lock().ok().and_then(|mut g| g.take()) {
                        cb(Some(lbl.clone()));
                    }
                    dlg.close();
                });
                grid.attach(&btn, (i % 3) as i32, (i / 3) as i32, 1, 1);
            }
            outer.append(&grid);
        }
        let cancel = gtk::Button::builder()
            .label(crate::i18n::tr("Cancel"))
            .build();
        cancel.add_css_class("destructive-action");
        let dlg = dialog.clone();
        let cb = on_pick.clone();
        cancel.connect_clicked(move |_| {
            if let Some(cb) = cb.lock().ok().and_then(|mut g| g.take()) {
                cb(None);
            }
            dlg.close();
        });
        outer.append(&cancel);
        let scrolled = gtk::ScrolledWindow::builder()
            .child(&outer)
            .vexpand(true)
            .build();
        dialog.set_child(Some(&scrolled));
        // Dismissals (Esc/X) return None.
        let cb = on_pick.clone();
        dialog.connect_closed(move |_| {
            if let Some(cb) = cb.lock().ok().and_then(|mut g| g.take()) {
                cb(None);
            }
        });
        dialog.present(Some(&parent));
    });
}
