

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

use crate::utils::error::{ Error, Result };
use crate::models::SessionController;


pub fn routes(mc: Arc<SessionController>) -> Router {
    Router::new()
        .route("/test", post(test_auth))
        .with_state(mc)
}

async fn test_auth() -> Result<()> {
    println!("test auth");
    Ok(())
}
