

// this is a auth routes

// implements the following apis

// get self

// create session
// upload file
// delete file
// get files
// add file to queue
// change queue order
// starts playback
use std::sync::Arc;
use axum::Router;
use axum::routing::post;
use axum::extract::State;
use axum::Extension;
use sqlx::PgPool;
use serde_json::{json, Value};
use axum::{Json};

use crate::utils::error::{ Error, Result };
use crate::models::SessionController;
use crate::ctx::Ctx;


pub fn routes(mc: Arc<SessionController>) -> Router {
    Router::new()
        .route("/test", post(test_auth))
        .with_state(mc)
}

async fn test_auth(
    State(mc): State<Arc<SessionController>>,
    Extension(pool) : Extension<PgPool>,
    ctx: Ctx,
) -> Result<Json<Value>> {

    let id = ctx.id();
    let name = ctx.name();
    let picture = ctx.picture();
    println!("->> test_auth id: {}, name: {}", id, name);

    Ok(Json(json!({
        "id": id,
        "name": name,
        "picture": picture,
    })))
}
