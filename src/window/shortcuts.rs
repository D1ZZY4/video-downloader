use adw::prelude::*;

use crate::window::Window;

pub fn show(parent: &Window) {
    let dialog = adw::ShortcutsDialog::new();

    let general = adw::ShortcutsSection::new(Some(&crate::i18n::tr("General")));
    general.add(adw::ShortcutsItem::new(
        &crate::i18n::tr("Open Menu"),
        "F10",
    ));
    general.add(adw::ShortcutsItem::new(
        &crate::i18n::tr("New Window"),
        "<Primary>N",
    ));
    general.add(adw::ShortcutsItem::new(
        &crate::i18n::tr("Close Window"),
        "<Primary>W",
    ));
    general.add(adw::ShortcutsItem::new(
        &crate::i18n::tr("Quit"),
        "<Primary>Q",
    ));
    general.add(adw::ShortcutsItem::new(
        &crate::i18n::tr("Keyboard Shortcuts"),
        "<Primary>question",
    ));

    let navigation = adw::ShortcutsSection::new(Some(&crate::i18n::tr("Navigation")));
    navigation.add(adw::ShortcutsItem::new(
        &crate::i18n::tr("Go back"),
        "<Alt>Left",
    ));
    navigation.add(adw::ShortcutsItem::new(
        &crate::i18n::tr("Switch to Audio"),
        "<Alt>A",
    ));
    navigation.add(adw::ShortcutsItem::new(
        &crate::i18n::tr("Switch to Video"),
        "<Alt>V",
    ));

    dialog.add(general);
    dialog.add(navigation);
    dialog.present(Some(parent));
}
