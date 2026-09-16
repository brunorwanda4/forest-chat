mod db;
mod files;
mod hub;
mod protocol;
mod session;

use std::{io, net::TcpListener, path::PathBuf};

use actix_web::{
    App, Error, HttpRequest, HttpResponse, HttpServer, dev::Server, middleware::Logger, rt, web,
};
use serde::Deserialize;

use crate::{db::Store, files::Uploads, hub::Hub, protocol::valid_name};


/// Release builds embed the UI so the binary is self-contained.
#[cfg(not(debug_assertions))]
async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(include_str!("../static/index.html"))
}

/// Debug builds read the UI from disk on every request and inject a small
/// script that reloads the page whenever `static/index.html` changes, so UI
/// edits show up instantly without rebuilding Rust.
#[cfg(debug_assertions)]
mod dev_reload {
    use std::{fs, time::UNIX_EPOCH};

    use actix_web::HttpResponse;

    const INDEX_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/static/index.html");

    const RELOAD_SCRIPT: &str = r#"<script>
(() => {
  let current = null;
  setInterval(async () => {
    try {
      const res = await fetch("/__dev/version", { cache: "no-store" });
      const version = await res.text();
      if (current === null) current = version;
      else if (version !== current) location.reload();
    } catch (_) {}
  }, 500);
})();
</script>"#;

    fn version() -> String {
        fs::metadata(INDEX_PATH)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|elapsed| elapsed.as_nanos().to_string())
            .unwrap_or_default()
    }

    pub async fn index() -> HttpResponse {
        let html = match fs::read_to_string(INDEX_PATH) {
            Ok(html) => html,
            Err(error) => {
                return HttpResponse::InternalServerError()
                    .body(format!("cannot read {INDEX_PATH}: {error}"));
            }
        };
        let html = match html.rfind("</body>") {
            Some(end) => format!("{}{RELOAD_SCRIPT}{}", &html[..end], &html[end..]),
            None => format!("{html}{RELOAD_SCRIPT}"),
        };

        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .insert_header(("Cache-Control", "no-store"))
            .body(html)
    }

    pub async fn version_route() -> HttpResponse {
        HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .body(version())
    }
}

#[cfg(debug_assertions)]
use dev_reload::index;

#[derive(Deserialize)]
struct AuthReq {
    name: String,
    password: Option<String>,
}

#[derive(Deserialize)]
struct VerifyReq {
    name: String,
    token: String,
}

#[derive(Deserialize)]
struct WsLogin {
    name: String,
    token: Option<String>,
}

async fn api_register(
    req: web::Json<AuthReq>,
    store: web::Data<Store>,
) -> HttpResponse {
    let name = req.name.trim();
    let password = req.password.as_deref().unwrap_or("").trim();
    if !valid_name(name) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Name must be 1-24 letters, numbers, - or _."
        }));
    }
    if password.len() < 3 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Password must be at least 3 characters."
        }));
    }

    match store.register_user(name, password).await {
        Ok(token) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "name": name,
            "token": token
        })),
        Err(err) => HttpResponse::BadRequest().json(serde_json::json!({
            "error": err
        })),
    }
}

async fn api_login(
    req: web::Json<AuthReq>,
    store: web::Data<Store>,
) -> HttpResponse {
    let name = req.name.trim();
    let password = req.password.as_deref().unwrap_or("").trim();
    if !valid_name(name) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid name."
        }));
    }

    match store.authenticate_user(name, password).await {
        Ok(token) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "name": name,
            "token": token
        })),
        Err(err) => HttpResponse::Unauthorized().json(serde_json::json!({
            "error": err
        })),
    }
}

async fn api_verify(
    req: web::Json<VerifyReq>,
    store: web::Data<Store>,
) -> HttpResponse {
    let valid = store.verify_token(req.name.trim(), req.token.trim()).await;
    HttpResponse::Ok().json(serde_json::json!({
        "valid": valid,
        "name": req.name.trim()
    }))
}

async fn chat_ws(
    req: HttpRequest,
    body: web::Payload,
    login: web::Query<WsLogin>,
    hub: web::Data<Hub>,
    store: web::Data<Store>,
) -> Result<HttpResponse, Error> {
    let q = login.into_inner();
    let name = q.name.trim().to_owned();
    if !valid_name(&name) {
        return Ok(HttpResponse::BadRequest().body("invalid name"));
    }

    if let Some(token) = q.token {
        if !store.verify_token(&name, token.trim()).await {
            return Ok(HttpResponse::Unauthorized().body("invalid session token"));
        }
    } else {
        return Ok(HttpResponse::Unauthorized().body("token required"));
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
    // Uploaded files live beside the database so both move together.
    let uploads_dir = db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("uploads");
    let uploads = Uploads::open(uploads_dir).await?;

    let db_path = db_path.to_string_lossy().into_owned();
    let store = Store::open(&db_path).await.map_err(io::Error::other)?;
    let hub = Hub::with_rooms(store.rooms().await.map_err(io::Error::other)?);

    Ok(HttpServer::new(move || {
        let app = App::new()
            .app_data(web::Data::new(hub.clone()))
            .app_data(web::Data::new(store.clone()))
            .app_data(web::Data::new(uploads.clone()))
            .wrap(Logger::new("%s %r %Dms"))
            .route("/api/upload", web::post().to(files::upload))
            .route("/files/{id}", web::get().to(files::download))
            .route("/api/auth/register", web::post().to(api_register))
            .route("/api/auth/login", web::post().to(api_login))
            .route("/api/auth/verify", web::post().to(api_verify))
            .route("/ws", web::get().to(chat_ws))
            .route("/", web::get().to(index));

        #[cfg(debug_assertions)]
        let app = app.route("/__dev/version", web::get().to(dev_reload::version_route));

        app
    })
    .listen(listener)?
    .run())
}
