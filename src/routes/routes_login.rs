use crate::{Error, Result};
use axum::Json;
use axum::routing::post;
use axum::Router;
use serde::Deserialize;
use serde_json::{json, Value};

// this is a no auth layer
pub fn routes() -> Router {
    Router::new().route("/api/login", post(api_login))
}

async fn api_login(payload: Json<LoginPayload>) -> Result<Json<Value>> {
    println!("->> {:<12} - api_login", "Handler");

    // set cookies 

    let body = Json(json!({
        "username": payload.username,
        "password": payload.password,
    }));

    Ok(body)
}

#[derive(Debug, Deserialize)]
struct LoginPayload {
    username: String,
    password: String,
}
