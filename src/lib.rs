mod db;
mod hub;
mod protocol;
mod session;

use std::{io, net::TcpListener, path::PathBuf};

use actix_web::{
    App, Error, HttpRequest, HttpResponse, HttpServer, dev::Server, middleware::Logger, rt, web,
};
use serde::Deserialize;

use crate::{db::Store, hub::Hub, protocol::valid_name};

#[derive(Deserialize)]
struct Login {
    name: String,
}

async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(include_str!("../static/index.html"))
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

/// Creates the chat server using a caller-provided listener.
///
/// Supplying the listener lets the desktop app reserve an available port before
/// it starts the Tauri window, while the standalone server can keep using `PORT`.
pub async fn create_server(listener: TcpListener, db_path: PathBuf) -> io::Result<Server> {
    let db_path = db_path.to_string_lossy().into_owned();
    let store = Store::open(&db_path).await.map_err(io::Error::other)?;
    let hub = Hub::with_rooms(store.rooms().await.map_err(io::Error::other)?);

    Ok(HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(hub.clone()))
            .app_data(web::Data::new(store.clone()))
            .wrap(Logger::new("%s %r %Dms"))
            .route("/ws", web::get().to(chat_ws))
            .route("/", web::get().to(index))
    })
    .listen(listener)?
    .run())
}
