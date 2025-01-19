use axum::middleware::Next;
use axum::http::Request;
use axum::http::request::Parts;
use axum::body::Body;
use axum::response::Response;
use serde_json::Value;

use async_trait::async_trait;
use axum::extract::{ FromRequestParts, State };

use crate::utils::error::{ Error, Result };
use crate::middlewares::AUTH_TOKEN;
use crate::ctx::Ctx;

pub async fn mw_require_auth<B>(
    ctx: Result<Ctx>,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response> {
    
    // returns error proparly
    ctx?;
    
    Ok(next.run(req).await)
}

#[async_trait]
impl<S: Send + Sync> FromRequestParts<S> for Ctx {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self> {

        // this function is called twice
        println!("from request parts");

        let auth_token = parts.headers.get(AUTH_TOKEN)
            .map(|value| value.to_str().unwrap().to_string())
            .ok_or(Error::AuthFailNoToken)?;
        
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

        Ok(Ctx::new(id.to_string(), name.to_string(), picture.to_string()))
    }
}