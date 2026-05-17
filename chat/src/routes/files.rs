//! `/files/*` — upload and download file attachments for DM rooms.

use axum::{
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    Json,
};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{auth::authenticate, models::FileRecord, AppState};

type ApiError = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: &str) -> ApiError {
    (status, Json(serde_json::json!({"detail": msg})))
}

const MAX_SIZE: usize = 50 * 1024 * 1024; // 50 MB

/// `POST /v1/files` — multipart upload (fields: `room_id`, `file`).
/// The caller must be a member of the room. Returns `{ id, url }`.
pub async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = authenticate(&state, &headers).await?;

    let mut filename = String::new();
    let mut mime_type = String::from("application/octet-stream");
    let mut room_id: Option<Uuid> = None;
    let mut bytes: Vec<u8> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| err(StatusCode::BAD_REQUEST, "Ugyldig multipart-anmodning"))?
    {
        match field.name().unwrap_or("") {
            "room_id" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| err(StatusCode::BAD_REQUEST, "Ugyldig room_id"))?;
                room_id = Some(
                    Uuid::parse_str(&text)
                        .map_err(|_| err(StatusCode::BAD_REQUEST, "Ugyldig UUID"))?,
                );
            }
            "file" => {
                filename = field
                    .file_name()
                    .unwrap_or("fil")
                    .to_string();
                if let Some(ct) = field.content_type() {
                    mime_type = ct.to_string();
                }
                bytes = field
                    .bytes()
                    .await
                    .map_err(|_| err(StatusCode::BAD_REQUEST, "Kunne ikke læse fil"))?
                    .to_vec();
                if bytes.len() > MAX_SIZE {
                    return Err(err(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "Filen er for stor (maks 50 MB)",
                    ));
                }
            }
            _ => {}
        }
    }

    let room_id = room_id.ok_or_else(|| err(StatusCode::BAD_REQUEST, "room_id mangler"))?;
    if filename.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "Ingen fil uploadet"));
    }

    let member = sqlx::query("SELECT 1 FROM room_members WHERE room_id = $1 AND user_id = $2")
        .bind(room_id)
        .bind(user.id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;
    if member.is_none() {
        return Err(err(StatusCode::FORBIDDEN, "Ikke adgang til dette rum"));
    }

    let id = Uuid::new_v4();
    let dir = PathBuf::from(&state.config.upload_dir);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Kunne ikke oprette upload-mappe"))?;

    let path = dir.join(id.to_string());
    let mut f = tokio::fs::File::create(&path)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Kunne ikke gemme fil"))?;
    f.write_all(&bytes)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Kunne ikke skrive fil"))?;

    sqlx::query(
        "INSERT INTO files (id, uploader_id, room_id, filename, mime_type, size)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(user.id)
    .bind(room_id)
    .bind(&filename)
    .bind(&mime_type)
    .bind(bytes.len() as i64)
    .execute(&state.db)
    .await
    .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    tracing::info!(file_id = %id, uploader = %user.id, room = %room_id, filename = %filename, "File uploaded");

    Ok(Json(serde_json::json!({
        "id": id,
        "url": format!("/v1/files/{}", id),
    })))
}

/// `GET /v1/files/:id` — stream a file. Caller must be a member of the room
/// the file was uploaded to.
pub async fn download(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let user = authenticate(&state, &headers).await?;

    let file = sqlx::query_as::<_, FileRecord>("SELECT * FROM files WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "Fil ikke fundet"))?;

    let member = sqlx::query("SELECT 1 FROM room_members WHERE room_id = $1 AND user_id = $2")
        .bind(file.room_id)
        .bind(user.id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;
    if member.is_none() {
        return Err(err(StatusCode::FORBIDDEN, "Ikke adgang til denne fil"));
    }

    let path = PathBuf::from(&state.config.upload_dir).join(id.to_string());
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Fil ikke fundet på disk"))?;

    // Sanitize filename for Content-Disposition — reject control characters
    let safe_name = file
        .filename
        .chars()
        .filter(|c| !c.is_control() && *c != '"')
        .collect::<String>();
    let disposition = format!("attachment; filename=\"{}\"", safe_name);

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, file.mime_type),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    ))
}
