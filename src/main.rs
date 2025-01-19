#![allow(unused)]

use self::utils::error::{Error, Result};

use serde::Deserialize;
use axum::response::{Html, IntoResponse};
use axum::routing::{get, get_service};
use axum::extract::{Path, Query};
use axum::Router;
use axum::Extension;
use axum::middleware;
use tower_http::services::ServeDir;
use std::net::SocketAddr;
use axum::response::Response;

use models::SessionController;
use axum::http::Method;
use std::sync::Arc;

use tower_http::cors::{Any, CorsLayer};
use tower_cookies::CookieManagerLayer;

// handles api routes
mod routes;
// handles server state and relevant struct
mod models;
// handles media files
mod media;
// handles db connetion
mod db;
// handles errors, logging, and utilities
mod utils;
// handles authenticaion
mod middlewares;
// handles context
mod ctx;

// import error.rs module
use crate::media::file_manager::FileManager;
use crate::ctx::Ctx;


#[tokio::main]
async fn main() -> Result<()> {

    // initialize session controller
    let mc = Arc::new(SessionController::new().await?);
    mc.create_session().await?;
    let pool = db::establish_connection().await?;

    // joining routes 
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::PUT])
        .allow_headers(Any);

    // auth required routes
    let routes_control = routes::routes_control::routes(mc.clone())
        .route_layer(middleware::from_fn(middlewares::mw::mw_require_auth));

    let main_router = Router::new()
        .merge(routes_hello())
        .merge(routes::routes_login::routes())
        .merge(routes::routes_session::routes(mc.clone()))
        .nest("/api", routes_control)
        .layer(middleware::map_response(main_response_mapper))
        .layer(middleware::from_fn_with_state(
            mc.clone(),
            middlewares::mw::mw_ctx_resolver,
        ))
        .layer(Extension(pool))
        .layer(cors)
        .fallback_service(routes_static());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("->> Server listening on port 3000");
    println!("");
    println!("");

    axum::Server::bind(&addr)
        .serve(main_router.into_make_service())
        .await
        .unwrap();

    Ok(())
}

async fn main_response_mapper(res: Response) -> Response {
    println!("->> {:<12} - main_response_mapper", "Mapper");
    println!();
    res
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