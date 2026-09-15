use std::{env, io, net::TcpListener, path::PathBuf};

#[actix_web::main]
async fn main() -> io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let db_path = env::var("DATABASE_PATH").unwrap_or_else(|_| "forest-chat.db".to_owned());
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8081);

    log::info!("database: {db_path}");
    log::info!("starting HTTP server at http://localhost:{port}");

    let listener = TcpListener::bind(("127.0.0.1", port))?;
    forest_chat::create_server(listener, PathBuf::from(db_path))
        .await?
        .await
}
