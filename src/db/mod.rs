use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use crate::utils::error::{ Error, Result };
use std::env;
use dotenvy::dotenv;

pub async fn establish_connection() -> Result<PgPool> {
    dotenv().ok();
    let db_url_dev = env::var("DATABASE_URL_DEV").expect("DATABASE_URL must be set");


    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url_dev)
        .await?;
    
    Ok(pool)
}
