#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, fs, net::TcpListener, sync::mpsc, thread, time::Duration};

use tauri::{Manager, WebviewUrl, webview::WebviewWindowBuilder};

mod lan_meeting;

fn main() {
    tauri::Builder::default()
        .manage(lan_meeting::commands::LanMeetingManager::default())
        .invoke_handler(tauri::generate_handler![
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

                    let listener = TcpListener::bind(("127.0.0.1", 0))?;
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
