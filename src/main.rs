#![allow(unused)]

pub use self::utils::error::{Error, Result};

use serde::Deserialize;
use axum::response::{Html, IntoResponse};
use axum::routing::{get, get_service};
use axum::extract::{Path, Query};
use axum::Router;
use tower_http::services::ServeDir;
use std::net::SocketAddr;

use models::SessionController;
use media::converter::Converter;
use tower_http::cors::{Any, CorsLayer};
use axum::http::Method;
use std::sync::Arc;

mod auth;
mod db;
mod media;
mod routes;
mod models;
mod utils;
// import error.rs module


#[tokio::main]
async fn main() -> Result<()> {
    

    // initialize gstreamer
    // Convearter::init();
    // Converter::convert_to_opus("test.mp3", "output.ogg");
    
    // initialize session controller
    let mc = Arc::new(SessionController::new().await?);
    mc.create_session().await?;
    
    // joining routes 
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::PUT])
        .allow_headers(Any);


    let routes_hello = Router::new()
    .merge(routes_hello())
    .merge(routes::routes_login::routes())
    .nest("/api", routes::session::routes(mc.clone()))
    .fallback_service(routes_static())
    .layer(cors);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("->> Server listening on port 3000");

    axum::Server::bind(&addr)
        .serve(routes_hello.into_make_service())
        .await
        .unwrap();

    Ok(())
}

fn routes_static() -> Router {
    Router::new().nest_service("/", get_service(ServeDir::new("./")))
}

// joined routes hello
fn routes_hello() -> Router {
    Router::new()
        .route("/hello", get(handler_hello))
        .route("/hellopath/:name", get(handler_hello_path))
}

// example for use of Query params
#[derive(Debug, Deserialize)]
struct HelloParams {
    name: Option<String>,
}

async fn handler_hello(Query(params): Query<HelloParams>) -> impl IntoResponse {
    println!("->> {:<12} - handler_hello", "Handler");

    let name = params.name.as_deref().unwrap_or("World");

    Html(format!("Hello, {name}!"))
}

// example for use of Path params
async fn handler_hello_path(Path(name): Path<String>) -> impl IntoResponse {
    println!("->> {:<12} - handler_hello_path - {name:?}", "Handler");
    Html(format!("Hello Path, {name}!")) 
}
