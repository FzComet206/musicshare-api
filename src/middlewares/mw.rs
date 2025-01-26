use axum::middleware::Next;
use axum::http::Request;
use axum::http::request::Parts;
use axum::body::Body;
use axum::response::Response;
use serde_json::Value;

use async_trait::async_trait;
use axum::extract::{ FromRequestParts, State };
use axum::Extension;
use std::sync::Arc;
use sqlx::PgPool;
use sqlx::Row;

use crate::utils::error::{ Error, Result };
use crate::middlewares::AUTH_TOKEN;
use crate::ctx::Ctx;
use crate::models::SessionController;
use tower_cookies::{Cookie, Cookies};

pub async fn mw_require_auth<B>(
    ctx: Result<Ctx>,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response> {
    
    ctx?;
    
    Ok(next.run(req).await)
}

pub async fn mw_ctx_resolver<B>(
    State(mc): State<Arc<SessionController>>,
    cookies: Cookies,
    Extension(pool) : Extension<PgPool>,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response> {

    // this middleware layers handles authentications and insert into request body
    println!("->> {:12} - ctx_resolver", "Middleware");
    
    // get the cookie named google_access_token
    let auth_token = match cookies.get(AUTH_TOKEN) {
        Some(cookie) => cookie.value().to_string(),
        None => return Err(Error::AuthFailNoToken),
    };

    let client = reqwest::Client::new();

    let response = client.get("https://www.googleapis.com/userinfo/v2/me")
        .bearer_auth(auth_token)
        .send()
        .await
        .map_err(|_| Error::AuthFailInvalidToken)?;
    
    let info = response.json::<Value>().await.map_err(|_| Error::AuthFailInvalidToken)?;
    let id = info.get("id").unwrap_or(&Value::Null).as_str().unwrap_or("");
    let name = info.get("name").unwrap_or(&Value::Null).as_str().unwrap_or("");
    let picture = info.get("picture").unwrap_or(&Value::Null).as_str().unwrap_or("");

    if id.is_empty() {
        return Err(Error::AuthFailInvalidToken);
    }

    match sqlx::query("SELECT * FROM users WHERE sub = $1")
        .bind(id)
        .fetch_all(&pool)
        .await {
            Ok(row) => {
                match row.len() {
                    1 => {
                        row.iter().for_each(|row| {
                            let name: String = row.get("name");
                            let picture: String = row.get("picture");
                            let sub: String = row.get("sub");
                            let result_ctx: Result<Ctx> = Ok(Ctx::new(sub, name, picture));
                            req.extensions_mut().insert(result_ctx);
                        });
                    }
                    0 => {
                        match sqlx::query(
                            "INSERT INTO 
                                users (oauth_type, sub, name, picture) 
                                VALUES ('google', $1, $2, $3)"
                            )
                            .bind(id)
                            .bind(name)
                            .bind(picture)
                            .execute(&pool)
                            .await {
                                Ok(_) => {
                                    let result_ctx: Result<Ctx> = Ok(Ctx::new(id.to_string(), name.to_string(), picture.to_string()));
                                    req.extensions_mut().insert(result_ctx);
                                }
                                Err(e) => return Err(Error::DBError { source: format!("{:?}", e) }),
                        };
                    }
                    _ => return Err(Error::DBError { source: "Multiple users found".to_string() }),
                }
            }
            Err(e) => {
                return Err(Error::DBError { source: format!("{:?}", e) });
            }
        };

    Ok(next.run(req).await)
}

#[async_trait]
impl<S: Send + Sync> FromRequestParts<S> for Ctx {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self> {
        // this function is run twice
        parts
            .extensions
            .get::<Result<Ctx>>()
            .ok_or(Error::AuthFailCtxNotFound)?
            .clone()
    }
}