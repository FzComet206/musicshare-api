#![allow(unused)]

use anyhow::Result;
use serde_json::json;

#[tokio::test]
async fn quick_dev() -> Result<()> {
    let hc = httpc_test::new_client("http://localhost:3000")?;

    hc.do_get("/hello?name=Mike").await?.print().await?;
    hc.do_get("/hellopath/Michael").await?.print().await?;
    hc.do_get("/src/main.rs").await?.print().await?;
    hc.do_post("/api/login", json!({
        "username": "mike",
        "password": "1234"
    })).await?.print().await?;

    Ok(())
}