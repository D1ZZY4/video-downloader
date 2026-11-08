//! Filename row shared by the Audio and Video pages.
//!
//! Empty means "use the video title" (the legacy behaviour). A custom
//! value is sanitized the same way as titles before becoming the base
//! filename (extension is added by the downloader).

use adw::prelude::*;

/// Build a Filename EntryRow (configured but not yet placed in a list).
pub fn filename_row() -> adw::EntryRow {
    let row = adw::EntryRow::new();
    row.set_title(&crate::i18n::tr("Filename"));
    row.set_activates_default(false);
    row.set_input_hints(
        gtk::InputHints::NO_SPELLCHECK | gtk::InputHints::NO_EMOJI | gtk::InputHints::NONE,
    );
    row.set_tooltip_text(Some(&crate::i18n::tr("Empty = use video title")));
    // Suffix: paste-from-clipboard button for convenience.
    let btn = gtk::Button::builder()
        .icon_name("edit-paste-symbolic")
        .tooltip_text(crate::i18n::tr("Paste"))
        .build();
    btn.add_css_class("flat");
    btn.set_can_focus(false);
    let entry = row.clone();
    btn.connect_clicked(move |_| {
        let entry = entry.clone();
        entry
            .clipboard()
            .read_text_async(Option::<&gio::Cancellable>::None, move |res| {
                if let Ok(Some(text)) = res {
                    entry.set_text(&text);
                }
            });
    });
    row.add_suffix(&btn);
    row
}
