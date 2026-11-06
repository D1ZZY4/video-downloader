use adw::prelude::*;

use crate::model::check_download_dir;
use crate::utils;
use crate::window::Window;

/// Show "Change Download Location" folder picker.
pub fn change_download_folder(parent: &Window) {
    let dialog = gtk::FileDialog::builder()
        .title("Change Download Location")
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
                        if check_download_dir(&path_str, false).is_none() {
                            let binding = w2.model();
                            let mut m = binding.lock().unwrap();
                            if let Some(m) = m.as_mut() {
                                m.download_folder = path_str;
                                m.save_settings();
                            }
                            drop(m);
                            w2.sync_from_model();
                            return;
                        }
                    }
                    show_error(
                        &w2,
                        "Invalid folder selected",
                        "Not a directory",
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
    dialog.add_response("cancel", "Cancel");
    if show_reset {
        dialog.add_response("reset", "Reset to Default");
    }
    dialog.add_response("change", "Change Download Location");
    dialog.set_close_response("cancel");
    let w = parent.clone();
    dialog.connect_response(None, move |d: &adw::AlertDialog, response: &str| {
        match response {
            "change" => {
                change_download_folder(&w);
            }
            "reset" => {
                let binding = w.model();
                let mut m = binding.lock().unwrap();
                if let Some(m) = m.as_mut() {
                    // Reset to GSettings default.
                    if let Some(s) = &m.settings {
                        s.reset("download-folder");
                        let v: String = s.get("download-folder");
                        m.download_folder = utils::expand_path(&v);
                        m.save_settings();
                    } else {
                        m.download_folder = "xdg-download/VideoDownloader".to_string();
                    }
                }
                drop(m);
                w.sync_from_model();
                // Retry download preparation if in prepare state.
                let binding = w.model();
                let mut m = binding.lock().unwrap();
                if let Some(m) = m.as_mut() {
                    // Re-enter prepare to attempt start.
                    m.state = "start".to_string();
                    m.set_state("prepare");
                }
            }
            _ => {
                // Cancel: return to start state if in prepare.
                let binding = w.model();
                let mut m = binding.lock().unwrap();
                if let Some(m) = m.as_mut() {
                    if m.state == "prepare" {
                        m.state = "start".to_string();
                    }
                }
                drop(m);
                w.sync_from_model();
            }
        }
        d.close();
    });
    dialog.present(Some(parent));
}
