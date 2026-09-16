//! File attachments: multipart upload to disk, and download with range support.
//!
//! Bytes never pass through the WebSocket. A client uploads to `/api/upload`,
//! gets back the metadata, then sends a normal chat message carrying that
//! metadata. Downloads go through `/files/{id}`, which streams from disk so
//! large videos seek without loading the whole file into memory.

use std::{io, path::PathBuf};

use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::{
    HttpRequest, HttpResponse,
    http::header::{ContentDisposition, DispositionParam, DispositionType},
    web,
};
use futures_util::TryStreamExt as _;
use serde::Deserialize;
use tokio::io::AsyncWriteExt as _;
use uuid::Uuid;

use crate::{
    db::Store,
    protocol::{Attachment, MAX_FILE_NAME_LEN},
};

/// Directory holding uploaded files, one file per attachment id.
#[derive(Clone)]
pub struct Uploads {
    dir: PathBuf,
}

impl Uploads {
    pub async fn open(dir: PathBuf) -> io::Result<Self> {
        tokio::fs::create_dir_all(&dir).await?;
        Ok(Self { dir })
    }

    pub fn path(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }
}

#[derive(Deserialize)]
pub struct Auth {
    name: String,
    token: String,
}

#[derive(Deserialize)]
pub struct FileQuery {
    name: String,
    token: String,
    /// `?download=1` forces a save prompt instead of inline display.
    #[serde(default)]
    download: Option<String>,
}

/// Attachment ids are generated here, so anything else is a probe.
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Keeps the visible file name only: no directories, no control characters.
fn clean_name(raw: &str) -> String {
    let base = raw
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("file")
        .trim()
        .trim_matches('.');
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && *c != '"')
        .take(MAX_FILE_NAME_LEN)
        .collect();
    if cleaned.is_empty() {
        "file".to_owned()
    } else {
        cleaned
    }
}

fn disposition(kind: DispositionType, name: &str) -> ContentDisposition {
    ContentDisposition {
        disposition: kind,
        parameters: vec![DispositionParam::Filename(name.to_owned())],
    }
}

/// `POST /api/upload?name=&token=` with a multipart body holding one `file` field.
///
/// There is no size limit: the body is streamed to disk chunk by chunk, so memory
/// stays flat no matter how big the file is.
pub async fn upload(
    auth: web::Query<Auth>,
    mut payload: Multipart,
    store: web::Data<Store>,
    uploads: web::Data<Uploads>,
) -> HttpResponse {
    if !store.verify_token(auth.name.trim(), auth.token.trim()).await {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Sign in again to send files."
        }));
    }
    let owner = auth.name.trim().to_owned();

    let field = match payload.try_next().await {
        Ok(Some(field)) => field,
        Ok(None) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "No file in the upload."
            }));
        }
        Err(err) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Upload failed: {err}")
            }));
        }
    };
    let mut field = field;

    let name = clean_name(
        field
            .content_disposition()
            .and_then(|cd| cd.get_filename())
            .unwrap_or("file"),
    );
    let mime = field
        .content_type()
        .map(|m| m.essence_str().to_owned())
        .filter(|m| m != "application/octet-stream")
        .unwrap_or_else(|| {
            mime_guess::from_path(&name)
                .first_or_octet_stream()
                .essence_str()
                .to_owned()
        });

    let id = Uuid::new_v4().to_string();
    let path = uploads.path(&id);
    let mut size: i64 = 0;

    let write_result = async {
        let mut file = tokio::fs::File::create(&path).await?;
        // MultipartError is neither Send nor Sync, so carry it as text.
        while let Some(chunk) = field
            .try_next()
            .await
            .map_err(|err| io::Error::other(err.to_string()))?
        {
            file.write_all(&chunk).await?;
            size += chunk.len() as i64;
        }
        file.flush().await?;
        Ok::<(), io::Error>(())
    }
    .await;

    if let Err(err) = write_result {
        let _ = tokio::fs::remove_file(&path).await;
        log::error!("upload from {owner} failed: {err}");
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Could not save the file."
        }));
    }

    let attachment = Attachment {
        id,
        name,
        mime,
        size,
    };
    if let Err(err) = store.save_attachment(&attachment, &owner).await {
        let _ = tokio::fs::remove_file(&path).await;
        log::error!("recording attachment for {owner} failed: {err}");
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Could not save the file."
        }));
    }

    log::info!("{owner} uploaded {} ({size} bytes)", attachment.name);
    HttpResponse::Ok().json(attachment)
}

/// `GET /files/{id}?name=&token=[&download=1]`
///
/// Serves inline by default so images, videos and PDFs can render in place;
/// `download=1` asks the browser or webview to save it instead.
pub async fn download(
    req: HttpRequest,
    id: web::Path<String>,
    query: web::Query<FileQuery>,
    store: web::Data<Store>,
    uploads: web::Data<Uploads>,
) -> HttpResponse {
    if !store
        .verify_token(query.name.trim(), query.token.trim())
        .await
    {
        return HttpResponse::Unauthorized().body("sign in again");
    }

    let id = id.into_inner();
    if !valid_id(&id) {
        return HttpResponse::NotFound().body("unknown file");
    }
    let Some(meta) = store.attachment(&id).await else {
        return HttpResponse::NotFound().body("unknown file");
    };

    let file = match NamedFile::open_async(uploads.path(&id)).await {
        Ok(file) => file,
        Err(err) => {
            log::warn!("attachment {id} missing on disk: {err}");
            return HttpResponse::NotFound().body("file is gone");
        }
    };

    let kind = if query.download.is_some() {
        DispositionType::Attachment
    } else {
        DispositionType::Inline
    };
    let content_type = meta
        .mime
        .parse()
        .unwrap_or(mime_guess::mime::APPLICATION_OCTET_STREAM);

    file.set_content_type(content_type)
        .set_content_disposition(disposition(kind, &meta.name))
        .into_response(&req)
}
