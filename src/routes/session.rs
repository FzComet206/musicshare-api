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

pub fn routes(mc: Arc<SessionController>) -> Router {
    Router::new()
        .route("/session", post(create_session))
        .route("/session", get(join_session))
        .route("/set_answer", post(set_answer))
        .route("/broadcast", get(broadcast))
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

    let offer = session.connect().await.unwrap();

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

async fn broadcast(
    // get request body
    State(mc): State<Arc<SessionController>>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - broadcast", "Handler");

    // get first element from mc.sessions
    let mut session = mc.get_session(0).await?;
    session.broadcaster.broadcast_audio_from_file("output2.ogg").await.unwrap();


    // broadcasting function blocks until it is done, and works

    // session.broadcaster.add_audio_track("audio/opus").await.unwrap();
    // println!("audio track: {:?}", session.broadcaster.audio_track);

    Ok(Json(json!({
        "status": "ok",
        "message": "Broadcasting",
    })))
}