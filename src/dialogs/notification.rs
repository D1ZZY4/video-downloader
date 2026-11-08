//! Notifications for background downloads.

use adw::prelude::*;
use gtk::subclass::prelude::*;

use crate::window::window_builder::VideoDownloaderWindow;

impl VideoDownloaderWindow {
    pub(crate) fn withdraw_notification(&self) {
        if let Some(app) = self.application() {
            let id = self.imp().notification_id.clone();
            app.withdraw_notification(&id);
        }
    }

    pub(crate) fn update_notification(&self, state: &str) {
        let id = self.imp().notification_id.clone();
        // Withdraw the previous notification on every state change, like
        // legacy `_hide_notification` + `_update_notification`.
        self.withdraw_notification();
        // Only notify when the window is in the background.
        if self.is_active() {
            return;
        }
        let (title, body, action) = match state {
            "error" => (
                crate::i18n::tr("Download failed"),
                String::new(),
                format!("app.notification-error--{}", id),
            ),
            "success" => {
                let body = {
                    let binding = self.model();
                    let m = crate::utils::lock_or_recover(&binding);
                    m.as_ref()
                        .map(|m| m.download_titles.join(" | "))
                        .unwrap_or_default()
                };
                (
                    crate::i18n::tr("Download finished"),
                    body,
                    format!("app.notification-success--{}", id),
                )
            }
            _ => return,
        };
        if let Some(app) = self.application() {
            let notification = gio::Notification::new(&title);
            if !body.is_empty() {
                notification.set_body(Some(&body));
            }
            notification.set_default_action(&action);
            if state == "success" {
                notification.add_button(
                    &crate::i18n::tr("Open Download Location"),
                    &format!("app.notification-open-finished-download-dir--{}", id),
                );
            }
            app.send_notification(Some(&id), &notification);
        }
    }
}
