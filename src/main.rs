pub use self::error::{Error, Result};

use serde::Deserialize;
use axum::response::{Html, IntoResponse};
use axum::routing::{get, get_service};
use axum::extract::{Path, Query};
use axum::Router;
use tower_http::services::ServeDir;
use std::net::SocketAddr;

mod error;
mod route;
// import error.rs module

#[tokio::main]
async fn main() {
    
    // joining routes 
    let routes_hello = Router::new()
    .merge(routes_hello())
    .merge(route::routes_login::routes())
    .fallback_service(routes_static());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("->> Server listening on port 3000");

    axum::Server::bind(&addr)
        .serve(routes_hello.into_make_service())
        .await
        .unwrap();
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
    Html(format!("Hello Path, {name}!")) }
