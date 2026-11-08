use adw::prelude::*;

use crate::model::check_download_dir;
use crate::utils;
use crate::window::Window;

/// Show "Change Download Location" folder picker.
pub fn change_download_folder(parent: &Window) {
    let dialog = gtk::FileDialog::builder()
        .title(crate::i18n::tr("Change Download Location"))
        .modal(true)
        .accept_label(crate::i18n::tr("Select Folder"))
        .build();
    let w = parent.clone();
    let w_parent = w.clone();
    dialog.select_folder(
        Some(&w_parent),
        Option::<&gio::Cancellable>::None,
        move |result| {
            let w2 = w.clone();
            match result {
                Ok(file) => {
                    if let Some(path) = file.path() {
                        let path_str = path.to_string_lossy().to_string();
                        if let Some(msg) = check_download_dir(&path_str, false) {
                            show_error(
                                &w2,
                                &crate::i18n::tr("Invalid folder selected"),
                                &msg,
                                Some(&path_str),
                                false,
                            );
                            return;
                        }
                        {
                            let binding = w2.model();
                            if let Some(m) = utils::lock_or_recover(&binding).as_mut() {
                                m.download_folder = path_str;
                                m.save_settings();
                            }
                        }
                        w2.sync_from_model();
                        // Retry the download when a folder error blocked it,
                        // like the legacy response chaining.
                        let retried = {
                            let binding = w2.model();
                            let mut guard = utils::lock_or_recover(&binding);
                            guard
                                .as_mut()
                                .map(|m| m.retry_pending_folder_download())
                                .unwrap_or(false)
                        };
                        if retried {
                            w2.sync_from_model();
                        }
                        return;
                    }
                    show_error(
                        &w2,
                        &crate::i18n::tr("Invalid folder selected"),
                        &crate::i18n::tr("Not a directory"),
                        None,
                        false,
                    );
                }
                Err(_) => {
                    // Dialog cancelled.
                }
            }
        },
    );
}

/// Show invalid download folder error with Reset / Change buttons.
pub fn show_error(
    parent: &Window,
    title: &str,
    message: &str,
    path: Option<&str>,
    show_reset: bool,
) {
    let detail = match path {
        Some(p) if !message.is_empty() => format!("{}: {:?}", message, p),
        Some(p) => format!("{:?}", p),
        None => message.to_string(),
    };
    let dialog = adw::AlertDialog::builder()
        .heading(title)
        .body(&detail)
        .build();
    dialog.add_response("cancel", &crate::i18n::tr("Cancel"));
    if show_reset {
        dialog.add_response("reset", &crate::i18n::tr("Reset to Default"));
    }
    dialog.add_response("change", &crate::i18n::tr("Change Download Location"));
    dialog.set_close_response("cancel");
    dialog.set_default_response(Some("change"));
    let w = parent.clone();
    dialog.connect_response(None, move |d: &adw::AlertDialog, response: &str| {
        match response {
            "change" => {
                change_download_folder(&w);
            }
            "reset" => {
                {
                    let binding = w.model();
                    let mut guard = utils::lock_or_recover(&binding);
                    if let Some(m) = guard.as_mut() {
                        if let Some(s) = &m.settings {
                            s.reset("download-folder");
                            let v: String = s.get("download-folder");
                            m.download_folder = utils::expand_path(&v);
                            m.save_settings();
                        } else {
                            m.download_folder = "xdg-download/VideoDownloader".to_string();
                        }
                    }
                }
                w.sync_from_model();
                let retried = {
                    let binding = w.model();
                    let mut guard = utils::lock_or_recover(&binding);
                    guard
                        .as_mut()
                        .map(|m| m.retry_pending_folder_download())
                        .unwrap_or(false)
                };
                if !retried {
                    // No pending retry: start fresh like the old behaviour.
                    let binding = w.model();
                    let mut guard = utils::lock_or_recover(&binding);
                    if let Some(m) = guard.as_mut() {
                        m.set_state("prepare");
                    }
                }
                w.sync_from_model();
            }
            _ => {
                // Cancel: return to start state and drop the retry flag.
                let binding = w.model();
                let mut m = utils::lock_or_recover(&binding);
                if let Some(m) = m.as_mut()
                    && m.state == "prepare"
                {
                    m.state = "start".to_string();
                    m.pending_folder_retry = false;
                }
                drop(m);
                w.sync_from_model();
            }
        }
        d.close();
    });
    dialog.present(Some(parent));
}
