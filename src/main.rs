mod db;
mod hub;
mod protocol;
mod session;

use std::{env, io};

use actix_files::Files;
use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, middleware::Logger, rt, web};
use serde::Deserialize;

use crate::{db::Store, hub::Hub, protocol::valid_name};

#[derive(Deserialize)]
struct Login {
    name: String,
}

async fn chat_ws(
    req: HttpRequest,
    body: web::Payload,
    login: web::Query<Login>,
    hub: web::Data<Hub>,
    store: web::Data<Store>,
) -> Result<HttpResponse, Error> {
    let name = login.into_inner().name.trim().to_owned();
    if !valid_name(&name) {
        return Ok(HttpResponse::BadRequest().body("invalid name"));
    }

    let (response, session, stream) = actix_ws::handle(&req, body)?;
    rt::spawn(session::run(
        name,
        session,
        stream,
        hub.get_ref().clone(),
        store.get_ref().clone(),
    ));

    Ok(response)
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let db_path = env::var("DATABASE_PATH").unwrap_or_else(|_| "forest-chat.db".to_owned());
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8081);

    let store = Store::open(&db_path).await.map_err(io::Error::other)?;
    let hub = Hub::with_rooms(store.rooms().await.map_err(io::Error::other)?);

    log::info!("database: {db_path}");
    log::info!("starting HTTP server at http://localhost:{port}");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(hub.clone()))
            .app_data(web::Data::new(store.clone()))
            .wrap(Logger::new("%s %r %Dms"))
            .route("/ws", web::get().to(chat_ws))
            .service(Files::new("/", "./static").index_file("index.html"))
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await
}
