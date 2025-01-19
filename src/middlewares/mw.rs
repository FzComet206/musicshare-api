use axum::middleware::Next;
use axum::http::Request;
use axum::body::Body;
use axum::response::Response;
use tower_cookies::{ Cookie, Cookies };
use serde_json::Value;

use crate::utils::error::{ Error, Result };
use crate::middlewares::AUTH_TOKEN;


pub async fn mw_require_auth<B>(
    cookies: Cookies,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response> {
    let headers = req.headers();
    let auth_token = headers.get(AUTH_TOKEN)
        .map(|value| value.to_str().unwrap().to_string())
        .ok_or(Error::AuthFailNoToken)?;
    
    let client = reqwest::Client::new();

    let response = client.get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(auth_token)
        .send()
        .await
        .map_err(|_| Error::AuthFailInvalidToken)?;
    
    let info = response.json::<Value>().await.map_err(|_| Error::AuthFailInvalidToken)?;
    let email = info.get("email").unwrap_or(&Value::Null).as_str().unwrap_or("");
    let id = info.get("id").unwrap_or(&Value::Null).as_str().unwrap_or("");
    let picture = info.get("picture").unwrap_or(&Value::Null).as_str().unwrap_or("");
    if email.is_empty() {
        return Err(Error::AuthFailInvalidToken);
    }

    println!("email: {}", email);
    println!("id: {}", id);
    println!("picture: {}", picture);
    // let id = info.get("id").unwrap().as_str().unwrap();
    // let picture = info.get("picture").unwrap().as_str().unwrap();
    
    // send a request to auth server

    Ok(next.run(req).await)
}