use axum::{
    response::{Html, IntoResponse}, routing::get, Router
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let routes_hello = Router::new().route(
        "/hello",
        get(handler_hello)
    );

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("->> Server listening on port 3000");

    axum::Server::bind(&addr)
        .serve(routes_hello.into_make_service())
        .await
        .unwrap();
}

async fn handler_hello() -> impl IntoResponse {
    println!("->> {:<12} - handler_hello", "Handler");

    Html("<h1>Hello this is Mike</h1>")
}
