//! A small always-on-top window that keeps the meeting visible while the user
//! works in another application.
//!
//! It loads `/pip` from the same local server as the main window and listens to
//! the same `meeting://*` events, so it needs no state of its own.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const PIP_LABEL: &str = "meeting-pip";

const PIP_WIDTH: f64 = 340.0;
const PIP_HEIGHT: f64 = 230.0;

/// Opens the floating window, or focuses it if it is already open.
pub fn show(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(PIP_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    let main = app
        .get_webview_window("main")
        .ok_or_else(|| "The main window is gone".to_string())?;

    let mut url = main.url().map_err(|e| e.to_string())?;
    url.set_path("/pip");

    let window = WebviewWindowBuilder::new(app, PIP_LABEL, WebviewUrl::External(url))
        .title("Forest Chat meeting")
        .inner_size(PIP_WIDTH, PIP_HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(true)
        .build()
        .map_err(|e| format!("Failed to open the meeting window: {e}"))?;

    // Bottom-right of the work area, out of the way of what the user is doing.
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let size = monitor.size().to_logical::<f64>(monitor.scale_factor());
        let _ = window.set_position(tauri::LogicalPosition::new(
            size.width - PIP_WIDTH - 24.0,
            size.height - PIP_HEIGHT - 72.0,
        ));
    }

    Ok(())
}

/// Closes the floating window if it is open.
pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PIP_LABEL) {
        let _ = window.close();
    }
}

/// Brings the main window back to the front, from the floating window's button.
#[tauri::command]
pub fn focus_main_window(app: AppHandle) -> Result<(), String> {
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| "The main window is gone".to_string())?;
    let _ = main.unminimize();
    let _ = main.show();
    main.set_focus().map_err(|e| e.to_string())
}

/// Shows or hides the floating window on request from the UI.
#[tauri::command]
pub fn set_meeting_pip_visible(visible: bool, app: AppHandle) -> Result<(), String> {
    if visible {
        show(&app)
    } else {
        hide(&app);
        Ok(())
    }
}
