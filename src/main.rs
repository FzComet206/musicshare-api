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

use tower_http::cors::{Any, CorsLayer, AllowOrigin, AllowHeaders, AllowMethods};
use tower_cookies::CookieManagerLayer;

use tokio::runtime::Builder;

use reqwest::header::HeaderValue;
use tower::ServiceBuilder;

mod routes;
mod models;
mod media;
mod db;
mod utils;
mod middlewares;
mod ctx;

// import error.rs module
use crate::media::file_manager::FileManager;
use crate::ctx::Ctx;


#[tokio::main]
async fn main() -> Result<()> {

    // initialize session controller
    let mc = Arc::new(SessionController::new().await?);
    let pool = db::establish_connection().await?;

    let api_cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_methods([Method::OPTIONS, Method::GET, Method::POST, Method::DELETE, Method::PUT])
        .allow_headers([reqwest::header::CONTENT_TYPE, reqwest::header::AUTHORIZATION])
        .allow_credentials(true);

    let routes_control = routes::routes_control::routes(mc.clone())
        .route_layer(middleware::from_fn(middlewares::mw::mw_require_auth))
        .layer(middleware::from_fn_with_state(
            mc.clone(),
            middlewares::mw::mw_ctx_resolver,
        ));

    // let routes_session = routes::routes_session::routes(mc.clone());

    let routes_session = routes::routes_session::routes(mc.clone())
        .route_layer(middleware::from_fn(middlewares::mw::mw_optional_auth))
        .layer(middleware::from_fn_with_state(
            mc.clone(),
            middlewares::mw::mw_optional_ctx_resolver,
        ));

    let main_router = Router::new()
        .nest("/api", routes_control)
        .nest("/session", routes_session)

        .layer(CookieManagerLayer::new())
        .layer(Extension(pool))
        .layer(middleware::map_response(main_response_mapper))
        .layer(api_cors)
        .fallback_service(routes_static());

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    println!("->> Server listening on port 8000");
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