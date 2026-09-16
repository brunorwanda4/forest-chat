#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, fs, io, net::TcpListener, path::Path, sync::mpsc, thread, time::Duration};

use tauri::{Manager, WebviewUrl, webview::WebviewWindowBuilder};

mod attachments;
mod lan_meeting;

/// Binds the chat server to the same port it used last time, when that port is
/// still free.
///
/// The UI keeps the session token in `localStorage`, which the webview scopes to
/// `http://127.0.0.1:<port>`. A fresh port on every launch would look like a new
/// origin and silently sign the user out, so the port is remembered next to the
/// database. If it is taken, the OS picks one and that choice is remembered
/// instead.
fn bind_stable_port(app_data: &Path) -> io::Result<TcpListener> {
    let port_file = app_data.join("port");

    if let Some(port) = fs::read_to_string(&port_file)
        .ok()
        .and_then(|saved| saved.trim().parse::<u16>().ok())
        .filter(|port| *port != 0)
    {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) {
            return Ok(listener);
        }
    }

    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let _ = fs::write(&port_file, listener.local_addr()?.port().to_string());
    Ok(listener)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(lan_meeting::commands::LanMeetingManager::default())
        .invoke_handler(tauri::generate_handler![
            attachments::open_attachment_externally,
            attachments::save_attachment_as,
            attachments::reveal_saved_file,
            lan_meeting::commands::get_lan_ip_info,
            lan_meeting::commands::create_lan_meeting,
            lan_meeting::commands::join_lan_meeting,
            lan_meeting::commands::leave_lan_meeting,
            lan_meeting::commands::toggle_meeting_mic,
            lan_meeting::commands::toggle_meeting_screen_share,
            lan_meeting::commands::send_meeting_chat,
            lan_meeting::commands::get_meeting_status,
        ])
        .setup(|app| {
            let url = match env::var("FOREST_CHAT_SERVER_URL") {
                Ok(server_url) => server_url.parse()?,
                Err(_) => {
                    let app_data = app.path().app_data_dir()?;
                    fs::create_dir_all(&app_data)?;

                    let listener = bind_stable_port(&app_data)?;
                    let port = listener.local_addr()?.port();
                    let database = app_data.join("forest-chat.db");
                    let (ready_tx, ready_rx) = mpsc::sync_channel(1);

                    thread::spawn(move || {
                        actix_web::rt::System::new().block_on(async move {
                            match forest_chat::create_server(listener, database).await {
                                Ok(server) => {
                                    let _ = ready_tx.send(Ok(()));
                                    if let Err(error) = server.await {
                                        eprintln!("chat server stopped: {error}");
                                    }
                                }
                                Err(error) => {
                                    let _ = ready_tx.send(Err(error.to_string()));
                                }
                            }
                        });
                    });

                    match ready_rx.recv_timeout(Duration::from_secs(15)) {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => return Err(error.into()),
                        Err(_) => return Err("chat server did not start in time".into()),
                    }

                    format!("http://127.0.0.1:{port}/").parse()?
                }
            };
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Forest Chat")
                .inner_size(1100.0, 720.0)
                .min_inner_size(640.0, 480.0)
                .center()
                .build()?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Forest Chat");
}
