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

pub fn routes(mc: Arc<SessionController>) -> Router {
    Router::new()
        .route("/get_offer", get(get_offer))
        .route("/set_answer", post(set_answer))
        .route("/get_ice", get(get_ice))
        .route("/set_ice", post(add_ice))
        .route("/broadcast", get(broadcast))
        .with_state(mc)
}

async fn get_offer(
    State(mc): State<Arc<SessionController>>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - join_session", "Handler");

    let session = mc.get_session(0).await?;
    let offer = session.get_offer().await?;

    Ok(Json(json!({
        "status": "ok",
        "message": "Session joined",
        "offer": offer,
    })))
}

async fn set_answer(
    // get request body
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<SDPAnswerRequest>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - set_answer", "Handler");

    let session = mc.get_session(0).await?;
    session.set_answer(body.sdp).await?;

    Ok(Json(json!({
        "status": "ok",
        "message": "Session joined",
    })))
}

async fn get_ice(
    State(mc): State<Arc<SessionController>>,
) -> Result<Json<Vec<RTCIceCandidate>>> {
    println!("->> {:<12} - get_ice", "Handler");

    let session = mc.get_session(0).await?;
    session.gathering_state.notified().await;
    let candidates = session.ice_candidates.lock().await.clone();
    println!("Sent ice");

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
        candidate
    ).await?;

    Ok(Json(json!({
        "status": "ok",
        "message": "ice added",
    })))
}

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

    session.broadcaster.broadcast_audio_from_file("output2.ogg").await?;

    Ok(Json(json!({
        "status": "ok",
        "message": "Broadcasting",
    })))
}

#[derive(Debug, Deserialize)]
struct SessionParams {
    id: u64,
}

#[derive(Debug, Deserialize)]
struct SDPAnswerRequest {
    sdp: String,
}

#[derive(Debug, Deserialize)]
struct ICECandidateRequest{
    candidate: String,
    sdp_mid: Option<String>,
    sdp_mline_index: Option<u16>,
    username_fragment: Option<String>,
}