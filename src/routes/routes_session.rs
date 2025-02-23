use crate::models::session::{
    SessionController,
    Session,
};

use crate::Ctx;
use crate::models::peer::Listener;
use crate::models::session::User;

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

use axum::response::{
    Sse,
    sse::Event,
    sse::KeepAlive,
};
use futures::Stream;
use core::result::Result as CoreResult;

use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::{StreamExt};
use axum::Extension;
use sqlx::PgPool;

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
struct SessionID {
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct SPID {
    session_id: String,
    peer_id: String,
}

#[derive(Debug, Serialize)]
struct SessionPreview {
    session_id: String,
    session_owner: User,
    session_start_time: u64,
    number_of_listeners: usize,
    listeners: Vec<Listener>,
    top_queue: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BrowseSessions {
    sessions: Vec<SessionPreview>,
}

// this is a no auth route layer
pub fn routes(mc: Arc<SessionController>) -> Router {
    Router::new()
        .route("/get_offer", get(get_offer))
        .route("/set_answer", post(set_answer))
        .route("/get_ice", post(get_ice))
        .route("/set_ice", post(add_ice))
        .route("/state", get(server_state))
        .route("/queue_position", get(get_initial_queue_position))
        .route("/queue", get(get_queue))
        .route("/queue_notify", get(queue_notify))
        .route("/session_stats", get(get_session_stats))
        .route("/session_listeners", get(get_session_listeners))
        .route("/browse", get(browse_sesions))
        .route("/leave", get(leave_session))
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
    ctx: Ctx,
    State(mc): State<Arc<SessionController>>,
    Query(params): Query<SessionID>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - get_offer - {:<12}", "Handler", ctx.name());

    let mut session = mc.get_session(params.session_id).await?;

    if session.get_session_owner().await?.id == ctx.id() {
        if session.check_owner_connect_duplicate().await? {
            return Err(Error::SessionError {
                msg: "Owner already connected".to_string(),
            });
        }
    }

    // allow max of 5 listeners including the owner
    if session.get_session_owner().await?.id != ctx.id()
        && session.get_number_of_listeners().await? >= 5
    {
        return Err(Error::SessionFull);
    }

    // let offer = session.get_offer("hi".to_string()).await?;
    let (mut uuid, mut rx) = session.create_peer(
        Listener {
            name: ctx.name(),
            picture: ctx.picture(),
            id: ctx.id(),
        }
    ).await?;

    // await for the rx oneshot before getting offer
    // possible deadklock here? session specific !!!!
    // thread 'tokio-runtime-worker' panicked at src/routes/routes_session.rs:131:14:
    // called `Result::unwrap()` on an `Err` value: RecvError(())
    match rx.await {
        Ok(_) => {
        },
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }

    // bug, play file after deletion freezes the session

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
    ctx: Ctx,
    Query(params): Query<SessionID>,
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<SDPAnswerRequest>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - set_answer - {:<12}", "Handler", ctx.name());

    let mut session = mc.get_session(params.session_id).await?;
    session.set_answer(body.sdp, body.peerid).await?;

    Ok(Json(json!({
        "status": "ok",
        "message": "Session joined",
    })))
}

async fn get_ice(
    ctx: Ctx,
    Query(params): Query<SessionID>,
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<GetIceRequest>,
) -> Result<Json<Vec<RTCIceCandidate>>> {

    println!("->> {:<12} - get_ice - {:<12}", "Handler", ctx.name());

    let mut session = mc.get_session(params.session_id).await?;
    let candidates = session.get_ice(body.peerid).await?.clone();

    Ok(Json(candidates))
}

async fn add_ice(
    ctx: Ctx,
    Query(params): Query<SessionID>,
    State(mc): State<Arc<SessionController>>,
    Json(body): Json<ICECandidateRequest>,
) -> Result<Json<Value>> {

    println!("->> {:<12} - add_ice - {:<12}", "Handler", ctx.name());

    let mut session = mc.get_session(params.session_id).await?;
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

async fn get_queue(
    State(mc): State<Arc<SessionController>>,
    Query(params): Query<SessionID>,
) -> Result<Json<Value>> {

    println!("->> {:<12} - get_queue", "Handler");

    let session = mc.get_session(params.session_id).await?;
    let queue = session.get_queue().await?;
    
    Ok(Json(json!({
        "status": "ok",
        "queue": queue,
    })))
}

async fn queue_notify(
    State(mc): State<Arc<SessionController>>,
    Query(params): Query<SessionID>,
) -> Sse<impl Stream<Item = CoreResult<Event, Infallible>>> {

    println!("->> {:<12} - queue_notify", "Handler");
    let session_id = params.session_id.clone();

    let mut session = mc.get_session(session_id).await.unwrap();
    let sender = session.get_sender().await.unwrap();
    let mut rx = sender.subscribe();
    // get sender from session

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

async fn get_session_stats(
    State(mc): State<Arc<SessionController>>,
    Query(params): Query<SessionID>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - get_session_stats", "Handler");

    // let session_owner = mc.get_session_owner(params.session_id.clone()).await?;
    let session = mc.get_session(params.session_id).await?;

    let session_owner = session.get_session_owner().await?;
    let session_start_time = session.get_session_start_time().await?;
    let number_of_listeners = session.get_number_of_listeners().await?;
    let listeners = session.get_listeners().await?;

    Ok(Json(json!({
        "status": "ok",
        "session_owner": session_owner,
        "session_start_time": session_start_time,
        "number_of_listeners": number_of_listeners,
        "listeners": listeners,
    })))
}

async fn get_session_listeners(
    State(mc): State<Arc<SessionController>>,
    Query(params): Query<SessionID>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - get_session_listeners", "Handler");

    let session = mc.get_session(params.session_id).await?;
    let number_of_listeners = session.get_number_of_listeners().await?;
    let listeners = session.get_listeners().await?;

    Ok(Json(json!({
        "status": "ok",
        "number_of_listeners": number_of_listeners,
        "listeners": listeners,
    })))
}

async fn get_initial_queue_position(
    State(mc): State<Arc<SessionController>>,
    Query(params): Query<SessionID>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - get_initial_queue_position", "Handler");

    let session = mc.get_session(params.session_id).await?;
    let queue_position = session.get_queue_position().await?;

    Ok(Json(json!({
        "status": "ok",
        "index": queue_position,
    })))
}

async fn browse_sesions(
    State(mc): State<Arc<SessionController>>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - browse_sesions", "Handler");

    let sessions = mc.get_sessions().await?;
    let mut result_sessions = Vec::new();

    for session in sessions {
        let session_owner: User = session.get_session_owner().await?;
        let session_start_time = session.get_session_start_time().await?;
        let number_of_listeners = session.get_number_of_listeners().await?;
        let listeners = session.get_listeners().await?;
        let top_queue = session.get_top_queue().await?;

        result_sessions.push(SessionPreview {
            session_id: session.uuid.clone(),
            session_owner,
            session_start_time,
            number_of_listeners,
            listeners,
            top_queue,
        });
    }

    Ok(Json(json!({
        "status": "ok",
        "sessions": result_sessions,
    })))
}

async fn leave_session(
    State(mc): State<Arc<SessionController>>,
    Query(params): Query<SPID>,
) -> Result<Json<Value>> {
    println!("->> {:<12} - leave_session", "Handler");

    let session = mc.get_session(params.session_id).await?;
    session.disconnect(params.peer_id).await?;

    Ok(Json(json!({
        "status": "ok",
        "message": "Session left",
    })))
}