use std::sync::Arc;
use axum::Router;
use axum::routing::{ get, post };
use axum::extract::State;
use axum::Extension;
use sqlx::PgPool;
use serde_json::{json, Value};
use axum::{Json};
use serde::Deserialize;
use tokio::sync::broadcast;
use axum::response::{
    Sse,
    sse::Event,
    sse::KeepAlive,
};
use futures::Stream;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::{StreamExt};
// use the Result enum
use core::result::Result as CoreResult;
use sqlx::Row;

use crate::utils::error::{ Error, Result, ClientError };
use crate::models::SessionController;
use crate::ctx::Ctx;
use crate::media::file_manager::{ FileManager, FMDownloadParams};
use crate::models::session::User;

#[derive(Debug, Deserialize)]
struct PlayTestRequest {
    url: String,
}

#[derive(Debug, Deserialize)]
struct DownloadRequest {
    titles: Vec<String>,
    urls: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AddQueue {
    session_id: String,
    key: String,
    title: String,
}

#[derive(Debug, Deserialize)]
struct RemoveQueue {
    session_id: String,
    key: String,
}

#[derive(Debug, Deserialize)]
struct ReorderQueue {
    session_id: String,
    old_index: usize,
    new_index: usize,
}

#[derive(Debug, Deserialize)]
struct NextQueue {
    session_id: String,
}

pub fn routes(mc: Arc<SessionController>) -> Router {
    Router::new()
        .route("/me", get(me))
        .route("/get_metadata", post(get_metadata))
        .route("/download", post(download))
        .route("/create_session", get(create_session))
        .route("/get_files", get(get_files))
        .route("/add_to_queue", post(add_to_queue))
        .route("/remove_from_queue", post(remove_from_queue))
        .route("/reorder_queue", post(reorder_queue))
        .route("/next_in_queue", post(next_in_queue))
        .route("/prev_in_queue", post(prev_in_queue))
        .route("/download_notify", get(download_notify))
        .with_state(mc)
}

async fn me(
    State(mc): State<Arc<SessionController>>,
    Extension(pool) : Extension<PgPool>,
    ctx: Ctx,
) -> Result<Json<Value>> {

    let id = ctx.id();
    let name = ctx.name();
    let picture = ctx.picture();
    let session = mc.get_user_session(id.clone()).await?;
    println!("->> user: {}, name: {}", id, name);

    Ok(Json(json!({
        "id": id,
        "name": name,
        "picture": picture,
        "session": session,
    })))
}

async fn create_session(
    State(mc): State<Arc<SessionController>>,
    ctx: Ctx,
) -> Result<Json<Value>> {
    println!("->> {:<12} - create_session", "Handler");

    // check if user already has a session
    let id = ctx.id();
    if (mc.check_user_has_session(id.clone()).await?) {
        return Err(Error::SessionExists);
    }

    let mut sessionid = mc.create_session(
        id,
        User {
            id: ctx.id(),
            name: ctx.name(),
            picture: ctx.picture(),
        }
    ).await?;
    
    Ok(Json(json!({
        "status": "ok",
        "session_id": sessionid
    })))
}

async fn get_files(
    State(mc): State<Arc<SessionController>>,
    Extension(pool): Extension<PgPool>,
    ctx: Ctx,
) -> Result<Json<Value>> {
    println!("->> {:<12} - get_files", "Handler");

    let id = ctx.id();
    let name = ctx.name();
    
    match sqlx::query("SELECT uuid, name FROM files WHERE user_id = $1 ORDER BY created_at DESC")
        .bind(&id.parse::<i32>().unwrap())
        .fetch_all(&pool)
        .await {
            Ok(files) => {
                Ok(Json(json!({
                    "status": "ok",
                    "files": files.iter().map(|f| {
                        json!({
                            "uuid": f.get::<String, &str>("uuid"),
                            "name": f.get::<String, &str>("name"),
                        })
                    }).collect::<Vec<Value>>()
                })))
            },
            Err(e) => {
                println!("Error: {:?}", e);
                // return empty list
                Err(Error::DBError { source: "Cannot fetch files".to_string() })
            }
        }
}

async fn download_notify(
    ctx: Ctx,
    State(mc): State<Arc<SessionController>>,
) -> Sse<impl Stream<Item = CoreResult<Event, Infallible>>> {

    println!("->> {:<12} - download_notify", "Handler");

    let user_id = ctx.id();
    let user_name = ctx.name();

    let (sender, _) = broadcast::channel(100);
    let mut rx = sender.subscribe();
    mc.add_sender_with_id(user_id.clone(), sender).await.unwrap();

    let stream = async_stream::stream! {
        while let Ok(msg) = rx.recv().await {
            yield Ok(Event::default().data(msg));
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive")
    )
}

async fn download(
    ctx: Ctx,
    State(mc): State<Arc<SessionController>>,
    Extension(pool): Extension<PgPool>,
    Json(body): Json<DownloadRequest>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - download", "Handler");

    let user_id = ctx.id();
    let user_name = ctx.name();
    println!("user_id: {}, user_name: {} is initiating download", user_id, user_name);
    // look up if the url is in db, if not, download
    // has to call fm.process_audio directly instead of wrapping
    // the function with session controller to ensure concurrent downloads

    let fm = mc.get_file_manager().await?;

    for i in 0..body.urls.len() {
        let url = body.urls[i].clone();
        let title = body.titles[i].clone();

        let pool = pool.clone();
        let user_id = user_id.clone();
        let fm = fm.clone();

        tokio::spawn(async move {
            fm.process_audio(
                FMDownloadParams {
                    url: url.clone(),
                    title: title.clone(),
                    userid: user_id.clone(),
                    pool: pool.clone(),
                }
            ).await?;
            Ok::<(), Error>(())
        });
    }
    
    Ok(Json(json!({
        "status": "ok",
        "message": "Download initiated",
    })))
}

async fn get_metadata(
    State(mc): State<Arc<SessionController>>,
    Extension(pool): Extension<PgPool>,
    Json(body): Json<PlayTestRequest>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - get_metadata", "Handler");

    println!("url: {}", body.url);
    let is_playlist = body.url.contains("playlist") || body.url.contains("list");


    if is_playlist {
        let list = FileManager::get_list(body.url.clone()).await?;
        Ok(Json(json!({
            "status": "ok",
            "list": list,
        })))
    } else {
        if FileManager::is_live(body.url.clone()).await? {
            Ok(Json(json!({
                "status": "ok",
                "message": "Live stream is not supported",
            })))
        } else {
            let result = FileManager::get_title(body.url.clone()).await?; 
            Ok(Json(json!({
                "status": "ok",
                "list": result,
            })))
        }
    }
}

async fn add_to_queue(
    ctx: Ctx,
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<AddQueue>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - add_to_queue", "Handler");

    // check if session belongs to user
    let user_id = ctx.id();
    let session_id = body.session_id.clone();

    if (!mc.check_user_own_session(user_id.clone(), session_id.clone()).await?) {
        return Err(Error::SessionNotOwned);
    }

    let key = body.key.clone();
    let title = body.title.clone();
    let session = mc.get_session(session_id).await?;

    session.add_to_queue(key, title).await?;
    
    Ok(Json(json!({
        "status": "ok",
        "message": "Download initiated",
    })))
}

async fn remove_from_queue(
    ctx: Ctx,
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<RemoveQueue>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - remove_from_queue", "Handler");

    let user_id = ctx.id();
    let session_id = body.session_id.clone();

    if (!mc.check_user_own_session(user_id.clone(), session_id.clone()).await?) {
        return Err(Error::SessionNotOwned);
    }

    let key = body.key.clone();
    let session = mc.get_session(session_id).await?;

    session.remove_from_queue(key).await?;
    
    Ok(Json(json!({
        "status": "ok",
        "message": "Removed",
    }))
)}

async fn reorder_queue(
    ctx: Ctx,
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<ReorderQueue>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - reorder_queue", "Handler");

    let user_id = ctx.id();
    let session_id = body.session_id.clone();

    if (!mc.check_user_own_session(user_id.clone(), session_id.clone()).await?) {
        return Err(Error::SessionNotOwned);
    }

    let old_index = body.old_index;
    let new_index = body.new_index;
    let session = mc.get_session(session_id).await?;

    session.reorder_queue(old_index, new_index).await?;
    
    Ok(Json(json!({
        "status": "ok",
        "message": "Removed",
    }))
)}

async fn next_in_queue(
    ctx: Ctx,
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<NextQueue>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - next_in_queue", "Handler");

    let user_id = ctx.id();
    let session_id = body.session_id.clone();

    if (!mc.check_user_own_session(user_id.clone(), session_id.clone()).await?) {
        return Err(Error::SessionNotOwned);
    }

    let session = mc.get_session(session_id).await?;
    session.next_in_queue().await?;
    
    Ok(Json(json!({
        "status": "ok",
        "message": "next",
    }))
)}

async fn prev_in_queue(
    ctx: Ctx,
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<NextQueue>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - prev_in_queue", "Handler");

    let user_id = ctx.id();
    let session_id = body.session_id.clone();

    if (!mc.check_user_own_session(user_id.clone(), session_id.clone()).await?) {
        return Err(Error::SessionNotOwned);
    }

    let session = mc.get_session(session_id).await?;
    session.prev_in_queue().await?;
    
    Ok(Json(json!({
        "status": "ok",
        "message": "previous",
    }))
)}