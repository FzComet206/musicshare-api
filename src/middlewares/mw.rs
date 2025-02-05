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

/// Trivial middleware that requires a valid context (set by a previous resolver).
pub async fn mw_require_auth(
    ctx: Result<Ctx>,
    req: Request<Body>,
    next: Next,
) -> Result<Response> {
    // Fail immediately if the context is not valid.
    ctx?;
    Ok(next.run(req).await)
}

/// A no-op middleware (you may remove or extend it as needed).
pub async fn mw_optional_auth(
    _ctx: Result<Ctx>,
    req: Request<Body>,
    next: Next,
) -> Result<Response> {
    Ok(next.run(req).await)
}

/// Helper: given a pool and auth token, fetch the user info from Google and then
/// retrieve or insert the corresponding user into the database.
async fn resolve_ctx_with_auth_token(pool: &PgPool, auth_token: &str) -> Result<Ctx> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://www.googleapis.com/userinfo/v2/me")
        .bearer_auth(auth_token)
        .send()
        .await
        .map_err(|_| Error::AuthFailInvalidToken)?;

    let info = response
        .json::<Value>()
        .await
        .map_err(|_| Error::AuthFailInvalidToken)?;

    let id = info.get("id").and_then(Value::as_str).unwrap_or("");
    if id.is_empty() {
        return Err(Error::AuthFailInvalidToken);
    }
    let name = info.get("name").and_then(Value::as_str).unwrap_or("");
    let picture = info.get("picture").and_then(Value::as_str).unwrap_or("");

    // Query for an existing user.
    let rows = sqlx::query("SELECT * FROM users WHERE sub = $1")
        .bind(id)
        .fetch_all(pool)
        .await
        .map_err(|e| Error::DBError {
            source: format!("{:?}", e),
        })?;

    if rows.len() == 1 {
        let row = &rows[0];
        let userid: i32 = row.get("user_id");
        let db_name: String = row.get("name");
        let db_picture: String = row.get("picture");
        Ok(Ctx::new(userid.to_string(), db_name, db_picture))
    } else if rows.is_empty() {
        // Insert new user.
        sqlx::query(
            "INSERT INTO users (oauth_type, sub, name, picture)
             VALUES ('google', $1, $2, $3)",
        )
        .bind(id)
        .bind(name)
        .bind(picture)
        .execute(pool)
        .await
        .map_err(|e| Error::DBError {
            source: format!("{:?}", e),
        })?;

        // Query again for the newly inserted user.
        let rows = sqlx::query("SELECT * FROM users WHERE sub = $1")
            .bind(id)
            .fetch_all(pool)
            .await
            .map_err(|e| Error::DBError {
                source: format!("{:?}", e),
            })?;

        if let Some(row) = rows.first() {
            let userid: i32 = row.get("user_id");
            let db_name: String = row.get("name");
            let db_picture: String = row.get("picture");
            Ok(Ctx::new(userid.to_string(), db_name, db_picture))
        } else {
            Err(Error::DBError {
                source: "Failed to retrieve newly inserted user".into(),
            })
        }
    } else {
        Err(Error::DBError {
            source: "Multiple users found".into(),
        })
    }
}

/// Middleware that *requires* an auth token and sets a valid context.
/// Returns an error if the token is missing or invalid.
pub async fn mw_ctx_resolver(
    State(_mc): State<Arc<SessionController>>,
    cookies: Cookies,
    Extension(pool): Extension<PgPool>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response> {
    println!("->> {:12} - ctx_resolver", "Middleware");

    let auth_token = cookies
        .get(AUTH_TOKEN)
        .ok_or(Error::AuthFailNoToken)?
        .value()
        .to_string();

    let ctx = resolve_ctx_with_auth_token(&pool, &auth_token).await?;
    // Insert the successful context (wrapped in Ok) into extensions.
    req.extensions_mut().insert(Ok::<Ctx, Error>(ctx));

    Ok(next.run(req).await)
}

/// Middleware that attempts to resolve a context if an auth token is present.
/// Otherwise, it simply inserts a default “anonymous” context.
pub async fn mw_optional_ctx_resolver(
    State(_mc): State<Arc<SessionController>>,
    cookies: Cookies,
    Extension(pool): Extension<PgPool>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response> {
    println!("->> {:12} - optional_ctx_resolver", "Middleware");

    let default_ctx = Ctx::new("-1".to_string(), "anonymous".to_string(), "".to_string());
    let ctx_result: Result<Ctx> = if let Some(cookie) = cookies.get(AUTH_TOKEN) {
        match resolve_ctx_with_auth_token(&pool, cookie.value()).await {
            Ok(ctx) => Ok(ctx),
            Err(e) => {
                eprintln!("Authentication error in optional resolver: {:?}", e);
                Ok(default_ctx)
            }
        }
    } else {
        Ok(default_ctx)
    };

    req.extensions_mut().insert(ctx_result);
    Ok(next.run(req).await)
}

#[async_trait]
impl<S: Send + Sync> FromRequestParts<S> for Ctx {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut axum::http::request::Parts, _state: &S) -> Result<Self> {
        parts
            .extensions
            .get::<Result<Ctx>>()
            .ok_or(Error::AuthFailCtxNotFound)?
            .clone()
    }
}