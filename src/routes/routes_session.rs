use crate::models::session::{
    SessionController,
    Session,
};

use crate::Result;
use serde::{
    Deserialize,
    Serialize,
};

use crate::utils::error::Error;
use axum::response::IntoResponse;
use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use axum::routing::{
    get,
    delete,
    post
};
use std::sync::Arc;
use serde_json::{json, Value};
use webrtc::ice_transport::ice_candidate::{
    RTCIceCandidateInit,
    RTCIceCandidate,
};

use crate::media::file_manager::{ FileManager, FMDownloadParams};
use axum::Extension;
use sqlx::PgPool;

#[derive(Debug, Deserialize)]
struct SessionParams {
    id: u64,
}

#[derive(Debug, Deserialize)]
struct SDPAnswerRequest {
    sdp: String,
    peerid: String,
}

#[derive(Debug, Deserialize)]
struct ICECandidateRequest {
    candidate: String,
    sdp_mid: Option<String>,
    sdp_mline_index: Option<u16>,
    username_fragment: Option<String>,
    peerid: String,
}

#[derive(Debug, Deserialize)]
struct GetIceRequest {
    peerid: String,
}

#[derive(Debug, Deserialize)]
struct PlayTestRequest {
    url: String,
}

#[derive(Debug, Deserialize)]
struct DownloadRequest {
    url: String,
    title: String,
}

// this is a no auth route layer

pub fn routes(mc: Arc<SessionController>) -> Router {
    Router::new()
        .route("/get_offer", get(get_offer))
        .route("/set_answer", post(set_answer))
        .route("/get_ice", post(get_ice))
        .route("/set_ice", post(add_ice))
        .route("/broadcast", get(broadcast))
        .route("/state", get(server_state))
        .route("/get_metadata", post(get_metadata))
        .route("/download", post(download))
        .with_state(mc)
}

async fn server_state(
    State(mc): State<Arc<SessionController>>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - server_state", "Handler");

    let sessions = mc.get_sessions().await?;
    let mut session_list = Vec::new();
    for session in sessions {
        let peers = session.get_peers().await?;
        let mut peer_list = Vec::new();
        for peer in peers {
            peer_list.push(json!({
                "peerid": peer,
            }));
        }
        session_list.push(json!({
            "session": session.uuid,
            "peers": peer_list,
        }));
    }

    Ok(Json(json!({
        "status": "ok",
        "message": "Server state",
        "sessions": session_list,
    })))
}

async fn get_offer(
    State(mc): State<Arc<SessionController>>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - join_session", "Handler");

    // in final impl, this should have request payload of session uuid

    let mut session = mc.get_session(0).await?;
    // let offer = session.get_offer("hi".to_string()).await?;
    let mut uuid = session.create_peer().await?;
    let id = uuid.clone();
    let offer = session.get_offer(uuid).await?;

    Ok(Json(json!({
        "status": "ok",
        "message": "Session joined",
        "offer": offer,
        "peerid": id,
    })))
}

async fn set_answer(
    // get request body
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<SDPAnswerRequest>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - set_answer", "Handler");

    let session = mc.get_session(0).await?;

    session.set_answer(body.sdp, body.peerid).await?;

    Ok(Json(json!({
        "status": "ok",
        "message": "Session joined",
    })))
}

async fn get_ice(
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<GetIceRequest>,
) -> Result<Json<Vec<RTCIceCandidate>>> {
    println!("->> {:<12} - get_ice", "Handler");

    let session = mc.get_session(0).await?;

    let candidates = session.get_ice(body.peerid).await?.clone();

    Ok(Json(candidates))
}

async fn add_ice(
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<ICECandidateRequest>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - add_ice_candidate", "Handler");

    let session = mc.get_session(0).await?;

    let candidate = RTCIceCandidateInit {
        candidate: body.candidate,
        ..Default::default()
    };
    
    session.add_ice(
        candidate,
        body.peerid,
    ).await?;

    Ok(Json(json!({
        "status": "ok",
        "message": "ice added",
    })))
}


// add following apis

// get all sessions with participants
// join session with session uuid
// exit session
// get participants
// get queue




async fn create_session(
    State(mc): State<Arc<SessionController>>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - create_session", "Handler");

    let mut session = mc.create_session().await?;
    
    Ok(Json(json!({
        "status": "ok",
        "message": "Session created",
    })))
}

async fn broadcast(
    // get request body
    State(mc): State<Arc<SessionController>>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - broadcast", "Handler");

    let mut session = mc.get_session(0).await?;

    session.broadcaster.broadcast_audio_from_file("output/converted.ogg").await?;

    Ok(Json(json!({
        "status": "ok",
        "message": "Broadcasting",
    })))
}

async fn download(
    State(mc): State<Arc<SessionController>>,
    Extension(pool): Extension<PgPool>,
    Json(body): Json<DownloadRequest>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - download", "Handler");

    let fm = mc.get_file_manager().await?;
    fm.process_audio(
        FMDownloadParams {
            url: body.url.clone(),
            title: body.title.clone(),
            uuid: uuid::Uuid::new_v4().to_string(),
            userid: 0,
            pool: pool.clone(),
        }
    ).await?;

    Ok(Json(json!({
        "status": "ok",
        "message": "Downloading",
    })))
}

async fn get_metadata(
    State(mc): State<Arc<SessionController>>,
    Extension(pool): Extension<PgPool>,
    Json(body): Json<PlayTestRequest>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - get_metadata", "Handler");

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