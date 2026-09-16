/// Commands the webview may invoke.
///
/// The UI is served over http://127.0.0.1:<port>, a remote origin as far as the
/// access control list is concerned, so every command must be declared here and
/// allowed in `capabilities/default.json`.
const COMMANDS: &[&str] = &[
    "open_attachment_externally",
    "save_attachment_as",
    "reveal_saved_file",
    "get_lan_ip_info",
    "create_lan_meeting",
    "join_lan_meeting",
    "leave_lan_meeting",
    "toggle_meeting_mic",
    "toggle_meeting_screen_share",
    "send_meeting_chat",
    "get_meeting_status",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri build script")
}
