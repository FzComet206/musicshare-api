use crate::sessions::model::{
    ModelController,
    Session,
    PlayQueue,
};

use crate::Result;
use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use axum::routing::{
    delete,
    post
};
use serde::Deserialize;

pub fn routes(mc: ModelController) -> Router {
    Router::new()
        .route("/session", post(create_session)
            .get(list_sesisons)
            .delete(delete_session))
        .with_state(mc)
}

#[derive(Debug, Deserialize)]
struct SessionParams {
    id: u64,
}

async fn create_session(
    State(mc): State<ModelController>,
) -> Result<Json<Session>> {
    println!("->> {:<12} - create_session", "Handler");

    let session = mc.create_session().await?;

    Ok(Json(session))
}

async fn list_sesisons(
    State(mc): State<ModelController>
) -> Result<Json<Vec<Session>>> {
    println!("->> {:<12} - list_sessions", "Handler");

    // list all sessions
    let session = mc.list_sessions().await?;

    Ok(Json(session))
}

async fn delete_session(
    State(mc): State<ModelController>,
    Query(params) : Query<SessionParams>,
) -> Result<Json<Session>> {
    println!("->> {:<12} - delete_session", "Handler");

    // list all sessions
    let session = mc.delete_session(params.id).await?;

    Ok(Json(session))
}