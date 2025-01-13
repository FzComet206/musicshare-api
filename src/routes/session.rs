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
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;

pub fn routes(mc: Arc<SessionController>) -> Router {
    Router::new()
        .route("/session", post(create_session))
        .route("/session", get(join_session))
        .route("/set_answer", post(set_answer))
        .route("/broadcast", get(broadcast))
        .route("/add_ice_candidate", post(add_ice_candidate))
        .with_state(mc)
}

#[derive(Debug, Deserialize)]
struct SessionParams {
    id: u64,
}

async fn play() -> Result<Json<()>> {
    // right now just play the rtc connection of first session in the list
    Ok(Json(()))
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

async fn join_session(
    State(mc): State<Arc<SessionController>>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - join_session", "Handler");

    // get first element from mc.sessions
    let session = mc.get_session(0).await?;

    // let offer = session.connect().await.unwrap();
    let offer = session.get_sdp_offer().await.unwrap();

    Ok(Json(json!({
        "status": "ok",
        "message": "Session joined",
        "offer": offer,
    })))
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

async fn set_answer(
    // get request body
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<SDPAnswerRequest>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - set_answer", "Handler");

    // get first element from mc.sessions
    let session = mc.get_session(0).await?;
    session.set_sdp_answer(body.sdp).await.unwrap();

    Ok(Json(json!({
        "status": "ok",
        "message": "Session joined",
    })))
}

async fn add_ice_candidate(
    // get request body
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<ICECandidateRequest>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - add_ice_candidate", "Handler");

    // get first element from mc.sessions
    let session = mc.get_session(0).await?;

    session.add_ice_candidate(
        RTCIceCandidateInit {
            candidate: body.candidate,
            sdp_mid: body.sdp_mid,
            sdp_mline_index: body.sdp_mline_index,
            username_fragment: body.username_fragment,
        }
    ).await?;

    Ok(Json(json!({
        "status": "ok",
        "message": "ice added",
    })))
}

async fn broadcast(
    // get request body
    State(mc): State<Arc<SessionController>>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - broadcast", "Handler");

    // get first element from mc.sessions
    let mut session = mc.get_session(0).await?;


    // broadcasting function blocks until it is done, and works
    // session.broadcaster.add_audio_track("audio/opus").await.unwrap();
    // println!("audio track: {:?}", session.broadcaster.audio_track);

    Ok(Json(json!({
        "status": "ok",
        "message": "Broadcasting",
    })))
}