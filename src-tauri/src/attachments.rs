//! Native file handling for chat attachments.
//!
//! The web UI can already show images, videos and PDFs inline. These commands
//! add the two things a webview cannot do: open a file in whatever application
//! Windows/macOS/Linux associates with it, and save it through the real system
//! save dialog. Both fetch the bytes over HTTP, so they work the same whether
//! the chat server is the bundled local one or a remote `FOREST_CHAT_SERVER_URL`.

use std::{
    fs,
    io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

/// Strips directories and characters Windows rejects in file names.
fn safe_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("file");
    let cleaned: String = base
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .take(200)
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_owned();
    if trimmed.is_empty() {
        "file".to_owned()
    } else {
        trimmed
    }
}

fn extension_of(name: &str) -> Option<String> {
    Path::new(name)
        .extension()
        .map(|ext| ext.to_string_lossy().into_owned())
}

/// Streams `url` into `target`, replacing whatever is there.
fn fetch_to(url: &str, target: &Path) -> Result<(), String> {
    let response = ureq::get(url)
        .call()
        .map_err(|err| format!("Could not download the file: {err}"))?;

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("Could not create folder: {err}"))?;
    }
    let mut file =
        fs::File::create(target).map_err(|err| format!("Could not write the file: {err}"))?;
    let mut reader = response.into_reader();
    io::copy(&mut reader, &mut file).map_err(|err| format!("Could not write the file: {err}"))?;
    Ok(())
}

/// Per-run scratch folder for files opened in an external application.
fn temp_target(file_name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir()
        .join("forest-chat-files")
        .join(stamp.to_string())
        .join(safe_file_name(file_name))
}

/// Downloads the attachment to a temporary folder and opens it in the system's
/// default application (Acrobat for PDFs, the video player for videos, ...).
#[tauri::command]
pub async fn open_attachment_externally(url: String, file_name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let target = temp_target(&file_name);
        fetch_to(&url, &target)?;
        tauri_plugin_opener::open_path(target.clone(), None::<&str>)
            .map_err(|err| format!("Could not open the file: {err}"))?;
        Ok(target.to_string_lossy().into_owned())
    })
    .await
    .map_err(|err| format!("Open failed: {err}"))?
}

/// Shows the system save dialog, then downloads the attachment to the chosen path.
///
/// Returns the saved path, or `None` when the user cancels the dialog.
#[tauri::command]
pub async fn save_attachment_as(
    app: AppHandle,
    url: String,
    file_name: String,
) -> Result<Option<String>, String> {
    let suggested = safe_file_name(&file_name);
    let mut builder = app.dialog().file().set_file_name(&suggested);
    if let Some(ext) = extension_of(&suggested) {
        builder = builder.add_filter(format!("{} file", ext.to_uppercase()), &[ext.as_str()]);
    }

    let Some(chosen) = builder.blocking_save_file() else {
        return Ok(None);
    };
    let path = chosen
        .into_path()
        .map_err(|err| format!("Unusable save location: {err}"))?;

    tauri::async_runtime::spawn_blocking(move || {
        fetch_to(&url, &path)?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|err| format!("Save failed: {err}"))?
}

/// Opens the folder containing a previously saved file and selects it.
#[tauri::command]
pub async fn reveal_saved_file(path: String) -> Result<(), String> {
    tauri_plugin_opener::reveal_item_in_dir(PathBuf::from(path))
        .map_err(|err| format!("Could not open the folder: {err}"))
}
